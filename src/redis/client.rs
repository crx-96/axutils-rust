use std::{fmt, sync::Arc, time::Duration};

#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

use ::redis::ConnectionLike;
use r2d2::{ManageConnection, Pool};
use serde::{de::DeserializeOwned, Serialize};

#[cfg(all(feature = "redis", feature = "tokio"))]
use super::config::{ASYNC_RECONNECT_MAX_DELAY, ASYNC_RECONNECT_RETRIES};
use super::{
    codec, commands,
    config::RedisConfig,
    error::{RedisError, RedisTransportErrorKind},
    lock::{self, RedisLockGuard},
    transaction::RedisTransaction,
};

#[cfg(all(feature = "redis", feature = "tokio"))]
use super::lock::RedisAsyncLockGuard;

type SinglePool = Pool<SingleManager>;
type ClusterPool = Pool<ClusterManager>;
/// 可复用的 Redis 客户端实例。
///
/// 一个实例可以独立配置为单机或 Cluster；`Clone` 只共享同一个连接池和异步连接状态，不会
/// 复制认证信息或预热额外连接。构造阶段只做本地配置和 backend 初始化，不访问 Redis；首次
/// 命令才可能报告连接失败。同步方法会阻塞当前线程，不会把调用转移到线程池，也不会创建
/// Tokio runtime；底层 `r2d2` 连接池的内部管理 worker 仍由连接池自身维护。
pub struct RedisClient {
    pub(crate) inner: Arc<RedisClientInner>,
}

pub(crate) struct RedisClientInner {
    pub(crate) config: RedisConfig,
    sync: SyncBackend,
    #[cfg(all(feature = "redis", feature = "tokio"))]
    async_backend: AsyncBackend,
}

enum SyncBackend {
    Single(SinglePool),
    Cluster(ClusterPool),
    #[cfg(test)]
    Fake(Arc<TestRedisBackend>),
}

#[cfg(all(feature = "redis", feature = "tokio"))]
enum AsyncBackend {
    Single {
        client: ::redis::Client,
        manager: tokio::sync::Mutex<Option<::redis::aio::ConnectionManager>>,
        transaction: tokio::sync::Mutex<Option<::redis::aio::MultiplexedConnection>>,
        transaction_lock: tokio::sync::Mutex<()>,
    },
    Cluster {
        client: ::redis::cluster::ClusterClient,
        connection: tokio::sync::Mutex<Option<::redis::cluster_async::ClusterConnection>>,
    },
    #[cfg(test)]
    Fake(Arc<TestRedisBackend>),
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TestRedisBackend {
    checkout_count: Arc<AtomicUsize>,
    command_count: Arc<AtomicUsize>,
    result: Arc<Mutex<Result<i64, RedisError>>>,
}

#[cfg(test)]
impl TestRedisBackend {
    fn new(result: Result<i64, RedisError>) -> Self {
        Self {
            checkout_count: Arc::new(AtomicUsize::new(0)),
            command_count: Arc::new(AtomicUsize::new(0)),
            result: Arc::new(Mutex::new(result)),
        }
    }

    pub(crate) fn checkout_count(&self) -> usize {
        self.checkout_count.load(Ordering::Relaxed)
    }

    pub(crate) fn command_count(&self) -> usize {
        self.command_count.load(Ordering::Relaxed)
    }

    fn execute<T: ::redis::FromRedisValue>(&self) -> Result<T, RedisError> {
        self.checkout_count.fetch_add(1, Ordering::Relaxed);
        self.command_count.fetch_add(1, Ordering::Relaxed);
        let result = *self
            .result
            .lock()
            .expect("test Redis backend result lock should not be poisoned");
        let value = result?;
        T::from_redis_value(::redis::Value::Int(value))
            .map_err(|_| RedisError::Transport(RedisTransportErrorKind::Protocol))
    }
}

struct SingleManager {
    client: ::redis::Client,
    connection_timeout: Duration,
    response_timeout: Duration,
}

struct ClusterManager {
    client: ::redis::cluster::ClusterClient,
    connection_timeout: Duration,
    response_timeout: Duration,
}

const MANAGER_ERROR_PREFIX: &str = "axutils-redis-manager:";

#[derive(Debug)]
struct SyncManagerError {
    kind: RedisTransportErrorKind,
}

impl SyncManagerError {
    fn new(kind: RedisTransportErrorKind) -> Self {
        Self { kind }
    }

    fn from_upstream(error: &::redis::RedisError) -> Self {
        let kind = match RedisError::from_upstream(error) {
            RedisError::Transport(kind) => kind,
            RedisError::CrossSlot => RedisTransportErrorKind::Server,
            _ => RedisTransportErrorKind::Other,
        };
        Self::new(kind)
    }
}

impl fmt::Display for SyncManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{MANAGER_ERROR_PREFIX}{}", self.kind)
    }
}

impl std::error::Error for SyncManagerError {}

struct ManagedConnection<C> {
    inner: C,
    broken: bool,
}

impl<C> ManagedConnection<C> {
    fn new(inner: C) -> Self {
        Self {
            inner,
            broken: false,
        }
    }

    fn mark_broken(&mut self) {
        self.broken = true;
    }
}

impl<C: ::redis::ConnectionLike> ::redis::ConnectionLike for ManagedConnection<C> {
    fn req_packed_command(&mut self, cmd: &[u8]) -> ::redis::RedisResult<::redis::Value> {
        self.inner.req_packed_command(cmd)
    }

    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        offset: usize,
        count: usize,
    ) -> ::redis::RedisResult<Vec<::redis::Value>> {
        self.inner.req_packed_commands(cmd, offset, count)
    }

    fn get_db(&self) -> i64 {
        self.inner.get_db()
    }

    fn supports_pipelining(&self) -> bool {
        self.inner.supports_pipelining()
    }

    fn check_connection(&mut self) -> bool {
        self.inner.check_connection()
    }

    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

