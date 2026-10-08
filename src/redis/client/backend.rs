//! 共享客户端状态与惰性后端装配；同步连接管理由 pool 模块负责。

use std::{fmt, sync::Arc};

#[cfg(feature = "redis-async")]
use ::redis::aio::{ConnectionManager, MultiplexedConnection};
#[cfg(feature = "redis-cluster")]
use ::redis::cluster::ClusterClient;
#[cfg(feature = "redis-cluster-async")]
use ::redis::cluster_async::ClusterConnection as AsyncClusterConnection;
use ::redis::Client as UpstreamClient;
use r2d2::Pool;
#[cfg(feature = "redis-async")]
use tokio::sync::Mutex as AsyncMutex;

use crate::redis::{RedisConfig, RedisError};

mod pool;
#[cfg(feature = "redis-async")]
pub(super) use pool::should_discard_multiplexed_transaction_connection;
pub(super) use pool::{
    pool_error, should_discard_connection, should_discard_transaction_connection,
};
#[cfg(feature = "redis-cluster")]
use pool::{ClusterManager, ClusterPool};
use pool::{SingleManager, SinglePool};

#[cfg(test)]
mod fake;
#[cfg(test)]
pub(crate) use fake::TestRedisBackend;

/// 可复用的 Redis 客户端实例。
///
/// 一个实例可以独立配置为单机或 Cluster；`Clone` 只共享同一个连接池和异步连接状态，不会
/// 复制认证信息或预热额外连接。构造阶段只做本地配置和 backend 初始化，不访问 Redis；首次
/// 命令才可能报告连接失败。同步方法会阻塞当前线程，不会把调用转移到线程池，也不会创建
/// Tokio runtime；底层 `r2d2` 连接池的内部管理 worker 仍由连接池自身维护。
pub struct RedisClient {
    /// 克隆共享的配置、同步池和异步连接槽；最后一个拥有者释放底层状态。
    pub(crate) inner: Arc<RedisClientInner>,
}

/// 一个 Redis 配置对应的共享状态；同步和异步连接分别拥有各自生命周期。
pub(crate) struct RedisClientInner {
    /// 本地校验后的不可变配置；只通过脱敏 Debug 对外展示。
    pub(crate) config: RedisConfig,
    /// 构造时建立但不预热连接的同步池。
    pub(super) sync: SyncBackend,
    /// 首次异步命令才在调用方 runtime 中初始化的连接槽。
    #[cfg(feature = "redis-async")]
    pub(super) async_backend: AsyncBackend,
}

/// 同步命令的具体连接池；选择由配置拓扑确定，不在命令失败时隐式切换。
pub(super) enum SyncBackend {
    /// 单机连接池。
    Single(SinglePool),
    /// 集群连接池，每条连接处理集群路由。
    #[cfg(feature = "redis-cluster")]
    Cluster(ClusterPool),
    #[cfg(test)]
    Fake(Arc<TestRedisBackend>),
}

/// 异步连接及初始化门闩；普通命令和事务不共享同一连接。
#[cfg(feature = "redis-async")]
pub(super) enum AsyncBackend {
    /// 单机普通命令使用可重连 manager，事务使用独立且串行借出的连接。
    Single {
        /// 只保存本地连接配置的上游客户端。
        client: UpstreamClient,
        /// 普通命令 manager 的惰性初始化槽，建立后克隆共享。
        manager: AsyncMutex<Option<ConnectionManager>>,
        /// 暂存上一笔可靠结束的事务连接；取出期间取消会丢弃连接。
        transaction: AsyncMutex<Option<MultiplexedConnection>>,
        /// 串行化事务获取、执行及归还，避免两个事务混用状态。
        transaction_lock: AsyncMutex<()>,
    },
    /// 集群异步连接独立于同步池，并在首次命令时初始化。
    #[cfg(feature = "redis-cluster-async")]
    Cluster {
        /// 仅包含本地引导节点配置的上游客户端。
        client: ClusterClient,
        /// 集群连接初始化槽；后续克隆共享上游路由状态。
        connection: AsyncMutex<Option<AsyncClusterConnection>>,
    },
    /// 只启用同步 Cluster 时保留此状态，异步命令返回 UnsupportedMode。
    #[cfg(all(feature = "redis-cluster", not(feature = "redis-cluster-async")))]
    UnsupportedCluster,
    #[cfg(test)]
    Fake(Arc<TestRedisBackend>),
}

impl Clone for RedisClient {
    /// 只增加共享状态的引用计数，不复制配置、不建立连接。
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
        // 按拓扑装配同步 manager；min_idle 为零，构造阶段不预热、不访问 Redis。
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

        // 异步后端这里只创建空槽，不依赖 runtime；首次命令再创建实际连接。
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

        // 两种执行方式共享同一份不可变预算和拓扑，所有克隆沿用此状态。
        Ok(Self {
            inner: Arc::new(RedisClientInner {
                config,
                sync,
                #[cfg(feature = "redis-async")]
                async_backend,
            }),
        })
    }
}
impl fmt::Debug for RedisClient {
    /// 复用配置的脱敏表示，不打印上游 client 或连接对象。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisClient")
            .field("config", &self.inner.config)
            .finish()
    }
}
