use std::{fmt, time::Duration};

use super::{client::RedisClient, error::RedisError};

const TOKEN_BYTES: usize = 32;
const MAX_LOCK_TTL: Duration = Duration::from_secs(24 * 60 * 60);

const RELEASE_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0"#;

const RENEW_SCRIPT: &str = r#"if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
end
return 0"#;

pub(crate) fn lock_ttl_millis(ttl: Duration) -> Result<i64, RedisError> {
    if ttl.is_zero() || ttl > MAX_LOCK_TTL {
        return Err(RedisError::invalid_config("ttl"));
    }
    super::commands::duration_millis(ttl)
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

fn token_with_rng<R: rand::TryRng>(rng: &mut R) -> Result<[u8; TOKEN_BYTES], RedisError> {
    let mut token = [0_u8; TOKEN_BYTES];
    rng.try_fill_bytes(&mut token)
        .map_err(|_| RedisError::Transport(super::error::RedisTransportErrorKind::Other))?;
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

pub(crate) fn script_result(value: i64) -> Result<bool, RedisError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(RedisError::Transport(
            super::error::RedisTransportErrorKind::Protocol,
        )),
    }
}

fn finish_release(active: &mut bool, result: Result<i64, RedisError>) -> Result<bool, RedisError> {
    let released = script_result(result?)?;
    *active = false;
    Ok(released)
}

fn finish_renew(
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

/// 同步 Redis 单键租约锁 guard。
///
/// guard 不实现 `Clone` 或 `Copy`，内部所有者 token 和完整 key 不会通过公共 API 暴露。
/// 锁 key 应使用稳定、粒度足够细的业务命名空间；不要把未经审查的用户输入直接作为
/// 跨业务共享 key。token 仅是内部所有权标记，不是业务身份、认证凭据或可持久化数据。
/// 该协议不覆盖主从异步复制故障切换造成的锁丢失，也不是跨独立主节点的 Redlock 或
/// fencing token；临界区仍需数据库条件更新、唯一约束、事务或幂等保护。
/// 正常路径必须显式调用 [`RedisLockGuard::release`] 并处理返回值；同步 guard 被丢弃时
/// 不会 checkout 连接池或发送 Redis 命令，锁由获取时或最近一次成功续租后的有效 TTL 兜底。
/// TTL 严格大于 0 且不超过 24 小时，因此正常退出和 panic unwind 都可能让远端锁继续残留至
/// 当前 TTL 到期；`Drop` 不提供释放确认，也不会创建线程或 runtime 来补做释放。
pub struct RedisLockGuard {
    client: RedisClient,
    key: Vec<u8>,
    token: [u8; TOKEN_BYTES],
    ttl: Duration,
    active: bool,
}

impl RedisLockGuard {
    pub(crate) fn new(
        client: RedisClient,
        key: Vec<u8>,
        token: [u8; TOKEN_BYTES],
        ttl: Duration,
    ) -> Self {
        Self {
            client,
            key,
            token,
            ttl,
            active: true,
        }
    }

    /// 使用当前 guard 的 token 原子释放锁。
    ///
    /// 返回 `Ok(true)` 表示删除了当前 token 对应的锁；`Ok(false)` 表示锁已过期、已由其他
    /// token 持有，或 guard 已经释放。只有 Redis 命令可靠返回后才会将 guard 标记为非活动；
    /// 传输或协议错误会原样返回，guard 仍保持活动以便调用方决定是否重试；`Drop` 不会
    /// 重试或补发释放命令。调用方应把 `Err` 和 `Ok(false)` 都视为无法继续依赖锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// fn release_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client.try_lock("receipt-audit:serial-1", Duration::from_secs(30))?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _released = lock.release()?;
    ///     Ok(())
    /// }
    ///
    /// let _ = release_lock;
    /// ```
    pub fn release(&mut self) -> Result<bool, RedisError> {
        if !self.active {
            return Ok(false);
        }
        let result = self.client.release_lock_sync(&self.key, &self.token);
        finish_release(&mut self.active, result)
    }

    /// 在当前 token 仍持有锁时原子刷新 TTL。
    ///
    /// TTL 必须大于 0 且不超过 24 小时；正但不足一毫秒的 duration 向上取 1 ms。返回
    /// `Ok(true)` 表示 TTL 已刷新，`Ok(false)` 表示锁已过期、所有权已丢失或 guard 已失效，
    /// 此时 guard 不可再用于受保护操作。传输或协议错误不会伪造成功，也不会自动修改活动
    /// 状态。释放后的 guard 不会再次发送续租命令。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// fn renew_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client.try_lock("receipt-audit:serial-1", Duration::from_secs(30))?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _still_owned = lock.renew(Duration::from_secs(30))?;
    ///     let _ = lock.release()?;
    ///     Ok(())
    /// }
    ///
    /// let _ = renew_lock;
    /// ```
    pub fn renew(&mut self, ttl: Duration) -> Result<bool, RedisError> {
        let ttl_millis = lock_ttl_millis(ttl)?;
        let effective_ttl = lock_ttl_duration(ttl)?;
        if !self.active {
            return Ok(false);
        }
        let result = self
            .client
            .renew_lock_sync(&self.key, &self.token, ttl_millis);
        finish_renew(&mut self.active, &mut self.ttl, effective_ttl, result)
    }
}

impl Drop for RedisLockGuard {
    fn drop(&mut self) {}
}

impl fmt::Debug for RedisLockGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisLockGuard")
            .field("ttl", &self.ttl)
            .field("active", &self.active)
            .finish()
    }
}

