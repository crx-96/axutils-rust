use std::{fmt, io};
/// Tokio facade 的稳定、脱敏错误分类。
///
/// # Examples
/// ```rust
/// # #[cfg(feature="tokio")] {
/// assert!(matches!(axutils::TokioUtils::try_current_handle(), Err(axutils::TokioError::RuntimeRequired)));
/// # }
/// ```
#[non_exhaustive]
pub enum TokioError {
    /// 有限配置字段非法。
    InvalidConfig {
        /// 稳定字段名。
        field: &'static str,
    },
    /// 当前线程不在 runtime context 中。
    RuntimeRequired,
    /// 拒绝在现有 runtime context 内创建第二个 runtime。
    NestedRuntime,
    /// Tokio runtime builder 返回 I/O 错误。
    RuntimeBuild(io::Error),
    /// JoinHandle 报告任务 panic 或取消。
    Join(::tokio::task::JoinError),
    /// future 等待超时。
    Timeout,
    /// 操作系统信号注册或等待失败。
    Signal(io::Error),
    /// 任务组关闭后拒绝登记。
    TaskGroupClosed,
    /// 任务组未在 grace 内清空。
    TaskGroupShutdownTimeout {
        /// 超时点的观测数量。
        remaining_tasks: usize,
    },
}
impl fmt::Debug for TokioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => f
                .debug_struct("InvalidConfig")
                .field("field", field)
                .finish(),
            Self::RuntimeRequired => f.write_str("RuntimeRequired"),
            Self::NestedRuntime => f.write_str("NestedRuntime"),
            Self::RuntimeBuild(_) => f.write_str("RuntimeBuild(<redacted>)"),
            Self::Join(_) => f.write_str("Join(<redacted>)"),
            Self::Timeout => f.write_str("Timeout"),
            Self::Signal(_) => f.write_str("Signal(<redacted>)"),
            Self::TaskGroupClosed => f.write_str("TaskGroupClosed"),
            Self::TaskGroupShutdownTimeout { remaining_tasks } => f
                .debug_struct("TaskGroupShutdownTimeout")
                .field("remaining_tasks", remaining_tasks)
                .finish(),
        }
    }
}
impl fmt::Display for TokioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => write!(f, "Tokio 配置无效：{field}"),
            Self::RuntimeRequired => f.write_str("当前线程不在 Tokio runtime context 中"),
            Self::NestedRuntime => {
                f.write_str("不能在现有 Tokio runtime context 内创建或运行另一 runtime")
            }
            Self::RuntimeBuild(_) => f.write_str("Tokio runtime 构建失败"),
            Self::Join(_) => f.write_str("Tokio 任务 join 失败"),
            Self::Timeout => f.write_str("操作超过时间预算"),
            Self::Signal(_) => f.write_str("操作系统关闭信号处理失败"),
            Self::TaskGroupClosed => f.write_str("Tokio 任务组已经关闭"),
            Self::TaskGroupShutdownTimeout { remaining_tasks } => {
                write!(f, "Tokio 任务组关闭超时，剩余任务：{remaining_tasks}")
            }
        }
    }
}
impl std::error::Error for TokioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RuntimeBuild(e) => Some(e),
            Self::Join(e) => Some(e),
            Self::Signal(e) => Some(e),
            _ => None,
        }
    }
}
