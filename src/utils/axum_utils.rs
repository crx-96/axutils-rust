use std::{future::Future, net::SocketAddr, sync::OnceLock};

use tokio::net::TcpListener;

use crate::axum::{
    AxumError, AxumServeOutcome, AxumServer, AxumShutdownHandle, AxumShutdownReason,
};

static SERVER: OnceLock<AxumServer> = OnceLock::new();

/// 进程内唯一默认 `AxumServer` 的静态入口。
///
/// 初始化只成功一次；成功后不能 reset 或 replace，所有方法和 server clone 共享同一单次状态机。
pub struct AxumUtils;
impl AxumUtils {
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
