use super::validation;
use super::{
    RedisConfig, DEFAULT_MAX_KEY_BYTES, MAX_BATCH_BYTES, MAX_BATCH_ITEMS, MAX_COLLECTION_ITEMS,
    MAX_RESPONSE_BYTES, MAX_TRANSACTION_BYTES, MAX_TRANSACTION_COMMANDS, MAX_VALUE_BYTES,
};
use crate::redis::RedisError;

impl RedisConfig {
    /// 设置 key 和 Hash field 的最大字节数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_key_bytes(1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_key_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_key_bytes = validation::bounded(limit, 1, DEFAULT_MAX_KEY_BYTES, "max_key_bytes")?;
        Ok(self)
    }

    /// 设置单值最大原始字节数，最大为 64 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_value_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_value_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_value_bytes = validation::bounded(limit, 1, MAX_VALUE_BYTES, "max_value_bytes")?;
        Ok(self)
    }

    /// 设置批量 key/field 的最大项数，最大为 16,384。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_batch_items(128)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_batch_items(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_batch_items = validation::bounded(limit, 1, MAX_BATCH_ITEMS, "max_batch_items")?;
        Ok(self)
    }

    /// 设置批量命令编码总字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_batch_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_batch_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_batch_bytes = validation::bounded(limit, 1, MAX_BATCH_BYTES, "max_batch_bytes")?;
        Ok(self)
    }

    /// 设置多项响应累计字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_response_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_response_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_response_bytes =
            validation::bounded(limit, 1, MAX_RESPONSE_BYTES, "max_response_bytes")?;
        Ok(self)
    }

    /// 设置集合读取最大项数，最大为 65,536。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_collection_items(256)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_collection_items(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_collection_items =
            validation::bounded(limit, 1, MAX_COLLECTION_ITEMS, "max_collection_items")?;
        Ok(self)
    }

    /// 设置事务最大排队命令数，最大为 1,024。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_transaction_commands(64)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_transaction_commands(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_transaction_commands = validation::bounded(
            limit,
            1,
            MAX_TRANSACTION_COMMANDS,
            "max_transaction_commands",
        )?;
        Ok(self)
    }

    /// 设置事务编码总字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_transaction_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_transaction_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_transaction_bytes =
            validation::bounded(limit, 1, MAX_TRANSACTION_BYTES, "max_transaction_bytes")?;
        Ok(self)
    }
}
