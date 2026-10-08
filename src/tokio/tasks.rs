use super::TokioError;
use ::tokio::{runtime::Handle, task::JoinHandle};
use ::tokio_util::{
    sync::CancellationToken,
    task::{task_tracker::TaskTrackerToken, TaskTracker},
};
use futures_timer::Delay;
use std::{
    future::{poll_fn, Future},
    pin::pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};

/// 共享 TaskTracker、协作式 CancellationToken 与线性化关闭门闩的任务组。
///
/// clone 共享同一组；Drop 不 abort 任务，blocking closure 开始后不能强制停止。
#[derive(Clone, Debug)]
pub struct TokioTaskGroup {
    /// 所有克隆共享任务计数、取消通知与登记门闩。
    inner: Arc<Inner>,
}
/// 任务组唯一的共享状态；同步门闩只保护是否接纳新任务。
#[derive(Debug)]
struct Inner {
    /// 统计已接纳且尚未完全析构的任务，供关闭流程等待。
    tracker: TaskTracker,
    /// 调用方任务主动观察的协作式取消通知。
    cancel: CancellationToken,
    /// `true` 表示永久拒绝新任务；登记计数必须在同一临界区完成。
    gate: Mutex<bool>,
}

/// 在取消路径中保证任务仍拥有的捕获值先析构、跟踪凭证后析构的阻塞任务所有者。
/// 闭包返回的结果已转移给 JoinHandle，不受该计数约束。
struct BlockingTask<F> {
    /// 用户闭包；声明在凭证之前，使未开始执行的取消路径也先释放用户资源。
    callback: F,
    /// 直到闭包返回或完成析构才释放的任务计数。
    token: TaskTrackerToken,
}

impl<F> BlockingTask<F> {
    /// 消费闭包并在执行结束后归还计数；panic unwind 同样保留正确的析构顺序。
    fn run<T>(self) -> T
    where
        F: FnOnce() -> T,
    {
        let result = (self.callback)();
        drop(self.token);
        result
    }
}

