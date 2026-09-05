use std::time::Duration;

use serde::Serialize;

use super::{
    codec,
    commands::{add_transaction_bytes, command, duration_millis, duration_seconds},
    config::RedisConfig,
    error::RedisError,
};

/// 单机 Redis 原子事务的本地排队上下文。
///
/// 该类型不持有连接，排队方法只做参数校验和 MessagePack 编码；网络操作在
/// [`crate::redis::RedisClient::transaction`] 或 `transaction_async` 的 callback 返回后才发生。
/// 第一阶段不提供读取、`WATCH`、CAS、自动重试或 callback 重放语义。
pub struct RedisTransaction {
    commands: Vec<::redis::Cmd>,
    encoded_bytes: usize,
    max_commands: usize,
    max_bytes: usize,
    max_value_bytes: usize,
    max_key_bytes: usize,
}

impl RedisTransaction {
    pub(crate) fn new(config: &RedisConfig) -> Self {
        Self {
            commands: Vec::new(),
            encoded_bytes: 0,
            max_commands: config.max_transaction_commands,
            max_bytes: config.max_transaction_bytes,
            max_value_bytes: config.max_value_bytes,
            max_key_bytes: config.max_key_bytes,
        }
    }

    /// 排队一个 MessagePack `SET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::set::<&str, u8>;
    /// ```
    pub fn set<K: AsRef<[u8]>, T: Serialize>(
        &mut self,
        key_value: K,
        value: T,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let value = codec::encode(&value, self.max_value_bytes)?;
        self.push(command("SET", [key_value, value]))
    }

    /// 排队一个带毫秒 TTL 的 MessagePack `SET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::set_with_expiry::<&str, u8>;
    /// ```
    pub fn set_with_expiry<K: AsRef<[u8]>, T: Serialize>(
        &mut self,
        key_value: K,
        value: T,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let value = codec::encode(&value, self.max_value_bytes)?;
        let millis = duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("PX").arg(millis);
        self.push(command)
    }

    /// 排队一个 raw `SET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::set_bytes::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &mut self,
        key_value: K,
        value: V,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let value = codec::raw(value, self.max_value_bytes)?;
        self.push(command("SET", [key_value, value]))
    }

    /// 排队一个带毫秒 TTL 的 raw `SET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::set_bytes_with_expiry::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &mut self,
        key_value: K,
        value: V,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let value = codec::raw(value, self.max_value_bytes)?;
        let millis = duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("PX").arg(millis);
        self.push(command)
    }

    /// 排队单 key `DEL`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::delete::<&str>;
    /// ```
    pub fn delete<K: AsRef<[u8]>>(&mut self, key_value: K) -> Result<(), RedisError> {
        self.push(command("DEL", [self.key(key_value)?]))
    }

    /// 排队 MessagePack `HSET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::hset::<&str, &str, u8>;
    /// ```
    pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        &mut self,
        key_value: K,
        field_value: F,
        value: T,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let field_value = self.field(field_value)?;
        let value = codec::encode(&value, self.max_value_bytes)?;
        self.push(command("HSET", [key_value, field_value, value]))
    }

    /// 排队 raw `HSET`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::hset_bytes::<&str, &str, Vec<u8>>;
    /// ```
    pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        &mut self,
        key_value: K,
        field_value: F,
        value: V,
    ) -> Result<(), RedisError> {
        let key_value = self.key(key_value)?;
        let field_value = self.field(field_value)?;
        let value = codec::raw(value, self.max_value_bytes)?;
        self.push(command("HSET", [key_value, field_value, value]))
    }

    /// 排队单 field `HDEL`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::hdel::<&str, &str>;
    /// ```
    pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &mut self,
        key_value: K,
        field_value: F,
    ) -> Result<(), RedisError> {
        self.push(command(
            "HDEL",
            [self.key(key_value)?, self.field(field_value)?],
        ))
    }

    /// 排队以秒为单位的 `EXPIRE`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::expire::<&str>;
    /// ```
    pub fn expire<K: AsRef<[u8]>>(
        &mut self,
        key_value: K,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let seconds = duration_seconds(ttl)?;
        let mut command = ::redis::cmd("EXPIRE");
        command.arg(self.key(key_value)?).arg(seconds);
        self.push(command)
    }

    /// 排队 `PERSIST`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisTransaction;
    ///
    /// let _ = RedisTransaction::persist::<&str>;
    /// ```
    pub fn persist<K: AsRef<[u8]>>(&mut self, key_value: K) -> Result<(), RedisError> {
        self.push(command("PERSIST", [self.key(key_value)?]))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub(crate) fn commands(&self) -> &[::redis::Cmd] {
        &self.commands
    }

    fn key<K: AsRef<[u8]>>(&self, value: K) -> Result<Vec<u8>, RedisError> {
        let value = value.as_ref();
        if value.is_empty() || value.len() > self.max_key_bytes {
            return Err(RedisError::InvalidKey);
        }
        Ok(value.to_vec())
    }

    fn field<F: AsRef<[u8]>>(&self, value: F) -> Result<Vec<u8>, RedisError> {
        let value = value.as_ref();
        if value.is_empty() || value.len() > self.max_key_bytes {
            return Err(RedisError::InvalidField);
        }
        Ok(value.to_vec())
    }

    fn push(&mut self, command: ::redis::Cmd) -> Result<(), RedisError> {
        if self.commands.len() >= self.max_commands {
            return Err(RedisError::ValueTooLarge {
                limit: self.max_commands,
            });
        }
        let bytes = command.get_packed_command().len();
        self.encoded_bytes = add_transaction_bytes(self.encoded_bytes, bytes, self.max_bytes)?;
        self.commands.push(command);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RedisTransaction;
    use crate::redis::{RedisConfig, RedisError};

    #[test]
    fn transaction_queue_is_local_and_bounded() {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .unwrap()
            .with_max_transaction_commands(1)
            .unwrap();
        let mut transaction = RedisTransaction::new(&config);
        transaction.set("key", "value").unwrap();
        assert_eq!(
            transaction.set("second", "value"),
            Err(RedisError::ValueTooLarge { limit: 1 })
        );

        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .unwrap()
            .with_max_transaction_bytes(1)
            .unwrap();
        let mut transaction = RedisTransaction::new(&config);
        assert_eq!(
            transaction.set("key", "value"),
            Err(RedisError::ValueTooLarge { limit: 1 })
        );
    }

    #[test]
    fn invalid_ttl_does_not_queue_a_command() {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
        let mut transaction = RedisTransaction::new(&config);
        assert_eq!(
            transaction.set_with_expiry("key", "value", Duration::ZERO),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
        assert!(transaction.is_empty());
    }
}
