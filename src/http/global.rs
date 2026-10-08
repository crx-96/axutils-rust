//! 一次初始化的 HTTP 客户端进程级入口。

use super::{HttpClient, HttpConfig, HttpError};
#[cfg(feature = "tracing")]
use crate::telemetry::http as http_trace;
#[cfg(feature = "tracing")]
use std::time::Instant;
use std::{fmt, sync::OnceLock};
/// 只发布一次的默认 HTTP 客户端，成功初始化后不允许覆盖。
static HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();
/// HTTP 全局客户端入口。
pub struct HttpUtils;
impl HttpUtils {
    /// 初始化一次性的全局 HTTP 客户端。
    pub fn init(config: HttpConfig) -> Result<(), HttpError> {
        // 构造不访问网络；只在成功后竞争全局槽位，失败不消耗初始化机会。
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
    /// 只展示全局入口是否已初始化，不展开客户端状态或配置。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpUtils")
            .field("initialized", &Self::is_initialized())
            .finish()
    }
}
