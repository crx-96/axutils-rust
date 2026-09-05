#![cfg(feature = "redis")]

use std::time::Duration;

use axutils::{
    redis::{RedisClient, RedisConfig, RedisError},
    utils::RedisUtils,
};

#[test]
fn config_defaults_and_boundaries_are_local() {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .expect("local URL should parse")
        .with_pool_size(8)
        .expect("pool size")
        .with_connection_timeout(Duration::from_secs(5))
        .expect("connection timeout")
        .with_pool_checkout_timeout(Duration::from_secs(5))
        .expect("checkout timeout")
        .with_response_timeout(Duration::from_secs(30))
        .expect("response timeout")
        .with_max_key_bytes(16 * 1024)
        .expect("key limit")
        .with_max_value_bytes(64 * 1024 * 1024)
        .expect("value limit")
        .with_max_batch_items(16_384)
        .expect("batch items")
        .with_max_batch_bytes(256 * 1024 * 1024)
        .expect("batch bytes")
        .with_max_response_bytes(256 * 1024 * 1024)
        .expect("response bytes")
        .with_max_collection_items(65_536)
        .expect("collection items")
        .with_max_transaction_commands(1_024)
        .expect("transaction commands")
        .with_max_transaction_bytes(256 * 1024 * 1024)
        .expect("transaction bytes");

    let debug = format!("{config:?}");
    assert!(!debug.contains("127.0.0.1"));
    assert!(debug.contains("endpoint_count"));
    assert!(matches!(
        config.with_pool_size(0),
        Err(RedisError::InvalidConfig { field: "pool_size" })
    ));
}

#[test]
fn rejects_invalid_single_node_urls() {
    assert!(matches!(
        RedisConfig::single("rediss://127.0.0.1:6379/0"),
        Err(RedisError::InvalidConfig { field: "scheme" })
    ));
    assert!(matches!(
        RedisConfig::single("redis://127.0.0.1:6379/1\n"),
        Err(RedisError::InvalidConfig { field: "url" })
    ));
}

#[cfg(feature = "redis-cluster")]
#[test]
fn rejects_inconsistent_cluster_credentials() {
    assert!(matches!(
        RedisConfig::cluster([
            "redis://user:one@127.0.0.1:7000/0",
            "redis://user:two@127.0.0.1:7001/0",
        ]),
        Err(RedisError::InvalidConfig {
            field: "credentials"
        })
    ));
}

#[test]
fn client_construction_and_local_validation_do_not_connect() {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
        .expect("client construction should be lazy");
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RedisClient>();
    assert_send_sync::<RedisUtils>();

    assert_eq!(client.get_bytes(""), Err(RedisError::InvalidKey));
    assert_eq!(client.delete_many(std::iter::empty::<&str>()), Ok(0));
    assert_eq!(
        client.mget_bytes(std::iter::empty::<&str>()),
        Ok(Vec::new())
    );
    assert_eq!(
        client.mset_bytes(std::iter::empty::<(&str, &[u8; 0])>()),
        Ok(())
    );
}

#[test]
fn lock_validation_is_local_and_enforces_the_24_hour_bound() {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
        .expect("client construction should be lazy");

    assert!(matches!(
        client.try_lock("", Duration::from_secs(1)),
        Err(RedisError::InvalidKey)
    ));
    for ttl in [
        Duration::ZERO,
        Duration::from_secs(24 * 60 * 60 + 1),
        Duration::MAX,
    ] {
        assert!(matches!(
            client.try_lock("lock:key", ttl),
            Err(RedisError::InvalidConfig { field: "ttl" })
        ));
    }
}

#[cfg(feature = "redis-async")]
#[tokio::test]
async fn async_lock_validation_is_local_and_enforces_the_24_hour_bound() {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
        .expect("client construction should be lazy");

    assert!(matches!(
        client.try_lock_async("", Duration::from_secs(1)).await,
        Err(RedisError::InvalidKey)
    ));
    for ttl in [
        Duration::ZERO,
        Duration::from_secs(24 * 60 * 60 + 1),
        Duration::MAX,
    ] {
        assert!(matches!(
            client.try_lock_async("lock:key", ttl).await,
            Err(RedisError::InvalidConfig { field: "ttl" })
        ));
    }
}

#[test]
fn transaction_callback_and_queue_limits_are_local() {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .unwrap()
        .with_max_transaction_commands(1)
        .unwrap();
    let client = RedisClient::new(config).unwrap();
    assert_eq!(
        client.transaction(|tx| tx.set("", 1_u8)),
        Err(RedisError::InvalidKey)
    );
    assert_eq!(client.transaction(|_| Ok(())), Ok(()));

    assert_eq!(
        client.transaction(|tx| {
            tx.set("key", 1_u8)?;
            tx.set("second", 2_u8)
        }),
        Err(RedisError::ValueTooLarge { limit: 1 })
    );
}