#[cfg(all(feature = "redis", feature = "tokio"))]
/// 异步 Redis 单键租约锁 guard。
///
/// guard 不实现 `Clone` 或 `Copy`，内部所有者 token 和完整 key 不会通过公共 API 暴露。
/// 锁 key 应使用稳定、粒度足够细的业务命名空间；不要把未经审查的用户输入直接作为
/// 跨业务共享 key。token 仅是内部所有权标记，不是业务身份、认证凭据或可持久化数据。
/// 该协议不覆盖主从异步复制故障切换造成的锁丢失，也不是跨独立主节点的 Redlock 或
/// fencing token；临界区仍需数据库条件更新、唯一约束、事务或幂等保护。
/// 异步 guard 的 `Drop` 不会 checkout 连接池、发送网络命令或创建后台任务；正常路径必须
/// 显式 `await release()`，取消、panic unwind 或 runtime 关闭时只能依赖获取时或最近一次
/// 成功续租后的有效 TTL 兜底，TTL 上限为 24 小时。
pub struct RedisAsyncLockGuard {
    client: RedisClient,
    key: Vec<u8>,
    token: [u8; TOKEN_BYTES],
    ttl: Duration,
    active: bool,
}

#[cfg(all(feature = "redis", feature = "tokio"))]
impl RedisAsyncLockGuard {
    pub(crate) fn new(
        client: RedisClient,
        key: Vec<u8>,
        token: [u8; TOKEN_BYTES],
        ttl: Duration,
    ) -> Self {
        Self {
            client,
            key,
            token,
            ttl,
            active: true,
        }
    }

    /// 使用当前 guard 的 token 原子释放锁。
    ///
    /// 返回 `Ok(true)` 表示删除了当前 token 对应的锁；`Ok(false)` 表示锁已过期、已由其他
    /// token 持有，或 guard 已经释放。只有 Redis 命令可靠返回后才会将 guard 标记为非活动；
    /// 传输或协议错误会原样返回。`Drop` 不会重试或创建后台任务，因此正常路径必须显式
    /// `await` 此方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// async fn release_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client
    ///         .try_lock_async("receipt-audit:serial-1", Duration::from_secs(30))
    ///         .await?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _released = lock.release().await?;
    ///     Ok(())
    /// }
    ///
    /// let _ = release_lock;
    /// ```
    pub async fn release(&mut self) -> Result<bool, RedisError> {
        if !self.active {
            return Ok(false);
        }
        let result = self.client.release_lock_async(&self.key, &self.token).await;
        finish_release(&mut self.active, result)
    }

