//! 单次服务生命周期、关闭原因和跨任务通知。

use std::{
    fmt,
    future::Future,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use super::AxumError;
use tokio::{signal, sync::Notify};

/// 首个触发 graceful shutdown 的可扩展原因。
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # #[cfg(feature = "axum")] {
/// assert_eq!(AxumShutdownReason::Programmatic.to_string(), "programmatic");
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AxumShutdownReason {
    /// 由 AxumShutdownHandle 触发。
    Programmatic,
    /// 跨平台 Ctrl+C。
    CtrlC,
    /// Unix SIGTERM。
    Sigterm,
    /// 宿主或测试提供的非敏感标签；Display 会包含该值，不能放 secret。
    Custom(String),
}
impl fmt::Display for AxumShutdownReason {
    /// 输出固定原因标签；自定义原因按调用方提供的非敏感文本展示。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Programmatic => f.write_str("programmatic"),
            Self::CtrlC => f.write_str("ctrl-c"),
            Self::Sigterm => f.write_str("sigterm"),
            Self::Custom(v) => write!(f, "custom:{v}"),
        }
    }
}
/// 一次 serve 完成后的地址和关闭原因。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxumServeOutcome {
    /// 实际绑定的地址，包括由操作系统分配的临时端口。
    local_addr: SocketAddr,
    /// 首次进入 draining 时记录的关闭原因。
    reason: AxumShutdownReason,
}
impl AxumServeOutcome {
    /// 在服务与受管后台任务完成关闭后创建执行结果。
    pub(crate) fn new(local_addr: SocketAddr, reason: AxumShutdownReason) -> Self {
        Self { local_addr, reason }
    }
    /// 返回实际监听地址，包括端口 0 bind 后的真实端口。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # use tokio::net::TcpListener;
    /// # async fn example(server: AxumServer, listener: TcpListener)->Result<(),AxumError>{
    /// let outcome=server.serve_with_shutdown(listener,async{AxumShutdownReason::Programmatic}).await?;let _=outcome.local_addr();
    /// # Ok(()) }
    /// ```
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    /// 返回首个关闭原因。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # use tokio::net::TcpListener;
    /// # async fn example(server: AxumServer, listener: TcpListener)->Result<(),AxumError>{
    /// let outcome=server.serve_with_shutdown(listener,async{AxumShutdownReason::Programmatic}).await?;assert_eq!(outcome.reason(),&AxumShutdownReason::Programmatic);
    /// # Ok(()) }
    /// ```
    pub fn reason(&self) -> &AxumShutdownReason {
        &self.reason
    }
}

/// 所有 server clone 共享的单次运行状态；锁保护检查和状态迁移的原子性。
#[derive(Clone, Debug)]
pub(super) enum Phase {
    /// 尚未开始，可申请启动。
    Ready,
    /// 已取得启动权，正在 bind 或检查 listener；失败可回滚。
    Starting,
    /// 正在服务，可以接受首次关闭请求。
    Running,
    /// 已请求停止接收新请求并排空已有请求；保存首个原因。
    Draining(AxumShutdownReason),
    /// 已完成正常关闭，不能重启。
    Stopped,
    /// 运行 future 被取消、panic 或异常离开，不能复用。
    Abandoned,
}
/// 服务身份及程序化关闭通知；不存在每个 clone 独立的启动机会。
pub(super) struct Shared {
    /// 生命周期状态；临界区仅执行同步检查和迁移，不跨 await。
    pub(super) phase: Mutex<Phase>,
    /// 唤醒唯一的关闭协调 future；notify_one 允许保存提前到达的通知。
    pub(super) notify: Notify,
}
impl Shared {
    /// 创建尚未启动的独立服务身份。
    pub(super) fn new() -> Self {
        Self {
            phase: Mutex::new(Phase::Ready),
            notify: Notify::new(),
        }
    }
}

/// 可 clone 的程序化关闭句柄；clone 共享同一服务身份。
///
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # #[cfg(feature = "axum")] {
/// let server = AxumApp::new().into_server_builder().build().unwrap();
/// let _handle = server.shutdown_handle();
/// # }
/// ```
#[derive(Clone)]
pub struct AxumShutdownHandle {
    /// 与 server 共用的状态和通知，不拥有额外监听器或启动权。
    pub(super) shared: Arc<Shared>,
}
impl AxumShutdownHandle {
    /// 请求 graceful shutdown。首次调用保存原因；draining 期间重复调用返回原原因。
    ///
    /// Ready/Starting 返回 NotRunning，Stopped/Abandoned 返回对应终态错误。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// let server = AxumApp::new().into_server_builder().build().unwrap();
    /// assert!(matches!(
    ///     server
    ///         .shutdown_handle()
    ///         .shutdown(AxumShutdownReason::Programmatic),
    ///     Err(AxumError::NotRunning)
    /// ));
    /// # }
    /// ```
    pub fn shutdown(&self, reason: AxumShutdownReason) -> Result<AxumShutdownReason, AxumError> {
        // 在同一临界区确定首个关闭原因，重复调用只读取它，不能覆盖。
        let mut phase = self.shared.phase.lock().expect("Axum phase mutex poisoned");
        match &*phase {
            Phase::Running => {
                *phase = Phase::Draining(reason.clone());
                // 关闭协调任务可能尚未首次 poll；保存许可以避免丢失提前通知。
                self.shared.notify.notify_one();
                Ok(reason)
            }
            Phase::Draining(first) => Ok(first.clone()),
            Phase::Stopped => Err(AxumError::AlreadyStopped),
            Phase::Abandoned => Err(AxumError::Abandoned),
            Phase::Ready | Phase::Starting => Err(AxumError::NotRunning),
        }
    }
}

