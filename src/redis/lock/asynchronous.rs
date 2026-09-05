//! 异步 Redis 锁 guard。

#[cfg(feature = "redis-async")]
use std::{fmt, time::Duration};

#[cfg(feature = "redis-async")]
use super::super::{client::RedisClient, error::RedisError};
#[cfg(feature = "redis-async")]
use super::common::{
    finish_release, finish_renew, lock_ttl_duration, lock_ttl_millis, TOKEN_BYTES,
};

#[cfg(feature = "redis-async")]
/// 异步 Redis 单键租约锁 guard。
///
/// guard 不实现 `Clone` 或 `Copy`，内部所有者 token 和完整 key 不会通过公共 API 暴露。
/// 锁 key 应使用稳定、粒度足够细的业务命名空间；不要把未经审查的用户输入直接作为
/// 跨业务共享 key。token 仅是内部所有权标记，不是业务身份、认证凭据或可持久化数据。
/// 该协议不覆盖主从异步复制故障切换造成的锁丢失，也不是跨独立主节点的 Redlock 或
/// fencing token；临界区仍需数据库条件更新、唯一约束、事务或幂等保护。
/// 异步 guard 的 `Drop` 不会 checkout 连接池、发送网络命令或创建后台任务；正常路径必须
/// 显式 `await release()`，取消、panic unwind 或 runtime 关闭时只能依赖获取时或最近一次
/// 成功续租后的有效 TTL 兜底，TTL 上限为 24 小时。
pub struct RedisAsyncLockGuard {
    client: RedisClient,
    key: Vec<u8>,
    token: [u8; TOKEN_BYTES],
    ttl: Duration,
    pub(super) active: bool,
}

#[cfg(feature = "redis-async")]
impl RedisAsyncLockGuard {
    pub(crate) fn new(
        client: RedisClient,
        key: Vec<u8>,
        token: [u8; TOKEN_BYTES],
        ttl: Duration,
    ) -> Self {
        Self {
            client,
            key,
            token,
            ttl,
            active: true,
        }
    }

    /// 使用当前 guard 的 token 原子释放锁。
    ///
    /// 返回 `Ok(true)` 表示删除了当前 token 对应的锁；`Ok(false)` 表示锁已过期、已由其他
    /// token 持有，或 guard 已经释放。只有 Redis 命令可靠返回后才会将 guard 标记为非活动；
    /// 传输或协议错误会原样返回。`Drop` 不会重试或创建后台任务，因此正常路径必须显式
    /// `await` 此方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// async fn release_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client
    ///         .try_lock_async("receipt-audit:serial-1", Duration::from_secs(30))
    ///         .await?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _released = lock.release().await?;
    ///     Ok(())
    /// }
    ///
    /// let _ = release_lock;
    /// ```
    pub async fn release(&mut self) -> Result<bool, RedisError> {
        if !self.active {
            return Ok(false);
        }
        let result = self.client.release_lock_async(&self.key, &self.token).await;
        finish_release(&mut self.active, result)
    }

    /// 在当前 token 仍持有锁时原子刷新 TTL。
    ///
    /// TTL 必须大于 0 且不超过 24 小时；正但不足一毫秒的 duration 向上取 1 ms。返回
    /// `Ok(true)` 表示 TTL 已刷新，`Ok(false)` 表示锁已过期、所有权已丢失或 guard 已失效，
    /// 此时 guard 不可再用于受保护操作。传输或协议错误不会伪造成功，也不会自动修改活动
    /// 状态。释放后的 guard 不会再次发送续租命令。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::{RedisClient, RedisError};
    /// use std::time::Duration;
    ///
    /// async fn renew_lock(client: &RedisClient) -> Result<(), RedisError> {
    ///     let Some(mut lock) = client
    ///         .try_lock_async("receipt-audit:serial-1", Duration::from_secs(30))
    ///         .await?
    ///     else {
    ///         return Ok(());
    ///     };
    ///     let _still_owned = lock.renew(Duration::from_secs(30)).await?;
    ///     let _ = lock.release().await?;
    ///     Ok(())
    /// }
    ///
    /// let _ = renew_lock;
    /// ```
    pub async fn renew(&mut self, ttl: Duration) -> Result<bool, RedisError> {
        let ttl_millis = lock_ttl_millis(ttl)?;
        let effective_ttl = lock_ttl_duration(ttl)?;
        if !self.active {
            return Ok(false);
        }
        let result = self
            .client
            .renew_lock_async(&self.key, &self.token, ttl_millis)
            .await;
        finish_renew(&mut self.active, &mut self.ttl, effective_ttl, result)
    }
}

#[cfg(feature = "redis-async")]
impl Drop for RedisAsyncLockGuard {
    fn drop(&mut self) {}
}

#[cfg(feature = "redis-async")]
impl fmt::Debug for RedisAsyncLockGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisAsyncLockGuard")
            .field("ttl", &self.ttl)
            .field("active", &self.active)
            .finish()
    }
}