    /// 在当前 token 仍持有锁时原子刷新 TTL。
    ///
    /// TTL 必须大于 0 且不超过 24 小时；正但不足一毫秒的 duration 向上取 1 ms。返回
    /// `Ok(true)` 表示 TTL 已刷新，`Ok(false)` 表示锁已过期、所有权已丢失或 guard 已失效，
    /// 此时 guard 不可再用于受保护操作。传输或协议错误不会伪造成功，也不会自动修改活动
    /// 状态。释放后的 guard 不会再次发送续租命令。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// async fn renew_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client
    ///         .try_lock_async("receipt-audit:serial-1", Duration::from_secs(30))
    ///         .await?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _still_owned = lock.renew(Duration::from_secs(30)).await?;
    ///     let _ = lock.release().await?;
    ///     Ok(())
    /// }
    ///
    /// let _ = renew_lock;
    /// ```
    pub async fn renew(&mut self, ttl: Duration) -> Result<bool, RedisError> {
        let ttl_millis = lock_ttl_millis(ttl)?;
        let effective_ttl = lock_ttl_duration(ttl)?;
        if !self.active {
            return Ok(false);
        }
        let result = self
            .client
            .renew_lock_async(&self.key, &self.token, ttl_millis)
            .await;
        finish_renew(&mut self.active, &mut self.ttl, effective_ttl, result)
    }
}

#[cfg(all(feature = "redis", feature = "tokio"))]
impl Drop for RedisAsyncLockGuard {
    fn drop(&mut self) {}
}

