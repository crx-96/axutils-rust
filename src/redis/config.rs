use std::{fmt, time::Duration};

use super::error::RedisError;

pub(crate) const MAX_ENDPOINT_BYTES: usize = 4 * 1024;
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
#[cfg(all(feature = "redis", feature = "tokio"))]
pub(crate) const ASYNC_RECONNECT_RETRIES: usize = 6;
#[cfg(all(feature = "redis", feature = "tokio"))]
pub(crate) const ASYNC_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);

enum RedisMode {
    Single(String),
    Cluster(Vec<String>),
}

/// 已校验的 Redis 单机或 Cluster 配置。
///
/// [`RedisConfig::single`] 和 [`RedisConfig::cluster`] 只进行本地 URL/边界校验，不连接
/// Redis。配置随后由 [`crate::RedisClient::new`] 消费；字段和认证信息不会通过 getter 暴露，
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

impl RedisConfig {
    /// 创建并校验单机 Redis 配置。
    ///
    /// 第一阶段只接受 `redis://`，不接受 `rediss://`。该方法不会建立网络连接。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
    /// let _ = config;
    /// ```
    pub fn single(url: impl Into<String>) -> Result<Self, RedisError> {
        let url = url.into();
        validate_endpoint(&url)?;
        validate_database(&url, false)?;
        ::redis::Client::open(url.as_str()).map_err(|_| RedisError::invalid_config("url"))?;
        Ok(Self::defaults(RedisMode::Single(url)))
    }

    /// 创建并校验 Redis Cluster 初始节点配置。
    ///
    /// 迭代器最多消费允许上限加一项；节点必须使用 `redis://`、数据库必须为 `0`。该方法
    /// 不建立网络连接。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
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
            validate_endpoint(&node)?;
            validate_database(&node, true)?;
            let node_credentials = endpoint_credentials(&node).map(str::to_owned);
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

        ::redis::cluster::ClusterClient::builder(collected.clone())
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

    /// 设置同步连接池最大连接数，范围为 `1..=64`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_pool_size(4)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_pool_size(mut self, max: usize) -> Result<Self, RedisError> {
        self.pool_size = bounded(max, 1, MAX_POOL_SIZE, "pool_size")?;
        Ok(self)
    }

    /// 设置建立网络连接的时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_connection_timeout(Duration::from_secs(2))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.connection_timeout = checked_timeout(timeout, "connection_timeout")?;
        Ok(self)
    }

    /// 设置同步连接池 checkout 的等待时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_pool_checkout_timeout(Duration::from_secs(2))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_pool_checkout_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.pool_checkout_timeout = checked_timeout(timeout, "pool_checkout_timeout")?;
        Ok(self)
    }

    /// 设置 Redis 命令响应时间预算，范围为 `1 ms..=5 min`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    /// use std::time::Duration;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_response_timeout(Duration::from_secs(10))
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_response_timeout(mut self, timeout: Duration) -> Result<Self, RedisError> {
        self.response_timeout = checked_timeout(timeout, "response_timeout")?;
        Ok(self)
    }

    /// 设置 key 和 Hash field 的最大字节数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_key_bytes(1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_key_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_key_bytes = bounded(limit, 1, DEFAULT_MAX_KEY_BYTES, "max_key_bytes")?;
        Ok(self)
    }

    /// 设置单值最大原始字节数，最大为 64 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_value_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_value_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_value_bytes = bounded(limit, 1, MAX_VALUE_BYTES, "max_value_bytes")?;
        Ok(self)
    }

    /// 设置批量 key/field 的最大项数，最大为 16,384。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_batch_items(128)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_batch_items(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_batch_items = bounded(limit, 1, MAX_BATCH_ITEMS, "max_batch_items")?;
        Ok(self)
    }

    /// 设置批量命令编码总字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_batch_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_batch_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_batch_bytes = bounded(limit, 1, MAX_BATCH_BYTES, "max_batch_bytes")?;
        Ok(self)
    }

    /// 设置多项响应累计字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_response_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_response_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_response_bytes = bounded(limit, 1, MAX_RESPONSE_BYTES, "max_response_bytes")?;
        Ok(self)
    }

    /// 设置集合读取最大项数，最大为 65,536。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_collection_items(256)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_collection_items(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_collection_items =
            bounded(limit, 1, MAX_COLLECTION_ITEMS, "max_collection_items")?;
        Ok(self)
    }

    /// 设置事务最大排队命令数，最大为 1,024。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_transaction_commands(64)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_transaction_commands(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_transaction_commands = bounded(
            limit,
            1,
            MAX_TRANSACTION_COMMANDS,
            "max_transaction_commands",
        )?;
        Ok(self)
    }

    /// 设置事务编码总字节数，最大为 256 MiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisConfig;
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    ///     .unwrap()
    ///     .with_max_transaction_bytes(1024 * 1024)
    ///     .unwrap();
    /// let _ = config;
    /// ```
    pub fn with_max_transaction_bytes(mut self, limit: usize) -> Result<Self, RedisError> {
        self.max_transaction_bytes =
            bounded(limit, 1, MAX_TRANSACTION_BYTES, "max_transaction_bytes")?;
        Ok(self)
    }

    pub(crate) fn is_cluster(&self) -> bool {
        matches!(&self.mode, RedisMode::Cluster(_))
    }

    pub(crate) fn single_url(&self) -> Option<&str> {
        match &self.mode {
            RedisMode::Single(url) => Some(url),
            RedisMode::Cluster(_) => None,
        }
    }

    pub(crate) fn cluster_nodes(&self) -> Option<&[String]> {
        match &self.mode {
            RedisMode::Single(_) => None,
            RedisMode::Cluster(nodes) => Some(nodes),
        }
    }
}

