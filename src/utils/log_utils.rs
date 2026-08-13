//! 面向应用的同步 tracing subscriber 初始化入口。
//!
//! 本模块只在 `logging` feature 下编译。它不会在库加载时自动安装全局 subscriber；应用必须
//! 显式调用 [`LogUtils::init`]。初始化成功后 subscriber 与文件 writer 会持续到进程结束，
//! 不能 reset、replace 或重新配置。

use std::{
    error::Error as StdError,
    fmt, io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use tracing_subscriber::{
    fmt::writer::{BoxMakeWriter, MakeWriterExt},
    EnvFilter,
};

static INITIALIZED: OnceLock<()> = OnceLock::new();
static INITIALIZATION_LOCK: Mutex<()> = Mutex::new(());

/// 日志级别过滤器。
///
/// # Examples
///
/// ```
/// use axutils::LogLevel;
///
/// let level = LogLevel::Info;
/// assert_eq!(level, LogLevel::default());
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LogLevel {
    /// 最详细的诊断事件。
    Trace,
    /// 调试事件。
    Debug,
    /// 常规运行信息，默认级别。
    #[default]
    Info,
    /// 可恢复问题或重试事件。
    Warn,
    /// 需要关注的失败事件。
    Error,
}

impl LogLevel {
    fn as_directive(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// 日志文件的切分策略。
///
/// # Examples
///
/// ```
/// use axutils::LogRotation;
///
/// assert_eq!(LogRotation::default(), LogRotation::Daily);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LogRotation {
    /// 不切分，始终写入传入的精确文件名。
    Never,
    /// 每分钟切分。
    Minutely,
    /// 每小时切分。
    Hourly,
    /// 每天切分，也是 [`LogFileConfig::new`] 的默认策略。
    #[default]
    Daily,
}

/// 日志文件输出配置。
///
/// `path` 是逻辑文件路径：父目录会在初始化时创建，文件名会作为
/// `tracing-appender` 的 filename prefix。`Never` 使用精确文件名，其他策略的后缀由当前
/// `tracing-appender` 版本决定。本类型不暴露路径 getter，也不承诺历史文件清理。
///
/// # Examples
///
/// ```no_run
/// use axutils::{LogFileConfig, LogRotation};
///
/// let _ = LogFileConfig::new("app.log").with_rotation(LogRotation::Never);
/// ```
#[derive(Clone)]
pub struct LogFileConfig {
    path: PathBuf,
    rotation: LogRotation,
}

impl LogFileConfig {
    /// 创建日志文件配置，默认按天切分。
    ///
    /// 路径只在 [`LogUtils::init`] 时校验和创建目录；构造本身不访问文件系统。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogFileConfig, LogRotation};
    ///
    /// let file = LogFileConfig::new(std::env::temp_dir().join("axutils.log"))
    ///     .with_rotation(LogRotation::Never);
    /// let _ = file;
    /// ```
    #[must_use]
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            rotation: LogRotation::Daily,
        }
    }

    /// 设置文件切分策略。
    ///
    /// 后一次调用会替换前一次策略；实际文件名后缀由 `tracing-appender` 决定。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogFileConfig, LogRotation};
    ///
    /// let _ = LogFileConfig::new("app.log").with_rotation(LogRotation::Hourly);
    /// ```
    #[must_use]
    pub fn with_rotation(mut self, rotation: LogRotation) -> Self {
        self.rotation = rotation;
        self
    }

    fn appender(&self) -> Result<tracing_appender::rolling::RollingFileAppender, LogError> {
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(LogError::InvalidPath)?;

        let directory = match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            Some(_) if self.path.is_relative() => Path::new("."),
            None if self.path.is_relative() => Path::new("."),
            _ => return Err(LogError::InvalidPath),
        };

        std::fs::create_dir_all(directory)
            .map_err(|error| LogError::FileInit { kind: error.kind() })?;

        let rotation = match self.rotation {
            LogRotation::Never => tracing_appender::rolling::Rotation::NEVER,
            LogRotation::Minutely => tracing_appender::rolling::Rotation::MINUTELY,
            LogRotation::Hourly => tracing_appender::rolling::Rotation::HOURLY,
            LogRotation::Daily => tracing_appender::rolling::Rotation::DAILY,
        };

        tracing_appender::rolling::RollingFileAppender::builder()
            .rotation(rotation)
            .filename_prefix(file_name)
            .build(directory)
            .map_err(|error| LogError::FileInit {
                kind: StdError::source(&error)
                    .and_then(|source| source.downcast_ref::<io::Error>())
                    .map_or(io::ErrorKind::Other, io::Error::kind),
            })
    }
}

