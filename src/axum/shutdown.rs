use std::{
    fmt,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use super::AxumError;
use tokio::sync::Notify;

/// 首个触发 graceful shutdown 的可扩展原因。
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # use axutils::axum::*;
/// # #[cfg(feature="axum")] { assert_eq!(AxumShutdownReason::Programmatic.to_string(),"programmatic"); }
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
    local_addr: SocketAddr,
    reason: AxumShutdownReason,
}
impl AxumServeOutcome {
    pub(crate) fn new(local_addr: SocketAddr, reason: AxumShutdownReason) -> Self {
        Self { local_addr, reason }
    }
    /// 返回实际监听地址，包括端口 0 bind 后的真实端口。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # use axutils::tokio::*;
    /// # use tokio::net::TcpListener;
    /// # use axutils::axum::*;
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
    /// # use axutils::tokio::*;
    /// # use tokio::net::TcpListener;
    /// # use axutils::axum::*;
    /// # async fn example(server: AxumServer, listener: TcpListener)->Result<(),AxumError>{
    /// let outcome=server.serve_with_shutdown(listener,async{AxumShutdownReason::Programmatic}).await?;assert_eq!(outcome.reason(),&AxumShutdownReason::Programmatic);
    /// # Ok(()) }
    /// ```
    pub fn reason(&self) -> &AxumShutdownReason {
        &self.reason
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Phase {
    Ready,
    Starting,
    Running,
    Draining(AxumShutdownReason),
    Stopped,
    Abandoned,
}
pub(crate) struct Shared {
    pub phase: Mutex<Phase>,
    pub notify: Notify,
}
impl Shared {
    pub fn new() -> Self {
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
/// # use axutils::axum::*;
/// # #[cfg(feature="axum")] { let server=AxumApp::new().into_server_builder().build().unwrap();let _handle=server.shutdown_handle(); }
/// ```
#[derive(Clone)]
pub struct AxumShutdownHandle {
    pub(crate) shared: Arc<Shared>,
}
impl AxumShutdownHandle {
    /// 请求 graceful shutdown。首次调用保存原因；draining 期间重复调用返回原原因。
    ///
    /// Ready/Starting 返回 NotRunning，Stopped/Abandoned 返回对应终态错误。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum")] { let server=AxumApp::new().into_server_builder().build().unwrap();assert!(matches!(server.shutdown_handle().shutdown(AxumShutdownReason::Programmatic),Err(AxumError::NotRunning))); }
    /// ```
    pub fn shutdown(&self, reason: AxumShutdownReason) -> Result<AxumShutdownReason, AxumError> {
        let mut phase = self.shared.phase.lock().expect("Axum phase mutex poisoned");
        match &*phase {
            Phase::Running => {
                *phase = Phase::Draining(reason.clone());
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