impl Default for TokioTaskGroup {
    /// 默认任务组为空且开放，不隐式获取 runtime。
    fn default() -> Self {
        Self::new()
    }
}
impl TokioTaskGroup {
    /// 创建打开的任务组，不创建 runtime 或任务。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// assert!(!TokioTaskGroup::new().is_closed());
    /// # }
    /// ```
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                tracker: TaskTracker::new(),
                cancel: CancellationToken::new(),
                gate: Mutex::new(false),
            }),
        }
    }

    /// 返回共享协作式取消 token；调用方任务必须主动观察它。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// assert!(!TokioTaskGroup::new().cancellation_token().is_cancelled());
    /// # }
    /// ```
    pub fn cancellation_token(&self) -> CancellationToken {
        self.inner.cancel.clone()
    }

    /// 返回关闭门闩状态。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// let g = TokioTaskGroup::new();
    /// g.close();
    /// assert!(g.is_closed());
    /// # }
    /// ```
    pub fn is_closed(&self) -> bool {
        *self.inner.gate.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 返回 tracker 当前任务数量；这是观测值，不是新的同步保证。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// assert_eq!(TokioTaskGroup::new().remaining_tasks(), 0);
    /// # }
    /// ```
    pub fn remaining_tasks(&self) -> usize {
        self.inner.tracker.len()
    }

    /// 在线性化门闩下登记异步任务；关闭后返回 TaskGroupClosed，缺少 runtime 返回 RuntimeRequired。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::utils::TokioUtils;
    /// # #[cfg(feature="task-group")] {
    /// let result=TokioUtils::run(&TokioConfig::new(),async{let g=TokioTaskGroup::new();g.spawn(async{1}).unwrap().await.unwrap()}).unwrap();assert_eq!(result,1);
    /// # }
    /// ```
    pub fn spawn<F>(&self, f: F) -> Result<JoinHandle<F::Output>, TokioError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        // 登记与 close 共用门闩；先确认 runtime，再在锁内增加跟踪计数。
        let g = self.inner.gate.lock().unwrap_or_else(|e| e.into_inner());
        if *g {
            return Err(TokioError::TaskGroupClosed);
        }
        let handle = Handle::try_current().map_err(|_| TokioError::RuntimeRequired)?;
        let tracked = self.inner.tracker.track_future(f);
        drop(g);
        // 已关闭的 runtime 可以在 spawn 中同步析构 future；用户 Drop 可能重入任务组。
        // TrackedFuture 先释放用户 future 再归还 token，使等待也涵盖该析构阶段。
        Ok(handle.spawn(tracked))
    }

    /// 在线性化门闩下登记 blocking closure；开始后不能强停。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::utils::TokioUtils;
    /// # #[cfg(feature="task-group")] {
    /// let result=TokioUtils::run(&TokioConfig::new(),async{let g=TokioTaskGroup::new();g.spawn_blocking(||2).unwrap().await.unwrap()}).unwrap();assert_eq!(result,2);
    /// # }
    /// ```
    pub fn spawn_blocking<F, T>(&self, f: F) -> Result<JoinHandle<T>, TokioError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        // 门闩内只校验与预留计数，不执行可能同步析构用户捕获值的提交操作。
        let g = self.inner.gate.lock().unwrap_or_else(|e| e.into_inner());
        if *g {
            return Err(TokioError::TaskGroupClosed);
        }
        let handle = Handle::try_current().map_err(|_| TokioError::RuntimeRequired)?;
        let task = BlockingTask {
            callback: f,
            token: self.inner.tracker.token(),
        };
        drop(g);
        // 通过方法捕获完整所有者，避免闭包按字段捕获后改变 callback/token 析构顺序。
        Ok(handle.spawn_blocking(move || task.run()))
    }

    /// 关闭登记门闩；返回后开始的 spawn 稳定失败，已有任务不被取消。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// let g = TokioTaskGroup::new();
    /// g.close();
    /// assert!(g.is_closed());
    /// # }
    /// ```
    pub fn close(&self) {
        // 同一门闩将拒绝登记与关闭 tracker 线性化；此前接纳的任务已持有计数。
        let mut g = self.inner.gate.lock().unwrap_or_else(|e| e.into_inner());
        if !*g {
            *g = true;
            self.inner.tracker.close();
        }
    }

    /// 广播协作式取消，不关闭登记门闩也不 abort 任务。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # #[cfg(feature = "task-group")] {
    /// let g = TokioTaskGroup::new();
    /// let t = g.cancellation_token();
    /// g.cancel();
    /// assert!(t.is_cancelled());
    /// # }
    /// ```
    pub fn cancel(&self) {
        self.inner.cancel.cancel();
    }

    /// close、cancel 并等待任务清空；grace 必须 <=300 秒，超时返回剩余数量。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::utils::TokioUtils;
    /// # #[cfg(feature="task-group")] {
    /// TokioUtils::run(&TokioConfig::new(),async{let g=TokioTaskGroup::new();g.shutdown(std::time::Duration::from_secs(1)).await}).unwrap().unwrap();
    /// # }
    /// ```
    pub async fn shutdown(&self, grace: Duration) -> Result<(), TokioError> {
        // 先验证预算，避免无效调用改变任务组的开放或取消状态。
        if grace > Duration::from_secs(300) {
            return Err(TokioError::InvalidConfig {
                field: "task_group_grace",
            });
        }
        self.close();
        self.cancel();
        // 使用独立计时 future，允许调用方的 Tokio runtime 未启用 time driver。
        let mut wait = pin!(self.inner.tracker.wait());
        let mut delay = pin!(Delay::new(grace));
        let completed = poll_fn(|cx| {
            if wait.as_mut().poll(cx).is_ready() {
                return Poll::Ready(true);
            }
            if delay.as_mut().poll(cx).is_ready() {
                return Poll::Ready(false);
            }
            Poll::Pending
        })
        .await;
        // 超时只报告观测数量；协作式取消不能强制停止任务或已开始的阻塞闭包。
        if completed {
            Ok(())
        } else {
            Err(TokioError::TaskGroupShutdownTimeout {
                remaining_tasks: self.remaining_tasks(),
            })
        }
    }
}
