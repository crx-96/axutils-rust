use std::{fmt, sync::Arc, time::Duration};

#[cfg(test)]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[cfg(feature = "redis-async")]
use ::redis::aio::{ConnectionManager, MultiplexedConnection};
#[cfg(feature = "redis-cluster")]
use ::redis::cluster::{ClusterClient, ClusterConfig, ClusterConnection};
#[cfg(feature = "redis-cluster-async")]
use ::redis::cluster_async::ClusterConnection as AsyncClusterConnection;
#[cfg(test)]
use ::redis::Value;
use ::redis::{
    Client as UpstreamClient, Connection, ConnectionLike, RedisError as UpstreamRedisError,
};
use r2d2::{ManageConnection, Pool};
#[cfg(feature = "redis-async")]
use tokio::sync::Mutex as AsyncMutex;

use super::super::{
    config::RedisConfig,
    error::{RedisError, RedisTransportErrorKind},
};

type SinglePool = Pool<SingleManager>;
#[cfg(feature = "redis-cluster")]
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
    pub(super) sync: SyncBackend,
    #[cfg(feature = "redis-async")]
    pub(super) async_backend: AsyncBackend,
}

pub(super) enum SyncBackend {
    Single(SinglePool),
    #[cfg(feature = "redis-cluster")]
    Cluster(ClusterPool),
    #[cfg(test)]
    Fake(Arc<TestRedisBackend>),
}

#[cfg(feature = "redis-async")]
pub(super) enum AsyncBackend {
    Single {
        client: ::redis::Client,
        manager: AsyncMutex<Option<ConnectionManager>>,
        transaction: AsyncMutex<Option<MultiplexedConnection>>,
        transaction_lock: AsyncMutex<()>,
    },
    #[cfg(feature = "redis-cluster-async")]
    Cluster {
        client: ClusterClient,
        connection: AsyncMutex<Option<AsyncClusterConnection>>,
    },
    #[cfg(all(feature = "redis-cluster", not(feature = "redis-cluster-async")))]
    UnsupportedCluster,
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

    pub(super) fn execute<T: ::redis::FromRedisValue>(&self) -> Result<T, RedisError> {
        self.checkout_count.fetch_add(1, Ordering::Relaxed);
        self.command_count.fetch_add(1, Ordering::Relaxed);
        let result = *self
            .result
            .lock()
            .expect("test Redis backend result lock should not be poisoned");
        let value = result?;
        T::from_redis_value(Value::Int(value))
            .map_err(|_| RedisError::Transport(RedisTransportErrorKind::Protocol))
    }
}

pub(super) struct SingleManager {
    client: UpstreamClient,
    connection_timeout: Duration,
    response_timeout: Duration,
}

#[cfg(feature = "redis-cluster")]
pub(super) struct ClusterManager {
    client: ClusterClient,
    connection_timeout: Duration,
    response_timeout: Duration,
}

const MANAGER_ERROR_PREFIX: &str = "axutils-redis-manager:";

#[derive(Debug)]
pub(super) struct SyncManagerError {
    kind: RedisTransportErrorKind,
}

impl SyncManagerError {
    fn new(kind: RedisTransportErrorKind) -> Self {
        Self { kind }
    }

    fn from_upstream(error: &UpstreamRedisError) -> Self {
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

pub(super) struct ManagedConnection<C> {
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

    pub(super) fn mark_broken(&mut self) {
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
    type Connection = ManagedConnection<Connection>;
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

#[cfg(feature = "redis-cluster")]
impl ManageConnection for ClusterManager {
    type Connection = ManagedConnection<ClusterConnection>;
    type Error = SyncManagerError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let cluster_config = ClusterConfig::new()
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
    /// use axutils::redis::{RedisClient, RedisConfig};
    /// let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
    ///     .unwrap();
    /// let _clone = client.clone();
    /// ```
    pub fn new(config: RedisConfig) -> Result<Self, RedisError> {
        let sync = if let Some(url) = config.single_url() {
            let client =
                UpstreamClient::open(url).map_err(|_| RedisError::invalid_config("url"))?;
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
            #[cfg(feature = "redis-cluster")]
            {
                let nodes = config
                    .cluster_nodes()
                    .ok_or(RedisError::invalid_config("nodes"))?;
                let client = ClusterClient::builder(nodes.to_vec())
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
            }
            #[cfg(not(feature = "redis-cluster"))]
            {
                return Err(RedisError::invalid_config("url"));
            }
        };

        #[cfg(feature = "redis-async")]
        let async_backend = if let Some(url) = config.single_url() {
            let client =
                UpstreamClient::open(url).map_err(|_| RedisError::invalid_config("url"))?;
            AsyncBackend::Single {
                client,
                manager: AsyncMutex::new(None),
                transaction: AsyncMutex::new(None),
                transaction_lock: AsyncMutex::new(()),
            }
        } else {
            #[cfg(feature = "redis-cluster-async")]
            {
                let nodes = config
                    .cluster_nodes()
                    .ok_or(RedisError::invalid_config("nodes"))?;
                let client = ClusterClient::builder(nodes.to_vec())
                    .connection_timeout(config.connection_timeout)
                    .response_timeout(config.response_timeout)
                    .build()
                    .map_err(|_| RedisError::invalid_config("nodes"))?;
                AsyncBackend::Cluster {
                    client,
                    connection: AsyncMutex::new(None),
                }
            }
            #[cfg(all(feature = "redis-cluster", not(feature = "redis-cluster-async")))]
            {
                AsyncBackend::UnsupportedCluster
            }
            #[cfg(not(feature = "redis-cluster"))]
            {
                return Err(RedisError::invalid_config("url"));
            }
        };

        Ok(Self {
            inner: Arc::new(RedisClientInner {
                config,
                sync,
                #[cfg(feature = "redis-async")]
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
        #[cfg(feature = "redis-async")]
        let async_backend = AsyncBackend::Fake(Arc::new(backend.clone()));

        (
            Self {
                inner: Arc::new(RedisClientInner {
                    config,
                    sync,
                    #[cfg(feature = "redis-async")]
                    async_backend,
                }),
            },
            backend,
        )
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
pub(super) fn pool_error(error: &r2d2::Error) -> RedisError {
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

pub(super) fn should_discard_connection(error: &RedisError, is_open: bool) -> bool {
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

pub(super) fn should_discard_transaction_connection(
    error: &UpstreamRedisError,
    is_open: bool,
) -> bool {
    should_discard_connection(&RedisError::from_upstream(error), is_open)
}

#[cfg(feature = "redis-async")]
pub(super) fn should_discard_multiplexed_transaction_connection(
    error: &::redis::RedisError,
) -> bool {
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
