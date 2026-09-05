use std::sync::OnceLock;

use super::{Scheduler, SchedulerConfig, SchedulerError};

static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();

/// 一次初始化、不可替换的进程级调度器便捷入口。
///
/// `shutdown` 后全局位置仍保持已初始化，不能 reset 或 replace；需要可控生命周期时直接持有
/// [`Scheduler`]。
pub struct SchedulerUtils;

impl SchedulerUtils {
    /// 校验配置并初始化全局调度器；不要求当前存在 Tokio runtime。
    pub fn init(config: SchedulerConfig) -> Result<(), SchedulerError> {
        let scheduler = Scheduler::new(config)?;
        SCHEDULER
            .set(scheduler)
            .map_err(|_| SchedulerError::AlreadyInitialized)
    }

    /// 返回全局调度器是否曾成功初始化；关闭后仍返回 `true`。
    pub fn is_initialized() -> bool {
        SCHEDULER.get().is_some()
    }

    /// 返回已初始化的全局调度器。
    ///
    /// 未初始化时返回 [`SchedulerError::NotInitialized`]。关闭后的 scheduler 仍可被取得，但其
    /// 注册操作会保留底层 [`SchedulerError::Shutdown`] 语义。
    pub fn scheduler() -> Result<&'static Scheduler, SchedulerError> {
        SCHEDULER.get().ok_or(SchedulerError::NotInitialized)
    }
}
