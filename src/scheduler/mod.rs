//! Tokio timer 驱动的一次、固定间隔和 IANA 时区 cron 调度器。
//!
//! 该模块仅在同时启用 `chrono`、`chrono_tz`、`tokio`、`croner` feature 时导出。调度器不会
//! 创建 runtime、调用 `block_on` 或接管 signal；注册任务时必须处于启用了 time driver 的 Tokio
//! runtime。

mod config;
mod cron;
mod error;
mod task;

use std::{future::Future, sync::Arc};

pub use config::{SchedulerConfig, TaskSchedule};
pub use error::SchedulerError;
pub use task::TaskId;

use task::Shared;

/// 拥有独立生命周期的有界 Tokio 调度器。
///
/// 同一任务的 callback 串行执行；取消和关闭是非阻塞取消请求。实例被丢弃时会请求取消全部活动任务。
pub struct Scheduler {
    shared: Arc<Shared>,
}

impl Scheduler {
    /// 校验配置并创建调度器；不会创建 runtime 或后台任务。
    ///
    /// # Errors
    ///
    /// `max_tasks` 不在 `1..=4096` 时返回 [`SchedulerError::InvalidConfig`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # fn example() -> Result<(), axutils::SchedulerError> {
    /// let scheduler = axutils::Scheduler::new(axutils::SchedulerConfig::default())?;
    /// scheduler.shutdown()?;
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn new(config: SchedulerConfig) -> Result<Self, SchedulerError> {
        config.validate()?;
        Ok(Self {
            shared: Arc::new(Shared::new(config)),
        })
    }

    /// 在当前启用了 time driver 的 Tokio runtime 中注册任务。
    ///
    /// callback 不会在注册阶段调用；同一任务不会重叠执行。活动任务达到配置上限、调度参数无效、
    /// runtime 不可用或调度器已关闭时均显式返回错误。
    ///
    /// # Errors
    ///
    /// 返回对应的 [`SchedulerError`] 分类，且不会保留半注册任务。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # async fn example() -> Result<(), axutils::SchedulerError> {
    /// let scheduler = axutils::Scheduler::new(axutils::SchedulerConfig::default())?;
    /// let id = scheduler.register(axutils::TaskSchedule::once(std::time::Duration::ZERO), || async {})?;
    /// let _ = scheduler.cancel(id)?;
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn register<F, Fut>(
        &self,
        schedule: TaskSchedule,
        callback: F,
    ) -> Result<TaskId, SchedulerError>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.shared.register(schedule, callback)
    }

    /// 移除活动任务并发出取消请求；未知或已完成任务返回 `Ok(false)`。
    ///
    /// # Errors
    ///
    /// 当前实现不产生错误；保留 `Result` 以维持统一公共契约。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # async fn example() -> Result<(), axutils::SchedulerError> {
    /// let scheduler = axutils::Scheduler::new(axutils::SchedulerConfig::default())?;
    /// let id = scheduler.register(axutils::TaskSchedule::once(std::time::Duration::from_secs(60)), || async {})?;
    /// assert!(scheduler.cancel(id)?);
    /// assert!(!scheduler.cancel(id)?);
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn cancel(&self, task_id: TaskId) -> Result<bool, SchedulerError> {
        Ok(self.shared.cancel(task_id))
    }

    /// 幂等关闭调度器并请求取消全部活动任务；不会等待 callback。
    ///
    /// # Errors
    ///
    /// 当前实现不产生错误；保留 `Result` 以维持统一公共契约。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # fn example() -> Result<(), axutils::SchedulerError> {
    /// let scheduler = axutils::Scheduler::new(axutils::SchedulerConfig::default())?;
    /// scheduler.shutdown()?;
    /// scheduler.shutdown()?;
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn shutdown(&self) -> Result<(), SchedulerError> {
        self.shared.shutdown();
        Ok(())
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shared.shutdown();
    }
}
