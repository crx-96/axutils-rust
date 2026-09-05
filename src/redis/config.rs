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

enum RedisMode {
    Single(String),
    #[cfg(feature = "redis-cluster")]
    Cluster(Vec<String>),
}

/// 已校验的 Redis 单机或 Cluster 配置。
///
/// [`RedisConfig::single`] 和 [`RedisConfig::cluster`] 只进行本地 URL/边界校验，不连接
/// Redis。配置随后由 [`crate::redis::RedisClient::new`] 消费；字段和认证信息不会通过 getter 暴露，
/// `Debug` 也不会打印 endpoint、用户名或密码。
pub struct RedisConfig {
    mode: RedisMode,
    pub(crate) pool_size: usize,
    pub(crate) connection_timeout: Duration,
    pub(crate) pool_checkout_timeout: Duration,
    pub(crate) response_timeout: Duration,
    pub(crate) max_key_bytes: usize,
    pub(crate) max_value_bytes: usize,
    pub(crate) max_batch_items: usize,
    pub(crate) max_batch_bytes: usize,
    pub(crate) max_response_bytes: usize,
    pub(crate) max_collection_items: usize,
    pub(crate) max_transaction_commands: usize,
    pub(crate) max_transaction_bytes: usize,
}

mod connection;
mod debug;
mod limits;
#[cfg(test)]
mod tests;
mod topology;
mod validation;
