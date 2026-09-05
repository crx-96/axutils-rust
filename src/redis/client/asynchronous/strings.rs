use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};

use super::super::super::{
    codec, commands,
    error::RedisError,
    lock::{self, RedisAsyncLockGuard},
};
use super::super::backend::RedisClient;

impl RedisClient {
    /// 异步读取 MessagePack 值；key 不存在时返回 `None`。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::get_async::<&str, u8>;
    /// ```
    pub async fn get_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("GET", [key_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 异步读取 raw 字节；key 不存在时返回 `None`。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::get_bytes_async::<&str>;
    /// ```
    pub async fn get_bytes_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("GET", [key_value]);
        let value: Option<Vec<u8>> = self.execute_async(&command).await?;
        value
            .map(|bytes| commands::check_value_response(&bytes, &self.inner.config).map(|()| bytes))
            .transpose()
    }

    /// 异步写入 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_async::<&str, u8>;
    /// ```
    pub async fn set_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("SET", [key_value, value]);
        self.execute_async::<()>(&command).await
    }

    /// 异步写入 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        value: V,
    ) -> Result<(), RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let command = commands::command("SET", [key_value, value]);
        self.execute_async::<()>(&command).await
    }

    /// 异步使用原子 `SET ... PX` 写入带 TTL 的 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_with_expiry_async::<&str, u8>;
    /// ```
    pub async fn set_with_expiry_async<K: AsRef<[u8]>, T: Serialize>(
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
        self.execute_async::<()>(&command).await
    }

    /// 异步使用原子 `SET ... PX` 写入带 TTL 的 raw 字节。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_bytes_with_expiry_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_with_expiry_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
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
        self.execute_async::<()>(&command).await
    }

    /// 异步仅在 key 不存在时写入 MessagePack 值。
    #[cfg(feature = "redis-async")]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::RedisClient;
    ///
    /// let _ = RedisClient::set_nx_async::<&str, u8>;
    /// ```
    pub async fn set_nx_async<K: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        value: T,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let mut command = ::redis::cmd("SET");
        command.arg(key_value).arg(value).arg("NX");
        let result: Option<String> = self.execute_async(&command).await?;
        Ok(result.is_some())
    }

    /// 异步仅在 key 不存在时以 `SET ... PX NX` 写入带 TTL 的 MessagePack 值。
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
    /// let _ = RedisClient::set_nx_with_expiry_async::<&str, u8>;
    /// ```
    pub async fn set_nx_with_expiry_async<K: AsRef<[u8]>, T: Serialize>(
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
        let result: Option<String> = self.execute_async(&command).await?;
        Ok(result.is_some())
    }

    #[cfg(feature = "redis-async")]
    /// 异步尝试获取一个带不可预测 token 和 TTL 的单键租约锁。
    ///
    /// 该方法使用原子 `SET key token PX ttl NX`，抢锁失败返回 `Ok(None)`。TTL 必须大于 0
    /// 且不超过 24 小时；正但不足一毫秒的 duration 向上取 1 ms。返回的
    /// [`RedisAsyncLockGuard`] 拥有一个 `RedisClient` clone；它的 `Drop` 不会发起网络操作，
    /// 正常路径必须显式 `await release()`，取消或 runtime 关闭时依赖 TTL 兜底。
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
    /// async fn enter(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client
    ///         .try_lock_async("receipt-audit:serial-1", Duration::from_secs(30))
    ///         .await?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _ = lock.release().await?;
    ///     Ok(())
    /// }
    ///
    /// let _ = enter;
    /// ```
    pub async fn try_lock_async<K: AsRef<[u8]>>(
        &self,
        key_value: K,
        ttl: Duration,
    ) -> Result<Option<RedisAsyncLockGuard>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let ttl_millis = lock::lock_ttl_millis(ttl)?;
        let token = lock::token()?;
        let command = lock::acquire_command(&key_value, &token, ttl_millis);
        let result: Option<String> = self.execute_async(&command).await?;
        if result.is_some() {
            Ok(Some(RedisAsyncLockGuard::new(
                self.clone(),
                key_value,
                token,
                ttl,
            )))
        } else {
            Ok(None)
        }
    }
}
