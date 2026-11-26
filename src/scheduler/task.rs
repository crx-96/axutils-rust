//! 有界任务登记、取消与完成清理；实际时间策略由 schedule 模块负责。

use super::{schedule, SchedulerConfig, SchedulerError, TaskSchedule};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    ops::RangeInclusive,
    sync::{Arc, Mutex, MutexGuard, Weak},
};
#[cfg(panic = "unwind")]
use std::{panic::catch_unwind, time::Duration};
#[cfg(panic = "unwind")]
use tokio::time;
use tokio::{
    runtime::{Handle, Id, Runtime},
    task::AbortHandle,
};

/// 每个 [`Scheduler`](super::Scheduler) 只领取一次实例标识，释放后也不复用。
static SCHEDULER_IDS: Mutex<RangeInclusive<u64>> = Mutex::new(1..=u64::MAX);

/// 仅在签发它的 [`Scheduler`](super::Scheduler) 中有效的任务标识。
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
pub struct TaskId {
    /// 签发此标识的调度器，用于区分不同实例的相同序号。
    scheduler_id: u64,
    /// 实例内单调递增的任务序号，溢出时拒绝新登记。
    sequence: u64,
}

/// 调度器所有登记操作共享的状态；不持有任务 callback。
pub(crate) struct Shared {
    /// 本实例的标识。实例标识耗尽时保留 None，注册任务时返回容量错误。
    scheduler_id: Option<u64>,
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
    /// 当前实例已通过 enable_time 构建且仍在借用作用域内的 runtime。
    timed_runtimes: HashSet<Id>,
}

impl Shared {
    /// 建立空注册表，不获取 runtime 或启动后台工作。
    pub(crate) fn new(config: SchedulerConfig) -> Self {
        // 仅构造实例时共享分配器，后续登记和取消只需锁住本实例。
        let scheduler_id = SCHEDULER_IDS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .next();
        Self {
            scheduler_id,
            state: Mutex::new(State {
                shutdown: false,
                max_tasks: config.max_tasks,
                next_task_id: 1,
                tasks: HashMap::new(),
                timed_runtimes: HashSet::new(),
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
        let handle = self.runtime_with_time_driver()?;
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
        let task_id = TaskId {
            scheduler_id: self.scheduler_id.ok_or(SchedulerError::TaskLimitExceeded)?,
            sequence: state.next_task_id,
        };
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

impl Shared {
    /// 在当前 runtime 上校验计时能力。abort 构建只接受宿主已显式启用的 driver。
    fn runtime_with_time_driver(&self) -> Result<Handle, SchedulerError> {
        let handle = Handle::try_current().map_err(|_| SchedulerError::RuntimeRequired)?;
        if self.lock().timed_runtimes.contains(&handle.id()) {
            return Ok(handle);
        }
        // Tokio 没有稳定的 driver 查询 API。仅在 panic 可展开时兼容外部构建的 runtime。
        #[cfg(panic = "unwind")]
        if catch_unwind(|| time::sleep(Duration::ZERO)).is_ok() {
            return Ok(handle);
        }
        Err(SchedulerError::RuntimeRequired)
    }

    /// 记录仍由调用作用域借用的 runtime。在 runtime 析构前撤销，避免 ID 复用导致误判。
    pub(super) fn with_enabled_time<T>(
        &self,
        runtime: &Runtime,
        run: impl FnOnce(&Runtime) -> T,
    ) -> T {
        let id = runtime.handle().id();
        self.lock().timed_runtimes.insert(id);
        let _registration = TimeRegistration { shared: self, id };
        run(runtime)
    }
}

/// 仅在显式启用 time 的宿主 runtime 存活期间保留其能力登记。
struct TimeRegistration<'a> {
    /// 接受本次能力登记的调度器状态。
    shared: &'a Shared,
    /// 由宿主作用域持有且仍存活的 runtime 标识。
    id: Id,
}

impl Drop for TimeRegistration<'_> {
    /// 正常退出或展开时都先移除能力登记，再由外层释放 runtime。
    fn drop(&mut self) {
        self.shared.lock().timed_runtimes.remove(&self.id);
    }
}

#[cfg(test)]
#[path = "task/tests.rs"]
mod tests;
