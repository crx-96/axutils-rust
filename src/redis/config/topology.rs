#[cfg(feature = "redis-cluster")]
use redis::cluster::ClusterClient;
use redis::Client as UpstreamClient;

use super::validation;
#[cfg(feature = "redis-cluster")]
use super::MAX_CLUSTER_NODES;
use super::{
    RedisConfig, RedisMode, DEFAULT_CONNECTION_TIMEOUT, DEFAULT_MAX_BATCH_BYTES,
    DEFAULT_MAX_BATCH_ITEMS, DEFAULT_MAX_COLLECTION_ITEMS, DEFAULT_MAX_KEY_BYTES,
    DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_MAX_TRANSACTION_BYTES, DEFAULT_MAX_TRANSACTION_COMMANDS,
    DEFAULT_MAX_VALUE_BYTES, DEFAULT_POOL_CHECKOUT_TIMEOUT, DEFAULT_POOL_SIZE,
    DEFAULT_RESPONSE_TIMEOUT,
};
use crate::redis::RedisError;

impl RedisConfig {
    /// 创建并校验单机 Redis 配置。
    ///
    /// 第一阶段只接受 `redis://`，不接受 `rediss://`。该方法不会建立网络连接。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
    /// let _ = config;
    /// ```
    pub fn single(url: impl Into<String>) -> Result<Self, RedisError> {
        let url = url.into();
        validation::validate_endpoint(&url)?;
        validation::validate_database(&url, false)?;
        UpstreamClient::open(url.as_str()).map_err(|_| RedisError::invalid_config("url"))?;
        Ok(Self::defaults(RedisMode::Single(url)))
    }

    #[cfg(feature = "redis-cluster")]
    /// 创建并校验 Redis Cluster 初始节点配置。
    ///
    /// 迭代器最多消费允许上限加一项；节点必须使用 `redis://`、数据库必须为 `0`。该方法
    /// 不建立网络连接。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::redis::RedisConfig;
    /// let config = RedisConfig::cluster([
    ///     "redis://127.0.0.1:7000/0",
    ///     "redis://127.0.0.1:7001/0",
    /// ]).unwrap();
    /// let _ = config;
    /// ```
    pub fn cluster<I, S>(nodes: I) -> Result<Self, RedisError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut collected = Vec::new();
        let mut credentials: Option<Option<String>> = None;
        for node in nodes {
            if collected.len() >= MAX_CLUSTER_NODES {
                return Err(RedisError::invalid_config("nodes"));
            }
            let node = node.into();
            validation::validate_endpoint(&node)?;
            validation::validate_database(&node, true)?;
            let node_credentials = validation::endpoint_credentials(&node).map(str::to_owned);
            if let Some(expected) = credentials.as_ref() {
                if expected.as_ref() != node_credentials.as_ref() {
                    return Err(RedisError::invalid_config("credentials"));
                }
            } else {
                credentials = Some(node_credentials);
            }
            collected.push(node);
        }
        if collected.is_empty() {
            return Err(RedisError::invalid_config("nodes"));
        }

        ClusterClient::builder(collected.clone())
            .build()
            .map_err(|error| {
                let detail = error.detail().unwrap_or_default().to_ascii_lowercase();
                if detail.contains("password") || detail.contains("username") {
                    RedisError::invalid_config("credentials")
                } else {
                    RedisError::invalid_config("nodes")
                }
            })?;
        Ok(Self::defaults(RedisMode::Cluster(collected)))
    }

    fn defaults(mode: RedisMode) -> Self {
        Self {
            mode,
            pool_size: DEFAULT_POOL_SIZE,
            connection_timeout: DEFAULT_CONNECTION_TIMEOUT,
            pool_checkout_timeout: DEFAULT_POOL_CHECKOUT_TIMEOUT,
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            max_key_bytes: DEFAULT_MAX_KEY_BYTES,
            max_value_bytes: DEFAULT_MAX_VALUE_BYTES,
            max_batch_items: DEFAULT_MAX_BATCH_ITEMS,
            max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_collection_items: DEFAULT_MAX_COLLECTION_ITEMS,
            max_transaction_commands: DEFAULT_MAX_TRANSACTION_COMMANDS,
            max_transaction_bytes: DEFAULT_MAX_TRANSACTION_BYTES,
        }
    }

    pub(crate) fn is_cluster(&self) -> bool {
        #[cfg(feature = "redis-cluster")]
        {
            matches!(&self.mode, RedisMode::Cluster(_))
        }
        #[cfg(not(feature = "redis-cluster"))]
        {
            false
        }
    }

    pub(crate) fn single_url(&self) -> Option<&str> {
        match &self.mode {
            RedisMode::Single(url) => Some(url),
            #[cfg(feature = "redis-cluster")]
            RedisMode::Cluster(_) => None,
        }
    }

    pub(crate) fn cluster_nodes(&self) -> Option<&[String]> {
        #[cfg(feature = "redis-cluster")]
        match &self.mode {
            RedisMode::Single(_) => None,
            RedisMode::Cluster(nodes) => Some(nodes),
        }
        #[cfg(not(feature = "redis-cluster"))]
        {
            None
        }
    }
}
