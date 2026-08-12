use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use sqlx::any::AnyConnectOptions;

use super::SqlxError;

const DEFAULT_MAX_CONNECTIONS: u32 = 10;
const DEFAULT_MEMORY_MAX_CONNECTIONS: u32 = 1;
const MAX_CONNECTIONS_LIMIT: u32 = 100;
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_ACQUIRE_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const DEFAULT_MAX_ROWS: usize = 1_024;
const MAX_ROWS_LIMIT: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlxDriver {
    PostgreSql,
    MySql,
    Sqlite,
}

/// SQLx Any 连接池的本地配置。
///
/// `new` 和所有 `with_*` 方法只做本地 URL/参数校验，不访问网络、不安装 Any driver，也不创建
/// 连接池。配置不提供 URL getter；`Debug` 只显示 driver 和非敏感的资源参数，不回显连接 URL、
/// 用户名、密码或查询参数。
///
/// 普通数据库 URL 的最大连接数默认为 `10`，SQLite 内存 URL 默认为 `1`；最大连接数只能是
/// `1..=100`。`min_connections` 默认为 `0`，且不能超过最大连接数。连接获取超时默认为 30 秒，
/// 允许范围是 `1ms..=5min`。`fetch_all`/`fetch_all_as` 的结果行数上限默认为 `1_024`，允许
/// 范围是 `1..=100_000`。
#[derive(Clone)]
pub struct SqlxConfig {
    pub(crate) connect_options: AnyConnectOptions,
    driver: SqlxDriver,
    pub(crate) sqlite_memory: bool,
    pub(crate) max_connections: u32,
    pub(crate) min_connections: u32,
    pub(crate) acquire_timeout: Duration,
    pub(crate) max_rows: usize,
}

impl SqlxConfig {
    /// 从数据库 URL 创建本地配置。
    ///
    /// 支持 `postgres://`、`postgresql://`、`mysql://`、`mariadb://` 以及 `sqlite:`/`sqlite://`
    /// URL。首版不配置 TLS；可在 URL 中本地识别的显式 TLS 要求会被拒绝。该方法不会连接数据库，
    /// SQLite 文件也只会在后续 [`crate::SqlxClient::connect`] 时产生文件 I/O。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() -> Result<(), axutils::SqlxError> {
    /// use axutils::SqlxConfig;
    ///
    /// let config = SqlxConfig::new("sqlite::memory:")?;
    /// assert!(format!("{config:?}").contains("Sqlite"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(url: impl AsRef<str>) -> Result<Self, SqlxError> {
        let connect_options = AnyConnectOptions::from_str(url.as_ref())
            .map_err(|_| SqlxError::InvalidConfig { field: "url" })?;
        let driver = match connect_options.database_url.scheme() {
            "postgres" | "postgresql" => SqlxDriver::PostgreSql,
            "mysql" | "mariadb" => SqlxDriver::MySql,
            "sqlite" => SqlxDriver::Sqlite,
            _ => {
                return Err(SqlxError::InvalidConfig {
                    field: "url_scheme",
                })
            }
        };

        if has_unsupported_tls(&connect_options) {
            return Err(SqlxError::InvalidConfig { field: "tls" });
        }

