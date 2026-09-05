//! HTTP 客户端外观与后端实例构造。

use std::fmt;
use std::sync::Mutex;

#[cfg(feature = "http-async")]
use reqwest::{redirect::Policy as RedirectPolicy, retry as reqwest_retry, Client as AsyncClient};
use ureq::Agent as SyncAgent;

#[cfg(feature = "http-async")]
use super::coalesce::AsyncState;
use super::coalesce::SyncState;
use super::{HttpConfig, HttpError};

/// HTTP 客户端。
///
/// 客户端持有独立的同步和异步连接池；同步入口使用 `ureq`，异步入口使用 `reqwest`。
/// 两者都关闭系统代理、自动重定向、自动压缩和隐式重试，并且不会把第三方错误文本
/// 直接暴露给调用方。
pub struct HttpClient {
    pub(super) config: HttpConfig,
    pub(super) sync_agent: SyncAgent,
    pub(super) sync_state: Mutex<SyncState>,
    #[cfg(feature = "http-async")]
    pub(super) async_client: AsyncClient,
    #[cfg(feature = "http-async")]
    pub(super) async_state: Mutex<AsyncState>,
}

impl HttpClient {
    /// 根据配置创建客户端。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpClient, HttpConfig};
    ///
    /// let client = HttpClient::new(HttpConfig::default()).unwrap();
    /// let _ = client;
    /// ```
    pub fn new(config: HttpConfig) -> Result<Self, HttpError> {
        let sync_agent = SyncAgent::config_builder()
            .http_status_as_error(false)
            .proxy(None)
            .max_redirects(0)
            .allow_non_standard_methods(true)
            .max_idle_connections_per_host(config.max_idle_connections_per_host())
            .max_idle_connections(config.max_idle_connections_per_host().saturating_mul(4))
            .max_idle_age(config.idle_connection_timeout())
            .timeout_global(Some(config.request_timeout()))
            .timeout_connect(Some(config.connect_timeout()))
            .accept_encoding("")
            .user_agent("")
            .build()
            .new_agent();

        #[cfg(feature = "http-async")]
        let async_client = AsyncClient::builder()
            .redirect(RedirectPolicy::none())
            .referer(false)
            .retry(reqwest_retry::never())
            .no_proxy()
            .no_gzip()
            .timeout(config.request_timeout())
            .connect_timeout(config.connect_timeout())
            .pool_idle_timeout(config.idle_connection_timeout())
            .pool_max_idle_per_host(config.max_idle_connections_per_host())
            .build()
            .map_err(|_| HttpError::ClientBuild)?;

        Ok(Self {
            config,
            sync_agent,
            sync_state: Mutex::new(SyncState::new()),
            #[cfg(feature = "http-async")]
            async_client,
            #[cfg(feature = "http-async")]
            async_state: Mutex::new(AsyncState::new()),
        })
    }

    /// 返回客户端配置。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpClient, HttpConfig};
    ///
    /// let client = HttpClient::new(HttpConfig::default()).unwrap();
    /// assert_eq!(client.config().request_timeout(), std::time::Duration::from_secs(30));
    /// ```
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }
}

impl fmt::Debug for HttpClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpClient")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::HttpClient;
    use crate::http::{
        policy, DeduplicationPolicy, HttpConfig, HttpHeaders, HttpMethod, HttpRequest, HttpResponse,
    };

    #[test]
    fn cross_origin_absolute_urls_drop_only_sensitive_default_headers() {
        let config = HttpConfig::builder()
            .base_url("https://api.example.com/v1/")
            .unwrap()
            .with_default_header("authorization", "Bearer default-secret")
            .unwrap()
            .with_default_header("x-client", "axutils")
            .unwrap()
            .build()
            .unwrap();
        let client = HttpClient::new(config).unwrap();

        let same_origin = client
            .prepare(HttpRequest::new(HttpMethod::Get, "/users").unwrap())
            .unwrap();
        assert_eq!(
            same_origin.headers.get("authorization"),
            Some(b"Bearer default-secret".as_slice())
        );

        let cross_origin = client
            .prepare(
                HttpRequest::new(HttpMethod::Get, "https://other.example/data")
                    .unwrap()
                    .with_header("authorization", "Bearer explicit-secret")
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            cross_origin.headers.get("authorization"),
            Some(b"Bearer explicit-secret".as_slice())
        );
        assert_eq!(
            cross_origin.headers.get("x-client"),
            Some(b"axutils".as_slice())
        );
    }

    #[test]
    fn completed_cache_rejects_request_cache_directives() {
        let policy =
            DeduplicationPolicy::with_completed_ttl(std::time::Duration::from_secs(1), 8, 4, 1024)
                .unwrap_or_else(|_| unreachable!());
        let config = HttpConfig::builder()
            .deduplication_policy(policy)
            .build()
            .unwrap_or_else(|_| unreachable!());
        let client = HttpClient::new(config).unwrap_or_else(|_| unreachable!());
        let request = HttpRequest::new(HttpMethod::Get, "https://example.com/resource")
            .unwrap_or_else(|_| unreachable!())
            .with_header("cache-control", "no-cache, no-store")
            .unwrap_or_else(|_| unreachable!());
        let prepared = client.prepare(request).unwrap_or_else(|_| unreachable!());
        let response = HttpResponse::new(200, HttpHeaders::new(), Vec::new(), 1);

        assert!(!policy::cache_eligible(&prepared, &response));
    }
}