impl fmt::Debug for RedisConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisConfig")
            .field(
                "mode",
                if self.is_cluster() {
                    &"cluster"
                } else {
                    &"single"
                },
            )
            .field(
                "endpoint_count",
                &self.cluster_nodes().map_or(1, |nodes| nodes.len()),
            )
            .field("endpoints", &"[REDACTED]")
            .field("pool_size", &self.pool_size)
            .field("connection_timeout", &self.connection_timeout)
            .field("pool_checkout_timeout", &self.pool_checkout_timeout)
            .field("response_timeout", &self.response_timeout)
            .field("max_key_bytes", &self.max_key_bytes)
            .field("max_value_bytes", &self.max_value_bytes)
            .field("max_batch_items", &self.max_batch_items)
            .field("max_batch_bytes", &self.max_batch_bytes)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("max_collection_items", &self.max_collection_items)
            .field("max_transaction_commands", &self.max_transaction_commands)
            .field("max_transaction_bytes", &self.max_transaction_bytes)
            .finish()
    }
}

fn validate_endpoint(endpoint: &str) -> Result<(), RedisError> {
    if endpoint.is_empty() {
        return Err(RedisError::invalid_config("url"));
    }
    if endpoint.len() > MAX_ENDPOINT_BYTES || endpoint.chars().any(char::is_control) {
        return Err(RedisError::invalid_config("url"));
    }
    if !endpoint.starts_with("redis://") {
        return Err(RedisError::invalid_config("scheme"));
    }
    Ok(())
}

fn validate_database(endpoint: &str, cluster: bool) -> Result<(), RedisError> {
    let after_scheme = &endpoint["redis://".len()..];
    let Some(slash) = after_scheme.find('/') else {
        return Ok(());
    };
    let database = &after_scheme[slash + 1..];
    let database = database.split(['?', '#']).next().unwrap_or_default();
    if database.is_empty() {
        return Ok(());
    }
    let Ok(number) = database.parse::<i64>() else {
        return Err(RedisError::invalid_config("database"));
    };
    if number < 0 || (cluster && number != 0) {
        return Err(RedisError::invalid_config("database"));
    }
    Ok(())
}

fn endpoint_credentials(endpoint: &str) -> Option<&str> {
    let authority = endpoint["redis://".len()..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    authority.rfind('@').map(|at| &authority[..at])
}

fn bounded(value: usize, min: usize, max: usize, field: &'static str) -> Result<usize, RedisError> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(RedisError::invalid_config(field))
    }
}

