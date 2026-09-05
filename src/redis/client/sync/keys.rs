use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{codec, commands, error::RedisError};
use super::super::{backend::RedisClient, input};

impl RedisClient {
    /// 删除一个 key 并返回实际删除数量。
    ///
    /// 这是无条件 `DEL`，不校验锁 token；不要直接用它释放由
    /// [`RedisClient::try_lock`] 获取的锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::delete::<&str>;
    /// ```
    pub fn delete<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("DEL", [key_value]);
        self.execute_sync(&command)
    }

    /// 有界批量删除 key，并返回实际删除数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::delete_many::<[&str; 1], &str>;
    /// ```
    pub fn delete_many<I, K>(&self, keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = input::collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(0);
        }
        let command = commands::command("DEL", keys);
        self.execute_sync(&command)
    }

    /// 判断 key 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::exists::<&str>;
    /// ```
    pub fn exists<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("EXISTS", [key_value]);
        self.execute_sync(&command)
    }

    /// 按输入顺序批量读取 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mget::<[&str; 1], &str, u8>;
    /// ```
    pub fn mget<I, K, T>(&self, keys: I) -> Result<Vec<Option<T>>, RedisError>
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
        let values: Vec<Option<Vec<u8>>> = self.execute_sync(&command)?;
        let mut response_bytes = 0;
        values
            .into_iter()
            .map(|value| {
                value
                    .map(|bytes| {
                        response_bytes = commands::add_response_bytes(
                            response_bytes,
                            &bytes,
                            &self.inner.config,
                        )?;
                        codec::decode(&bytes, self.inner.config.max_value_bytes)
                    })
                    .transpose()
            })
            .collect()
    }

    /// 按输入顺序批量读取 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mget_bytes::<[&str; 1], &str>;
    /// ```
    pub fn mget_bytes<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = input::collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let command = commands::command("MGET", keys);
        let values: Vec<Option<Vec<u8>>> = self.execute_sync(&command)?;
        let mut response_bytes = 0;
        values
            .into_iter()
            .map(|value| {
                value
                    .map(|bytes| {
                        response_bytes = commands::add_response_bytes(
                            response_bytes,
                            &bytes,
                            &self.inner.config,
                        )?;
                        Ok(bytes)
                    })
                    .transpose()
            })
            .collect()
    }

    /// 有界批量写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mset::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub fn mset<I, K, T>(&self, entries: I) -> Result<(), RedisError>
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
        self.execute_sync::<()>(&command)
    }

    /// 有界批量写入 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub fn mset_bytes<I, K, V>(&self, entries: I) -> Result<(), RedisError>
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
        self.execute_sync::<()>(&command)
    }
}