impl ManageConnection for SingleManager {
    type Connection = ManagedConnection<::redis::Connection>;
    type Error = SyncManagerError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let connection = self
            .client
            .get_connection_with_timeout(self.connection_timeout)
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        connection
            .set_read_timeout(Some(self.response_timeout))
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        connection
            .set_write_timeout(Some(self.response_timeout))
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        Ok(ManagedConnection::new(connection))
    }

    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        // r2d2 checkout validation is intentionally local only. A PING here would add network
        // I/O to every checkout and change the documented no-implicit-health-check contract.
        if connection.is_open() {
            Ok(())
        } else {
            Err(SyncManagerError::new(RedisTransportErrorKind::Connection))
        }
    }

    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.broken || !connection.is_open()
    }
}

impl ManageConnection for ClusterManager {
    type Connection = ManagedConnection<::redis::cluster::ClusterConnection>;
    type Error = SyncManagerError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let cluster_config = ::redis::cluster::ClusterConfig::new()
            .set_connection_timeout(self.connection_timeout)
            .set_response_timeout(self.response_timeout);
        let connection = self
            .client
            .get_connection_with_config(cluster_config)
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        Ok(ManagedConnection::new(connection))
    }

    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        // Cluster checkout validation is intentionally local only; it does not send PING.
        if connection.is_open() {
            Ok(())
        } else {
            Err(SyncManagerError::new(RedisTransportErrorKind::Connection))
        }
    }

    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.broken || !connection.is_open()
    }
}

impl Clone for RedisClient {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl RedisClient {
    /// 根据已校验配置创建客户端。
    ///
    /// 此方法不建立网络连接；连接池采用惰性连接，异步 manager 也只在第一次异步命令时
    /// 在调用方 runtime 中创建。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{RedisClient, RedisConfig};
    /// let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
    ///     .unwrap();
    /// let _clone = client.clone();
    /// ```
    pub fn new(config: RedisConfig) -> Result<Self, RedisError> {
        let sync = if let Some(url) = config.single_url() {
            let client =
                ::redis::Client::open(url).map_err(|_| RedisError::invalid_config("url"))?;
            let manager = SingleManager {
                client: client.clone(),
                connection_timeout: config.connection_timeout,
                response_timeout: config.response_timeout,
            };
            let pool = Pool::builder()
                .max_size(config.pool_size as u32)
                .min_idle(Some(0))
                .connection_timeout(config.pool_checkout_timeout)
                .build(manager)
                .map_err(|_| RedisError::Pool)?;
            SyncBackend::Single(pool)
        } else {
            let nodes = config
                .cluster_nodes()
                .ok_or(RedisError::invalid_config("nodes"))?;
            let client = ::redis::cluster::ClusterClient::builder(nodes.to_vec())
                .connection_timeout(config.connection_timeout)
                .response_timeout(config.response_timeout)
                .build()
                .map_err(|_| RedisError::invalid_config("nodes"))?;
            let manager = ClusterManager {
                client,
                connection_timeout: config.connection_timeout,
                response_timeout: config.response_timeout,
            };
            let pool = Pool::builder()
                .max_size(config.pool_size as u32)
                .min_idle(Some(0))
                .connection_timeout(config.pool_checkout_timeout)
                .build(manager)
                .map_err(|_| RedisError::Pool)?;
            SyncBackend::Cluster(pool)
        };

        #[cfg(all(feature = "redis", feature = "tokio"))]
        let async_backend = if let Some(url) = config.single_url() {
            let client =
                ::redis::Client::open(url).map_err(|_| RedisError::invalid_config("url"))?;
            AsyncBackend::Single {
                client,
                manager: tokio::sync::Mutex::new(None),
                transaction: tokio::sync::Mutex::new(None),
                transaction_lock: tokio::sync::Mutex::new(()),
            }
        } else {
            let nodes = config
                .cluster_nodes()
                .ok_or(RedisError::invalid_config("nodes"))?;
            let client = ::redis::cluster::ClusterClient::builder(nodes.to_vec())
                .connection_timeout(config.connection_timeout)
                .response_timeout(config.response_timeout)
                .build()
                .map_err(|_| RedisError::invalid_config("nodes"))?;
            AsyncBackend::Cluster {
                client,
                connection: tokio::sync::Mutex::new(None),
            }
        };

        Ok(Self {
            inner: Arc::new(RedisClientInner {
                config,
                sync,
                #[cfg(all(feature = "redis", feature = "tokio"))]
                async_backend,
            }),
        })
    }

    #[cfg(test)]
    pub(crate) fn test_fake(result: Result<i64, RedisError>) -> (Self, TestRedisBackend) {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .expect("test fake Redis URL should be valid");
        let backend = TestRedisBackend::new(result);
        let sync = SyncBackend::Fake(Arc::new(backend.clone()));
        #[cfg(all(feature = "redis", feature = "tokio"))]
        let async_backend = AsyncBackend::Fake(Arc::new(backend.clone()));

        (
            Self {
                inner: Arc::new(RedisClientInner {
                    config,
                    sync,
                    #[cfg(all(feature = "redis", feature = "tokio"))]
                    async_backend,
                }),
            },
            backend,
        )
    }

    /// 读取 MessagePack 值；key 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::{RedisClient, RedisError};
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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

