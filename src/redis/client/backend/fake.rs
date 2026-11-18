//! 仅单元测试使用的无网络后端；记录 checkout 和命令次数。

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use ::redis::Value;

#[cfg(feature = "redis-async")]
use super::AsyncBackend;
use super::{RedisClient, RedisClientInner, SyncBackend};
use crate::redis::{RedisConfig, RedisError};

#[derive(Clone)]
pub(crate) struct TestRedisBackend {
    checkout_count: Arc<AtomicUsize>,
    command_count: Arc<AtomicUsize>,
    result: Result<Value, RedisError>,
}

impl TestRedisBackend {
    fn new(result: Result<Value, RedisError>) -> Self {
        Self {
            checkout_count: Arc::new(AtomicUsize::new(0)),
            command_count: Arc::new(AtomicUsize::new(0)),
            result,
        }
    }

    pub(crate) fn checkout_count(&self) -> usize {
        self.checkout_count.load(Ordering::Relaxed)
    }

    pub(crate) fn command_count(&self) -> usize {
        self.command_count.load(Ordering::Relaxed)
    }

    pub(in crate::redis::client) fn execute<T: ::redis::FromRedisValue>(
        &self,
    ) -> Result<T, RedisError> {
        self.checkout_count.fetch_add(1, Ordering::Relaxed);
        self.command_count.fetch_add(1, Ordering::Relaxed);
        T::from_redis_value(self.result.clone()?)
            .map_err(|error| RedisError::from_upstream(&error.into()))
    }
}

impl RedisClient {
    pub(crate) fn test_fake(result: Result<i64, RedisError>) -> (Self, TestRedisBackend) {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .expect("test fake Redis URL should be valid");
        Self::test_response(config, result.map(Value::Int))
    }

    pub(in crate::redis::client) fn test_response(
        config: RedisConfig,
        result: Result<Value, RedisError>,
    ) -> (Self, TestRedisBackend) {
        let backend = TestRedisBackend::new(result);
        let sync = SyncBackend::Fake(Arc::new(backend.clone()));
        #[cfg(feature = "redis-async")]
        let async_backend = AsyncBackend::Fake(Arc::new(backend.clone()));

        (
            Self {
                inner: Arc::new(RedisClientInner {
                    config,
                    sync,
                    #[cfg(feature = "redis-async")]
                    async_backend,
                }),
            },
            backend,
        )
    }
}
