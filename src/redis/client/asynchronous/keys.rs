use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{commands, error::RedisError};
use super::super::{backend::RedisClient, decode, input};

impl RedisClient {
    /// 异步仅在 key 不存在时写入 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_nx_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_nx_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("NX");
        let result: Option<String> = self.execute_async(&command).await?;
        Ok(result.is_some())
    }

    /// 异步仅在 key 不存在时以 `SET ... PX NX` 写入带 TTL 的 raw 字节。
    ///
    /// 这是通用 NX 写入原语，不记录所有者，也不会自动删除；锁场景应使用
    /// [`RedisClient::try_lock_async`]。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_nx_with_expiry_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
        ttl: Duration,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command
            .arg(key_value)
            .arg(value)
            .arg("PX")
            .arg(millis)
            .arg("NX");
        let result: Option<String> = self.execute_async(&command).await?;
        Ok(result.is_some())
    }

    /// 异步删除一个 key 并返回实际删除数量。
    ///
    /// 这是无条件 `DEL`，不校验锁 token；不要直接用它释放由
    /// [`RedisClient::try_lock_async`] 获取的锁。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::delete_async::<&str>;
    /// ```
    pub async fn delete_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("DEL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步有界批量删除 key。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::delete_many_async::<[&str; 1], &str>;
    /// ```
    pub async fn delete_many_async<I, K>(&self, keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = input::collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(0);
        }
        let command = commands::command("DEL", keys);
        self.execute_async(&command).await
    }

    /// 异步判断 key 是否存在。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::exists_async::<&str>;
    /// ```
    pub async fn exists_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("EXISTS", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步按输入顺序批量读取 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mget_async::<[&str; 1], &str, u8>;
    /// ```
    pub async fn mget_async<I, K, T>(&self, keys: I) -> Result<Vec<Option<T>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
        T: DeserializeOwned,
    {
        let keys = input::collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let command = commands::command("MGET", keys);
        let values: Vec<Option<Vec<u8>>> = self.execute_async(&command).await?;
        decode::decode_optional_values(values, &self.inner.config)
    }

    /// 异步按输入顺序批量读取 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mget_bytes_async::<[&str; 1], &str>;
    /// ```
    pub async fn mget_bytes_async<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = input::collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let command = commands::command("MGET", keys);
        let values: Vec<Option<Vec<u8>>> = self.execute_async(&command).await?;
        decode::check_optional_values(values, &self.inner.config)
    }

    /// 异步有界批量写入 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mset_async::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub async fn mset_async<I, K, T>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, T)>,
        K: AsRef<[u8]>,
        T: Serialize,
    {
        let args = input::collect_value_pairs(entries, &self.inner.config)?;
        if args.is_empty() {
            return Ok(());
        }
        let command = commands::command("MSET", args);
        self.execute_async::<()>(&command).await
    }

    /// 异步有界批量写入 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub async fn mset_bytes_async<I, K, V>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let args = input::collect_raw_pairs(entries, &self.inner.config)?;
        if args.is_empty() {
            return Ok(());
        }
        let command = commands::command("MSET", args);
        self.execute_async::<()>(&command).await
    }
}
