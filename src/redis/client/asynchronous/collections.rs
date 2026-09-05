use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{codec, commands, error::RedisError};
use super::super::{backend::RedisClient, decode};

impl RedisClient {
    /// 异步以秒为单位设置 key 的 TTL。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::expire_async::<&str>;
    /// ```
    pub async fn expire_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        ttl: Duration,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let seconds = commands::duration_seconds(ttl)?;
        let mut command = ::redis::cmd("EXPIRE");
        command.arg(key_value).arg(seconds);
        self.execute_async(&command).await
    }

    /// 异步以毫秒为单位设置 key 的 TTL。
    ///
    /// 这是无条件 `PEXPIRE`，不校验锁 token；不要直接用它续租由
    /// [`RedisClient::try_lock_async`] 获取的锁。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::pexpire_async::<&str>;
    /// ```
    pub async fn pexpire_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        ttl: Duration,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("PEXPIRE");
        command.arg(key_value).arg(millis);
        self.execute_async(&command).await
    }

    /// 异步删除 key 的 TTL。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::persist_async::<&str>;
    /// ```
    pub async fn persist_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PERSIST", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步返回 Redis 原生 TTL 秒数。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::ttl_async::<&str>;
    /// ```
    pub async fn ttl_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("TTL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步返回 Redis 原生 TTL 毫秒数。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::pttl_async::<&str>;
    /// ```
    pub async fn pttl_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PTTL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数加一。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::incr_async::<&str>;
    /// ```
    pub async fn incr_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.incr_by_async(key_value, 1).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数增加指定值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::incr_by_async::<&str>;
    /// ```
    pub async fn incr_by_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        amount: i64,
    ) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut command = ::redis::cmd("INCRBY");
        command.arg(key_value).arg(amount);
        self.execute_async(&command).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数减一。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::decr_async::<&str>;
    /// ```
    pub async fn decr_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.decr_by_async(key_value, 1).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数减少指定值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::decr_by_async::<&str>;
    /// ```
    pub async fn decr_by_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        amount: i64,
    ) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut command = ::redis::cmd("DECRBY");
        command.arg(key_value).arg(amount);
        self.execute_async(&command).await
    }

    /// 异步从列表左侧压入一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lpush_async::<&str, u8>;
    /// ```
    pub async fn lpush_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("LPUSH", [key_value, value]);
        self.execute_async(&command).await
    }

    /// 异步从列表右侧压入一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::rpush_async::<&str, u8>;
    /// ```
    pub async fn rpush_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("RPUSH", [key_value, value]);
        self.execute_async(&command).await
    }

    /// 异步从列表左侧弹出一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lpop_async::<&str, u8>;
    /// ```
    pub async fn lpop_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("LPOP", [key_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 异步从列表右侧弹出一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::rpop_async::<&str, u8>;
    /// ```
    pub async fn rpop_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("RPOP", [key_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 异步读取列表范围。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::lrange_async::<&str, u8>;
    /// ```
    pub async fn lrange_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
        start: isize,
        stop: isize,
    ) -> Result<Vec<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        commands::check_lrange_request(start, stop, &self.inner.config)?;
        let mut command = ::redis::cmd("LRANGE");
        command.arg(key_value).arg(start).arg(stop);
        let values: Vec<Vec<u8>> = self.execute_async(&command).await?;
        decode::decode_collection(values, &self.inner.config)
    }

    /// 异步向集合加入一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::sadd_async::<&str, u8>;
    /// ```
    pub async fn sadd_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SADD", [key_value, value]);
        self.execute_async(&command).await
    }

    /// 异步从集合移除一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::srem_async::<&str, u8>;
    /// ```
    pub async fn srem_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SREM", [key_value, value]);
        self.execute_async(&command).await
    }

    /// 异步判断集合是否包含一个 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::sismember_async::<&str, u8>;
    /// ```
    pub async fn sismember_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SISMEMBER", [key_value, value]);
        self.execute_async(&command).await
    }

    /// 异步读取集合全部 MessagePack 成员。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::smembers_async::<&str, u8>;
    /// ```
    pub async fn smembers_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Vec<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("SMEMBERS", [key_value]);
        let values: Vec<Vec<u8>> = self.execute_async(&command).await?;
        decode::decode_collection(values, &self.inner.config)
    }

    /// 异步向 Redis 发送 `PING`。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::ping_async;
    /// ```
    pub async fn ping_async(&self) -> Result<String, RedisError> {
        let command = ::redis::cmd("PING");
        self.execute_async(&command).await
    }
}