/// `LogUtils` 的输出和过滤配置。
///
/// 默认只输出到标准输出，级别为 [`LogLevel::Info`]，并关闭 ANSI 转义码。通过
/// [`Self::with_file`] 可以改为文件输出或同时输出到标准输出和文件；一个配置至多安装一个
/// 文件 writer，后一次 `with_file` 会替换前一次配置。通过 [`Self::with_directives`] 可以
/// 为 `tracing` target 设置更具体的级别。
///
/// # Examples
///
/// ```no_run
/// use axutils::{LogConfig, LogLevel};
///
/// let _ = LogConfig::new().with_level(LogLevel::Debug);
/// ```
#[derive(Clone)]
pub struct LogConfig {
    stdout: bool,
    level: LogLevel,
    file: Option<LogFileConfig>,
    directives: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            stdout: true,
            level: LogLevel::Info,
            file: None,
            directives: None,
        }
    }
}

impl LogConfig {
    /// 创建默认日志配置，等价于 [`Default::default`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogConfig;
    ///
    /// let config = LogConfig::new();
    /// let _ = config;
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置是否输出到标准输出。
    ///
    /// 文件输出是否启用由 [`Self::with_file`] 单独决定；因此传入 `false` 并不会删除文件
    /// 配置。标准输出和文件都关闭时，初始化返回 [`LogError::InvalidConfig`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogConfig;
    ///
    /// let _ = LogConfig::new().with_stdout(false);
    /// ```
    #[must_use]
    pub fn with_stdout(mut self, enabled: bool) -> Self {
        self.stdout = enabled;
        self
    }

    /// 设置两个输出目标共用的默认最低日志级别。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogConfig, LogLevel};
    ///
    /// let _ = LogConfig::new().with_level(LogLevel::Debug);
    /// ```
    #[must_use]
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// 设置按 `tracing` target 匹配的级别 directive。
    ///
    /// 多条 directive 使用逗号分隔；没有裸级别时，默认级别由 [`Self::with_level`] 提供，更
    /// 具体的 target directive 会覆盖默认级别。例如
    /// `lettre=off,rustls=off,tower_http=debug,sqlx::query=warn`。也可以显式传入裸级别 `off`
    /// 作为默认值，例如 `off,axutils=info,axutils::http=debug`，这样未匹配的 target 默认关闭。
    /// 支持的级别是 `trace`、`debug`、`info`、`warn`、`error` 和 `off`；`warning` 不是合法写法。
    ///
    /// 字符串会在 [`LogUtils::init`] 时解析；解析失败返回
    /// `LogError::InvalidConfig { field: "filter" }`。后一次调用会替换前一次配置；空白字符串
    /// 表示不追加 target directive。逗号、等号两侧的空白会被规范化，directive 内部的空白
    /// 会被拒绝。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogConfig, LogLevel};
    ///
    /// let config = LogConfig::new()
    ///     .with_level(LogLevel::Info)
    ///     .with_directives("lettre=off,rustls=off,tower_http=debug,sqlx::query=warn");
    /// let _ = config;
    /// ```
    #[must_use]
    pub fn with_directives(mut self, directives: impl Into<String>) -> Self {
        let directives = directives.into();
        self.directives = if directives.trim().is_empty() {
            None
        } else {
            Some(directives)
        };
        self
    }

    /// 设置唯一的日志文件输出配置。
    ///
    /// 调用方传入的父目录会在初始化时创建；文件权限沿用操作系统默认创建权限/umask/ACL，
    /// 本 crate 不清理历史轮转文件。日志写入是同步 I/O，会占用产生日志的线程。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogConfig, LogFileConfig, LogRotation};
    ///
    /// let file = LogFileConfig::new("app.log").with_rotation(LogRotation::Never);
    /// let _ = LogConfig::new().with_stdout(false).with_file(file);
    /// ```
    #[must_use]
    pub fn with_file(mut self, file: LogFileConfig) -> Self {
        self.file = Some(file);
        self
    }
}

