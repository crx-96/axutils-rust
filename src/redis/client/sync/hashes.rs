use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{codec, commands, error::RedisError};
use super::super::{backend::RedisClient, decode, input};

impl RedisClient {
    /// 读取 Hash 中的 MessagePack 值；field 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hget::<&str, &str, u8>;
    /// ```
    pub fn hget<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<T>, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        // 缺失值保留 None；存在值经单值预算检查后才返回或反序列化。
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 读取 Hash 中的 raw 字节；field 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hget_bytes::<&str, &str>;
    /// ```
    pub fn hget_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        // 缺失值保留 None；存在值经单值预算检查后才返回或反序列化。
        value
            .map(|bytes| commands::check_value_response(&bytes, &self.inner.config).map(|()| bytes))
            .transpose()
    }

    /// 写入一个 MessagePack Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset::<&str, &str, u8>;
    /// ```
    pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        field_value: F,
        value: T,
    ) -> Result<u64, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_sync(&command)
    }

    /// 写入一个 raw Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_bytes::<&str, &str, Vec<u8>>;
    /// ```
    pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
        value: V,
    ) -> Result<u64, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_sync(&command)
    }

    /// 读取 Hash 全部 field 和 MessagePack 值，保留 Redis 返回顺序。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hgetall::<&str, u8>;
    /// ```
    pub fn hgetall<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, T)>, RedisError> {
        // 先复用 raw 响应的结构和预算检查，再按项解码，不返回部分成功结果。
        let entries = self.hgetall_bytes(key_value)?;
        entries
            .into_iter()
            .map(|(field_value, bytes)| {
                codec::decode(&bytes, self.inner.config.max_value_bytes)
                    .map(|value| (field_value, value))
            })
            .collect()
    }

    /// 读取 Hash 全部 field 和 raw 值，保留 Redis 返回顺序。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hgetall_bytes::<&str>;
    /// ```
    #[allow(clippy::type_complexity)]
    pub fn hgetall_bytes<K: AsRef<[u8]>>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HGETALL", [key_value]);
        let flat: Vec<Vec<u8>> = self.execute_sync(&command)?;
        decode::decode_hash_entries(flat, &self.inner.config)
    }

    /// 删除一个 Hash field，并返回实际删除数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hdel::<&str, &str>;
    /// ```
    pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<u64, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HDEL", [key_value, field_value]);
        self.execute_sync(&command)
    }

    /// 判断 Hash field 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hexists::<&str, &str>;
    /// ```
    pub fn hexists<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<bool, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HEXISTS", [key_value, field_value]);
        self.execute_sync(&command)
    }

    /// 返回 Hash field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hlen::<&str>;
    /// ```
    pub fn hlen<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        // 在取得连接前校验并拥有 key；后续参数校验失败也不会发送命令。
        let key_value = commands::key(key_value, &self.inner.config)?;
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HLEN", [key_value]);
        self.execute_sync(&command)
    }

    /// 有界批量写入 MessagePack Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_many::<[(&str, u8); 1], &str, &str, u8>;
    /// ```
    pub fn hset_many<I, K, F, T>(&self, key_value: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, T)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        T: Serialize,
    {
        // 共享参数收集器按数量、字节和适用的 slot 约束完整校验后才返回命令参数。
        let args = input::collect_hash_pairs(key_value, entries, &self.inner.config)?;
        // 只有 Hash key 而没有 field/value 时，保持本地空操作。
        if args.len() == 1 {
            return Ok(0);
        }
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HSET", args);
        self.execute_sync(&command)
    }

    /// 有界批量写入 raw Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    /// ```
    pub fn hset_many_bytes<I, K, F, V>(&self, key_value: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, V)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        // 共享参数收集器按数量、字节和适用的 slot 约束完整校验后才返回命令参数。
        let args = input::collect_hash_raw_pairs(key_value, entries, &self.inner.config)?;
        // 只有 Hash key 而没有 field/value 时，保持本地空操作。
        if args.len() == 1 {
            return Ok(0);
        }
        // 按 Redis 参数顺序装配已校验输入，二进制内容不经字符串拼接。
        let command = commands::command("HSET", args);
        self.execute_sync(&command)
    }
}
