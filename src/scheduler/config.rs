use std::time::Duration;

use super::SchedulerError;

pub(crate) const DEFAULT_MAX_TASKS: usize = 256;
pub(crate) const MAX_TASKS: usize = 4096;

/// 调度器资源配置。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerConfig {
    /// 同时保留在调度器注册表中的最大活动任务数。
    pub max_tasks: usize,
}

impl SchedulerConfig {
    /// 创建配置；`max_tasks` 必须在 `1..=4096`。
    ///
    /// # Errors
    ///
    /// 超出范围时返回 [`SchedulerError::InvalidConfig`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # fn example() -> Result<(), axutils::SchedulerError> {
    /// let config = axutils::SchedulerConfig::new(32)?;
    /// assert_eq!(config.max_tasks, 32);
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn new(max_tasks: usize) -> Result<Self, SchedulerError> {
        let config = Self { max_tasks };
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn validate(&self) -> Result<(), SchedulerError> {
        if !(1..=MAX_TASKS).contains(&self.max_tasks) {
            return Err(SchedulerError::InvalidConfig { field: "max_tasks" });
        }
        Ok(())
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_tasks: DEFAULT_MAX_TASKS,
        }
    }
}

/// 一个任务的触发方式。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskSchedule {
    /// 在指定延迟后异步执行一次；零延迟合法。
    Once(Duration),
    /// 按 monotonic timer 固定间隔串行执行。
    Interval(Duration),
    /// 使用六段 POSIX/Vixie cron 和显式 IANA 时区执行。
    Cron {
        /// 六段 cron：秒、分、时、日、月、周。
        expression: String,
        /// IANA 时区名称。
        timezone: String,
    },
}

impl TaskSchedule {
    /// 创建一次性任务调度。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let schedule = axutils::TaskSchedule::once(std::time::Duration::ZERO);
    /// assert!(matches!(schedule, axutils::TaskSchedule::Once(_)));
    /// # }
    /// ```
    pub fn once(delay: Duration) -> Self {
        Self::Once(delay)
    }

    /// 创建固定间隔任务调度；零间隔会在注册时拒绝。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let schedule = axutils::TaskSchedule::interval(std::time::Duration::from_secs(5));
    /// assert!(matches!(schedule, axutils::TaskSchedule::Interval(_)));
    /// # }
    /// ```
    pub fn interval(period: Duration) -> Self {
        Self::Interval(period)
    }

    /// 创建 cron 调度；表达式和时区在注册时校验。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let schedule = axutils::TaskSchedule::cron("0 0 0 * * *", "Asia/Shanghai");
    /// assert!(matches!(schedule, axutils::TaskSchedule::Cron { .. }));
    /// # }
    /// ```
    pub fn cron(expression: impl Into<String>, timezone: impl Into<String>) -> Self {
        Self::Cron {
            expression: expression.into(),
            timezone: timezone.into(),
        }
    }
}