/// 预留启动权的守卫；bind 失败或启动阶段取消时自动恢复 Ready。
pub(super) struct StartGuard {
    /// 预留所属的服务身份。
    shared: Arc<Shared>,
    /// 是否已将清理责任交给运行阶段；为真时不回滚状态。
    committed: bool,
}
impl StartGuard {
    /// 仅允许 Ready 进入 Starting；其他状态返回对应生命周期错误。
    pub(super) fn reserve(shared: Arc<Shared>) -> Result<Self, AxumError> {
        // 状态检查和占位在同一把锁下完成，阻止 server clone 并发启动。
        let mut p = shared.phase.lock().expect("Axum phase mutex poisoned");
        match *p {
            Phase::Ready => {
                *p = Phase::Starting;
                drop(p);
                Ok(Self {
                    shared,
                    committed: false,
                })
            }
            Phase::Starting | Phase::Running | Phase::Draining(_) => Err(AxumError::AlreadyRunning),
            Phase::Stopped => Err(AxumError::AlreadyStopped),
            Phase::Abandoned => Err(AxumError::Abandoned),
        }
    }
    /// listener 已准备好，由紧接着创建的运行守卫接管异常退出处理。
    pub(super) fn commit(&mut self) {
        self.committed = true
    }
}
impl Drop for StartGuard {
    /// 只回滚仍属于本次未提交启动的 Starting 状态。
    fn drop(&mut self) {
        if !self.committed {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            if matches!(*p, Phase::Starting) {
                *p = Phase::Ready;
            }
        }
    }
}
/// 运行期间的异常退出守卫；未完成时标记 Abandoned 并释放关闭协调任务。
pub(super) struct ActiveGuard {
    /// 当前运行服务的共享状态。
    pub(super) shared: Arc<Shared>,
    /// 是否已完成全部正常关闭步骤；仅成功返回前设为真。
    pub(super) complete: bool,
}
impl Drop for ActiveGuard {
    /// 取消或错误返回不能被误报为 Stopped，同时确保提前通知不会丢失。
    fn drop(&mut self) {
        if !self.complete {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            *p = Phase::Abandoned;
            // Axum 将 graceful future 放入独立 task；它可能尚未首次 poll，必须保存通知许可。
            self.shared.notify.notify_one();
        }
    }
}
/// 等待宿主 future 或程序化通知，并优先保留已登记的首个关闭原因。
pub(super) async fn coordinated_custom_shutdown<F>(
    shared: Arc<Shared>,
    shutdown: F,
) -> Result<AxumShutdownReason, AxumError>
where
    F: Future<Output = AxumShutdownReason>,
{
    // 获胜分支返回后丢弃另一个 future；这也释放宿主关闭 future 捕获的资源。
    tokio::select! {
        _ = shared.notify.notified() => {
            let phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            Ok(match &*phase { Phase::Draining(reason) => reason.clone(), _ => AxumShutdownReason::Programmatic })
        },
        reason = shutdown => {
            let phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            Ok(match &*phase { Phase::Draining(first) => first.clone(), _ => reason })
        },
    }
}

/// 等待程序化通知或平台信号；注册失败交由运行路径作为错误处理。
pub(super) async fn default_shutdown(shared: Arc<Shared>) -> Result<AxumShutdownReason, AxumError> {
    // 信号监听只在服务运行时启动，库初始化与 builder 构造不修改信号状态。
    tokio::select! {
        _ = shared.notify.notified() => {
            let phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            Ok(match &*phase { Phase::Draining(reason) => reason.clone(), _ => AxumShutdownReason::Programmatic })
        },
        result = wait_os_signal() => result,
    }
}
#[cfg(unix)]
/// Unix 上同时监听 Ctrl+C 和 SIGTERM，返回先到达的信号类别。
async fn wait_os_signal() -> Result<AxumShutdownReason, AxumError> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).map_err(AxumError::Signal)?;
    tokio::select! {
        result = signal::ctrl_c() => { result.map_err(AxumError::Signal)?; Ok(AxumShutdownReason::CtrlC) },
        _ = term.recv() => Ok(AxumShutdownReason::Sigterm),
    }
}
#[cfg(not(unix))]
/// 非 Unix 平台使用 Tokio 的 Ctrl+C 监听，保持跨平台关闭入口一致。
async fn wait_os_signal() -> Result<AxumShutdownReason, AxumError> {
    signal::ctrl_c().await.map_err(AxumError::Signal)?;
    Ok(AxumShutdownReason::CtrlC)
}

#[cfg(test)]
mod tests {
    use super::{coordinated_custom_shutdown, ActiveGuard};
    use crate::axum::{
        shutdown::{Phase, Shared},
        AxumShutdownReason,
    };
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::time;

    struct PendingSignal(Arc<AtomicBool>);

    impl Future for PendingSignal {
        type Output = AxumShutdownReason;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    impl Drop for PendingSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn regression_cancellation_before_shutdown_first_poll_releases_signal() {
        let shared = Arc::new(Shared::new());
        *shared.phase.lock().unwrap() = Phase::Running;
        let dropped = Arc::new(AtomicBool::new(false));
        let shutdown = coordinated_custom_shutdown(shared.clone(), PendingSignal(dropped.clone()));
        drop(ActiveGuard {
            shared: shared.clone(),
            complete: false,
        });

        let reason = time::timeout(Duration::from_millis(100), shutdown)
            .await
            .expect("early cancellation must remain observable before the first signal poll")
            .unwrap();
        assert_eq!(reason, AxumShutdownReason::Programmatic);
        assert!(dropped.load(Ordering::SeqCst));
        assert!(matches!(*shared.phase.lock().unwrap(), Phase::Abandoned));
    }
}
