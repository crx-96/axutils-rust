//! 一次初始化的 HTTP 客户端进程级入口。

use super::{HttpClient, HttpConfig, HttpError};
#[cfg(feature = "tracing")]
use crate::telemetry::http as http_trace;
#[cfg(feature = "tracing")]
use std::time::Instant;
use std::{fmt, sync::OnceLock};
static HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();
/// HTTP 全局客户端入口。
pub struct HttpUtils;
impl HttpUtils {
    /// 初始化一次性的全局 HTTP 客户端。
    pub fn init(config: HttpConfig) -> Result<(), HttpError> {
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = match HttpClient::new(config) {
            Ok(client) => HTTP_CLIENT
                .set(client)
                .map_err(|_| HttpError::AlreadyInitialized),
            Err(error) => Err(error),
        };
        #[cfg(feature = "tracing")]
        http_trace::record_client_init(&result, started);
        result
    }
    /// 返回全局客户端是否已经初始化。
    pub fn is_initialized() -> bool {
        HTTP_CLIENT.get().is_some()
    }
    /// 返回一次初始化的 HTTP 客户端。
    pub fn client() -> Result<&'static HttpClient, HttpError> {
        HTTP_CLIENT.get().ok_or(HttpError::NotInitialized)
    }
}
impl fmt::Debug for HttpUtils {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpUtils")
            .field("initialized", &Self::is_initialized())
            .finish()
    }
}
