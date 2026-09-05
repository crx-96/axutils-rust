use super::shutdown::{Phase, Shared};
#[cfg(feature = "axum-tower-http")]
use super::AxumTimeoutStatus;
use super::{AxumConfig, AxumError, AxumServeOutcome, AxumShutdownHandle, AxumShutdownReason};
use axum::Router;
#[cfg(feature = "axum-governor")]
use futures_timer::Delay;
use std::{future::Future, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::signal;
#[cfg(feature = "axum-governor")]
use tokio::{
    sync::watch::{channel as watch_channel, Sender as WatchSender},
    task::JoinHandle,
};

/// 已收敛 state 的 Axum server builder；构造和 build 不访问网络。
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # use axutils::axum::*;
/// # #[cfg(feature="axum")] { let _builder=AxumApp::new().into_server_builder(); }
/// ```
pub struct AxumServerBuilder {
    pub(crate) router: Router,
    config: AxumConfig,
    build_error: Option<AxumError>,
    #[cfg(feature = "axum-tower-http")]
    pub(crate) request_id_installed: bool,
    #[cfg(feature = "axum-tower-http")]
    pub(crate) timeout_layer: Option<(std::time::Duration, AxumTimeoutStatus)>,
    #[cfg(feature = "axum-tower-http")]
    pub(crate) catch_panic_installed: bool,
    #[cfg(all(feature = "axum-tower-http", feature = "tracing"))]
    pub(crate) http_trace_installed: bool,
    #[cfg(feature = "axum-governor")]
    pub(crate) governor_cleanup: Vec<Arc<dyn Fn() + Send + Sync>>,
}
impl AxumServerBuilder {
    pub(crate) fn new_with_error(
        router: Router,
        config: AxumConfig,
        build_error: Option<AxumError>,
    ) -> Self {
        Self {
            router,
            config,
            build_error,
            #[cfg(feature = "axum-tower-http")]
            request_id_installed: false,
            #[cfg(feature = "axum-tower-http")]
            timeout_layer: None,
            #[cfg(feature = "axum-tower-http")]
            catch_panic_installed: false,
            #[cfg(all(feature = "axum-tower-http", feature = "tracing"))]
            http_trace_installed: false,
            #[cfg(feature = "axum-governor")]
            governor_cleanup: Vec::new(),
        }
    }
    /// 替换 immutable 服务边界配置；不会自动安装 provider middleware。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum")] { let _=AxumApp::new().into_server_builder().config(AxumConfig::new()); }
    /// ```
    pub fn config(mut self, config: AxumConfig) -> Self {
        self.config = config;
        self
    }
    /// 构建单次运行服务；配置错误返回 AxumError，此操作不 bind 或访问网络。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum")] { let _server=AxumApp::new().into_server_builder().build().unwrap(); }
    /// ```
    pub fn build(self) -> Result<AxumServer, AxumError> {
        #[cfg(feature = "axum-tower-http")]
        let self_ = self.finalize_tower_http();
        #[cfg(not(feature = "axum-tower-http"))]
        let self_ = self;
        if let Some(error) = self_.build_error {
            return Err(error);
        }
        Ok(AxumServer {
            router: self_.router,
            config: self_.config,
            shared: Arc::new(Shared::new()),
            #[cfg(feature = "axum-governor")]
            governor_cleanup: self_.governor_cleanup,
        })
    }
}
/// 可 clone 的单次运行 Axum HTTP/1 服务；clone 共享状态机和 limiter。
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # use axutils::axum::*;
/// # #[cfg(feature="axum")] { let server=AxumApp::new().into_server_builder().build().unwrap();let _clone=server.clone(); }
/// ```
#[derive(Clone)]
pub struct AxumServer {
    router: Router,
    config: AxumConfig,
    shared: Arc<Shared>,
    #[cfg(feature = "axum-governor")]
    governor_cleanup: Vec<Arc<dyn Fn() + Send + Sync>>,
}
impl AxumServer {
    /// 返回构建时的 immutable 配置。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum")] { let server=AxumApp::new().into_server_builder().build().unwrap();let _=server.config(); }
    /// ```
    pub fn config(&self) -> &AxumConfig {
        &self.config
    }
    /// 返回共享 shutdown handle；Ready 状态调用其 shutdown 会返回 NotRunning。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum")] { let server=AxumApp::new().into_server_builder().build().unwrap();let _=server.shutdown_handle(); }
    /// ```
    pub fn shutdown_handle(&self) -> AxumShutdownHandle {
        AxumShutdownHandle {
            shared: self.shared.clone(),
        }
    }
    /// bind 地址并运行，默认等待程序化 handle 或 OS signal；bind 失败回滚 Ready。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # async fn example(server:AxumServer)->Result<(),AxumError>{let _=server.serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
    /// ```
    pub async fn serve_addr(&self, addr: SocketAddr) -> Result<AxumServeOutcome, AxumError> {
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let listener = TcpListener::bind(addr).await.map_err(AxumError::Io)?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(listener, local, default_shutdown(self.shared.clone()))
            .await
    }
    /// 使用已 bind listener 运行，默认等待程序化 handle 或 OS signal。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::tokio::*;
    /// # use tokio::net::TcpListener;
    /// # use axutils::axum::*;
    /// # async fn example(server:AxumServer,listener:TcpListener)->Result<(),AxumError>{let _=server.serve(listener).await?;Ok(())}
    /// ```
    pub async fn serve(&self, listener: TcpListener) -> Result<AxumServeOutcome, AxumError> {
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(listener, local, default_shutdown(self.shared.clone()))
            .await
    }
    /// 使用自定义原因 future 运行，适合宿主协调和测试；future 必须 Send + 'static。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # use axutils::tokio::*;
    /// # use tokio::net::TcpListener;
    /// # use axutils::axum::*;
    /// # async fn example(server:AxumServer,listener:TcpListener)->Result<(),AxumError>{let _=server.serve_with_shutdown(listener,async{AxumShutdownReason::Custom("host".into())}).await?;Ok(())}
    /// ```
    pub async fn serve_with_shutdown<F>(
        &self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<AxumServeOutcome, AxumError>
    where
        F: Future<Output = AxumShutdownReason> + Send + 'static,
    {
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(
            listener,
            local,
            coordinated_custom_shutdown(self.shared.clone(), shutdown),
        )
        .await
    }
    async fn run<F>(
        &self,
        listener: TcpListener,
        local: SocketAddr,
        shutdown: F,
    ) -> Result<AxumServeOutcome, AxumError>
    where
        F: Future<Output = Result<AxumShutdownReason, AxumError>> + Send + 'static,
    {
        {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            *p = Phase::Running;
        }
        let mut active = ActiveGuard {
            shared: self.shared.clone(),
            complete: false,
        };
        let shared = self.shared.clone();
        let reason = Arc::new(std::sync::Mutex::new(None));
        let captured = reason.clone();
        let graceful = async move {
            let result = shutdown.await;
            let mut phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            let result = match (result, &*phase) {
                (Ok(reason), Phase::Running) => {
                    *phase = Phase::Draining(reason.clone());
                    Ok(reason)
                }
                (Ok(_), Phase::Draining(first)) => Ok(first.clone()),
                (other, _) => other,
            };
            if result.is_err() && matches!(*phase, Phase::Running) {
                *phase = Phase::Draining(AxumShutdownReason::Programmatic);
            }
            drop(phase);
            *captured.lock().expect("shutdown result mutex poisoned") = Some(result);
        };
        #[cfg(feature = "axum-governor")]
        let cleanup = GovernorCleanupGuard::start(&self.governor_cleanup);
        axum::serve(
            listener,
            self.router
                .clone()
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(graceful)
        .await
        .map_err(AxumError::Io)?;
        #[cfg(feature = "axum-governor")]
        cleanup.stop().await?;
        let result = reason
            .lock()
            .expect("shutdown result mutex poisoned")
            .take()
            .unwrap_or_else(|| Ok(AxumShutdownReason::Custom("serve-completed".into())));
        let reason = result?;
        {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            *p = Phase::Stopped;
        }
        active.complete = true;
        Ok(AxumServeOutcome::new(local, reason))
    }
}
struct StartGuard {
    shared: Arc<Shared>,
    committed: bool,
}
impl StartGuard {
    fn reserve(shared: Arc<Shared>) -> Result<Self, AxumError> {
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
    fn commit(&mut self) {
        self.committed = true
    }
}
impl Drop for StartGuard {
    fn drop(&mut self) {
        if !self.committed {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            if matches!(*p, Phase::Starting) {
                *p = Phase::Ready;
            }
        }
    }
}
struct ActiveGuard {
    shared: Arc<Shared>,
    complete: bool,
}
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        if !self.complete {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            *p = Phase::Abandoned;
            self.shared.notify.notify_waiters();
        }
    }
}
async fn coordinated_custom_shutdown<F>(
    shared: Arc<Shared>,
    shutdown: F,
) -> Result<AxumShutdownReason, AxumError>
where
    F: Future<Output = AxumShutdownReason>,
{
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

async fn default_shutdown(shared: Arc<Shared>) -> Result<AxumShutdownReason, AxumError> {
    tokio::select! {
        _ = shared.notify.notified() => {
            let phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            Ok(match &*phase { Phase::Draining(reason) => reason.clone(), _ => AxumShutdownReason::Programmatic })
        },
        result = wait_os_signal() => result,
    }
}
#[cfg(unix)]
async fn wait_os_signal() -> Result<AxumShutdownReason, AxumError> {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).map_err(AxumError::Signal)?;
    tokio::select! {
        result = signal::ctrl_c() => { result.map_err(AxumError::Signal)?; Ok(AxumShutdownReason::CtrlC) },
        _ = term.recv() => Ok(AxumShutdownReason::Sigterm),
    }
}
#[cfg(not(unix))]
async fn wait_os_signal() -> Result<AxumShutdownReason, AxumError> {
    signal::ctrl_c().await.map_err(AxumError::Signal)?;
    Ok(AxumShutdownReason::CtrlC)
}

#[cfg(feature = "axum-governor")]
struct GovernorCleanupGuard {
    stop: WatchSender<bool>,
    handles: Vec<JoinHandle<()>>,
}
#[cfg(feature = "axum-governor")]
impl GovernorCleanupGuard {
    fn start(jobs: &[Arc<dyn Fn() + Send + Sync>]) -> Self {
        let (stop, receiver) = watch_channel(false);
        let handles = jobs
            .iter()
            .cloned()
            .map(|job| {
                let mut receiver = receiver.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            changed = receiver.changed() => {
                                if changed.is_err() || *receiver.borrow() { break; }
                            }
                            _ = Delay::new(std::time::Duration::from_secs(60)) => job(),
                        }
                    }
                })
            })
            .collect();
        Self { stop, handles }
    }
    async fn stop(mut self) -> Result<(), AxumError> {
        let _ = self.stop.send(true);
        let mut failed = false;
        for handle in std::mem::take(&mut self.handles) {
            failed |= handle.await.is_err();
        }
        if failed {
            Err(AxumError::BackgroundTask)
        } else {
            Ok(())
        }
    }
}
#[cfg(feature = "axum-governor")]
impl Drop for GovernorCleanupGuard {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

#[cfg(feature = "axum-tower")]
impl AxumServerBuilder {
    /// 安装 Tower fail-fast 全局并发限制；范围 1..=65,536，满载立即返回脱敏 503。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # use axutils::axum::*;
    /// # #[cfg(feature="axum-tower")] { let _=AxumApp::new().into_server_builder().with_concurrency_limit(1).unwrap(); }
    /// ```
    pub fn with_concurrency_limit(mut self, max: usize) -> Result<Self, AxumError> {
        use axum::{error_handling::HandleErrorLayer, http::StatusCode, BoxError};
        use tower::{
            limit::ConcurrencyLimitLayer,
            load_shed::{error::Overloaded, LoadShedLayer},
            ServiceBuilder,
        };
        if !(1..=65_536).contains(&max) {
            return Err(AxumError::InvalidConfig {
                field: "max_concurrency",
            });
        }
        let stack = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|error: BoxError| async move {
                if error.is::<Overloaded>() {
                    (StatusCode::SERVICE_UNAVAILABLE, "service overloaded")
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, "service error")
                }
            }))
            .layer(LoadShedLayer::new())
            .layer(ConcurrencyLimitLayer::new(max));
        self.router = self.router.layer(stack);
        Ok(self)
    }
}