/// 日志初始化失败或状态冲突。
///
/// 错误不携带路径、第三方 appender 错误对象、日志正文或其他用户输入。`FileInit` 只保留
/// 标准库的 [`io::ErrorKind`]；文件权限和历史文件 retention 由操作系统与部署环境负责。
///
/// # Examples
///
/// ```
/// use axutils::LogError;
///
/// let error = LogError::InvalidConfig { field: "output" };
/// assert_eq!(error.to_string(), "invalid log configuration field: output");
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LogError {
    /// 配置无效；`field` 只返回固定字段类别，例如 `"output"` 或 `"filter"`。
    InvalidConfig { field: &'static str },
    /// 文件路径没有可用的 UTF-8 basename、为空、为根路径或无法拆出父目录。
    InvalidPath,
    /// 创建目录或初始化 file appender 失败。
    FileInit { kind: io::ErrorKind },
    /// 本 crate 已成功安装过 subscriber。
    AlreadyInitialized,
    /// 进程中已有其他全局 tracing subscriber。
    GlobalSubscriberAlreadySet,
    /// 初始化锁已因其他线程 panic 而中毒。
    InitializationLockPoisoned,
    /// 全局状态违反了“安装成功后只写入一次”的内部不变量。
    InitializationStateCorrupted,
}

impl fmt::Display for LogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid log configuration field: {field}")
            }
            Self::InvalidPath => formatter.write_str("invalid log file path"),
            Self::FileInit { kind } => write!(formatter, "failed to initialize log file: {kind}"),
            Self::AlreadyInitialized => formatter.write_str("LogUtils is already initialized"),
            Self::GlobalSubscriberAlreadySet => {
                formatter.write_str("a global tracing subscriber is already installed")
            }
            Self::InitializationLockPoisoned => {
                formatter.write_str("LogUtils initialization lock is poisoned")
            }
            Self::InitializationStateCorrupted => {
                formatter.write_str("LogUtils initialization state is inconsistent")
            }
        }
    }
}

impl std::error::Error for LogError {}

/// 面向应用的进程级日志初始化入口。
///
/// 该类型是无状态的；成功调用 [`Self::init`] 后，全局 subscriber 不能 reset、replace、关闭
/// 或重新配置。库不会在加载时自动初始化日志，也不会因为没有 subscriber 而改变其他 API 的
/// 返回值。`init` 使用同步 formatter 和同步 writer，文件日志可能阻塞产生日志的线程。
///
/// # Examples
///
/// ```no_run
/// use axutils::LogUtils;
///
/// let _ = LogUtils::is_initialized();
/// ```
pub struct LogUtils;

impl LogUtils {
    /// 安装本 crate 约定的全局 tracing subscriber。
    ///
    /// 默认配置只写标准输出；通过 [`LogConfig::with_file`] 可启用文件或双输出，通过
    /// [`LogConfig::with_directives`] 可配置本库和第三方 target 的级别。该方法只在 `logging`
    /// feature 下存在，不创建 Tokio runtime，不调用 `block_on`，也不暴露
    /// `tracing-subscriber`/`tracing-appender` 的内部类型。路径校验、目录创建、过滤规则解析和
    /// file appender 构造失败不会消耗初始化机会；已有本 crate subscriber 时返回
    /// [`LogError::AlreadyInitialized`]，已有其他全局 subscriber 时返回
    /// [`LogError::GlobalSubscriberAlreadySet`]。
    ///
    /// 初始化成功后，formatter 至少输出时间、级别、target 和事件字段，且 ANSI 固定关闭。
    /// 过滤规则只来自 [`LogConfig`]，不会自动读取 `RUST_LOG`。文件权限使用操作系统默认行为，
    /// 轮转不负责历史文件清理。writer 运行时失败不会传播到业务 API，也不会递归记录新的日志
    /// 错误。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{LogConfig, LogError, LogUtils};
    ///
    /// # fn main() -> Result<(), LogError> {
    /// LogUtils::init(LogConfig::default())?;
    /// assert!(LogUtils::is_initialized());
    /// # Ok(())
    /// # }
    /// ```
    pub fn init(config: LogConfig) -> Result<(), LogError> {
        let _lock = INITIALIZATION_LOCK
            .lock()
            .map_err(|_| LogError::InitializationLockPoisoned)?;
        if INITIALIZED.get().is_some() {
            return Err(LogError::AlreadyInitialized);
        }
        if !config.stdout && config.file.is_none() {
            return Err(LogError::InvalidConfig { field: "output" });
        }
        let filter = build_filter(&config)?;
        let writer = match config.file.as_ref() {
            Some(file) if config.stdout => {
                BoxMakeWriter::new(std::io::stdout.and(file.appender()?))
            }
            Some(file) => BoxMakeWriter::new(file.appender()?),
            None => BoxMakeWriter::new(std::io::stdout),
        };
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_target(true)
            .with_writer(writer)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .map_err(|_| LogError::GlobalSubscriberAlreadySet)?;
        if INITIALIZED.set(()).is_err() {
            return Err(LogError::InitializationStateCorrupted);
        }

        crate::tracing::application::record_init();
        Ok(())
    }