        let sqlite_memory = driver == SqlxDriver::Sqlite && is_sqlite_memory(&connect_options);
        Ok(Self {
            connect_options,
            driver,
            sqlite_memory,
            max_connections: if sqlite_memory {
                DEFAULT_MEMORY_MAX_CONNECTIONS
            } else {
                DEFAULT_MAX_CONNECTIONS
            },
            min_connections: 0,
            acquire_timeout: DEFAULT_ACQUIRE_TIMEOUT,
            max_rows: DEFAULT_MAX_ROWS,
        })
    }

    /// 设置连接池最大连接数。
    ///
    /// 允许范围是 `1..=100`。SQLite 内存 URL 只能使用 `1`，否则每条连接可能拥有不同的内存
    /// 数据库。该方法仍然不会连接数据库。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() -> Result<(), axutils::SqlxError> {
    /// use axutils::SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_max_connections(1)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_max_connections(mut self, max_connections: u32) -> Result<Self, SqlxError> {
        if !(1..=MAX_CONNECTIONS_LIMIT).contains(&max_connections) {
            return Err(SqlxError::InvalidConfig {
                field: "max_connections",
            });
        }
        if self.sqlite_memory && max_connections != 1 {
            return Err(SqlxError::InvalidConfig {
                field: "max_connections",
            });
        }
        if self.min_connections > max_connections {
            return Err(SqlxError::InvalidConfig {
                field: "min_connections",
            });
        }
        self.max_connections = max_connections;
        Ok(self)
    }

    /// 设置连接池最小连接数。
    ///
    /// `0` 表示不预先保持连接；该值不能超过最大连接数。`min_connections > 0` 可能在连接阶段
    /// 建立多个连接，因此会产生网络、认证和数据库资源副作用。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() -> Result<(), axutils::SqlxError> {
    /// use axutils::SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_min_connections(0)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_min_connections(mut self, min_connections: u32) -> Result<Self, SqlxError> {
        if min_connections > self.max_connections {
            return Err(SqlxError::InvalidConfig {
                field: "min_connections",
            });
        }
        self.min_connections = min_connections;
        Ok(self)
    }

    /// 设置连接池获取连接的等待时间。
    ///
    /// 允许范围是 `1ms..=5min`，不接受零值或无限等待。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() -> Result<(), axutils::SqlxError> {
    /// use std::time::Duration;
    /// use axutils::SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?
    ///     .with_acquire_timeout(Duration::from_secs(5))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_acquire_timeout(mut self, acquire_timeout: Duration) -> Result<Self, SqlxError> {
        if !(MIN_ACQUIRE_TIMEOUT..=MAX_ACQUIRE_TIMEOUT).contains(&acquire_timeout) {
            return Err(SqlxError::InvalidConfig {
                field: "acquire_timeout",
            });
        }
        self.acquire_timeout = acquire_timeout;
        Ok(self)
    }

    /// 设置 `fetch_all_async` 和 `fetch_all_as_async` 的最大结果行数。
    ///
    /// 允许范围是 `1..=100_000`。达到上限本身仍成功，只有读取到第 `max_rows + 1` 行才返回
    /// [`SqlxError::RowLimitExceeded`]；该方法不会改变单行查询行为。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() -> Result<(), axutils::SqlxError> {
    /// use axutils::SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_max_rows(100)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_max_rows(mut self, max_rows: usize) -> Result<Self, SqlxError> {
        if !(1..=MAX_ROWS_LIMIT).contains(&max_rows) {
            return Err(SqlxError::InvalidConfig { field: "max_rows" });
        }
        self.max_rows = max_rows;
        Ok(self)
    }

    pub(crate) fn validate(&self) -> Result<(), SqlxError> {
        if !(1..=MAX_CONNECTIONS_LIMIT).contains(&self.max_connections)
            || self.min_connections > self.max_connections
            || (self.sqlite_memory && self.max_connections != 1)
        {
            return Err(SqlxError::InvalidConfig {
                field: "max_connections",
            });
        }
        if !(MIN_ACQUIRE_TIMEOUT..=MAX_ACQUIRE_TIMEOUT).contains(&self.acquire_timeout) {
            return Err(SqlxError::InvalidConfig {
                field: "acquire_timeout",
            });
        }
        if !(1..=MAX_ROWS_LIMIT).contains(&self.max_rows) {
            return Err(SqlxError::InvalidConfig { field: "max_rows" });
        }
        Ok(())
    }
}

impl fmt::Debug for SqlxConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlxConfig")
            .field("driver", &self.driver)
            .field("sqlite_memory", &self.sqlite_memory)
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .field("max_rows", &self.max_rows)
            .finish()
    }
}

fn is_sqlite_memory(options: &AnyConnectOptions) -> bool {
    if options.database_url.scheme() != "sqlite" {
        return false;
    }

    let path = options.database_url.path().trim_start_matches('/');
    path.eq_ignore_ascii_case(":memory:")
        || options.database_url.query_pairs().any(|(key, value)| {
            key.eq_ignore_ascii_case("mode") && value.eq_ignore_ascii_case("memory")
        })
}

fn has_unsupported_tls(options: &AnyConnectOptions) -> bool {
    options.database_url.query_pairs().any(|(key, value)| {
        let key = key.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        match key.as_str() {
            "sslmode" | "ssl-mode" => !matches!(value.as_str(), "disable" | "disabled"),
            "sslrootcert" | "ssl-root-cert" | "sslcert" | "ssl-cert" | "sslkey" | "ssl-key"
            | "tls" | "tlsmode" | "tls-mode" => true,
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::SqlxConfig;
    use crate::sqlx::SqlxError;

    #[test]
    fn defaults_are_bounded_and_debug_is_redacted() {
        let config = SqlxConfig::new("postgres://user:secret@localhost/db").unwrap();
        let debug = format!("{config:?}");
        assert!(debug.contains("max_connections: 10"));
        assert!(!debug.contains("postgres://"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn memory_sqlite_defaults_to_one_and_rejects_more_connections() {
        let config = SqlxConfig::new("sqlite::memory:").unwrap();
        assert!(config.clone().with_max_connections(2).is_err());
        assert!(config.with_max_connections(1).is_ok());
    }

    #[test]
    fn validates_supported_schemes_and_tls() {
        assert!(SqlxConfig::new("postgresql://localhost/db").is_ok());
        assert!(SqlxConfig::new("mariadb://localhost/db").is_ok());
        assert!(SqlxConfig::new("sqlite://file.db").is_ok());
        assert!(SqlxConfig::new("ftp://localhost/db").is_err());
        assert!(matches!(
            SqlxConfig::new("postgres://localhost/db?sslmode=require"),
            Err(SqlxError::InvalidConfig { field: "tls" })
        ));
    }

    #[test]
    fn validates_builder_ranges_and_cross_field_constraints() {
        let config = SqlxConfig::new("sqlite://file.db").unwrap();
        assert!(config.clone().with_max_connections(0).is_err());
        assert!(config.clone().with_max_connections(101).is_err());
        assert!(config.clone().with_min_connections(11).is_err());
        assert!(config.clone().with_acquire_timeout(Duration::ZERO).is_err());
        assert!(config.clone().with_max_rows(0).is_err());
        assert!(config.clone().with_max_rows(100_001).is_err());
        assert!(config
            .with_max_connections(2)
            .unwrap()
            .with_min_connections(2)
            .is_ok());
    }
}
