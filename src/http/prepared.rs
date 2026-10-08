//! 请求准备与共享错误映射。

use std::time::Duration;

use url::Url;

use super::headers::HttpHeaders;
use super::request::{HttpMethod, HttpRequest};
use super::retry::RetryPolicy;
use super::DeduplicationPolicy;
use super::{HttpClient, HttpError, HttpTransportErrorKind};

/// 已完成本地校验、可交给传输层执行的请求。
pub(super) struct PreparedRequest {
    /// 已解析并验证的最终 HTTP(S) 绝对地址。
    pub(super) url: Url,
    /// 即将执行的方法，传输层仍验证公开 Custom 变体的 token。
    pub(super) method: HttpMethod,
    /// 完成跨源过滤与默认/请求合并的有界 header。
    pub(super) headers: HttpHeaders,
    /// 可选的已限长拥有型正文；None 与空正文在去重语义上不同。
    pub(super) body: Option<Vec<u8>>,
    /// 本次调用最终生效的总网络时间预算。
    pub(super) timeout: Duration,
    /// 本次调用最终生效的重试策略。
    pub(super) retry_policy: RetryPolicy,
    /// 本次调用最终生效的合并与完成缓存策略。
    pub(super) deduplication_policy: DeduplicationPolicy,
    /// 请求是否显式设置合并策略；用于允许带正文或非安全方法参与合并。
    pub(super) deduplication_opt_in: bool,
}

/// 单次传输尝试的本地或传输失败。
pub(super) enum AttemptError {
    /// 后端网络或协议失败，可由重试策略决定是否再次尝试。
    Transport(HttpTransportErrorKind),
    /// 本地校验或大小限制失败，不重试网络调用。
    Local(HttpError),
}

impl HttpClient {
    /// 合并配置和请求选项，并在进入网络层前执行 URL、Header 与请求体限制检查。
    pub(super) fn prepare(&self, request: HttpRequest) -> Result<PreparedRequest, HttpError> {
        // 先解析最终地址；绝对请求可以覆盖基地址，但跨源时不携带默认敏感 header。
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
        // 合并保持普通重复 header 的顺序；敏感默认项与请求项冲突时明确报错。
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
        // 转移正文所有权并应用客户端预算，确保任何网络尝试之前完成本地限制检查。
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

/// 按协议、规范化主机和有效端口比较源；显式默认端口与省略形式视为同源。
fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}