fn checked_timeout(value: Duration, field: &'static str) -> Result<Duration, RedisError> {
    if (MIN_TIMEOUT..=MAX_TIMEOUT).contains(&value) {
        Ok(value)
    } else {
        Err(RedisError::invalid_config(field))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::time::Duration;

    use super::{
        RedisConfig, DEFAULT_MAX_KEY_BYTES, MAX_BATCH_BYTES, MAX_BATCH_ITEMS, MAX_CLUSTER_NODES,
        MAX_COLLECTION_ITEMS, MAX_ENDPOINT_BYTES, MAX_POOL_SIZE, MAX_RESPONSE_BYTES,
        MAX_TRANSACTION_BYTES, MAX_TRANSACTION_COMMANDS, MAX_VALUE_BYTES,
    };
    use crate::redis::RedisError;

    #[test]
    fn validates_scheme_database_and_bounded_nodes() {
        assert!(matches!(
            RedisConfig::single("rediss://127.0.0.1:6379"),
            Err(RedisError::InvalidConfig { field: "scheme" })
        ));
        assert!(matches!(
            RedisConfig::cluster(["redis://127.0.0.1:7000/1"]),
            Err(RedisError::InvalidConfig { field: "database" })
        ));
        let nodes =
            (0..=MAX_CLUSTER_NODES).map(|index| format!("redis://127.0.0.1:{}/0", 7000 + index));
        assert!(matches!(
            RedisConfig::cluster(nodes),
            Err(RedisError::InvalidConfig { field: "nodes" })
        ));
    }

    #[test]
    fn bounds_empty_and_overlong_cluster_inputs() {
        assert!(matches!(
            RedisConfig::cluster(std::iter::empty::<&str>()),
            Err(RedisError::InvalidConfig { field: "nodes" })
        ));

        let oversized = format!("redis://{}", "a".repeat(MAX_ENDPOINT_BYTES));
        assert!(matches!(
            RedisConfig::single(oversized),
            Err(RedisError::InvalidConfig { field: "url" })
        ));

        let pulls = Cell::new(0);
        let result = RedisConfig::cluster(std::iter::from_fn(|| {
            let index = pulls.get();
            pulls.set(index + 1);
            Some(format!("redis://127.0.0.1:{}/0", 7000 + index))
        }));
        assert!(matches!(
            result,
            Err(RedisError::InvalidConfig { field: "nodes" })
        ));
        assert_eq!(pulls.get(), MAX_CLUSTER_NODES + 1);
    }

    #[test]
    fn validates_timeout_and_debug_redaction() {
        let config = RedisConfig::single("redis://:secret@redis.example.com:6379/0")
            .expect("config")
            .with_connection_timeout(Duration::ZERO);
        assert!(matches!(
            config,
            Err(RedisError::InvalidConfig {
                field: "connection_timeout"
            })
        ));

        let config =
            RedisConfig::single("redis://:secret@redis.example.com:6379/0").expect("config");
        let debug = format!("{config:?}");
        assert!(!debug.contains("redis.example.com"));
        assert!(!debug.contains("secret"));
        assert!(debug.contains("endpoint_count"));
    }

    #[test]
    fn defaults_are_stable_and_debug_is_fully_redacted() {
        let config =
            RedisConfig::single("redis://:password@redis.example.com:6379/0").expect("config");
        assert_eq!(config.pool_size, super::DEFAULT_POOL_SIZE);
        assert_eq!(config.connection_timeout, super::DEFAULT_CONNECTION_TIMEOUT);
        assert_eq!(
            config.pool_checkout_timeout,
            super::DEFAULT_POOL_CHECKOUT_TIMEOUT
        );
        assert_eq!(config.response_timeout, super::DEFAULT_RESPONSE_TIMEOUT);
        assert_eq!(config.max_key_bytes, super::DEFAULT_MAX_KEY_BYTES);
        assert_eq!(config.max_value_bytes, super::DEFAULT_MAX_VALUE_BYTES);
        assert_eq!(config.max_batch_items, super::DEFAULT_MAX_BATCH_ITEMS);
        assert_eq!(config.max_batch_bytes, super::DEFAULT_MAX_BATCH_BYTES);
        assert_eq!(config.max_response_bytes, super::DEFAULT_MAX_RESPONSE_BYTES);
        assert_eq!(
            config.max_collection_items,
            super::DEFAULT_MAX_COLLECTION_ITEMS
        );
        assert_eq!(
            config.max_transaction_commands,
            super::DEFAULT_MAX_TRANSACTION_COMMANDS
        );
        assert_eq!(
            config.max_transaction_bytes,
            super::DEFAULT_MAX_TRANSACTION_BYTES
        );
        let debug = format!("{config:?}");
        assert!(!debug.contains("redis.example.com"));
        assert!(!debug.contains("password"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn rejects_zero_and_over_max_builder_limits() {
        let config = || RedisConfig::single("redis://127.0.0.1:6379/0").expect("config");
        let over_timeout = Duration::from_secs(5 * 60) + Duration::from_millis(1);

        assert!(matches!(
            config().with_pool_size(0),
            Err(RedisError::InvalidConfig { field: "pool_size" })
        ));
        assert!(matches!(
            config().with_pool_size(MAX_POOL_SIZE + 1),
            Err(RedisError::InvalidConfig { field: "pool_size" })
        ));
        assert!(matches!(
            config().with_connection_timeout(over_timeout),
            Err(RedisError::InvalidConfig {
                field: "connection_timeout"
            })
        ));
        assert!(matches!(
            config().with_pool_checkout_timeout(over_timeout),
            Err(RedisError::InvalidConfig {
                field: "pool_checkout_timeout"
            })
        ));
        assert!(matches!(
            config().with_response_timeout(over_timeout),
            Err(RedisError::InvalidConfig {
                field: "response_timeout"
            })
        ));
        assert!(matches!(
            config().with_max_key_bytes(0),
            Err(RedisError::InvalidConfig {
                field: "max_key_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_key_bytes(DEFAULT_MAX_KEY_BYTES + 1),
            Err(RedisError::InvalidConfig {
                field: "max_key_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_value_bytes(0),
            Err(RedisError::InvalidConfig {
                field: "max_value_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_value_bytes(MAX_VALUE_BYTES + 1),
            Err(RedisError::InvalidConfig {
                field: "max_value_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_batch_items(0),
            Err(RedisError::InvalidConfig {
                field: "max_batch_items"
            })
        ));
        assert!(matches!(
            config().with_max_batch_items(MAX_BATCH_ITEMS + 1),
            Err(RedisError::InvalidConfig {
                field: "max_batch_items"
            })
        ));
        assert!(matches!(
            config().with_max_batch_bytes(0),
            Err(RedisError::InvalidConfig {
                field: "max_batch_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_batch_bytes(MAX_BATCH_BYTES + 1),
            Err(RedisError::InvalidConfig {
                field: "max_batch_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_response_bytes(0),
            Err(RedisError::InvalidConfig {
                field: "max_response_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_response_bytes(MAX_RESPONSE_BYTES + 1),
            Err(RedisError::InvalidConfig {
                field: "max_response_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_collection_items(0),
            Err(RedisError::InvalidConfig {
                field: "max_collection_items"
            })
        ));
        assert!(matches!(
            config().with_max_collection_items(MAX_COLLECTION_ITEMS + 1),
            Err(RedisError::InvalidConfig {
                field: "max_collection_items"
            })
        ));
        assert!(matches!(
            config().with_max_transaction_commands(0),
            Err(RedisError::InvalidConfig {
                field: "max_transaction_commands"
            })
        ));
        assert!(matches!(
            config().with_max_transaction_commands(MAX_TRANSACTION_COMMANDS + 1),
            Err(RedisError::InvalidConfig {
                field: "max_transaction_commands"
            })
        ));
        assert!(matches!(
            config().with_max_transaction_bytes(0),
            Err(RedisError::InvalidConfig {
                field: "max_transaction_bytes"
            })
        ));
        assert!(matches!(
            config().with_max_transaction_bytes(MAX_TRANSACTION_BYTES + 1),
            Err(RedisError::InvalidConfig {
                field: "max_transaction_bytes"
            })
        ));
    }
}
