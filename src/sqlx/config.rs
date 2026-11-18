use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use sqlx::{any::AnyConnectOptions, ConnectOptions};

use super::SqlxError;

/// 普通数据库默认允许同时持有的连接数。
const DEFAULT_MAX_CONNECTIONS: u32 = 10;
/// 单个客户端允许配置的最大连接数，防止无界占用数据库资源。
const MAX_CONNECTIONS_LIMIT: u32 = 100;
/// 获取连接的默认等待预算，也用于首次连接池建立。
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
/// 可配置获取预算的最小值，拒绝零超时。
const MIN_ACQUIRE_TIMEOUT: Duration = Duration::from_millis(1);
/// 可配置获取预算的最大值，避免无限等待。
const MAX_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// 多行查询默认允许收集的行数；不限制单行字节数。
const DEFAULT_MAX_ROWS: usize = 1_024;
/// 多行查询可以配置的行数上界。
const MAX_ROWS_LIMIT: usize = 100_000;

/// 由 URL scheme 得到的固定驱动分类；不代表对应 driver 已编译或可连接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlxDriver {
    /// PostgreSQL 的 postgres/postgresql scheme。
    PostgreSql,
    /// MySQL 或 MariaDB 的 mysql/mariadb scheme。
    MySql,
    /// SQLite 文件或内存数据库。
    Sqlite,
}

/// 区分普通连接池和两种需要保留唯一 SQLite 连接的数据库。
#[derive(Clone, Copy, PartialEq, Eq)]
enum DatabaseLifetime {
    /// 普通文件或其他驱动使用常规连接池。
    Persistent,
    /// 空文件名创建的临时库只属于打开它的连接。
    Temporary,
    /// 内存库在最后一个连接关闭时释放。
    Memory,
}

/// SQLx Any 连接池的本地配置。
///
/// `new` 和所有 `with_*` 方法只做本地 URL/参数校验，不访问网络、不安装 Any driver，也不创建
/// 连接池。配置不提供 URL getter；`Debug` 只显示 driver 和非敏感的资源参数，不回显连接 URL、
/// 用户名、密码或查询参数。
/// 创建的连接选项关闭 SQLx 普通和慢查询语句日志，避免 SQL 字面量随日志输出；这不控制应用
/// 自行使用 SQLx 的日志，也不保证屏蔽 SQLx 的其他底层诊断。
///
/// 普通数据库 URL 的最大连接数默认为 `10`，SQLite 内存库和空文件名临时库固定为 `1`。
/// 最大连接数只能是 `1..=100`。`min_connections` 默认为 `0`，且不能超过最大连接数。连接获取超时默认为 30 秒，
/// 允许范围是 `1ms..=5min`。`fetch_all`/`fetch_all_as` 的结果行数上限默认为 `1_024`，允许
/// 范围是 `1..=100_000`。
#[derive(Clone)]
pub struct SqlxConfig {
    /// 已解析 URL 和禁用语句日志的连接选项；只供内部建池，不能直接输出。
    pub(crate) connect_options: AnyConnectOptions,
    /// 脱敏 Debug 与 telemetry 使用的驱动分类。
    driver: SqlxDriver,
    /// 在构造时按连接选项分类，供建池、预算校验和脱敏诊断共用。
    lifetime: DatabaseLifetime,
    /// 同时连接数上限，允许 1..=100，SQLite 内存库和临时库固定为 1。
    pub(crate) max_connections: u32,
    /// 保持的最小连接数，默认为 0，不能大于 max_connections。
    pub(crate) min_connections: u32,
    /// 连接获取的最长等待时间，允许 1 毫秒至 5 分钟。
    pub(crate) acquire_timeout: Duration,
    /// 多行收集的最大行数，允许 1..=100_000。
    pub(crate) max_rows: usize,
}

impl SqlxConfig {
    /// 从数据库 URL 创建本地配置。
    ///
    /// 支持 `postgres://`、`postgresql://`、`mysql://`、`mariadb://` 以及 `sqlite:`/`sqlite://`
    /// URL。首版不配置 TLS；可在 URL 中本地识别的显式 TLS 要求会被拒绝。该方法不会连接数据库，
    /// SQLite 文件也只会在后续 [`crate::sqlx::SqlxClient::connect`] 时产生文件 I/O。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::sqlx::{SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # fn main() -> Result<(), SqlxError> {
    /// use SqlxConfig;
    ///
    /// let config = SqlxConfig::new("sqlite::memory:")?;
    /// assert!(format!("{config:?}").contains("Sqlite"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(url: impl AsRef<str>) -> Result<Self, SqlxError> {
        // 仅本地解析；语句日志在连接创建前关闭，避免绕过本库的脱敏 telemetry。
        let connect_options = AnyConnectOptions::from_str(url.as_ref())
            .map_err(|_| SqlxError::InvalidConfig { field: "url" })?
            .disable_statement_logging();
        // 不访问 driver 注册器，因此仅启用某一个 driver 时仍可离线解析其他受支持 scheme。
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

