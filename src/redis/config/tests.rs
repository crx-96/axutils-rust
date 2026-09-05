#[cfg(feature = "redis-cluster")]
use std::cell::Cell;
use std::time::Duration;

use super::{
    RedisConfig, DEFAULT_MAX_KEY_BYTES, MAX_BATCH_BYTES, MAX_BATCH_ITEMS, MAX_COLLECTION_ITEMS,
    MAX_POOL_SIZE, MAX_RESPONSE_BYTES, MAX_TRANSACTION_BYTES, MAX_TRANSACTION_COMMANDS,
    MAX_VALUE_BYTES,
};
#[cfg(feature = "redis-cluster")]
use super::{MAX_CLUSTER_NODES, MAX_ENDPOINT_BYTES};
use crate::redis::RedisError;

#[cfg(feature = "redis-cluster")]
#[test]
fn validates_scheme_database_and_bounded_nodes() {
    assert!(matches!(
        RedisConfig::single("rediss://127.0.0.1:6379"),
        Err(RedisError::InvalidConfig { field: "scheme" })
    ));
    assert!(matches!(
        RedisConfig::cluster(["redis://127.0.0.1:7000/1"]),
        Err(RedisError::InvalidConfig { field: "database" })
    ));
    let nodes =
        (0..=MAX_CLUSTER_NODES).map(|index| format!("redis://127.0.0.1:{}/0", 7000 + index));
    assert!(matches!(
        RedisConfig::cluster(nodes),
        Err(RedisError::InvalidConfig { field: "nodes" })
    ));
}

#[cfg(feature = "redis-cluster")]
#[test]
fn bounds_empty_and_overlong_cluster_inputs() {
    assert!(matches!(
        RedisConfig::cluster(std::iter::empty::<&str>()),
        Err(RedisError::InvalidConfig { field: "nodes" })
    ));

    let oversized = format!("redis://{}", "a".repeat(MAX_ENDPOINT_BYTES));
    assert!(matches!(
        RedisConfig::single(oversized),
        Err(RedisError::InvalidConfig { field: "url" })
    ));

    let pulls = Cell::new(0);
    let result = RedisConfig::cluster(std::iter::from_fn(|| {
        let index = pulls.get();
        pulls.set(index + 1);
        Some(format!("redis://127.0.0.1:{}/0", 7000 + index))
    }));
    assert!(matches!(
        result,
        Err(RedisError::InvalidConfig { field: "nodes" })
    ));
    assert_eq!(pulls.get(), MAX_CLUSTER_NODES + 1);
}

#[test]
fn validates_timeout_and_debug_redaction() {
    let config = RedisConfig::single("redis://:secret@redis.example.com:6379/0")
        .expect("config")
        .with_connection_timeout(Duration::ZERO);
    assert!(matches!(
        config,
        Err(RedisError::InvalidConfig {
            field: "connection_timeout"
        })
    ));

    let config = RedisConfig::single("redis://:secret@redis.example.com:6379/0").expect("config");
    let debug = format!("{config:?}");
    assert!(!debug.contains("redis.example.com"));
    assert!(!debug.contains("secret"));
    assert!(debug.contains("endpoint_count"));
}

