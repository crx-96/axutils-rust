//! Redis 锁的共享 token、TTL 与 Lua 辅助逻辑。

use std::time::Duration;

use super::super::{
    commands,
    error::{RedisError, RedisTransportErrorKind},
};

/// 锁所有者 token 的固定字节数，由操作系统随机源填充。
pub(super) const TOKEN_BYTES: usize = 32;
/// 单次租约或续租允许的最大时间，限制丢弃 guard 后远端残留。
const MAX_LOCK_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// 只删除 token 仍匹配的 key，避免旧所有者释放替代持有者的锁。
pub(super) const RELEASE_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0"#;

/// 只续租 token 仍匹配的 key，保持比较与 TTL 更新原子执行。
pub(super) const RENEW_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
end
return 0"#;

/// 检查正值和 24 小时租约上限，再向上取整为命令使用的毫秒数。
pub(crate) fn lock_ttl_millis(ttl: Duration) -> Result<i64, RedisError> {
    if ttl.is_zero() || ttl > MAX_LOCK_TTL {
        return Err(RedisError::invalid_config("ttl"));
    }
    commands::duration_millis(ttl)
}

/// 返回 Redis 实际采用的毫秒粒度 TTL，供 guard 状态和 Debug 使用。
pub(crate) fn lock_ttl_duration(ttl: Duration) -> Result<Duration, RedisError> {
    let millis = lock_ttl_millis(ttl)?;
    let millis = u64::try_from(millis).map_err(|_| RedisError::invalid_config("ttl"))?;
    Ok(Duration::from_millis(millis))
}

/// 从操作系统随机源产生不可预测的所有者 token，随机失败不提供弱随机回退。
pub(crate) fn token() -> Result<[u8; TOKEN_BYTES], RedisError> {
    use rand::rngs::SysRng;

    token_with_rng(&mut SysRng)
}

/// 通过可失败随机源填满固定 token；错误只保留脱敏传输类别。
pub(super) fn token_with_rng<R: rand::TryRng>(
    rng: &mut R,
) -> Result<[u8; TOKEN_BYTES], RedisError> {
    let mut token = [0_u8; TOKEN_BYTES];
    rng.try_fill_bytes(&mut token)
        .map_err(|_| RedisError::Transport(RedisTransportErrorKind::Other))?;
    Ok(token)
}

/// 装配带有效毫秒 TTL 的 SET NX，作为单键租约的原子获取操作。
pub(crate) fn acquire_command(key: &[u8], token: &[u8], ttl_millis: i64) -> ::redis::Cmd {
    let mut command = ::redis::cmd("SET");
    command
        .arg(key)
        .arg(token)
        .arg("PX")
        .arg(ttl_millis)
        .arg("NX");
    command
}

/// 装配校验 token 后删除的 Lua 调用，不暴露 token 或 key。
pub(crate) fn release_command(key: &[u8], token: &[u8]) -> ::redis::Cmd {
    let mut command = ::redis::cmd("EVAL");
    command.arg(RELEASE_SCRIPT).arg(1).arg(key).arg(token);
    command
}

/// 装配校验 token 后更新 TTL 的 Lua 调用，所有参数作为独立 RESP 参数发送。
pub(crate) fn renew_command(key: &[u8], token: &[u8], ttl_millis: i64) -> ::redis::Cmd {
    let mut command = ::redis::cmd("EVAL");
    command
        .arg(RENEW_SCRIPT)
        .arg(1)
        .arg(key)
        .arg(token)
        .arg(ttl_millis);
    command
}

/// 只接受 Lua 约定的 0/1 响应，其他整数保留为协议错误。
pub(super) fn script_result(value: i64) -> Result<bool, RedisError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RedisError::Transport(RedisTransportErrorKind::Protocol)),
    }
}

/// 完整的 0/1 响应使 guard 失效；错误保持本地状态以允许调用方决定重试。
pub(super) fn finish_release(
    active: &mut bool,
    result: Result<i64, RedisError>,
) -> Result<bool, RedisError> {
    // 只有可靠响应才改变活动状态；错误和取消不能伪造释放成功。
    let released = script_result(result?)?;
    *active = false;
    Ok(released)
}

/// 成功续租更新有效 TTL，明确失去所有权则失效；传输/协议错误不改状态。
pub(super) fn finish_renew(
    active: &mut bool,
    ttl: &mut Duration,
    effective_ttl: Duration,
    result: Result<i64, RedisError>,
) -> Result<bool, RedisError> {
    // 本地 TTL 记录服务端确认的租约；不把网络失败误认为所有权已丢失或续租成功。
    let renewed = script_result(result?)?;
    if renewed {
        *ttl = effective_ttl;
    } else {
        *active = false;
    }
    Ok(renewed)
}