    /// 发出一条 `TRACE` 级别、target 为 `axutils::log` 的应用事件。
    ///
    /// 该方法只提交 tracing 事件，是否输出由已安装的 subscriber 和过滤规则决定；调用方必须
    /// 确保消息不包含密码、token、密钥、正文或其他敏感数据。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// LogUtils::trace("详细诊断信息");
    /// ```
    pub fn trace(message: impl fmt::Display) {
        crate::tracing::application::trace(message);
    }

    /// 发出一条 `DEBUG` 级别、target 为 `axutils::log` 的应用事件。
    ///
    /// 该方法只提交 tracing 事件，是否输出由已安装的 subscriber 和过滤规则决定；调用方必须
    /// 确保消息不包含密码、token、密钥、正文或其他敏感数据。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// LogUtils::debug("调试信息");
    /// ```
    pub fn debug(message: impl fmt::Display) {
        crate::tracing::application::debug(message);
    }

    /// 发出一条 `INFO` 级别、target 为 `axutils::log` 的应用事件。
    ///
    /// 该方法只提交 tracing 事件，是否输出由已安装的 subscriber 和过滤规则决定；调用方必须
    /// 确保消息不包含密码、token、密钥、正文或其他敏感数据。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// LogUtils::info("服务已启动");
    /// ```
    pub fn info(message: impl fmt::Display) {
        crate::tracing::application::info(message);
    }

    /// 发出一条 `WARN` 级别、target 为 `axutils::log` 的应用事件。
    ///
    /// 该方法只提交 tracing 事件，是否输出由已安装的 subscriber 和过滤规则决定；调用方必须
    /// 确保消息不包含密码、token、密钥、正文或其他敏感数据。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// LogUtils::warn("即将重试操作");
    /// ```
    pub fn warn(message: impl fmt::Display) {
        crate::tracing::application::warn(message);
    }

    /// 发出一条 `ERROR` 级别、target 为 `axutils::log` 的应用事件。
    ///
    /// 该方法只提交 tracing 事件，是否输出由已安装的 subscriber 和过滤规则决定；调用方必须
    /// 确保消息不包含密码、token、密钥、正文或其他敏感数据。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// LogUtils::error("操作失败");
    /// ```
    pub fn error(message: impl fmt::Display) {
        crate::tracing::application::error(message);
    }

    /// 返回本 crate 是否已经成功安装了自己的全局 subscriber。
    ///
    /// 外部应用已经安装的 subscriber 不会被误报为 `true`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::LogUtils;
    ///
    /// let _ = LogUtils::is_initialized();
    /// ```
    pub fn is_initialized() -> bool {
        INITIALIZED.get().is_some()
    }
}

fn build_filter(config: &LogConfig) -> Result<EnvFilter, LogError> {
    let mut directives = config.level.as_directive().to_owned();
    if let Some(extra) = config.directives.as_deref() {
        let extra =
            normalize_directives(extra).map_err(|_| LogError::InvalidConfig { field: "filter" })?;
        directives.push(',');
        directives.push_str(&extra);
    }

    EnvFilter::try_new(directives).map_err(|_| LogError::InvalidConfig { field: "filter" })
}

fn normalize_directives(value: &str) -> Result<String, ()> {
    let mut normalized = String::new();
    for (index, raw_directive) in value.split(',').enumerate() {
        let directive = raw_directive.trim();
        if directive.is_empty() {
            return Err(());
        }

        let directive = if let Some((target, level)) = directive.split_once('=') {
            let target = target.trim();
            let level = level.trim();
            if target.is_empty()
                || level.is_empty()
                || target.chars().any(char::is_whitespace)
                || level.chars().any(char::is_whitespace)
            {
                return Err(());
            }
            format!("{target}={level}")
        } else {
            if directive.chars().any(char::is_whitespace) {
                return Err(());
            }
            directive.to_owned()
        };

        if index > 0 {
            normalized.push(',');
        }
        normalized.push_str(&directive);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_directives;

    #[test]
    fn normalizes_directive_separator_and_assignment_whitespace() {
        assert_eq!(
            normalize_directives("  lettre = off, rustls= off  ").unwrap(),
            "lettre=off,rustls=off"
        );
        assert_eq!(normalize_directives("off").unwrap(), "off");
    }

    #[test]
    fn rejects_empty_or_internal_directive_whitespace() {
        for value in ["lettre=off,,rustls=off", "rust ls=off", "rustls=off now"] {
            assert!(normalize_directives(value).is_err(), "accepted {value:?}");
        }
    }
}