        // 当前未开放 TLS 配置，显式要求 TLS 的 URL 必须在任何连接尝试前被拒绝。
        if has_unsupported_tls(&connect_options) {
            return Err(SqlxError::InvalidConfig { field: "tls" });
        }

        let lifetime = database_lifetime(&connect_options);
        let mut config = Self {
            connect_options,
            driver,
            lifetime,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            min_connections: 0,
            acquire_timeout: DEFAULT_ACQUIRE_TIMEOUT,
            max_rows: DEFAULT_MAX_ROWS,
        };
        // 内存库和空文件名临时库的状态依赖连接存续，默认值与建池时使用同一判断。
        if config.requires_single_connection() {
            config.max_connections = 1;
        }
        Ok(config)
    }

    /// SQLite 内存库和空文件名临时库需要固定连接，以免换连接后丢失状态。
    pub(super) fn requires_single_connection(&self) -> bool {
        self.lifetime != DatabaseLifetime::Persistent
    }

    /// Debug 和连接事件仅把真正的内存库标记为 memory。
    pub(super) fn sqlite_memory(&self) -> bool {
        self.lifetime == DatabaseLifetime::Memory
    }

    /// 设置连接池最大连接数。
    ///
    /// 允许范围是 `1..=100`。SQLite 内存库和空文件名临时库只能使用 `1`，以保持各次查询
    /// 访问同一数据库。该方法只校验本地配置。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::sqlx::{SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # fn main() -> Result<(), SqlxError> {
    /// use SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_max_connections(1)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_max_connections(mut self, max_connections: u32) -> Result<Self, SqlxError> {
        // 先检查单字段范围，再维护内存库及已有最小连接数的跨字段约束。
        if !(1..=MAX_CONNECTIONS_LIMIT).contains(&max_connections) {
            return Err(SqlxError::InvalidConfig {
                field: "max_connections",
            });
        }
        if self.requires_single_connection() && max_connections != 1 {
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
    /// use axutils::sqlx::{SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # fn main() -> Result<(), SqlxError> {
    /// use SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_min_connections(0)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_min_connections(mut self, min_connections: u32) -> Result<Self, SqlxError> {
        // 允许零连接下界，但不允许构造无法满足的最小/最大数量组合。
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
    /// use axutils::sqlx::{SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # fn main() -> Result<(), SqlxError> {
    /// use std::time::Duration;
    /// use SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?
    ///     .with_acquire_timeout(Duration::from_secs(5))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_acquire_timeout(mut self, acquire_timeout: Duration) -> Result<Self, SqlxError> {
        // 在建池前收敛等待预算，失败时不发布修改后的配置。
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
    /// use axutils::sqlx::{SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # fn main() -> Result<(), SqlxError> {
    /// use SqlxConfig;
    /// let _config = SqlxConfig::new("sqlite::memory:")?.with_max_rows(100)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_max_rows(mut self, max_rows: usize) -> Result<Self, SqlxError> {
        // 正行数预算同时保证查询阶段的 sentinel 行计算可表示。
        if !(1..=MAX_ROWS_LIMIT).contains(&max_rows) {
            return Err(SqlxError::InvalidConfig { field: "max_rows" });
        }
        self.max_rows = max_rows;
        Ok(self)
    }

    /// 建池之前再次核对内部配置不变量，防止内部装配绕过 builder 校验。
    pub(crate) fn validate(&self) -> Result<(), SqlxError> {
        // 跨字段约束先于时间、结果预算检查，保持现有错误优先级。
        if !(1..=MAX_CONNECTIONS_LIMIT).contains(&self.max_connections)
            || self.min_connections > self.max_connections
            || (self.requires_single_connection() && self.max_connections != 1)
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

    #[cfg(feature = "tracing")]
    /// 返回固定且不敏感的驱动 token，供连接事件使用。
    pub(crate) fn driver_name(&self) -> &'static str {
        match self.driver {
            SqlxDriver::PostgreSql => "postgres",
            SqlxDriver::MySql => "mysql",
            SqlxDriver::Sqlite => "sqlite",
        }
    }
}

impl fmt::Debug for SqlxConfig {
    /// 只展示驱动及公开预算，不调用原生连接选项的 Debug。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlxConfig")
            .field("driver", &self.driver)
            .field("sqlite_memory", &self.sqlite_memory())
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .field("max_rows", &self.max_rows)
            .finish()
    }
}

