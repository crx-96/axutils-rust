use ::tokio::task::JoinHandle;
use std::fmt;

/// 接管单个 Tokio 任务句柄，并在释放时请求取消的轻量所有权守卫。
///
/// 需要 `tokio` feature；`T` 是任务结果类型，守卫不读取结果。已完成任务的结果由句柄
/// 持有，释放守卫时一并丢弃；任务尚未完成时，结果及任务资源的清理由 Tokio 后续执行。
/// 将守卫放入 [`std::sync::Arc`] 时，只有最后一个强引用释放才触发取消。该守卫自身
/// 不可克隆，不创建 runtime，也不改变 [`super::TokioConfig`] 或任务组的关闭行为。
///
/// `Drop` 调用 Tokio 的 `abort`，只发出取消请求，不等待任务实际退出。已发出的外部
/// 操作不会撤销，已经开始的 `spawn_blocking` closure 不能被强制停止；任务需再次被
/// runtime 调度才能处理取消。需要在释放守卫后观察任务完成时，可提前保留句柄的
/// [`JoinHandle::abort_handle`]；需要确认特定资源已释放时，应由任务或结果提供清理通知。
/// 需要读取任务结果时应直接管理 `JoinHandle`。
#[must_use = "必须持有守卫，释放它会请求取消任务"]
pub struct TokioTaskGuard<T = ()> {
    /// 接管的任务句柄；结果可以是任意类型，守卫释放时对其发出 abort。
    task: JoinHandle<T>,
}

impl<T> TokioTaskGuard<T> {
    /// 接管已创建任务的句柄，使取消责任随守卫生命周期转移。
    ///
    /// 不创建或推进任务；必须在期望任务存活的整个期间持有返回值。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axutils::{
    ///     tokio::{TokioConfig, TokioTaskGuard},
    ///     utils::TokioUtils,
    /// };
    ///
    /// TokioUtils::run(&TokioConfig::new(), async {
    ///     let task = TokioUtils::spawn(std::future::pending::<u32>()).unwrap();
    ///     let owner = TokioTaskGuard::new(task);
    ///     assert!(!owner.is_finished());
    ///     drop(owner); // 请求取消；此处不保证任务已经退出。
    /// }).unwrap();
    /// ```
    pub fn new(task: JoinHandle<T>) -> Self {
        Self { task }
    }

    /// 观察任务是否已退出；仅发出 abort 请求时仍可能返回 `false`。
    ///
    /// 这是 Tokio 句柄的完成状态快照，不等待退出，也不说明任务成功、失败或取消。
    /// 已完成任务的结果仍由句柄持有，因此返回 `true` 不表示结果中的资源已经释放。
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
}

impl<T> fmt::Debug for TokioTaskGuard<T> {
    /// 只展示任务是否退出，不要求或输出任务结果的调试信息。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokioTaskGuard")
            .field("is_finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

impl<T> Drop for TokioTaskGuard<T> {
    /// 请求取消任务而不等待；重复或已完成任务的 abort 沿用 Tokio 的无额外作用语义。
    fn drop(&mut self) {
        // 取消只影响仍可取消的任务 future，不撤销外部副作用或强制终止 blocking closure。
        self.task.abort();
    }
}