#[cfg(all(feature = "redis", feature = "tokio"))]
impl fmt::Debug for RedisAsyncLockGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisAsyncLockGuard")
            .field("ttl", &self.ttl)
            .field("active", &self.active)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use redis_test::{MockCmd, MockRedisConnection};

    use super::{
        acquire_command, finish_release, finish_renew, lock_ttl_duration, lock_ttl_millis,
        release_command, renew_command, script_result, token, token_with_rng, RELEASE_SCRIPT,
        RENEW_SCRIPT,
    };
    use crate::redis::{RedisClient, RedisConfig, RedisError};

    #[test]
    fn lock_ttl_is_positive_and_bounded() {
        assert_eq!(
            lock_ttl_millis(Duration::ZERO),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
        assert_eq!(
            lock_ttl_millis(Duration::from_secs(24 * 60 * 60 + 1)),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
        assert_eq!(lock_ttl_millis(Duration::from_nanos(1)), Ok(1));
        assert_eq!(
            lock_ttl_millis(Duration::from_secs(24 * 60 * 60)),
            Ok(24 * 60 * 60 * 1000)
        );
    }

    #[test]
    fn lock_ttl_debug_state_matches_redis_rounding() {
        assert_eq!(
            lock_ttl_duration(Duration::from_nanos(1_000_001)).unwrap(),
            Duration::from_millis(2)
        );
    }

    #[test]
    fn guard_result_transitions_are_explicit_and_preserve_retry_after_errors() {
        let mut active = true;
        assert_eq!(finish_release(&mut active, Ok(1)), Ok(true));
        assert!(!active);

        let mut active = true;
        let error = RedisError::Transport(crate::redis::RedisTransportErrorKind::Network);
        assert_eq!(finish_release(&mut active, Err(error)), Err(error));
        assert!(active);

        let mut active = true;
        let mut ttl = Duration::from_millis(1);
        assert_eq!(
            finish_renew(&mut active, &mut ttl, Duration::from_millis(2), Ok(1),),
            Ok(true)
        );
        assert!(active);
        assert_eq!(ttl, Duration::from_millis(2));

        assert_eq!(
            finish_renew(&mut active, &mut ttl, Duration::from_millis(3), Ok(0),),
            Ok(false)
        );
        assert!(!active);
    }

    #[test]
    fn token_uses_fixed_opaque_length() {
        let first = token().expect("OS random source should be available");
        let second = token().expect("OS random source should be available");
        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        assert_ne!(first, second);
    }

    #[derive(Debug)]
    struct FailingRng;

    impl std::error::Error for FailingRng {}

    impl std::fmt::Display for FailingRng {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("test RNG failure")
        }
    }

    impl rand::TryRng for FailingRng {
        type Error = Self;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(Self)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(Self)
        }

        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            Err(Self)
        }
    }

    #[test]
    fn token_rng_failure_maps_to_transport_error_without_fallback() {
        let mut rng = FailingRng;
        assert_eq!(
            token_with_rng(&mut rng),
            Err(RedisError::Transport(
                crate::redis::RedisTransportErrorKind::Other
            ))
        );
    }

    #[test]
    fn guard_debug_omits_key_and_token() {
        let client = RedisClient::new(
            RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config"),
        )
        .expect("client construction should be local");
        let mut guard = super::RedisLockGuard::new(
            client,
            b"secret-lock-key".to_vec(),
            [b'S'; 32],
            Duration::from_secs(30),
        );
        let debug = format!("{guard:?}");
        guard.active = false;

        assert!(debug.contains("RedisLockGuard"));
        assert!(!debug.contains("secret-lock-key"));
        assert!(!debug.contains("SSSS"));
    }

    #[test]
    fn inactive_guard_release_and_renew_are_local_and_idempotent() {
        let client =
            RedisClient::new(RedisConfig::single("redis://127.0.0.1:1/0").expect("fixture config"))
                .expect("client construction should be local");
        let mut guard = super::RedisLockGuard::new(
            client,
            b"inactive-lock-key".to_vec(),
            [b'I'; 32],
            Duration::from_secs(30),
        );
        guard.active = false;

        assert_eq!(guard.release(), Ok(false));
        assert_eq!(guard.renew(Duration::from_secs(30)), Ok(false));
        assert_eq!(guard.release(), Ok(false));
    }

    #[test]
    fn active_sync_guard_drop_does_not_checkout_or_send_command() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        {
            let _guard = super::RedisLockGuard::new(
                client,
                b"drop-lock-key".to_vec(),
                [b'D'; 32],
                Duration::from_secs(30),
            );
        }

        assert_eq!(backend.checkout_count(), 0);
        assert_eq!(backend.command_count(), 0);
    }

    #[test]
    fn explicit_sync_release_is_observable_and_repeated_release_is_local() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        let mut guard = super::RedisLockGuard::new(
            client,
            b"explicit-release-key".to_vec(),
            [b'R'; 32],
            Duration::from_secs(30),
        );

        assert_eq!(guard.release(), Ok(true));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        assert_eq!(guard.release(), Ok(false));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        drop(guard);
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
    }

    #[test]
    fn sync_release_error_is_observable_and_drop_does_not_retry() {
        let error = RedisError::Transport(crate::redis::RedisTransportErrorKind::Network);
        let (client, backend) = RedisClient::test_fake(Err(error));
        let mut guard = super::RedisLockGuard::new(
            client,
            b"release-error-key".to_vec(),
            [b'E'; 32],
            Duration::from_secs(30),
        );

        assert_eq!(guard.release(), Err(error));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        drop(guard);
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
    }

    #[test]
    fn sync_drop_after_successful_renew_uses_no_remote_cleanup() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        let mut guard = super::RedisLockGuard::new(
            client,
            b"renewed-drop-key".to_vec(),
            [b'N'; 32],
            Duration::from_secs(1),
        );

        assert!(guard.renew(Duration::from_secs(60)).unwrap());
        assert_eq!(guard.ttl, Duration::from_secs(60));
        let checkout_count = backend.checkout_count();
        let command_count = backend.command_count();
        assert_eq!(checkout_count, 1);
        assert_eq!(command_count, 1);
        drop(guard);
        assert_eq!(backend.checkout_count(), checkout_count);
        assert_eq!(backend.command_count(), command_count);
    }

    #[test]
    fn sync_drop_during_panic_does_not_attempt_remote_release() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = super::RedisLockGuard::new(
                client,
                b"panic-drop-key".to_vec(),
                [b'P'; 32],
                Duration::from_secs(30),
            );
            panic!("test panic while a Redis lock guard is alive");
        }));

        assert!(result.is_err());
        assert_eq!(backend.checkout_count(), 0);
        assert_eq!(backend.command_count(), 0);
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[test]
    fn async_guard_debug_omits_key_and_token() {
        let client = RedisClient::new(
            RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config"),
        )
        .expect("client construction should be local");
        let mut guard = super::RedisAsyncLockGuard::new(
            client,
            b"secret-async-lock-key".to_vec(),
            [b'A'; 32],
            Duration::from_secs(30),
        );
        let debug = format!("{guard:?}");
        guard.active = false;

        assert!(debug.contains("RedisAsyncLockGuard"));
        assert!(!debug.contains("secret-async-lock-key"));
        assert!(!debug.contains("AAAA"));
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[tokio::test(flavor = "current_thread")]
    async fn active_async_guard_drop_does_not_checkout_or_send_command() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        {
            let _guard = super::RedisAsyncLockGuard::new(
                client,
                b"async-drop-lock-key".to_vec(),
                [b'D'; 32],
                Duration::from_secs(30),
            );
        }

        assert_eq!(backend.checkout_count(), 0);
        assert_eq!(backend.command_count(), 0);
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[tokio::test(flavor = "current_thread")]
    async fn explicit_async_release_is_observable_and_repeated_release_is_local() {
        let (client, backend) = RedisClient::test_fake(Ok(1));
        let mut guard = super::RedisAsyncLockGuard::new(
            client,
            b"async-explicit-release-key".to_vec(),
            [b'R'; 32],
            Duration::from_secs(30),
        );

        assert_eq!(guard.release().await, Ok(true));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        assert_eq!(guard.release().await, Ok(false));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        drop(guard);
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[tokio::test(flavor = "current_thread")]
    async fn async_release_error_is_observable_and_drop_does_not_retry() {
        let error = RedisError::Transport(crate::redis::RedisTransportErrorKind::Network);
        let (client, backend) = RedisClient::test_fake(Err(error));
        let mut guard = super::RedisAsyncLockGuard::new(
            client,
            b"async-release-error-key".to_vec(),
            [b'E'; 32],
            Duration::from_secs(30),
        );

        assert_eq!(guard.release().await, Err(error));
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
        drop(guard);
        assert_eq!(backend.checkout_count(), 1);
        assert_eq!(backend.command_count(), 1);
    }

    #[test]
    fn scripts_only_accept_zero_or_one() {
        assert_eq!(script_result(0), Ok(false));
        assert_eq!(script_result(1), Ok(true));
        assert_eq!(
            script_result(2),
            Err(RedisError::Transport(
                crate::redis::RedisTransportErrorKind::Protocol
            ))
        );
    }

    #[test]
    fn lock_commands_preserve_single_key_and_token_arguments() {
        let key = b"lock:key";
        let token = b"opaque-token";
        let mut connection = MockRedisConnection::new([
            MockCmd::new(
                ::redis::cmd("SET")
                    .arg(key)
                    .arg(token)
                    .arg("PX")
                    .arg(30_i64)
                    .arg("NX"),
                Ok("OK"),
            ),
            MockCmd::new(
                ::redis::cmd("EVAL")
                    .arg(RELEASE_SCRIPT)
                    .arg(1)
                    .arg(key)
                    .arg(token),
                Ok(1_i64),
            ),
            MockCmd::new(
                ::redis::cmd("EVAL")
                    .arg(RENEW_SCRIPT)
                    .arg(1)
                    .arg(key)
                    .arg(token)
                    .arg(45_i64),
                Ok(0_i64),
            ),
        ])
        .assert_all_commands_consumed();

        let acquired: Option<String> = acquire_command(key, token, 30)
            .query(&mut connection)
            .expect("SET command should match");
        assert_eq!(acquired.as_deref(), Some("OK"));
        let released: i64 = release_command(key, token)
            .query(&mut connection)
            .expect("release script should match");
        assert_eq!(script_result(released), Ok(true));
        let renewed: i64 = renew_command(key, token, 45)
            .query(&mut connection)
            .expect("renew script should match");
        assert_eq!(script_result(renewed), Ok(false));
    }
}