/// 按 SQLx 传给 SQLite 的文件名和打开模式分类；驱动专属参数仍由驱动在连接时校验。
fn database_lifetime(options: &AnyConnectOptions) -> DatabaseLifetime {
    let url = &options.database_url;
    if url.scheme() != "sqlite" {
        return DatabaseLifetime::Persistent;
    }

    // SQLx 从序列化 URL 去掉 scheme（含可选的 //），所以 authority 也属于文件名。
    let database = url
        .as_str()
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:");
    let (name, params) = database.split_once('?').unwrap_or((database, ""));
    let filename = decode_filename_part(name);
    let mut memory = false;
    let mut immutable = false;
    let mut vfs = false;
    for (key, value) in sqlite_params(params) {
        match key.as_ref() {
            "mode" if value == "memory" => memory = true,
            "immutable" => immutable = matches!(value.as_ref(), "1" | "true"),
            "vfs" => vfs = true,
            _ => {}
        }
    }

    // immutable/vfs 会使 SQLx 重新转义整个文件名，原有的 file: 此时只是文件名的一部分。
    let mut path = filename.as_ref();
    let uri_path;
    if let Some(uri) = path.strip_prefix("file:").filter(|_| !immutable && !vfs) {
        // SQLite 的 file: URI 再解码一次；先分隔 fragment 和参数，避免把转义分隔符当语法。
        let uri = uri.split('#').next().unwrap_or_default();
        let (name, params) = uri.split_once('?').unwrap_or((uri, ""));
        let name = match name.strip_prefix("//") {
            Some(authority_path) => {
                let end = authority_path.find('/').unwrap_or(authority_path.len());
                if !matches!(&authority_path[..end], "" | "localhost") {
                    return DatabaseLifetime::Persistent;
                }
                &authority_path[end..]
            }
            None => name,
        };
        uri_path = decode_filename_part(name);
        path = uri_path.split('\0').next().unwrap_or_default();
        // 内嵌 URI 的 mode 由 SQLite 最后应用，按出现顺序覆盖 SQLx 的打开标志。
        for (key, value) in sqlite_params(params) {
            if key.split('\0').next() == Some("mode") {
                memory = value.split('\0').next() == Some("memory");
            }
        }
    }
    match path {
        ":memory:" => DatabaseLifetime::Memory,
        _ if memory => DatabaseLifetime::Memory,
        "" => DatabaseLifetime::Temporary,
        _ => DatabaseLifetime::Persistent,
    }
}

/// 拆分两层参数中的键值，对决定生命周期的 ASCII 标记做百分号解码。
fn sqlite_params(params: &str) -> impl Iterator<Item = (Cow<'_, str>, Cow<'_, str>)> {
    params.split('&').map(|param| {
        let (key, value) = param.split_once('=').unwrap_or((param, ""));
        (decode_filename_part(key), decode_filename_part(value))
    })
}

/// 解码一层文件名或 URI 组件的 %HH，保留字面加号；无转义时借用原文本。
fn decode_filename_part(text: &str) -> Cow<'_, str> {
    let Some((prefix, rest)) = text.split_once('%') else {
        return Cow::Borrowed(text);
    };
    let mut bytes = Vec::with_capacity(text.len());
    bytes.extend_from_slice(prefix.as_bytes());
    for part in rest.split('%') {
        // 只有完整的两个十六进制数字构成转义，其余百分号按原样传递。
        if let Some(hex) = part
            .get(..2)
            .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                bytes.push(byte);
                bytes.extend_from_slice(&part.as_bytes()[2..]);
                continue;
            }
        }
        bytes.push(b'%');
        bytes.extend_from_slice(part.as_bytes());
    }
    // 无效 UTF-8 不是特殊库名；连接失败仍由 SQLx 分类。
    String::from_utf8(bytes).map_or(Cow::Borrowed(text), Cow::Owned)
}

/// 检查可以本地识别的显式 TLS 要求；不把未知 URL 参数解释成安全保证。
fn has_unsupported_tls(options: &AnyConnectOptions) -> bool {
    // 使用固定字段分类返回错误，不把证书路径、URL 参数或认证信息带入诊断。
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
        for (url, memory) in [
            ("sqlite::memory:", true),
            ("sqlite:%3Amemory%3A", true),
            ("sqlite:%66ile:%253Amemory%253A", true),
            ("sqlite:file:", false),
        ] {
            let mut config = SqlxConfig::new(url).unwrap();
            assert_eq!(config.sqlite_memory(), memory, "{url}");
            assert_eq!(config.max_connections, 1, "{url}");
            assert!(config.clone().with_max_connections(1).is_ok());
            assert!(matches!(
                config.clone().with_min_connections(2),
                Err(SqlxError::InvalidConfig {
                    field: "min_connections"
                })
            ));
            config.max_connections = 2;
            assert!(matches!(
                config.validate(),
                Err(SqlxError::InvalidConfig {
                    field: "max_connections"
                })
            ));
        }
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
