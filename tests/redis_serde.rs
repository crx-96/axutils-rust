#![cfg(feature = "redis")]

use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use axutils::{RedisClient, RedisConfig, RedisError};

struct FailingValue;

impl Serialize for FailingValue {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(S::Error::custom("intentional test failure"))
    }
}

#[test]
fn serialization_failure_is_stable_and_does_not_connect() {
    let client =
        RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap()).unwrap();
    assert_eq!(client.set("key", FailingValue), Err(RedisError::Serialize));
    assert_eq!(
        client.set_with_expiry("key", FailingValue, std::time::Duration::from_secs(1)),
        Err(RedisError::Serialize)
    );
}

#[test]
fn messagepack_queue_accepts_nested_owned_values() {
    let client =
        RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap()).unwrap();
    let result = client.transaction(|tx| {
        tx.set("nested", vec![Some("a"), None, Some("b")])?;
        tx.hset("hash", "field", ("tuple", 7_u8))?;
        Err(RedisError::InvalidKey)
    });
    // callback 返回错误时不会 checkout 连接；该测试只验证排队阶段可接受嵌套值。
    assert_eq!(result, Err(RedisError::InvalidKey));
}
