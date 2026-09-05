use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{codec, commands, error::RedisError};
use super::super::{backend::RedisClient, decode};

impl RedisClient {
    /// 以秒为单位设置 key 的 TTL；返回 key 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::expire::<&str>;
    /// ```
    pub fn expire<K: AsRef<[u8]>>(&self, key_value: K, ttl: Duration) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let seconds = commands::duration_seconds(ttl)?;
        let mut command = ::redis::cmd("EXPIRE");
        command.arg(key_value).arg(seconds);
        self.execute_sync(&command)
    }

    /// 以毫秒为单位设置 key 的 TTL；返回 key 是否存在。
    ///
    /// 这是无条件 `PEXPIRE`，不校验锁 token；不要直接用它续租由
    /// [`RedisClient::try_lock`] 获取的锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::pexpire::<&str>;
    /// ```
    pub fn pexpire<K: AsRef<[u8]>>(&self, key_value: K, ttl: Duration) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("PEXPIRE");
        command.arg(key_value).arg(millis);
        self.execute_sync(&command)
    }

    /// 删除 key 的 TTL；返回操作是否生效。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::persist::<&str>;
    /// ```
    pub fn persist<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PERSIST", [key_value]);
        self.execute_sync(&command)
    }

    /// 返回 Redis 原生 TTL 秒数；`-1` 表示无过期，`-2` 表示 key 不存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::ttl::<&str>;
    /// ```
    pub fn ttl<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("TTL", [key_value]);
        self.execute_sync(&command)
    }

    /// 返回 Redis 原生 TTL 毫秒数；`-1` 表示无过期，`-2` 表示 key 不存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::pttl::<&str>;
    /// ```
    pub fn pttl<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PTTL", [key_value]);
        self.execute_sync(&command)
    }

    /// 将 key 作为 Redis 原生十进制整数加一。它不兼容 MessagePack `set` 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::incr::<&str>;
    /// ```
    pub fn incr<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.incr_by(key_value, 1)
    }

    /// 将 key 作为 Redis 原生十进制整数增加指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::incr_by::<&str>;
    /// ```
    pub fn incr_by<K: AsRef<[u8]>>(&self, key_value: K, amount: i64) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut command = ::redis::cmd("INCRBY");
        command.arg(key_value).arg(amount);
        self.execute_sync(&command)
    }

    /// 将 key 作为 Redis 原生十进制整数减一。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::decr::<&str>;
    /// ```
    pub fn decr<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.decr_by(key_value, 1)
    }

    /// 将 key 作为 Redis 原生十进制整数减少指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::decr_by::<&str>;
    /// ```
    pub fn decr_by<K: AsRef<[u8]>>(&self, key_value: K, amount: i64) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut command = ::redis::cmd("DECRBY");
        command.arg(key_value).arg(amount);
        self.execute_sync(&command)
    }

    /// 从列表左侧压入一个 MessagePack 值，并返回列表长度。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lpush::<&str, u8>;
    /// ```
    pub fn lpush<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("LPUSH", [key_value, value]);
        self.execute_sync(&command)
    }

    /// 从列表右侧压入一个 MessagePack 值，并返回列表长度。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::rpush::<&str, u8>;
    /// ```
    pub fn rpush<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("RPUSH", [key_value, value]);
        self.execute_sync(&command)
    }

    /// 从列表左侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lpop::<&str, u8>;
    /// ```
    pub fn lpop<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("LPOP", [key_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 从列表右侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::rpop::<&str, u8>;
    /// ```
    pub fn rpop<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("RPOP", [key_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 读取列表范围，并返回 MessagePack 值集合。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lrange::<&str, u8>;
    /// ```
    pub fn lrange<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
        start: isize,
        stop: isize,
    ) -> Result<Vec<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        commands::check_lrange_request(start, stop, &self.inner.config)?;
        let mut command = ::redis::cmd("LRANGE");
        command.arg(key_value).arg(start).arg(stop);
        let values: Vec<Vec<u8>> = self.execute_sync(&command)?;
        decode::decode_collection(values, &self.inner.config)
    }

    /// 向集合加入一个 MessagePack 值，并返回新增成员数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::sadd::<&str, u8>;
    /// ```
    pub fn sadd<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SADD", [key_value, value]);
        self.execute_sync(&command)
    }

    /// 从集合移除一个 MessagePack 值，并返回实际移除成员数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::srem::<&str, u8>;
    /// ```
    pub fn srem<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SREM", [key_value, value]);
        self.execute_sync(&command)
    }

    /// 判断集合是否包含一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::sismember::<&str, u8>;
    /// ```
    pub fn sismember<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SISMEMBER", [key_value, value]);
        self.execute_sync(&command)
    }

    /// 读取集合全部 MessagePack 成员；Redis 不保证返回顺序。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::smembers::<&str, u8>;
    /// ```
    pub fn smembers<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Vec<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("SMEMBERS", [key_value]);
        let values: Vec<Vec<u8>> = self.execute_sync(&command)?;
        decode::decode_collection(values, &self.inner.config)
    }

    /// 向 Redis 发送 `PING` 并返回服务端响应。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::ping;
    /// ```
    pub fn ping(&self) -> Result<String, RedisError> {
        let command = ::redis::cmd("PING");
        self.execute_sync(&command)
    }
}
