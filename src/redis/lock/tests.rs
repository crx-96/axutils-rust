use std::time::Duration;

use redis_test::{MockCmd, MockRedisConnection};

use super::{
    acquire_command, finish_release, finish_renew, lock_ttl_duration, lock_ttl_millis,
    release_command, renew_command, script_result, token, token_with_rng, RELEASE_SCRIPT,
    RENEW_SCRIPT,
};
#[cfg(feature = "redis-async")]
use crate::redis::RedisAsyncLockGuard;
use crate::redis::{RedisClient, RedisConfig, RedisError, RedisLockGuard, RedisTransportErrorKind};

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
    let error = RedisError::Transport(RedisTransportErrorKind::Network);
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
        Err(RedisError::Transport(RedisTransportErrorKind::Other))
    );
}

#[test]
fn guard_debug_omits_key_and_token() {
    let client =
        RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config"))
            .expect("client construction should be local");
    let mut guard = RedisLockGuard::new(
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
    let mut guard = RedisLockGuard::new(
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
        let _guard = RedisLockGuard::new(
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
    let mut guard = RedisLockGuard::new(
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
    let error = RedisError::Transport(RedisTransportErrorKind::Network);
    let (client, backend) = RedisClient::test_fake(Err(error));
    let mut guard = RedisLockGuard::new(
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
    let mut guard = RedisLockGuard::new(
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
        let _guard = RedisLockGuard::new(
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

#[cfg(feature = "redis-async")]
#[test]
fn async_guard_debug_omits_key_and_token() {
    let client =
        RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config"))
            .expect("client construction should be local");
    let mut guard = RedisAsyncLockGuard::new(
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

#[cfg(feature = "redis-async")]
#[tokio::test(flavor = "current_thread")]
async fn active_async_guard_drop_does_not_checkout_or_send_command() {
    let (client, backend) = RedisClient::test_fake(Ok(1));
    {
        let _guard = RedisAsyncLockGuard::new(
            client,
            b"async-drop-lock-key".to_vec(),
            [b'D'; 32],
            Duration::from_secs(30),
        );
    }

    assert_eq!(backend.checkout_count(), 0);
    assert_eq!(backend.command_count(), 0);
}

#[cfg(feature = "redis-async")]
#[tokio::test(flavor = "current_thread")]
async fn explicit_async_release_is_observable_and_repeated_release_is_local() {
    let (client, backend) = RedisClient::test_fake(Ok(1));
    let mut guard = RedisAsyncLockGuard::new(
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

#[cfg(feature = "redis-async")]
#[tokio::test(flavor = "current_thread")]
async fn async_release_error_is_observable_and_drop_does_not_retry() {
    let error = RedisError::Transport(RedisTransportErrorKind::Network);
    let (client, backend) = RedisClient::test_fake(Err(error));
    let mut guard = RedisAsyncLockGuard::new(
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
        Err(RedisError::Transport(RedisTransportErrorKind::Protocol))
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