    /// 删除一个 key 并返回实际删除数量。
    ///
    /// 这是无条件 `DEL`，不校验锁 token；不要直接用它释放由
    /// [`RedisClient::try_lock`] 获取的锁。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::delete_many::<[&str; 1], &str>;
    /// ```
    pub fn delete_many<I, K>(&self, keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mget::<[&str; 1], &str, u8>;
    /// ```
    pub fn mget<I, K, T>(&self, keys: I) -> Result<Vec<Option<T>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
        T: DeserializeOwned,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mget_bytes::<[&str; 1], &str>;
    /// ```
    pub fn mget_bytes<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mset::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub fn mset<I, K, T>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, T)>,
        K: AsRef<[u8]>,
        T: Serialize,
    {
        let args = collect_value_pairs(entries, &self.inner.config)?;
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub fn mset_bytes<I, K, V>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let args = collect_raw_pairs(entries, &self.inner.config)?;
        if args.is_empty() {
            return Ok(());
        }
        let command = commands::command("MSET", args);
        self.execute_sync::<()>(&command)
    }

    /// 读取 Hash 中的 MessagePack 值；field 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hget::<&str, &str, u8>;
    /// ```
    pub fn hget<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<T>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| codec::decode(&bytes, self.inner.config.max_value_bytes))
            .transpose()
    }

    /// 读取 Hash 中的 raw 字节；field 不存在时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hget_bytes::<&str, &str>;
    /// ```
    pub fn hget_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HGET", [key_value, field_value]);
        let value: Option<Vec<u8>> = self.execute_sync(&command)?;
        value
            .map(|bytes| commands::check_value_response(&bytes, &self.inner.config).map(|()| bytes))
            .transpose()
    }

    /// 写入一个 MessagePack Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hset::<&str, &str, u8>;
    /// ```
    pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        &self,
        key_value: K,
        field_value: F,
        value: T,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::encoded(&value, &self.inner.config)?;
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_sync(&command)
    }

    /// 写入一个 raw Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hset_bytes::<&str, &str, Vec<u8>>;
    /// ```
    pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
        value: V,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let value = commands::raw(value, &self.inner.config)?;
        let command = commands::command("HSET", [key_value, field_value, value]);
        self.execute_sync(&command)
    }

    /// 读取 Hash 全部 field 和 MessagePack 值，保留 Redis 返回顺序。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hgetall::<&str, u8>;
    /// ```
    pub fn hgetall<K: AsRef<[u8]>, T: DeserializeOwned>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, T)>, RedisError> {
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
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hgetall_bytes::<&str>;
    /// ```
    #[allow(clippy::type_complexity)]
    pub fn hgetall_bytes<K: AsRef<[u8]>>(
        &self,
        key_value: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("HGETALL", [key_value]);
        let flat: Vec<Vec<u8>> = self.execute_sync(&command)?;
        decode_hash_entries(flat, &self.inner.config)
    }

    /// 删除一个 Hash field，并返回实际删除数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hdel::<&str, &str>;
    /// ```
    pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HDEL", [key_value, field_value]);
        self.execute_sync(&command)
    }

    /// 判断 Hash field 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hexists::<&str, &str>;
    /// ```
    pub fn hexists<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        &self,
        key_value: K,
        field_value: F,
    ) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let field_value = commands::field(field_value, &self.inner.config)?;
        let command = commands::command("HEXISTS", [key_value, field_value]);
        self.execute_sync(&command)
    }

    /// 返回 Hash field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hlen::<&str>;
    /// ```
    pub fn hlen<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("HLEN", [key_value]);
        self.execute_sync(&command)
    }

    /// 有界批量写入 MessagePack Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut args = vec![key_value];
        let mut total = args[0].len();
        for (field_value, value) in entries {
            if (args.len() - 1) / 2 >= self.inner.config.max_batch_items {
                return Err(RedisError::ValueTooLarge {
                    limit: self.inner.config.max_batch_items,
                });
            }
            let field_value = commands::field(field_value, &self.inner.config)?;
            let value = commands::encoded(&value, &self.inner.config)?;
            total = commands::add_batch_bytes(total, field_value.len(), &self.inner.config)?;
            total = commands::add_batch_bytes(total, value.len(), &self.inner.config)?;
            args.push(field_value);
            args.push(value);
        }
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_sync(&command)
    }

    /// 有界批量写入 raw Hash field，并返回新增 field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        let key_value = commands::key(key_value, &self.inner.config)?;
        let mut args = vec![key_value];
        let mut total = args[0].len();
        for (field_value, value) in entries {
            if (args.len() - 1) / 2 >= self.inner.config.max_batch_items {
                return Err(RedisError::ValueTooLarge {
                    limit: self.inner.config.max_batch_items,
                });
            }
            let field_value = commands::field(field_value, &self.inner.config)?;
            let value = commands::raw(value, &self.inner.config)?;
            total = commands::add_batch_bytes(total, field_value.len(), &self.inner.config)?;
            total = commands::add_batch_bytes(total, value.len(), &self.inner.config)?;
            args.push(field_value);
            args.push(value);
        }
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_sync(&command)
    }

    /// 以秒为单位设置 key 的 TTL；返回 key 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
        decode_collection(values, &self.inner.config)
    }

    /// 向集合加入一个 MessagePack 值，并返回新增成员数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
    /// use axutils::RedisClient;
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
        decode_collection(values, &self.inner.config)
    }

    /// 向 Redis 发送 `PING` 并返回服务端响应。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::ping;
    /// ```
    pub fn ping(&self) -> Result<String, RedisError> {
        let command = ::redis::cmd("PING");
        self.execute_sync(&command)
    }

    /// 异步读取 MessagePack 值；key 不存在时返回 `None`。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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

    #[cfg(all(feature = "redis", feature = "tokio"))]
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
    /// use axutils::{RedisClient, RedisError};
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

    /// 异步仅在 key 不存在时写入 raw 字节。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::delete_async::<&str>;
    /// ```
    pub async fn delete_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("DEL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步有界批量删除 key。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::delete_many_async::<[&str; 1], &str>;
    /// ```
    pub async fn delete_many_async<I, K>(&self, keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(0);
        }
        let command = commands::command("DEL", keys);
        self.execute_async(&command).await
    }

    /// 异步判断 key 是否存在。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::exists_async::<&str>;
    /// ```
    pub async fn exists_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("EXISTS", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步按输入顺序批量读取 MessagePack 值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mget_async::<[&str; 1], &str, u8>;
    /// ```
    pub async fn mget_async<I, K, T>(&self, keys: I) -> Result<Vec<Option<T>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
        T: DeserializeOwned,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let command = commands::command("MGET", keys);
        let values: Vec<Option<Vec<u8>>> = self.execute_async(&command).await?;
        decode_optional_values(values, &self.inner.config)
    }

    /// 异步按输入顺序批量读取 raw 字节。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mget_bytes_async::<[&str; 1], &str>;
    /// ```
    pub async fn mget_bytes_async<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let keys = collect_keys(keys, &self.inner.config)?;
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let command = commands::command("MGET", keys);
        let values: Vec<Option<Vec<u8>>> = self.execute_async(&command).await?;
        check_optional_values(values, &self.inner.config)
    }

    /// 异步有界批量写入 MessagePack 值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mset_async::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub async fn mset_async<I, K, T>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, T)>,
        K: AsRef<[u8]>,
        T: Serialize,
    {
        let args = collect_value_pairs(entries, &self.inner.config)?;
        if args.is_empty() {
            return Ok(());
        }
        let command = commands::command("MSET", args);
        self.execute_async::<()>(&command).await
    }

    /// 异步有界批量写入 raw 字节。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub async fn mset_bytes_async<I, K, V>(&self, entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let args = collect_raw_pairs(entries, &self.inner.config)?;
        if args.is_empty() {
            return Ok(());
        }
        let command = commands::command("MSET", args);
        self.execute_async::<()>(&command).await
    }

    /// 异步读取 Hash 中的 MessagePack 值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        decode_hash_entries(flat, &self.inner.config)
    }

    /// 异步删除一个 Hash field。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::hlen_async::<&str>;
    /// ```
    pub async fn hlen_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<u64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("HLEN", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步有界批量写入 MessagePack Hash field。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        let args = collect_hash_pairs(key_value, entries, &self.inner.config)?;
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_async(&command).await
    }

    /// 异步有界批量写入 raw Hash field。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        let args = collect_hash_raw_pairs(key_value, entries, &self.inner.config)?;
        if args.len() == 1 {
            return Ok(0);
        }
        let command = commands::command("HSET", args);
        self.execute_async(&command).await
    }

    /// 异步以秒为单位设置 key 的 TTL。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::persist_async::<&str>;
    /// ```
    pub async fn persist_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<bool, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PERSIST", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步返回 Redis 原生 TTL 秒数。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::ttl_async::<&str>;
    /// ```
    pub async fn ttl_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("TTL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步返回 Redis 原生 TTL 毫秒数。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::pttl_async::<&str>;
    /// ```
    pub async fn pttl_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        let key_value = commands::key(key_value, &self.inner.config)?;
        let command = commands::command("PTTL", [key_value]);
        self.execute_async(&command).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数加一。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::incr_async::<&str>;
    /// ```
    pub async fn incr_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.incr_by_async(key_value, 1).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数增加指定值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::decr_async::<&str>;
    /// ```
    pub async fn decr_async<K: AsRef<[u8]>>(&self, key_value: K) -> Result<i64, RedisError> {
        self.decr_by_async(key_value, 1).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数减少指定值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        decode_collection(values, &self.inner.config)
    }

    /// 异步向集合加入一个 MessagePack 值。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
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
        decode_collection(values, &self.inner.config)
    }

    /// 异步向 Redis 发送 `PING`。
    #[cfg(all(feature = "redis", feature = "tokio"))]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisClient;
    ///
    /// let _ = RedisClient::ping_async;
    /// ```
    pub async fn ping_async(&self) -> Result<String, RedisError> {
        let command = ::redis::cmd("PING");
        self.execute_async(&command).await
    }

    /// 同步执行一个原子 MULTI/EXEC 事务。
    ///
    /// callback 只允许同步排队写入命令；它返回错误时不会 checkout 连接或发送任何命令。
    /// 空事务直接返回成功。Cluster 模式返回 [`RedisError::UnsupportedMode`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError, RedisTransaction};
    ///
    /// let _ = RedisClient::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub fn transaction<F>(&self, callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        if self.inner.config.is_cluster() {
            return Err(RedisError::UnsupportedMode);
        }
        let mut transaction = RedisTransaction::new(&self.inner.config);
        callback(&mut transaction)?;
        if transaction.is_empty() {
            return Ok(());
        }

        let mut connection = match &self.inner.sync {
            SyncBackend::Single(pool) => pool.get().map_err(|error| pool_error(&error))?,
            SyncBackend::Cluster(_) => return Err(RedisError::UnsupportedMode),
            #[cfg(test)]
            SyncBackend::Fake(_) => return Err(RedisError::UnsupportedMode),
        };
        let mut pipeline = ::redis::pipe();
        pipeline.atomic();
        for command in transaction.commands() {
            pipeline.add_command(command.clone());
        }
        match pipeline.exec(&mut *connection) {
            Ok(()) => Ok(()),
            Err(error) => {
                if should_discard_transaction_connection(&error, connection.is_open()) {
                    connection.mark_broken();
                }
                Err(RedisError::transaction_failure(&error))
            }
        }
    }

    fn execute_sync<T: ::redis::FromRedisValue>(
        &self,
        command: &::redis::Cmd,
    ) -> Result<T, RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let mut connection_discarded = false;
        #[cfg(feature = "tracing")]
        let backend = if self.inner.config.is_cluster() {
            "cluster"
        } else {
            "single"
        };
        let result = match &self.inner.sync {
            SyncBackend::Single(pool) => match pool.get() {
                Ok(mut connection) => match command.query(&mut *connection) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mapped = RedisError::from_upstream(&error);
                        if should_discard_connection(&mapped, connection.is_open()) {
                            connection.mark_broken();
                            connection_discarded = true;
                        }
                        Err(mapped)
                    }
                },
                Err(error) => Err(pool_error(&error)),
            },
            SyncBackend::Cluster(pool) => match pool.get() {
                Ok(mut connection) => match command.query(&mut *connection) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mapped = RedisError::from_upstream(&error);
                        if should_discard_connection(&mapped, connection.is_open()) {
                            connection.mark_broken();
                            connection_discarded = true;
                        }
                        Err(mapped)
                    }
                },
                Err(error) => Err(pool_error(&error)),
            },
            #[cfg(test)]
            SyncBackend::Fake(backend) => backend.execute(),
        };
        #[cfg(not(feature = "tracing"))]
        let _ = connection_discarded;
        #[cfg(feature = "tracing")]
        crate::tracing::redis::record_command(
            "sync",
            backend,
            &result,
            connection_discarded,
            started,
        );
        result
    }

    pub(crate) fn release_lock_sync(&self, key: &[u8], token: &[u8]) -> Result<i64, RedisError> {
        let command = lock::release_command(key, token);
        self.execute_sync(&command)
    }

    pub(crate) fn renew_lock_sync(
        &self,
        key: &[u8],
        token: &[u8],
        ttl_millis: i64,
    ) -> Result<i64, RedisError> {
        let command = lock::renew_command(key, token, ttl_millis);
        self.execute_sync(&command)
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    async fn execute_async<T: ::redis::FromRedisValue>(
        &self,
        command: &::redis::Cmd,
    ) -> Result<T, RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        #[cfg(feature = "tracing")]
        let backend = if self.inner.config.is_cluster() {
            "cluster"
        } else {
            "single"
        };
        let result = if tokio::runtime::Handle::try_current().is_err() {
            Err(RedisError::RuntimeRequired)
        } else {
            match &self.inner.async_backend {
                AsyncBackend::Single { .. } => match self.async_single_connection().await {
                    Ok(mut connection) => command
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| RedisError::from_upstream(&error)),
                    Err(error) => Err(error),
                },
                AsyncBackend::Cluster { .. } => match self.async_cluster_connection().await {
                    Ok(mut connection) => command
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| RedisError::from_upstream(&error)),
                    Err(error) => Err(error),
                },
                #[cfg(test)]
                AsyncBackend::Fake(backend) => backend.execute(),
            }
        };
        #[cfg(feature = "tracing")]
        crate::tracing::redis::record_command("async", backend, &result, false, started);
        result
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    pub(crate) async fn release_lock_async(
        &self,
        key: &[u8],
        token: &[u8],
    ) -> Result<i64, RedisError> {
        let command = lock::release_command(key, token);
        self.execute_async(&command).await
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    pub(crate) async fn renew_lock_async(
        &self,
        key: &[u8],
        token: &[u8],
        ttl_millis: i64,
    ) -> Result<i64, RedisError> {
        let command = lock::renew_command(key, token, ttl_millis);
        self.execute_async(&command).await
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    async fn async_single_connection(&self) -> Result<::redis::aio::ConnectionManager, RedisError> {
        let AsyncBackend::Single {
            client, manager, ..
        } = &self.inner.async_backend
        else {
            return Err(RedisError::UnsupportedMode);
        };
        let mut guard = manager.lock().await;
        if let Some(connection) = guard.as_ref() {
            return Ok(connection.clone());
        }
        let config = ::redis::aio::ConnectionManagerConfig::new()
            .set_number_of_retries(ASYNC_RECONNECT_RETRIES)
            .set_max_delay(ASYNC_RECONNECT_MAX_DELAY)
            .set_connection_timeout(Some(self.inner.config.connection_timeout))
            .set_response_timeout(Some(self.inner.config.response_timeout));
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let connection_result = client
            .get_connection_manager_lazy(config)
            .map_err(|error| RedisError::from_upstream(&error));
        #[cfg(feature = "tracing")]
        match &connection_result {
            Ok(_) => crate::tracing::redis::record_connection(
                "connection_manager_init",
                "single",
                "ready",
                None,
                started,
            ),
            Err(error) => crate::tracing::redis::record_connection(
                "connection_manager_init",
                "single",
                "error",
                Some(error),
                started,
            ),
        }
        let connection = connection_result;
        let connection = connection?;
        *guard = Some(connection.clone());
        Ok(connection)
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    async fn async_cluster_connection(
        &self,
    ) -> Result<::redis::cluster_async::ClusterConnection, RedisError> {
        let AsyncBackend::Cluster { client, connection } = &self.inner.async_backend else {
            return Err(RedisError::UnsupportedMode);
        };
        // The first cluster connection is established while holding this slot lock. Concurrent
        // first commands therefore serialize behind the bounded connection timeout; cancellation
        // releases the lock, and later commands only clone an established connection.
        let mut guard = connection.lock().await;
        if let Some(connection) = guard.as_ref() {
            return Ok(connection.clone());
        }
        let config = ::redis::cluster::ClusterConfig::new()
            .set_connection_timeout(self.inner.config.connection_timeout)
            .set_response_timeout(self.inner.config.response_timeout);
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let connection_result = client
            .get_async_connection_with_config(config)
            .await
            .map_err(|error| RedisError::from_upstream(&error));
        #[cfg(feature = "tracing")]
        match &connection_result {
            Ok(_) => crate::tracing::redis::record_connection(
                "connection",
                "cluster",
                "success",
                None,
                started,
            ),
            Err(error) => crate::tracing::redis::record_connection(
                "connection",
                "cluster",
                "error",
                Some(error),
                started,
            ),
        }
        let connection = connection_result;
        let connection = connection?;
        *guard = Some(connection.clone());
        Ok(connection)
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    /// 异步执行单机 MULTI/EXEC 事务。
    ///
    /// 事务 callback 是一次性的同步排队闭包，不接受 async callback，也不会被重放。专用
    /// multiplexed connection 与普通命令分离；future 取消或连接状态不再可靠时该连接会被丢弃。
    /// 已完整读取响应的普通 Redis 服务端命令错误不会淘汰健康连接。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisClient, RedisError, RedisTransaction};
    ///
    /// let _ = RedisClient::transaction_async::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub async fn transaction_async<F>(&self, callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        if self.inner.config.is_cluster() {
            return Err(RedisError::UnsupportedMode);
        }
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(RedisError::RuntimeRequired);
        }
        let mut transaction = RedisTransaction::new(&self.inner.config);
        callback(&mut transaction)?;
        if transaction.is_empty() {
            return Ok(());
        }

        let AsyncBackend::Single {
            client,
            transaction: slot,
            transaction_lock,
            ..
        } = &self.inner.async_backend
        else {
            return Err(RedisError::UnsupportedMode);
        };
        let _serial = transaction_lock.lock().await;
        let mut connection = {
            let mut guard = slot.lock().await;
            guard.take()
        };
        if connection.is_none() {
            let config = ::redis::AsyncConnectionConfig::new()
                .set_connection_timeout(Some(self.inner.config.connection_timeout))
                .set_response_timeout(Some(self.inner.config.response_timeout));
            connection = Some(
                client
                    .get_multiplexed_async_connection_with_config(&config)
                    .await
                    .map_err(|error| RedisError::from_upstream(&error))?,
            );
        }
        let Some(mut connection) = connection else {
            return Err(RedisError::TransactionFailed);
        };
        let mut pipeline = ::redis::pipe();
        pipeline.atomic();
        for command in transaction.commands() {
            pipeline.add_command(command.clone());
        }
        match pipeline.exec_async(&mut connection).await {
            Ok(()) => {
                *slot.lock().await = Some(connection);
                Ok(())
            }
            Err(error) => {
                // MultiplexedConnection has no active `is_open` probe. A complete server error
                // is retained; connection/protocol/network/timeout errors are discarded. An
                // unknown dead connection can consequently be tried once more and will be
                // discarded when the next transaction reports its transport error.
                if !should_discard_multiplexed_transaction_connection(&error) {
                    *slot.lock().await = Some(connection);
                }
                Err(RedisError::transaction_failure(&error))
            }
        }
    }
}