#[test]
fn defaults_are_stable_and_debug_is_fully_redacted() {
    let config = RedisConfig::single("redis://:password@redis.example.com:6379/0").expect("config");
    assert_eq!(config.pool_size, super::DEFAULT_POOL_SIZE);
    assert_eq!(config.connection_timeout, super::DEFAULT_CONNECTION_TIMEOUT);
    assert_eq!(
        config.pool_checkout_timeout,
        super::DEFAULT_POOL_CHECKOUT_TIMEOUT
    );
    assert_eq!(config.response_timeout, super::DEFAULT_RESPONSE_TIMEOUT);
    assert_eq!(config.max_key_bytes, super::DEFAULT_MAX_KEY_BYTES);
    assert_eq!(config.max_value_bytes, super::DEFAULT_MAX_VALUE_BYTES);
    assert_eq!(config.max_batch_items, super::DEFAULT_MAX_BATCH_ITEMS);
    assert_eq!(config.max_batch_bytes, super::DEFAULT_MAX_BATCH_BYTES);
    assert_eq!(config.max_response_bytes, super::DEFAULT_MAX_RESPONSE_BYTES);
    assert_eq!(
        config.max_collection_items,
        super::DEFAULT_MAX_COLLECTION_ITEMS
    );
    assert_eq!(
        config.max_transaction_commands,
        super::DEFAULT_MAX_TRANSACTION_COMMANDS
    );
    assert_eq!(
        config.max_transaction_bytes,
        super::DEFAULT_MAX_TRANSACTION_BYTES
    );
    let debug = format!("{config:?}");
    assert!(!debug.contains("redis.example.com"));
    assert!(!debug.contains("password"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn rejects_zero_and_over_max_builder_limits() {
    let config = || RedisConfig::single("redis://127.0.0.1:6379/0").expect("config");
    let over_timeout = Duration::from_secs(5 * 60) + Duration::from_millis(1);

    assert!(matches!(
        config().with_pool_size(0),
        Err(RedisError::InvalidConfig { field: "pool_size" })
    ));
    assert!(matches!(
        config().with_pool_size(MAX_POOL_SIZE + 1),
        Err(RedisError::InvalidConfig { field: "pool_size" })
    ));
    assert!(matches!(
        config().with_connection_timeout(over_timeout),
        Err(RedisError::InvalidConfig {
            field: "connection_timeout"
        })
    ));
    assert!(matches!(
        config().with_pool_checkout_timeout(over_timeout),
        Err(RedisError::InvalidConfig {
            field: "pool_checkout_timeout"
        })
    ));
    assert!(matches!(
        config().with_response_timeout(over_timeout),
        Err(RedisError::InvalidConfig {
            field: "response_timeout"
        })
    ));
    assert!(matches!(
        config().with_max_key_bytes(0),
        Err(RedisError::InvalidConfig {
            field: "max_key_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_key_bytes(DEFAULT_MAX_KEY_BYTES + 1),
        Err(RedisError::InvalidConfig {
            field: "max_key_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_value_bytes(0),
        Err(RedisError::InvalidConfig {
            field: "max_value_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_value_bytes(MAX_VALUE_BYTES + 1),
        Err(RedisError::InvalidConfig {
            field: "max_value_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_batch_items(0),
        Err(RedisError::InvalidConfig {
            field: "max_batch_items"
        })
    ));
    assert!(matches!(
        config().with_max_batch_items(MAX_BATCH_ITEMS + 1),
        Err(RedisError::InvalidConfig {
            field: "max_batch_items"
        })
    ));
    assert!(matches!(
        config().with_max_batch_bytes(0),
        Err(RedisError::InvalidConfig {
            field: "max_batch_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_batch_bytes(MAX_BATCH_BYTES + 1),
        Err(RedisError::InvalidConfig {
            field: "max_batch_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_response_bytes(0),
        Err(RedisError::InvalidConfig {
            field: "max_response_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_response_bytes(MAX_RESPONSE_BYTES + 1),
        Err(RedisError::InvalidConfig {
            field: "max_response_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_collection_items(0),
        Err(RedisError::InvalidConfig {
            field: "max_collection_items"
        })
    ));
    assert!(matches!(
        config().with_max_collection_items(MAX_COLLECTION_ITEMS + 1),
        Err(RedisError::InvalidConfig {
            field: "max_collection_items"
        })
    ));
    assert!(matches!(
        config().with_max_transaction_commands(0),
        Err(RedisError::InvalidConfig {
            field: "max_transaction_commands"
        })
    ));
    assert!(matches!(
        config().with_max_transaction_commands(MAX_TRANSACTION_COMMANDS + 1),
        Err(RedisError::InvalidConfig {
            field: "max_transaction_commands"
        })
    ));
    assert!(matches!(
        config().with_max_transaction_bytes(0),
        Err(RedisError::InvalidConfig {
            field: "max_transaction_bytes"
        })
    ));
    assert!(matches!(
        config().with_max_transaction_bytes(MAX_TRANSACTION_BYTES + 1),
        Err(RedisError::InvalidConfig {
            field: "max_transaction_bytes"
        })
    ));
}
