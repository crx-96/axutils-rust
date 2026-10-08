//! 有界任务登记、取消与完成清理；实际时间策略由 schedule 模块负责。

use super::{schedule, SchedulerConfig, SchedulerError, TaskSchedule};
use std::{
    collections::HashMap,
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, MutexGuard, Weak},
    time::Duration,
};
use tokio::{runtime::Handle, task::AbortHandle, time};

/// 单个 [`Scheduler`](super::Scheduler) 内不复用的任务标识。
///
/// # Examples
///
/// ```rust
/// # use axutils::scheduler::*;
/// # #[cfg(feature="scheduler")] {
/// let _cancel: fn(&Scheduler, TaskId)
///     -> Result<bool, SchedulerError> = Scheduler::cancel;
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(
    /// 调度器内单调递增的标识值，溢出时拒绝新登记而不复用。
    u64,
);

/// 调度器所有登记操作共享的状态；不持有任务 callback。
pub(crate) struct Shared {
    /// 只在短暂登记/移除阶段持有的注册表锁。
    state: Mutex<State>,
}

/// 单个调度器的有界活动任务注册表。
struct State {
    /// 永久关闭标记，关闭后不接纳任何新任务。
    shutdown: bool,
    /// 同时保留的登记数上限，包含正在提交的占位项。
    max_tasks: usize,
    /// 下次登记使用的 ID；到达 u64 上界时返回容量错误。
    next_task_id: u64,
    /// None 为锁外提交期间的占位，Some 为已发布的取消句柄。
    tasks: HashMap<TaskId, Option<AbortHandle>>,
}

impl Shared {
    /// 建立空注册表，不获取 runtime 或启动后台工作。
    pub(crate) fn new(config: SchedulerConfig) -> Self {
        Self {
            state: Mutex::new(State {
                shutdown: false,
                max_tasks: config.max_tasks,
                next_task_id: 1,
                tasks: HashMap::new(),
            }),
        }
    }

    /// 校验运行条件、预留容量，再在锁外提交；失败或完成都会撤销自己的登记。
    pub(crate) fn register<F, Fut>(
        self: &Arc<Self>,
        schedule: TaskSchedule,
        callback: F,
    ) -> Result<TaskId, SchedulerError>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        // 关闭错误优先；校验完成后仍在预留阶段重新检查，以覆盖并发关闭。
        if self.lock().shutdown {
            return Err(SchedulerError::Shutdown);
        }
        let schedule = schedule::validate(schedule)?;
        let handle = runtime_with_time_driver()?;
        let task_id = self.reserve()?;
        let cleanup = TaskCleanup {
            shared: Arc::downgrade(self),
            task_id,
        };

        // Tokio 可以同步析构拒绝接纳的任务；用户析构和完成清理均不能发生在注册表锁内。
        let task = handle.spawn(async move {
            let _cleanup = cleanup;
            schedule::run(schedule, callback).await;
        });
        self.publish(task_id, task.abort_handle());
        drop(task);
        Ok(task_id)
    }

    /// 在同一临界区拒绝关闭/满额状态，并为锁外提交保留唯一 ID 与容量。
    fn reserve(&self) -> Result<TaskId, SchedulerError> {
        let mut state = self.lock();
        if state.shutdown {
            return Err(SchedulerError::Shutdown);
        }
        if state.tasks.len() >= state.max_tasks {
            return Err(SchedulerError::TaskLimitExceeded);
        }
        let task_id = TaskId(state.next_task_id);
        state.next_task_id = state
            .next_task_id
            .checked_add(1)
            .ok_or(SchedulerError::TaskLimitExceeded)?;
        state.tasks.insert(task_id, None);
        Ok(task_id)
    }

    /// 仅填充仍存在的占位；完成、取消或关闭已移除占位时，在锁外取消新句柄。
    fn publish(&self, task_id: TaskId, abort: AbortHandle) {
        let mut state = self.lock();
        if let Some(slot) = state.tasks.get_mut(&task_id) {
            *slot = Some(abort);
        } else {
            // 不重新插入已取消/完成的任务，避免复活任务并泄漏容量。
            drop(state);
            abort.abort();
        }
    }

    /// 移除单个登记后在锁外请求取消；尚未发布的占位由 publish 补发取消。
    pub(crate) fn cancel(&self, task_id: TaskId) -> bool {
        let removed = self.lock().tasks.remove(&task_id);
        match removed {
            Some(abort) => {
                if let Some(abort) = abort {
                    abort.abort();
                }
                true
            }
            None => false,
        }
    }

    /// 原子关闭并取出全部登记，在锁外取消任务与释放句柄。
    pub(crate) fn shutdown(&self) {
        let tasks = {
            let mut state = self.lock();
            state.shutdown = true;
            std::mem::take(&mut state.tasks)
        };
        // 预留项无需句柄；提交方稍后观察占位消失后会取消自己的任务。
        for abort in tasks.into_values().flatten() {
            abort.abort();
        }
    }

    /// 取得只保护内部登记元数据的锁，中毒后仍允许尽力关闭和清理。
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

/// future 完成、panic、abort 或 runtime 拒绝接纳时均移除自己的登记。
struct TaskCleanup {
    /// 不延长调度器生命周期；调度器已释放时无需再清理注册表。
    shared: Weak<Shared>,
    /// 本 future 独占的登记 ID，永不与后续任务复用。
    task_id: TaskId,
}

impl Drop for TaskCleanup {
    /// 清理占位或已发布句柄；发布方在占位消失时不会再恢复登记。
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            let removed = shared.lock().tasks.remove(&self.task_id);
            drop(removed);
        }
    }
}

/// 检查当前 context 和 timer driver；不创建 runtime 或修改全局 panic hook。
fn runtime_with_time_driver() -> Result<Handle, SchedulerError> {
    let handle = Handle::try_current().map_err(|_| SchedulerError::RuntimeRequired)?;
    let timer_available = {
        let _entered = handle.enter();
        catch_unwind(AssertUnwindSafe(|| time::sleep(Duration::ZERO))).is_ok()
    };
    if timer_available {
        Ok(handle)
    } else {
        Err(SchedulerError::RuntimeRequired)
    }
}

#[cfg(test)]
#[path = "task/tests.rs"]
mod tests;
