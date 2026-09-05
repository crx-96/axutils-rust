//! 请求准备与共享错误映射。

use std::time::Duration;

use url::Url;

use super::config::DeduplicationPolicy;
use super::headers::HttpHeaders;
use super::request::{HttpMethod, HttpRequest};
use super::retry::RetryPolicy;
use super::{HttpClient, HttpError, HttpTransportErrorKind};

/// 已完成本地校验、可交给传输层执行的请求。
pub(super) struct PreparedRequest {
    pub(super) url: Url,
    pub(super) method: HttpMethod,
    pub(super) headers: HttpHeaders,
    pub(super) body: Option<Vec<u8>>,
    pub(super) timeout: Duration,
    pub(super) retry_policy: RetryPolicy,
    pub(super) deduplication_policy: DeduplicationPolicy,
    pub(super) deduplication_opt_in: bool,
}

/// 单次传输尝试的本地或传输失败。
pub(super) enum AttemptError {
    Transport(HttpTransportErrorKind),
    Local(HttpError),
}

impl HttpClient {
    /// 合并配置和请求选项，并在进入网络层前执行 URL、Header 与请求体限制检查。
    pub(super) fn prepare(&self, request: HttpRequest) -> Result<PreparedRequest, HttpError> {
        let url = request.resolve(self.config.base_url_ref())?;
        let filtered_defaults;
        let defaults = if self
            .config
            .base_url_ref()
            .is_some_and(|base_url| !same_origin(base_url, &url))
        {
            filtered_defaults = self.config.default_headers().without_sensitive();
            &filtered_defaults
        } else {
            self.config.default_headers()
        };
        let headers = HttpHeaders::merge(defaults, request.headers())?;
        let method = request.method().clone();
        let timeout = request.timeout().unwrap_or(self.config.request_timeout());
        let retry_policy = request
            .retry_policy()
            .cloned()
            .unwrap_or_else(|| self.config.retry_policy().clone());
        let deduplication_policy = request
            .deduplication_policy()
            .cloned()
            .unwrap_or_else(|| self.config.deduplication_policy().clone());
        let deduplication_opt_in = request.deduplication_policy().is_some();
        let body = request.into_body();
        if let Some(body) = &body {
            if body.len() > self.config.max_request_body_bytes() {
                return Err(HttpError::RequestBodyTooLarge {
                    limit: self.config.max_request_body_bytes(),
                });
            }
        }
        Ok(PreparedRequest {
            url,
            method,
            headers,
            body,
            timeout,
            retry_policy,
            deduplication_policy,
            deduplication_opt_in,
        })
    }
}

/// 将底层传输失败转换成不包含第三方错误文本的公共错误。
pub(super) fn transport_error(
    kind: HttpTransportErrorKind,
    attempts: u32,
    exhausted: bool,
) -> HttpError {
    HttpError::Transport {
        kind,
        attempts,
        exhausted,
    }
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}
