use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{
    codec, commands,
    error::RedisError,
    lock::{self, RedisLockGuard},
};
use super::super::backend::RedisClient;

impl RedisClient {
    /// 读取 MessagePack 值；key 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::get::<&str, u8>;
    /// ```
    pub fn get<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("GET", [key_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 读取 raw 字节；key 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::get_bytes::<&str>;
    /// ```
    pub fn get_bytes<K: AsRef<[u8]>>(&self, key_value: K) -> Result<Option<Vec<u8>>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("GET", [key_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| commands::check_value_response(&bytes, &self.inner.config).map(|()| bytes))
            .transpose()
    }

    /// 写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set::<&str, u8>;
    /// ```
    pub fn set<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SET", [key_value, value]);
        self.execute_sync::<()>(&command)
    }

    /// 写入 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let command = commands::command("SET", [key_value, value]);
        self.execute_sync::<()>(&command)
    }

    /// 使用一个原子 `SET ... PX` 写入带毫秒 TTL 的 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_with_expiry::<&str, u8>;
    /// ```
    pub fn set_with_expiry<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("PX").arg(millis);
        self.execute_sync::<()>(&command)
    }

    /// 使用一个原子 `SET ... PX` 写入带毫秒 TTL 的 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_with_expiry::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("PX").arg(millis);
        self.execute_sync::<()>(&command)
    }

    /// 仅在 key 不存在时写入 MessagePack 值，并返回是否写入成功。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_nx::<&str, u8>;
    /// ```
    pub fn set_nx<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("NX");
        let result: Option<String> = self.execute_sync(&command)?;
        Ok(result.is_some())
    }

    /// 仅在 key 不存在时使用原子 `SET ... PX NX` 写入带 TTL 的 MessagePack 值。
    ///
    /// 这是通用 NX 写入原语，不记录所有者，也不会在业务方法返回时自动删除；锁场景应
    /// 使用 [`RedisClient::try_lock`]。不要用无 token 的 [`RedisClient::delete`] 或
    /// [`RedisClient::pexpire`] 释放/续租锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_nx_with_expiry::<&str, u8>;
    /// ```
    pub fn set_nx_with_expiry<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
        ttl: Duration,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let millis = commands::duration_millis(ttl)?;
        let mut command = ::redis::cmd("SET");
        command
            .arg(key_value)
            .arg(value)
            .arg("PX")
            .arg(millis)
            .arg("NX");
        let result: Option<String> = self.execute_sync(&command)?;
        Ok(result.is_some())
    }

    /// 尝试获取一个带不可预测 token 和 TTL 的单键租约锁。
    ///
    /// 该方法使用原子 `SET key token PX ttl NX`，同一 Redis 逻辑主节点上的同一 key 同时
    /// 最多返回一个 guard。抢锁失败返回 `Ok(None)`；连接、协议、随机源或参数错误返回
    /// `Err`。TTL 必须大于 0 且不超过 24 小时，正但不足一毫秒的 duration 向上取 1 ms。
    /// 返回的 [`RedisLockGuard`] 拥有一个 `RedisClient` clone，因此不会借用全局客户端或
    /// 持有连接池连接；正常路径必须显式调用 `release`，同步 guard 被丢弃时只会再做一次
    /// 带 token 校验的最佳努力释放，TTL 是最终兜底。
    ///
    /// 这是单 Redis 逻辑主节点/单 Redis Cluster 拓扑的单键锁，不是跨独立主节点的
    /// Redlock，也不提供 fencing token。锁不能替代数据库条件更新、唯一约束、事务或幂等
    /// 设计；锁丢失或续租失败后，调用方必须停止继续执行受保护写入。调用方应使用稳定、
    /// 粒度足够细的业务 key，不要把未经审查的用户输入直接作为跨业务共享 key；token 仅
    /// 是内部所有权标记，不是业务身份、认证凭据或可持久化数据。主从异步复制故障切换
    /// 可能导致锁丢失，不能把该 API 当作跨独立主节点的一致性锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// fn enter(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client.try_lock("receipt-audit:serial-1", Duration::from_secs(30))?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     // 临界区仍应使用数据库条件更新或幂等逻辑。
    ///     let _ = lock.release()?;
    ///     Ok(())
    /// }
    ///
    /// let _ = enter;
    /// ```
    pub fn try_lock<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        ttl: Duration,
    ) -> Result<Option<RedisLockGuard>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let ttl_millis = lock::lock_ttl_millis(ttl)?;
        let token = lock::token()?;
        let command = lock::acquire_command(&key_value, &token, ttl_millis);
        let result: Option<String> = self.execute_sync(&command)?;
        if result.is_some() {
            Ok(Some(RedisLockGuard::new(
                self.clone(),
                key_value,
                token,
                ttl,
            )))
        } else {
            Ok(None)
        }
    }

    /// 仅在 key 不存在时写入 raw 字节，并返回是否写入成功。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_nx::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_nx<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("NX");
        let result: Option<String> = self.execute_sync(&command)?;
        Ok(result.is_some())
    }

    /// 仅在 key 不存在时使用原子 `SET ... PX NX` 写入带 TTL 的 raw 字节。
    ///
    /// 这是通用 NX 写入原语，不记录所有者，也不会自动删除；锁场景应使用
    /// [`RedisClient::try_lock`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_nx_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(
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
        let result: Option<String> = self.execute_sync(&command)?;
        Ok(result.is_some())
    }
}
