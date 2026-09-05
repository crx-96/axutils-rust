use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{codec, commands, error::RedisError};
use super::super::{backend::RedisClient, decode, input};

impl RedisClient {
    /// 异步读取 Hash 中的 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hget_async::<&str, &str, u8>;
    /// ```
    pub async fn hget_async<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 异步读取 Hash 中的 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hget_bytes_async::<&str, &str>;
    /// ```
    pub async fn hget_bytes_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| commands::check_value_response(&bytes, &self.inner.config).map(|()| bytes))
            .transpose()
    }

    /// 异步写入一个 MessagePack Hash field。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_async::<&str, &str, u8>;
    /// ```
    pub async fn hset_async<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        field_value: F,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_async(&command).await
    }

    /// 异步写入一个 raw Hash field。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_bytes_async::<&str, &str, Vec<u8>>;
    /// ```
    pub async fn hset_bytes_async<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
        value: V,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_async(&command).await
    }

    /// 异步读取 Hash 全部 MessagePack field/value。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hgetall_async::<&str, u8>;
    /// ```
    pub async fn hgetall_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, T)>, RedisError> {
        let entries = self.hgetall_bytes_async(key_value).await?;
        entries
            .into_iter()
            .map(|(field_value, bytes)| {
                codec::decode(&bytes, self.inner.config.max_value_bytes)
                    .map(|value| (field_value, value))
            })
            .collect()
    }

    /// 异步读取 Hash 全部 raw field/value。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hgetall_bytes_async::<&str>;
    /// ```
    pub async fn hgetall_bytes_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("HGETALL", [key_value]);
        let flat: Vec<Vec<u8>> = self.execute_async(&command).await?;
        decode::decode_hash_entries(flat, &self.inner.config)
    }

    /// 异步删除一个 Hash field。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hdel_async::<&str, &str>;
    /// ```
    pub async fn hdel_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HDEL", [key_value, field_value]);
        self.execute_async(&command).await
    }

    /// 异步判断 Hash field 是否存在。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hexists_async::<&str, &str>;
    /// ```
    pub async fn hexists_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HEXISTS", [key_value, field_value]);
        self.execute_async(&command).await
    }

    /// 异步返回 Hash field 数量。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hlen_async::<&str>;
    /// ```
    pub async fn hlen_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("HLEN", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步有界批量写入 MessagePack Hash field。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
    /// ```
    pub async fn hset_many_async<I, K, F, T>(
        &self,
        key_value: K,
        entries: I,
    ) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, T)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        T: Serialize,
    {
        let args = input::collect_hash_pairs(key_value, entries, &self.inner.config)?;
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_async(&command).await
    }

    /// 异步有界批量写入 raw Hash field。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    /// ```
    pub async fn hset_many_bytes_async<I, K, F, V>(
        &self,
        key_value: K,
        entries: I,
    ) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, V)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let args = input::collect_hash_raw_pairs(key_value, entries, &self.inner.config)?;
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_async(&command).await
    }
}
