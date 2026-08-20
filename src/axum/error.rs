use std::{error::Error, fmt, io};

/// Axum 服务构建和生命周期错误。Display/Debug 不包含请求、Header、body 或 provider 原始消息。
///
/// # Examples
/// ```rust
/// # #[cfg(all(feature="axum",feature="tokio"))] {
/// let error=axutils::AxumConfig::new().with_max_body_bytes(0).unwrap_err();
/// assert!(matches!(error,axutils::AxumError::InvalidConfig{field:"max_body_bytes"}));
/// # }
/// ```
#[non_exhaustive]
pub enum AxumError {
    /// 配置字段越界或组合无效。
    InvalidConfig {
        /// 稳定配置字段名，不含调用方值。
        field: &'static str,
    },
    /// 服务正在启动、运行或 draining。
    AlreadyRunning,
    /// 单次服务已经停止，不能重新启动。
    AlreadyStopped,
    /// serve future 异常离开，服务状态不可复用。
    Abandoned,
    /// 当前状态不能触发 shutdown。
    NotRunning,
    /// 全局入口尚未初始化。
    NotInitialized,
    /// 全局入口已经初始化。
    AlreadyInitialized,
    /// listener bind 或 local address 查询失败。
    Io(io::Error),
    /// OS shutdown signal 注册失败。
    Signal(io::Error),
    /// 内部生命周期任务异常退出；不暴露 panic payload。
    BackgroundTask,
}
impl fmt::Debug for AxumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => f
                .debug_struct("InvalidConfig")
                .field("field", field)
                .finish(),
            Self::AlreadyRunning => f.write_str("AlreadyRunning"),
            Self::AlreadyStopped => f.write_str("AlreadyStopped"),
            Self::Abandoned => f.write_str("Abandoned"),
            Self::NotRunning => f.write_str("NotRunning"),
            Self::NotInitialized => f.write_str("NotInitialized"),
            Self::AlreadyInitialized => f.write_str("AlreadyInitialized"),
            Self::Io(_) => f.write_str("Io(<redacted>)"),
            Self::Signal(_) => f.write_str("Signal(<redacted>)"),
            Self::BackgroundTask => f.write_str("BackgroundTask"),
        }
    }
}
impl fmt::Display for AxumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => write!(f, "invalid Axum configuration: {field}"),
            Self::AlreadyRunning => f.write_str("Axum server is already running"),
            Self::AlreadyStopped => f.write_str("Axum server has already stopped"),
            Self::Abandoned => f.write_str("Axum server future was abandoned"),
            Self::NotRunning => f.write_str("Axum server is not running"),
            Self::NotInitialized => f.write_str("AxumUtils is not initialized"),
            Self::AlreadyInitialized => f.write_str("AxumUtils is already initialized"),
            Self::Io(_) => f.write_str("Axum listener operation failed"),
            Self::Signal(_) => f.write_str("shutdown signal registration failed"),
            Self::BackgroundTask => f.write_str("Axum background task failed"),
        }
    }
}
impl Error for AxumError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) | Self::Signal(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for AxumError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
