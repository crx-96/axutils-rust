#![cfg(feature = "redis")]

use axutils::{RedisConfig, RedisError, RedisUtils};

#[test]
fn global_entry_reports_state_without_exposing_a_client() {
    assert!(!RedisUtils::is_initialized());
    assert!(matches!(
        RedisUtils::get::<_, u8>("missing"),
        Err(RedisError::NotInitialized)
    ));
    assert!(matches!(
        RedisConfig::single("rediss://127.0.0.1:6379/0"),
        Err(RedisError::InvalidConfig { field: "scheme" })
    ));

    let results = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    RedisUtils::init(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("initialization thread should not panic")
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(RedisError::AlreadyInitialized)))
            .count(),
        7
    );
    assert!(RedisUtils::is_initialized());
    assert!(matches!(
        RedisUtils::init(RedisConfig::single("redis://127.0.0.1:6380/0").unwrap()),
        Err(RedisError::AlreadyInitialized)
    ));
    assert_eq!(RedisUtils::get_bytes(""), Err(RedisError::InvalidKey));
}
