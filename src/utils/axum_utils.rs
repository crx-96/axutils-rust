use std::{future::Future, net::SocketAddr, sync::OnceLock};

use axum::Router;
use tokio::net::TcpListener;

use crate::axum::{
    AxumApp, AxumError, AxumServeOutcome, AxumServer, AxumShutdownHandle, AxumShutdownReason,
};

static SERVER: OnceLock<AxumServer> = OnceLock::new();

/// Axum 空对象工厂与进程内唯一默认 `AxumServer` 的静态入口。
///
/// `create_router`/`create_app` 不读取全局状态；server 初始化只成功一次，成功后不能 reset 或
/// replace，服务操作和 server clone 共享同一单次状态机。
pub struct AxumUtils;
impl AxumUtils {
    /// 创建一个不含路由、fallback 或已注入 state 的原生 `axum::Router<S>`。
    ///
    /// 本方法复用 [`AxumApp::create_router`]，不会读取或修改全局 server，也不会失败、bind、创建
    /// runtime 或访问网络。泛型 `S` 保留 Router 的 missing-state 类型；调用方负责原生 Router 的
    /// 路由、layer、state 与资源边界，并满足 Axum 对 state 的 clone、并发与 `'static` 约束。
    ///
    /// # Examples
    /// ```rust,no_run
    /// # #[cfg(all(feature="axum",feature="tokio"))] {
    /// let router: axum::Router<String> = axutils::AxumUtils::create_router();
    /// let _builder = axutils::AxumApp::from_router(router).with_state("axutils".to_owned());
    /// # }
    /// ```
    pub fn create_router<S>() -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        AxumApp::<S>::create_router()
    }

    /// 创建一个不含路由、fallback 或延迟 layer 的 [`AxumApp<()>`]。
    ///
    /// 本方法复用 [`AxumApp::new`]，不会读取或修改全局 server，也不会失败、bind、创建 runtime
    /// 或访问网络。返回的 app 与 `AxumApp::new()` 相同，可继续注册路由并转换为 server builder。
    ///
    /// # Examples
    /// ```rust,no_run
    /// # #[cfg(all(feature="axum",feature="tokio"))] {
    /// let app = axutils::AxumUtils::create_app();
    /// let _builder = app.into_server_builder();
    /// # }
    /// ```
    pub fn create_app() -> AxumApp<()> {
        AxumApp::new()
    }

    /// 初始化默认服务；并发调用只有一个成功，失败值不占用初始化机会。
    /// # Examples
    /// ```rust,no_run
    /// # #[cfg(all(feature="axum",feature="tokio"))] { let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=axutils::AxumUtils::init(server); }
    /// ```
    pub fn init(server: AxumServer) -> Result<(), AxumError> {
        SERVER
            .set(server)
            .map_err(|_| AxumError::AlreadyInitialized)
    }
    /// 返回是否已经初始化；服务停止或 abandoned 后仍为 true。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { let _=axutils::AxumUtils::is_initialized(); }
    /// ```
    pub fn is_initialized() -> bool {
        SERVER.get().is_some()
    }
    fn server() -> Result<&'static AxumServer, AxumError> {
        SERVER.get().ok_or(AxumError::NotInitialized)
    }
    /// 返回共享关闭句柄；未初始化返回 NotInitialized。
    /// # Examples
    /// ```rust,no_run
    /// # #[cfg(all(feature="axum",feature="tokio"))] { let _=axutils::AxumUtils::shutdown_handle(); }
    /// ```
    pub fn shutdown_handle() -> Result<AxumShutdownHandle, AxumError> {
        Ok(Self::server()?.shutdown_handle())
    }
    /// 请求 graceful shutdown 并保留首个原因；未初始化或状态不匹配返回错误。
    /// # Examples
    /// ```rust,no_run
    /// # #[cfg(all(feature="axum",feature="tokio"))] { let _=axutils::AxumUtils::shutdown(axutils::AxumShutdownReason::Programmatic); }
    /// ```
    pub fn shutdown(reason: AxumShutdownReason) -> Result<AxumShutdownReason, AxumError> {
        Self::shutdown_handle()?.shutdown(reason)
    }
    /// bind 地址并运行默认单次服务；使用调用方 Tokio runtime。
    /// # Examples
    /// ```rust,no_run
    /// # async fn example()->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
    /// ```
    pub async fn serve_addr(addr: SocketAddr) -> Result<AxumServeOutcome, AxumError> {
        Self::server()?.serve_addr(addr).await
    }
    /// 使用已 bind listener 运行默认单次服务。
    /// # Examples
    /// ```rust,no_run
    /// # async fn example(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve(listener).await?;Ok(())}
    /// ```
    pub async fn serve(listener: TcpListener) -> Result<AxumServeOutcome, AxumError> {
        Self::server()?.serve(listener).await
    }
    /// 使用自定义 Send + 'static shutdown future 运行默认服务。
    /// # Examples
    /// ```rust,no_run
    /// # async fn example(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;Ok(())}
    /// ```
    pub async fn serve_with_shutdown<F>(
        listener: TcpListener,
        shutdown: F,
    ) -> Result<AxumServeOutcome, AxumError>
    where
        F: Future<Output = AxumShutdownReason> + Send + 'static,
    {
        Self::server()?
            .serve_with_shutdown(listener, shutdown)
            .await
    }
}
