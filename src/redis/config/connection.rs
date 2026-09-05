use std::time::Duration;

use super::validation;
use super::{RedisConfig, MAX_POOL_SIZE};
use crate::redis::RedisError;

impl RedisConfig {
    /// 设置同步连接池最大连接数，范围为 `1..=64`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_pool_size(4)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_pool_size(mut self, max: usize) -> Result<Self, RedisError> {
        self.pool_size = validation::bounded(max, 1, MAX_POOL_SIZE, "pool_size")?;
        Ok(self)
    }

    /// 设置建立网络连接的时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_connection_timeout(Duration::from_secs(2))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.connection_timeout = validation::checked_timeout(timeout, "connection_timeout")?;
        Ok(self)
    }

    /// 设置同步连接池 checkout 的等待时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_pool_checkout_timeout(Duration::from_secs(2))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_pool_checkout_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.pool_checkout_timeout = validation::checked_timeout(timeout, "pool_checkout_timeout")?;
        Ok(self)
    }

    /// 设置 Redis 命令响应时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_response_timeout(Duration::from_secs(10))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_response_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.response_timeout = validation::checked_timeout(timeout, "response_timeout")?;
        Ok(self)
    }
}
