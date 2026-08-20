use std::{error::Error, fmt};

/// 调度器配置、注册和全局入口错误。
///
/// 该枚举是可扩展的；调用方匹配时必须保留 wildcard 分支。错误不会包含原始 cron、时区或
/// callback 数据。
///
/// # Examples
///
/// ```rust
/// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
/// let error = axutils::SchedulerConfig::new(0).unwrap_err();
/// assert!(matches!(error, axutils::SchedulerError::InvalidConfig { .. }));
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SchedulerError {
    /// 配置字段无效。
    InvalidConfig { field: &'static str },
    /// 一次或固定间隔调度参数无效。
    InvalidSchedule,
    /// cron 表达式或未来触发时间无效。
    InvalidCron,
    /// IANA 时区无效。
    InvalidTimezone,
    /// 当前线程没有启用 time driver 的 Tokio runtime。
    RuntimeRequired,
    /// 全局调度器已经初始化。
    AlreadyInitialized,
    /// 全局调度器尚未初始化。
    NotInitialized,
    /// 活动任务数量或任务 ID 已达到上限。
    TaskLimitExceeded,
    /// 调度器已经关闭。
    Shutdown,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid scheduler configuration: {field}")
            }
            Self::InvalidSchedule => formatter.write_str("invalid task schedule"),
            Self::InvalidCron => formatter.write_str("invalid cron schedule"),
            Self::InvalidTimezone => formatter.write_str("invalid IANA timezone"),
            Self::RuntimeRequired => {
                formatter.write_str("a Tokio runtime with an enabled time driver is required")
            }
            Self::AlreadyInitialized => formatter.write_str("scheduler is already initialized"),
            Self::NotInitialized => formatter.write_str("scheduler is not initialized"),
            Self::TaskLimitExceeded => formatter.write_str("scheduler task limit exceeded"),
            Self::Shutdown => formatter.write_str("scheduler is shut down"),
        }
    }
}

impl Error for SchedulerError {}
