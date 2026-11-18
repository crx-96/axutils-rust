//! Tokio timer 驱动的一次、固定间隔和 IANA 时区 cron 调度器。
//!
//! 该模块仅在启用 `scheduler` feature 时导出。注册使用宿主当前的计时 runtime。
//! `panic = "abort"` 时宿主通过 [`Scheduler::with_time_runtime`] 显式构建并驱动该 runtime。

mod config;
mod cron;
mod error;
pub(crate) mod global;
mod schedule;
mod task;

use std::{future::Future, io, sync::Arc};
use tokio::runtime::{Builder, Runtime};

pub use config::{SchedulerConfig, TaskSchedule};
pub use error::SchedulerError;
pub use task::TaskId;

use task::Shared;

/// 拥有独立生命周期的有界 Tokio 调度器。
///
/// 同一任务的 callback 串行执行；取消和关闭是非阻塞取消请求。实例被丢弃时会请求取消全部活动任务。
pub struct Scheduler {
    /// 共享有界注册表；任务仅持 Weak 清理引用，不延长实例生命周期。
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
    /// # use axutils::scheduler::*;
    /// # use axutils::scheduler::*;
    /// # #[cfg(feature="scheduler")]
    /// # fn example() -> Result<(), SchedulerError> {
    /// let scheduler = Scheduler::new(SchedulerConfig::default())?;
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

    /// 使用宿主提供的 builder 启用时间并构建 runtime，在 `run` 返回前允许本实例在其中注册任务。
    ///
    /// 适用于同步宿主入口，包括 `panic = "abort"`。调用方在 `run` 中驱动 runtime，
    /// 其他驱动与线程设置由 builder 决定。作用域退出时释放 runtime，构建失败返回 I/O 错误。
    ///
    /// # Panics
    ///
    /// 应在同步宿主入口调用。在异步任务中嵌套 `block_on` 或销毁 runtime 可能触发 Tokio panic。
    pub fn with_time_runtime<T>(
        &self,
        builder: &mut Builder,
        run: impl FnOnce(&Runtime) -> T,
    ) -> io::Result<T> {
        // 由这一入口启用实际使用的 time driver，避免把外部 runtime 的配置猜测当作能力证明。
        let runtime = builder.enable_time().build()?;
        Ok(self.shared.with_enabled_time(&runtime, run))
    }

    /// 在当前启用了 time driver 的 Tokio runtime 中注册任务。
    ///
    /// `panic = "abort"` 宿主需通过本实例的 [`Self::with_time_runtime`] 构建 runtime。
    /// Tokio 缺少稳定的 driver 查询接口，无法确认时返回 [`SchedulerError::RuntimeRequired`]。
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
    /// # use axutils::scheduler::*;
    /// # use axutils::scheduler::*;
    /// # #[cfg(feature="scheduler")]
    /// # async fn example() -> Result<(), SchedulerError> {
    /// let scheduler = Scheduler::new(SchedulerConfig::default())?;
    /// let id = scheduler.register(TaskSchedule::once(std::time::Duration::ZERO), || async {})?;
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
    /// # use axutils::scheduler::*;
    /// # use axutils::scheduler::*;
    /// # #[cfg(feature="scheduler")]
    /// # async fn example() -> Result<(), SchedulerError> {
    /// let scheduler = Scheduler::new(SchedulerConfig::default())?;
    /// let id = scheduler.register(TaskSchedule::once(std::time::Duration::from_secs(60)), || async {})?;
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
    /// # use axutils::scheduler::*;
    /// # use axutils::scheduler::*;
    /// # #[cfg(feature="scheduler")]
    /// # fn example() -> Result<(), SchedulerError> {
    /// let scheduler = Scheduler::new(SchedulerConfig::default())?;
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
    /// 请求取消所有登记的任务，不阻塞等待 callback 或其资源实际释放。
    fn drop(&mut self) {
        self.shared.shutdown();
    }
}