impl fmt::Debug for RedisClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisClient")
            .field("config", &self.inner.config)
            .finish()
    }
}

fn collect_keys<I, K>(keys: I, config: &RedisConfig) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = K>,
    K: AsRef<[u8]>,
{
    let mut collected = Vec::new();
    let mut total = 0;
    for key_value in keys {
        if collected.len() >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        collected.push(key_value);
    }
    Ok(collected)
}

fn collect_value_pairs<I, K, T>(
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (K, T)>,
    K: AsRef<[u8]>,
    T: Serialize,
{
    let mut args = Vec::new();
    let mut total = 0;
    for (key_value, value) in entries {
        if args.len() / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        let value = commands::encoded(&value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(key_value);
        args.push(value);
    }
    Ok(args)
}

fn collect_raw_pairs<I, K, V>(entries: I, config: &RedisConfig) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    let mut args = Vec::new();
    let mut total = 0;
    for (key_value, value) in entries {
        if args.len() / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        let value = commands::raw(value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(key_value);
        args.push(value);
    }
    Ok(args)
}

#[cfg(all(feature = "redis", feature = "tokio"))]
fn collect_hash_pairs<I, K, F, T>(
    key_value: K,
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (F, T)>,
    K: AsRef<[u8]>,
    F: AsRef<[u8]>,
    T: Serialize,
{
    let key_value = commands::key(key_value, config)?;
    let mut args = vec![key_value];
    let mut total = args[0].len();
    for (field_value, value) in entries {
        if (args.len() - 1) / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let field_value = commands::field(field_value, config)?;
        let value = commands::encoded(&value, config)?;
        total = commands::add_batch_bytes(total, field_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(field_value);
        args.push(value);
    }
    Ok(args)
}

#[cfg(all(feature = "redis", feature = "tokio"))]
fn collect_hash_raw_pairs<I, K, F, V>(
    key_value: K,
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (F, V)>,
    K: AsRef<[u8]>,
    F: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    let key_value = commands::key(key_value, config)?;
    let mut args = vec![key_value];
    let mut total = args[0].len();
    for (field_value, value) in entries {
        if (args.len() - 1) / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let field_value = commands::field(field_value, config)?;
        let value = commands::raw(value, config)?;
        total = commands::add_batch_bytes(total, field_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(field_value);
        args.push(value);
    }
    Ok(args)
}

#[cfg(any(test, all(feature = "redis", feature = "tokio")))]
fn check_optional_values(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<Vec<u8>>>, RedisError> {
    let mut response_bytes = 0;
    values
        .into_iter()
        .map(|value| {
            value
                .map(|bytes| {
                    response_bytes = commands::add_response_bytes(response_bytes, &bytes, config)?;
                    Ok(bytes)
                })
                .transpose()
        })
        .collect()
}

#[cfg(all(feature = "redis", feature = "tokio"))]
fn decode_optional_values<T: DeserializeOwned>(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<T>>, RedisError> {
    check_optional_values(values, config)?
        .into_iter()
        .map(|value| {
            value
                .map(|bytes| codec::decode(&bytes, config.max_value_bytes))
                .transpose()
        })
        .collect()
}

#[allow(clippy::type_complexity)]
fn decode_hash_entries(
    flat: Vec<Vec<u8>>,
    config: &RedisConfig,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
    if !flat.len().is_multiple_of(2) {
        return Err(RedisError::Transport(
            super::error::RedisTransportErrorKind::Protocol,
        ));
    }
    let count = flat.len() / 2;
    if count > config.max_collection_items {
        return Err(RedisError::CollectionTooLarge {
            limit: config.max_collection_items,
        });
    }
    let mut response_bytes = 0;
    let mut entries = Vec::with_capacity(count);
    let mut values = flat.into_iter();
    for _ in 0..count {
        let Some(field_value) = values.next() else {
            return Err(RedisError::Transport(
                super::error::RedisTransportErrorKind::Protocol,
            ));
        };
        if field_value.is_empty() || field_value.len() > config.max_key_bytes {
            return Err(RedisError::InvalidField);
        }
        response_bytes = add_response_part(response_bytes, field_value.len(), config)?;
        let Some(value) = values.next() else {
            return Err(RedisError::Transport(
                super::error::RedisTransportErrorKind::Protocol,
            ));
        };
        response_bytes = commands::add_response_bytes(response_bytes, &value, config)?;
        entries.push((field_value, value));
    }
    Ok(entries)
}

fn decode_collection<T: DeserializeOwned>(
    values: Vec<Vec<u8>>,
    config: &RedisConfig,
) -> Result<Vec<T>, RedisError> {
    if values.len() > config.max_collection_items {
        return Err(RedisError::CollectionTooLarge {
            limit: config.max_collection_items,
        });
    }
    let mut response_bytes = 0;
    values
        .into_iter()
        .map(|bytes| {
            response_bytes = commands::add_response_bytes(response_bytes, &bytes, config)?;
            codec::decode(&bytes, config.max_value_bytes)
        })
        .collect()
}

fn add_response_part(
    current: usize,
    bytes: usize,
    config: &RedisConfig,
) -> Result<usize, RedisError> {
    let total = current
        .checked_add(bytes)
        .ok_or(RedisError::ResponseTooLarge {
            limit: config.max_response_bytes,
        })?;
    if total > config.max_response_bytes {
        return Err(RedisError::ResponseTooLarge {
            limit: config.max_response_bytes,
        });
    }
    Ok(total)
}

fn pool_error(error: &r2d2::Error) -> RedisError {
    let detail = error.to_string().to_ascii_lowercase();
    if let Some(kind) = detail
        .split_once(MANAGER_ERROR_PREFIX)
        .and_then(|(_, kind)| parse_manager_error_kind(kind.trim()))
    {
        return RedisError::Transport(kind);
    }
    if detail.trim() == "timed out waiting for connection" {
        RedisError::Timeout
    } else {
        RedisError::Pool
    }
}

fn should_discard_connection(error: &RedisError, is_open: bool) -> bool {
    !is_open
        || matches!(
            error,
            RedisError::Transport(
                RedisTransportErrorKind::Connection
                    | RedisTransportErrorKind::Network
                    | RedisTransportErrorKind::Protocol
                    | RedisTransportErrorKind::Timeout
            )
        )
}

fn should_discard_transaction_connection(error: &::redis::RedisError, is_open: bool) -> bool {
    should_discard_connection(&RedisError::from_upstream(error), is_open)
}

#[cfg(feature = "tokio")]
fn should_discard_multiplexed_transaction_connection(error: &::redis::RedisError) -> bool {
    should_discard_transaction_connection(error, true)
}

fn parse_manager_error_kind(value: &str) -> Option<RedisTransportErrorKind> {
    match value {
        "connection" => Some(RedisTransportErrorKind::Connection),
        "authentication" => Some(RedisTransportErrorKind::Authentication),
        "timeout" => Some(RedisTransportErrorKind::Timeout),
        "protocol" => Some(RedisTransportErrorKind::Protocol),
        "server" => Some(RedisTransportErrorKind::Server),
        "network" => Some(RedisTransportErrorKind::Network),
        "other" => Some(RedisTransportErrorKind::Other),
        _ => None,
    }
}

fn _assert_send_sync<T: Send + Sync>() {}

#[allow(dead_code)]
fn compile_assertions() {
    _assert_send_sync::<RedisClient>();
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "tokio")]
    use super::should_discard_multiplexed_transaction_connection;
    use super::{
        check_optional_values, collect_keys, collect_raw_pairs, decode_collection,
        decode_hash_entries, should_discard_connection, should_discard_transaction_connection,
        RedisClient,
    };
    use crate::redis::{RedisConfig, RedisError, RedisTransportErrorKind};

    #[test]
    fn construction_is_local_and_supports_clone() {
        let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
            .expect("client construction should not connect");
        let clone = client.clone();
        assert!(format!("{clone:?}").contains("RedisClient"));
    }

    #[test]
    fn cluster_transaction_is_rejected_before_callback() {
        let client = RedisClient::new(RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).unwrap())
            .expect("client construction should not connect");
        let result = client.transaction(|_| panic!("callback must not run"));
        assert_eq!(result, Err(RedisError::UnsupportedMode));
    }

    #[test]
    fn local_batch_and_response_limits_are_checked_before_network() {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .unwrap()
            .with_max_batch_items(1)
            .unwrap()
            .with_max_batch_bytes(2)
            .unwrap()
            .with_max_response_bytes(3)
            .unwrap()
            .with_max_collection_items(1)
            .unwrap();
        assert_eq!(
            collect_keys(["a", "b"], &config),
            Err(RedisError::ValueTooLarge { limit: 1 })
        );
        assert_eq!(
            collect_raw_pairs([("a", [1_u8, 2, 3])], &config),
            Err(RedisError::ValueTooLarge { limit: 2 })
        );
        assert_eq!(
            decode_collection::<u8>(vec![vec![1], vec![2]], &config),
            Err(RedisError::CollectionTooLarge { limit: 1 })
        );
        assert_eq!(
            check_optional_values(vec![Some(vec![1, 2]), Some(vec![3, 4])], &config),
            Err(RedisError::ResponseTooLarge { limit: 3 })
        );
    }

    #[test]
    fn hash_response_shape_and_limits_are_checked_locally() {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .unwrap()
            .with_max_key_bytes(8)
            .unwrap()
            .with_max_response_bytes(5)
            .unwrap()
            .with_max_collection_items(1)
            .unwrap();

        assert_eq!(
            decode_hash_entries(vec![b"f".to_vec(), b"v".to_vec()], &config),
            Ok(vec![(b"f".to_vec(), b"v".to_vec())])
        );
        assert_eq!(
            decode_hash_entries(vec![b"f".to_vec()], &config),
            Err(RedisError::Transport(RedisTransportErrorKind::Protocol))
        );
        assert_eq!(
            decode_hash_entries(vec![Vec::new(), b"v".to_vec()], &config),
            Err(RedisError::InvalidField)
        );
        assert_eq!(
            decode_hash_entries(
                vec![b"f".to_vec(), b"v".to_vec(), b"g".to_vec(), b"w".to_vec()],
                &config,
            ),
            Err(RedisError::CollectionTooLarge { limit: 1 })
        );
        assert_eq!(
            decode_hash_entries(vec![b"field".to_vec(), b"v".to_vec()], &config),
            Err(RedisError::ResponseTooLarge { limit: 5 })
        );
    }

    #[test]
    fn uncertain_transport_errors_discard_open_connections() {
        assert!(should_discard_connection(
            &RedisError::Transport(RedisTransportErrorKind::Protocol),
            true
        ));
        assert!(should_discard_connection(
            &RedisError::Transport(RedisTransportErrorKind::Timeout),
            true
        ));
        assert!(!should_discard_connection(
            &RedisError::Transport(RedisTransportErrorKind::Server),
            true
        ));
        assert!(should_discard_connection(
            &RedisError::Transport(RedisTransportErrorKind::Server),
            false
        ));
    }

    #[test]
    fn transaction_discards_only_connections_with_unreliable_state() {
        let server_error = ::redis::RedisError::from((
            ::redis::ErrorKind::Server(::redis::ServerErrorKind::ResponseError),
            "server error",
            "WRONGTYPE operation against a key holding the wrong kind of value".to_owned(),
        ));
        assert!(!should_discard_transaction_connection(&server_error, true));
        assert!(should_discard_transaction_connection(&server_error, false));

        let protocol_error =
            ::redis::RedisError::from((::redis::ErrorKind::Parse, "invalid Redis response"));
        assert!(should_discard_transaction_connection(&protocol_error, true));
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn multiplexed_transaction_keeps_complete_server_errors_but_discards_transport_errors() {
        let server_error = ::redis::RedisError::from((
            ::redis::ErrorKind::Server(::redis::ServerErrorKind::ResponseError),
            "server error",
            "WRONGTYPE operation against a key holding the wrong kind of value".to_owned(),
        ));
        let protocol_error =
            ::redis::RedisError::from((::redis::ErrorKind::Parse, "invalid Redis response"));

        // MultiplexedConnection has no active liveness probe; a complete server response is
        // observable and can be retained, while protocol/transport errors make state uncertain.
        assert!(!should_discard_multiplexed_transaction_connection(
            &server_error
        ));
        assert!(should_discard_multiplexed_transaction_connection(
            &protocol_error
        ));
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[test]
    fn async_commands_require_a_runtime_before_network_access() {
        use std::{
            future::Future,
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
        };

        struct NoopWaker;

        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }

        let client = RedisClient::new(
            RedisConfig::single("redis://127.0.0.1:6379/0").expect("local URL should parse"),
        )
        .expect("client construction should not connect");
        let mut future = Box::pin(client.get_async::<_, u8>("runtime:key"));
        let waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);

        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(RedisError::RuntimeRequired))
        ));
    }

    #[cfg(all(feature = "redis", feature = "tokio"))]
    #[test]
    fn async_cluster_transaction_is_rejected_before_runtime_check() {
        use std::{
            future::Future,
            sync::Arc,
            task::{Context, Poll, Wake, Waker},
        };

        struct NoopWaker;

        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }

        let client = RedisClient::new(
            RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).expect("cluster URL should parse"),
        )
        .expect("client construction should not connect");
        let mut future = Box::pin(
            client.transaction_async(|_| panic!("cluster transaction callback must not run")),
        );
        let waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);

        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(RedisError::UnsupportedMode))
        ));
    }
}
