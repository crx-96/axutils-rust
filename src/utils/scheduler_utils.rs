use std::{future::Future, sync::OnceLock};

use crate::scheduler::{Scheduler, SchedulerConfig, SchedulerError, TaskId, TaskSchedule};

static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();

/// 一次初始化、不可替换的进程级调度器便捷入口。
///
/// `shutdown` 后全局位置仍保持已初始化，不能 reset 或 replace；需要可控生命周期时直接持有
/// [`Scheduler`]。
pub struct SchedulerUtils;

impl SchedulerUtils {
    /// 校验配置并初始化全局调度器；不要求当前存在 Tokio runtime。
    ///
    /// # Errors
    ///
    /// 配置无效时不占用初始化机会；成功初始化后再次调用返回
    /// [`SchedulerError::AlreadyInitialized`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let _ = axutils::SchedulerUtils::init(axutils::SchedulerConfig::default());
    /// # }
    /// ```
    pub fn init(config: SchedulerConfig) -> Result<(), SchedulerError> {
        let scheduler = Scheduler::new(config)?;
        SCHEDULER
            .set(scheduler)
            .map_err(|_| SchedulerError::AlreadyInitialized)
    }

    /// 返回全局调度器是否曾成功初始化；关闭后仍返回 `true`。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let _ = axutils::SchedulerUtils::is_initialized();
    /// # }
    /// ```
    pub fn is_initialized() -> bool {
        SCHEDULER.get().is_some()
    }

    /// 通过全局调度器注册任务。
    ///
    /// # Errors
    ///
    /// 未初始化时返回 [`SchedulerError::NotInitialized`]，否则转发实例注册错误。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))]
    /// # async fn example() -> Result<(), axutils::SchedulerError> {
    /// let _ = axutils::SchedulerUtils::register(
    ///     axutils::TaskSchedule::once(std::time::Duration::ZERO),
    ///     || async {},
    /// );
    /// # Ok(()) }
    /// # fn main() {}
    /// ```
    pub fn register<F, Fut>(schedule: TaskSchedule, callback: F) -> Result<TaskId, SchedulerError>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self::scheduler()?.register(schedule, callback)
    }

    /// 通过全局调度器请求取消任务。
    ///
    /// # Errors
    ///
    /// 未初始化时返回 [`SchedulerError::NotInitialized`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let _cancel: fn(axutils::TaskId) -> Result<bool, axutils::SchedulerError> =
    ///     axutils::SchedulerUtils::cancel;
    /// # }
    /// ```
    pub fn cancel(task_id: TaskId) -> Result<bool, SchedulerError> {
        Self::scheduler()?.cancel(task_id)
    }

    /// 幂等关闭全局调度器；全局位置不会恢复成未初始化。
    ///
    /// # Errors
    ///
    /// 未初始化时返回 [`SchedulerError::NotInitialized`]。
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
    /// let _ = axutils::SchedulerUtils::shutdown();
    /// # }
    /// ```
    pub fn shutdown() -> Result<(), SchedulerError> {
        Self::scheduler()?.shutdown()
    }

    fn scheduler() -> Result<&'static Scheduler, SchedulerError> {
        SCHEDULER.get().ok_or(SchedulerError::NotInitialized)
    }
}
