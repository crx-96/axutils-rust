use std::time::Duration;

pub(crate) const MAX_ENDPOINT_BYTES: usize = 4 * 1024;
#[cfg(feature = "redis-cluster")]
pub(crate) const MAX_CLUSTER_NODES: usize = 16;
pub(crate) const DEFAULT_MAX_KEY_BYTES: usize = 16 * 1024;
pub(crate) const DEFAULT_MAX_VALUE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_VALUE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_BATCH_ITEMS: usize = 1_024;
pub(crate) const MAX_BATCH_ITEMS: usize = 16_384;
pub(crate) const DEFAULT_MAX_BATCH_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_BATCH_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_COLLECTION_ITEMS: usize = 4_096;
pub(crate) const MAX_COLLECTION_ITEMS: usize = 65_536;
pub(crate) const DEFAULT_MAX_TRANSACTION_COMMANDS: usize = 128;
pub(crate) const MAX_TRANSACTION_COMMANDS: usize = 1_024;
pub(crate) const DEFAULT_MAX_TRANSACTION_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_TRANSACTION_BYTES: usize = 256 * 1024 * 1024;
pub(crate) const DEFAULT_POOL_SIZE: usize = 8;
pub(crate) const MAX_POOL_SIZE: usize = 64;
pub(crate) const DEFAULT_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const DEFAULT_POOL_CHECKOUT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const DEFAULT_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const MIN_TIMEOUT: Duration = Duration::from_millis(1);
pub(crate) const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);
#[cfg(feature = "redis-async")]
pub(crate) const ASYNC_RECONNECT_RETRIES: usize = 6;
#[cfg(feature = "redis-async")]
pub(crate) const ASYNC_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);

/// 经过校验的连接拓扑；端点可能含认证信息，不实现输出原值的 Debug。
enum RedisMode {
    /// 单机连接 URL，包含选择的数据库和可选认证信息。
    Single(String),
    /// Cluster 引导节点 URL；节点非空、认证一致且均选择数据库 0。
    #[cfg(feature = "redis-cluster")]
    Cluster(Vec<String>),
}

/// 已校验的 Redis 单机或 Cluster 配置。
///
/// [`RedisConfig::single`] 和仅在 `redis-cluster` 下提供的 `RedisConfig::cluster` 只进行本地
/// URL/边界校验，不连接 Redis。配置随后由 [`crate::redis::RedisClient::new`] 消费；字段和
/// 认证信息不会通过 getter 暴露，`Debug` 也不会打印 endpoint、用户名或密码。
pub struct RedisConfig {
    /// 单机或 Cluster 的已校验端点；仅供内部创建后端，不能用于日志输出。
    mode: RedisMode,
    /// 同步连接池最大连接数，默认 8，允许 1..=64。
    pub(crate) pool_size: usize,
    /// 建立网络连接的时间预算，默认 5 秒，允许 1 毫秒至 5 分钟。
    pub(crate) connection_timeout: Duration,
    /// 同步连接池获取连接的等待预算，默认 5 秒，允许 1 毫秒至 5 分钟。
    pub(crate) pool_checkout_timeout: Duration,
    /// Redis 命令响应的时间预算，默认 30 秒，允许 1 毫秒至 5 分钟。
    pub(crate) response_timeout: Duration,
    /// 单个 key 或 Hash field 的字节上限，默认 16 KiB，允许 1 字节至 16 KiB。
    pub(crate) max_key_bytes: usize,
    /// 单值原始或 MessagePack 编码字节上限，默认 16 MiB，允许 1 字节至 64 MiB。
    pub(crate) max_value_bytes: usize,
    /// 批量 key/field 操作的项数上限，默认 1,024，允许 1..=16,384。
    pub(crate) max_batch_items: usize,
    /// 批量参数累计字节预算，不含 RESP 开销；默认 64 MiB，允许 1 字节至 256 MiB。
    pub(crate) max_batch_bytes: usize,
    /// 多项响应有效载荷的累计字节上限，默认 64 MiB，允许 1 字节至 256 MiB。
    pub(crate) max_response_bytes: usize,
    /// 集合读取的最大项数，默认 4,096，允许 1..=65,536。
    pub(crate) max_collection_items: usize,
    /// 单个事务最多排队的命令数，默认 128，允许 1..=1,024。
    pub(crate) max_transaction_commands: usize,
    /// 事务中排队命令的 RESP 编码累计字节预算，默认 64 MiB，允许 1 字节至 256 MiB。
    pub(crate) max_transaction_bytes: usize,
}

mod connection;
mod debug;
mod limits;
#[cfg(test)]
mod tests;
mod topology;
mod validation;
