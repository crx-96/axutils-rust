//! Redis 锁的共享 token、TTL 与 Lua 辅助逻辑。

use std::time::Duration;

use super::super::{
    commands,
    error::{RedisError, RedisTransportErrorKind},
};

pub(super) const TOKEN_BYTES: usize = 32;
const MAX_LOCK_TTL: Duration = Duration::from_secs(24 * 60 * 60);

pub(super) const RELEASE_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0"#;

pub(super) const RENEW_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
end
return 0"#;

pub(crate) fn lock_ttl_millis(ttl: Duration) -> Result<i64, RedisError> {
    if ttl.is_zero() || ttl > MAX_LOCK_TTL {
        return Err(RedisError::invalid_config("ttl"));
    }
    commands::duration_millis(ttl)
}

pub(crate) fn lock_ttl_duration(ttl: Duration) -> Result<Duration, RedisError> {
    let millis = lock_ttl_millis(ttl)?;
    let millis = u64::try_from(millis).map_err(|_| RedisError::invalid_config("ttl"))?;
    Ok(Duration::from_millis(millis))
}

pub(crate) fn token() -> Result<[u8; TOKEN_BYTES], RedisError> {
    use rand::rngs::SysRng;

    token_with_rng(&mut SysRng)
}

pub(super) fn token_with_rng<R: rand::TryRng>(
    rng: &mut R,
) -> Result<[u8; TOKEN_BYTES], RedisError> {
    let mut token = [0_u8; TOKEN_BYTES];
    rng.try_fill_bytes(&mut token)
        .map_err(|_| RedisError::Transport(RedisTransportErrorKind::Other))?;
    Ok(token)
}

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

pub(crate) fn release_command(key: &[u8], token: &[u8]) -> ::redis::Cmd {
    let mut command = ::redis::cmd("EVAL");
    command.arg(RELEASE_SCRIPT).arg(1).arg(key).arg(token);
    command
}

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

pub(super) fn script_result(value: i64) -> Result<bool, RedisError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RedisError::Transport(RedisTransportErrorKind::Protocol)),
    }
}

pub(super) fn finish_release(
    active: &mut bool,
    result: Result<i64, RedisError>,
) -> Result<bool, RedisError> {
    let released = script_result(result?)?;
    *active = false;
    Ok(released)
}

pub(super) fn finish_renew(
    active: &mut bool,
    ttl: &mut Duration,
    effective_ttl: Duration,
    result: Result<i64, RedisError>,
) -> Result<bool, RedisError> {
    let renewed = script_result(result?)?;
    if renewed {
        *ttl = effective_ttl;
    } else {
        *active = false;
    }
    Ok(renewed)
}
