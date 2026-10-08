//! Server 构造和监听编排；状态迁移与 provider 后台任务由各自模块管理。

#[cfg(feature = "axum-governor")]
use super::middleware::GovernorCleanupGuard;
use super::shutdown::{self, ActiveGuard, Phase, Shared, StartGuard};
#[cfg(feature = "axum-tower-http")]
use super::AxumTimeoutStatus;
use super::{AxumConfig, AxumError, AxumServeOutcome, AxumShutdownHandle, AxumShutdownReason};
use axum::Router;
use std::{future::Future, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

/// 已收敛 state 的 Axum server builder；构造和 build 不访问网络。
/// # Examples
/// ```rust
/// # use axutils::axum::*;
/// # #[cfg(feature = "axum")] {
/// let _builder = AxumApp::new().into_server_builder();
/// # }
/// ```
pub struct AxumServerBuilder {
    /// 已注入应用状态、尚未开始监听的 Router；provider 仅在构造期间添加 layer。
    pub(super) router: Router,
    /// 供调用方查询的有限声明值，不隐式安装 middleware。
    config: AxumConfig,
    /// 路由收敛时延迟记录的错误，build 时返回。
    build_error: Option<AxumError>,
    /// 是否在最终最外层生成并传播内部 request ID。
    #[cfg(feature = "axum-tower-http")]
    pub(super) request_id_installed: bool,
    /// 最后一次设置的 service future 预算与超时状态码；None 表示未安装。
    #[cfg(feature = "axum-tower-http")]
    pub(super) timeout_layer: Option<(std::time::Duration, AxumTimeoutStatus)>,
    /// 是否在业务层外捕获 unwind 并返回脱敏 500。
    #[cfg(feature = "axum-tower-http")]
    pub(super) catch_panic_installed: bool,
    /// 是否在完成响应时记录脱敏 HTTP 事件。
    #[cfg(all(feature = "axum-tower-http", feature = "tracing"))]
    pub(super) http_trace_installed: bool,
    /// 已安装限流器的 stale-key 清理操作，只有 serve 期间会启动对应任务。
    #[cfg(feature = "axum-governor")]
    pub(super) governor_cleanup: Vec<Arc<dyn Fn() + Send + Sync>>,
}
impl AxumServerBuilder {
    /// 从收敛后的 Router 与可选错误创建 builder，暂不执行最终 layer 排序。
    pub(super) fn new_with_error(
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
    /// # #[cfg(feature = "axum")] {
    /// let _ = AxumApp::new()
    ///     .into_server_builder()
    ///     .config(AxumConfig::new());
    /// # }
    /// ```
    pub fn config(mut self, config: AxumConfig) -> Self {
        self.config = config;
        self
    }
    /// 构建单次运行服务；配置错误返回 AxumError，此操作不 bind 或访问网络。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// let _server = AxumApp::new().into_server_builder().build().unwrap();
    /// # }
    /// ```
    pub fn build(self) -> Result<AxumServer, AxumError> {
        // 在统一出口安装有顺序要求的 layer，确保 timeout/panic 响应也携带 request ID。
        #[cfg(feature = "axum-tower-http")]
        let self_ = self.finalize_tower_http();
        #[cfg(not(feature = "axum-tower-http"))]
        let self_ = self;
        // 路由构造错误不能发布为可运行实例；此时尚未 bind 或启动后台任务。
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
/// # #[cfg(feature = "axum")] {
/// let server = AxumApp::new().into_server_builder().build().unwrap();
/// let _clone = server.clone();
/// # }
/// ```
#[derive(Clone)]
pub struct AxumServer {
    /// 构建完成且不能再追加路由的服务模板；每次连接由 Axum 克隆所需状态。
    router: Router,
    /// 构建时的声明配置，不代表对应 middleware 一定已安装。
    config: AxumConfig,
    /// clone 共用的单次运行状态与关闭通知。
    shared: Arc<Shared>,
    /// 当前 server 安装的 Governor 清理操作，随运行守卫启动和停止。
    #[cfg(feature = "axum-governor")]
    governor_cleanup: Vec<Arc<dyn Fn() + Send + Sync>>,
}
impl AxumServer {
    /// 返回构建时的 immutable 配置。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// let server = AxumApp::new().into_server_builder().build().unwrap();
    /// let _ = server.config();
    /// # }
    /// ```
    pub fn config(&self) -> &AxumConfig {
        &self.config
    }
    /// 返回共享 shutdown handle；Ready 状态调用其 shutdown 会返回 NotRunning。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// let server = AxumApp::new().into_server_builder().build().unwrap();
    /// let _ = server.shutdown_handle();
    /// # }
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
    /// async fn example(server: AxumServer) -> Result<(), AxumError> {
    ///     let _ = server.serve_addr("127.0.0.1:0".parse().unwrap()).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn serve_addr(&self, addr: SocketAddr) -> Result<AxumServeOutcome, AxumError> {
        // 先取得唯一启动权；bind/local_addr 失败或取消时，由 StartGuard 恢复 Ready。
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let listener = TcpListener::bind(addr).await.map_err(AxumError::Io)?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(
            listener,
            local,
            shutdown::default_shutdown(self.shared.clone()),
        )
        .await
    }
    /// 使用已 bind listener 运行，默认等待程序化 handle 或 OS signal。
    /// # Examples
    /// ```rust,no_run
    /// # use tokio::net::TcpListener;
    /// # use axutils::axum::*;
    /// async fn example(server: AxumServer, listener: TcpListener) -> Result<(), AxumError> {
    ///     let _ = server.serve(listener).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn serve(&self, listener: TcpListener) -> Result<AxumServeOutcome, AxumError> {
        // listener 所有权随调用转入运行；查询失败仍释放本次预留的启动权。
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(
            listener,
            local,
            shutdown::default_shutdown(self.shared.clone()),
        )
        .await
    }
    /// 使用自定义原因 future 运行，适合宿主协调和测试；future 必须 Send + 'static。
    ///
    /// 关闭 future panic 时，在已有连接完成 drain 后返回 [`AxumError::BackgroundTask`]，
    /// 服务进入不可复用的 abandoned 状态；错误不包含 panic payload。
    /// # Examples
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// # use tokio::net::TcpListener;
    /// async fn example(server: AxumServer, listener: TcpListener) -> Result<(), AxumError> {
    ///     let _ = server
    ///         .serve_with_shutdown(listener, async {
    ///             AxumShutdownReason::Custom("host".into())
    ///         })
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn serve_with_shutdown<F>(
        &self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<AxumServeOutcome, AxumError>
    where
        F: Future<Output = AxumShutdownReason> + Send + 'static,
    {
        // 启动检查与默认入口一致，关闭 future 由协调器与程序化 handle 共同驱动。
        let mut start = StartGuard::reserve(self.shared.clone())?;
        let local = listener.local_addr().map_err(AxumError::Io)?;
        start.commit();
        self.run(
            listener,
            local,
            shutdown::coordinated_custom_shutdown(self.shared.clone(), shutdown),
        )
        .await
    }
    /// 执行唯一一次监听与 drain，成功时等待 provider 清理任务停止并返回首个关闭原因。
    async fn run<F>(
        &self,
        listener: TcpListener,
        local: SocketAddr,
        shutdown: F,
    ) -> Result<AxumServeOutcome, AxumError>
    where
        F: Future<Output = Result<AxumShutdownReason, AxumError>> + Send + 'static,
    {
        // listener 已可用，进入 Running 后立即安装守卫；后续取消或错误都会进入 Abandoned。
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
        // Axum 会在独立任务中等待该 future，因此结果通过共享槽传回完成路径。
        let graceful = async move {
            let result = shutdown.await;
            let mut phase = shared.phase.lock().expect("Axum phase mutex poisoned");
            // 程序化关闭可能先登记原因；宿主 future 或信号不能覆盖已有 Draining 原因。
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
        // Provider 后台清理只存在于运行期间；异常退出时守卫也会发出停止通知。
        #[cfg(feature = "axum-governor")]
        let cleanup = GovernorCleanupGuard::start(&self.governor_cleanup);
        // 给每个连接注入真实 peer 地址，供按 IP 限流等 layer 使用。
        axum::serve(
            listener,
            self.router
                .clone()
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(graceful)
        .await
        .map_err(AxumError::Io)?;
        // Drain 完成后先等待 provider 任务退出，再发布终态与调用结果。
        #[cfg(feature = "axum-governor")]
        cleanup.stop().await?;
        // 协调任务 panic 也会触发 Axum drain，但不会写入结果；不能将它误报为正常关闭。
        let result = reason
            .lock()
            .expect("shutdown result mutex poisoned")
            .take()
            .ok_or(AxumError::BackgroundTask)?;
        let reason = result?;
        {
            let mut p = self.shared.phase.lock().expect("Axum phase mutex poisoned");
            *p = Phase::Stopped;
        }
        active.complete = true;
        Ok(AxumServeOutcome::new(local, reason))
    }
}
