use super::TokioError;
use ::tokio::{runtime::Handle, task::JoinHandle};
use ::tokio_util::{sync::CancellationToken, task::TaskTracker};
use futures_timer::Delay;
use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

/// 共享 TaskTracker、协作式 CancellationToken 与线性化关闭门闩的任务组。
///
/// clone 共享同一组；Drop 不 abort 任务，blocking closure 开始后不能强制停止。
#[derive(Clone, Debug)]
pub struct TokioTaskGroup {
    inner: Arc<Inner>,
}
#[derive(Debug)]
struct Inner {
    tracker: TaskTracker,
    cancel: CancellationToken,
    gate: Mutex<bool>,
}
impl Default for TokioTaskGroup {
    fn default() -> Self {
        Self::new()
    }
}
impl TokioTaskGroup {
    /// 创建打开的任务组，不创建 runtime 或任务。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { assert!(!TokioTaskGroup::new().is_closed()); }
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
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { assert!(!TokioTaskGroup::new().cancellation_token().is_cancelled()); }
    /// ```
    pub fn cancellation_token(&self) -> CancellationToken {
        self.inner.cancel.clone()
    }

    /// 返回关闭门闩状态。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { let g=TokioTaskGroup::new();g.close();assert!(g.is_closed()); }
    /// ```
    pub fn is_closed(&self) -> bool {
        *self.inner.gate.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 返回 tracker 当前任务数量；这是观测值，不是新的同步保证。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { assert_eq!(TokioTaskGroup::new().remaining_tasks(),0); }
    /// ```
    pub fn remaining_tasks(&self) -> usize {
        self.inner.tracker.len()
    }

    /// 在线性化门闩下登记异步任务；关闭后返回 TaskGroupClosed，缺少 runtime 返回 RuntimeRequired。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
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
        let g = self.inner.gate.lock().unwrap_or_else(|e| e.into_inner());
        if *g {
            return Err(TokioError::TaskGroupClosed);
        }
        Handle::try_current().map_err(|_| TokioError::RuntimeRequired)?;
        let h = self.inner.tracker.spawn(f);
        drop(g);
        Ok(h)
    }

    /// 在线性化门闩下登记 blocking closure；开始后不能强停。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
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
        let g = self.inner.gate.lock().unwrap_or_else(|e| e.into_inner());
        if *g {
            return Err(TokioError::TaskGroupClosed);
        }
        Handle::try_current().map_err(|_| TokioError::RuntimeRequired)?;
        let h = self.inner.tracker.spawn_blocking(f);
        drop(g);
        Ok(h)
    }

    /// 关闭登记门闩；返回后开始的 spawn 稳定失败，已有任务不被取消。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { let g=TokioTaskGroup::new();g.close();assert!(g.is_closed()); }
    /// ```
    pub fn close(&self) {
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
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="task-group")] { let g=TokioTaskGroup::new();let t=g.cancellation_token();g.cancel();assert!(t.is_cancelled()); }
    /// ```
    pub fn cancel(&self) {
        self.inner.cancel.cancel();
    }

    /// close、cancel 并等待任务清空；grace 必须 <=300 秒，超时返回剩余数量。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # use axutils::utils::TokioUtils;
    /// # #[cfg(feature="task-group")] {
    /// TokioUtils::run(&TokioConfig::new(),async{let g=TokioTaskGroup::new();g.shutdown(std::time::Duration::from_secs(1)).await}).unwrap().unwrap();
    /// # }
    /// ```
    pub async fn shutdown(&self, grace: Duration) -> Result<(), TokioError> {
        if grace > Duration::from_secs(300) {
            return Err(TokioError::InvalidConfig {
                field: "task_group_grace",
            });
        }
        self.close();
        self.cancel();
        let mut wait = std::pin::pin!(self.inner.tracker.wait());
        let mut delay = std::pin::pin!(Delay::new(grace));
        let completed = std::future::poll_fn(|cx| {
            if wait.as_mut().poll(cx).is_ready() {
                return std::task::Poll::Ready(true);
            }
            if delay.as_mut().poll(cx).is_ready() {
                return std::task::Poll::Ready(false);
            }
            std::task::Poll::Pending
        })
        .await;
        if completed {
            Ok(())
        } else {
            Err(TokioError::TaskGroupShutdownTimeout {
                remaining_tasks: self.remaining_tasks(),
            })
        }
    }
}
