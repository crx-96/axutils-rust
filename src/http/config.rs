//! HTTP 客户端配置与去重策略。

use std::fmt;
use std::time::Duration;

use url::Url;

use super::headers::HttpHeaders;
use super::request;
use super::retry::RetryPolicy;
use super::{DeduplicationPolicy, HttpError};

/// 客户端请求或响应体可配置的绝对字节上限。
const MAX_REQUEST_OR_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// HTTP 客户端配置。
#[derive(Clone, Eq, PartialEq)]
pub struct HttpConfig {
    /// 解析相对请求的可选 HTTP(S) 基地址；None 时只允许绝对 URL。
    base_url: Option<Url>,
    /// 已验证的默认 header，跨源请求会过滤敏感项。
    default_headers: HttpHeaders,
    /// 一次 execute 的总网络预算，默认 30 秒，包含重试与等待。
    request_timeout: Duration,
    /// 单次连接建立预算，默认不超过 10 秒或请求总预算。
    connect_timeout: Duration,
    /// 单次请求体字节上限，默认 1 MiB，最大 16 MiB。
    max_request_body_bytes: usize,
    /// 返回响应体的字节上限，默认 1 MiB，最大 16 MiB。
    max_response_body_bytes: usize,
    /// 每个主机可保留的空闲连接数，默认 8，范围 1..=64。
    max_idle_connections_per_host: usize,
    /// 空闲连接保留时长，默认 60 秒，范围 1 秒至 1 小时。
    idle_connection_timeout: Duration,
    /// 未被请求覆盖时使用的总尝试次数、退避和安全方法策略。
    retry_policy: RetryPolicy,
    /// 未被请求覆盖时使用的合并与有限完成缓存策略。
    deduplication_policy: DeduplicationPolicy,
}

impl Default for HttpConfig {
    /// 通过同一 builder 路径生成有限默认值，避免两套校验和默认值漂移。
    fn default() -> Self {
        Self::builder()
            .build()
            .expect("default HTTP configuration is valid")
    }
}

impl HttpConfig {
    /// 创建配置 builder。
    pub fn builder() -> HttpConfigBuilder {
        HttpConfigBuilder::default()
    }

    /// 返回基地址；URL 内容不会出现在 `Debug` 或错误中。
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_ref().map(Url::as_str)
    }

    /// 内部借用已解析基地址，避免每次请求重新解析配置文本。
    pub(super) fn base_url_ref(&self) -> Option<&Url> {
        self.base_url.as_ref()
    }

    /// 返回默认 Header。
    pub fn default_headers(&self) -> &HttpHeaders {
        &self.default_headers
    }

    /// 返回单个请求的总时间预算。
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// 返回单次连接建立时间预算。
    pub fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    /// 返回请求体上限。
    pub fn max_request_body_bytes(&self) -> usize {
        self.max_request_body_bytes
    }

    /// 返回响应体上限。
    pub fn max_response_body_bytes(&self) -> usize {
        self.max_response_body_bytes
    }

    /// 返回每个主机的最大空闲连接数。
    pub fn max_idle_connections_per_host(&self) -> usize {
        self.max_idle_connections_per_host
    }

    /// 返回空闲连接保留时间。
    pub fn idle_connection_timeout(&self) -> Duration {
        self.idle_connection_timeout
    }

    /// 返回默认重试策略。
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// 返回默认去重策略。
    pub fn deduplication_policy(&self) -> &DeduplicationPolicy {
        &self.deduplication_policy
    }
}

impl fmt::Debug for HttpConfig {
    /// 仅输出预算和策略；URL 只显示是否配置，header 只显示数量和大小。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpConfig")
            .field("base_url_configured", &self.base_url.is_some())
            .field("default_headers", &self.default_headers)
            .field("request_timeout", &self.request_timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("max_request_body_bytes", &self.max_request_body_bytes)
            .field("max_response_body_bytes", &self.max_response_body_bytes)
            .field(
                "max_idle_connections_per_host",
                &self.max_idle_connections_per_host,
            )
            .field("idle_connection_timeout", &self.idle_connection_timeout)
            .field("retry_policy", &self.retry_policy)
            .field("deduplication_policy", &self.deduplication_policy)
            .finish()
    }
}

/// [`HttpConfig`] 的 builder。
#[derive(Clone, Default)]
pub struct HttpConfigBuilder {
    /// 可选基地址；None 在 build 后保持未配置。
    base_url: Option<Url>,
    /// 已通过 header 校验的默认集合，初始为空。
    default_headers: HttpHeaders,
    /// 可选请求总预算；None 使用 30 秒。
    request_timeout: Option<Duration>,
    /// 可选连接预算；None 使用 10 秒与请求预算的较小值。
    connect_timeout: Option<Duration>,
    /// 可选请求体字节上限；None 使用 1 MiB。
    max_request_body_bytes: Option<usize>,
    /// 可选响应体字节上限；None 使用 1 MiB。
    max_response_body_bytes: Option<usize>,
    /// 可选单主机空闲连接数；None 使用 8。
    max_idle_connections_per_host: Option<usize>,
    /// 可选空闲连接寿命；None 使用 60 秒。
    idle_connection_timeout: Option<Duration>,
    /// 可选默认重试策略；None 使用 RetryPolicy::default。
    retry_policy: Option<RetryPolicy>,
    /// 可选默认合并策略；None 仅合并正在执行的安全请求。
    deduplication_policy: Option<DeduplicationPolicy>,
}

impl fmt::Debug for HttpConfigBuilder {
    /// 展示配置项的存在与预算，不回显 URL 和默认 header 值。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpConfigBuilder")
            .field("base_url_configured", &self.base_url.is_some())
            .field("default_headers", &self.default_headers)
            .field("request_timeout", &self.request_timeout)
            .field("connect_timeout", &self.connect_timeout)
            .field("max_request_body_bytes", &self.max_request_body_bytes)
            .field("max_response_body_bytes", &self.max_response_body_bytes)
            .field(
                "max_idle_connections_per_host",
                &self.max_idle_connections_per_host,
            )
            .field("idle_connection_timeout", &self.idle_connection_timeout)
            .field("retry_policy", &self.retry_policy)
            .field("deduplication_policy", &self.deduplication_policy)
            .finish()
    }
}

impl HttpConfigBuilder {
    /// 设置相对请求解析使用的 HTTP/HTTPS 基地址。
    ///
    /// 不调用此方法时基地址保持为空；此时请求仍可使用完整的绝对 HTTP/HTTPS URL，
    /// 但相对 URL 会在执行时返回 [`HttpError::InvalidUrl`]。
    pub fn base_url(mut self, value: impl AsRef<str>) -> Result<Self, HttpError> {
        // parser 会忽略部分控制字符，因此必须在规范化前校验原始文本。
        request::validate_raw_url(value.as_ref())?;
        let url = Url::parse(value.as_ref()).map_err(|_| HttpError::InvalidUrl)?;
        request::validate_absolute_url(&url)?;
        self.base_url = Some(url);
        Ok(self)
    }

    /// 替换默认 Header 集合。
    pub fn default_headers(mut self, headers: HttpHeaders) -> Self {
        self.default_headers = headers;
        self
    }

    /// 添加一个默认 Header。
    pub fn with_default_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        // 容器负责原子替换和大小检查，失败不留下半更新的 header 集合。
        self.default_headers.set(name, value)?;
        Ok(self)
    }

    /// 设置请求总时间预算。
    pub fn request_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        // 保存前校验有限预算；与连接预算的相对关系在 build 时检查。
        validate_timeout(timeout, "request_timeout")?;
        self.request_timeout = Some(timeout);
        Ok(self)
    }

    /// 设置连接建立时间预算。
    pub fn connect_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        // 先限制单值范围，最终 build 再保证连接预算不超过请求总预算。
        validate_timeout(timeout, "connect_timeout")?;
        self.connect_timeout = Some(timeout);
        Ok(self)
    }

    /// 设置请求体上限。
    pub fn max_request_body_bytes(mut self, limit: usize) -> Result<Self, HttpError> {
        // 请求级 16 MiB 上限之外，客户端可以采用更小的实际发送预算。
        validate_byte_limit(limit, "max_request_body_bytes")?;
        self.max_request_body_bytes = Some(limit);
        Ok(self)
    }

    /// 设置响应体上限。
    pub fn max_response_body_bytes(mut self, limit: usize) -> Result<Self, HttpError> {
        // 响应按读取后的字节计数，拒绝零值和超过绝对上限的配置。
        validate_byte_limit(limit, "max_response_body_bytes")?;
        self.max_response_body_bytes = Some(limit);
        Ok(self)
    }

    /// 设置每个主机允许保留的最大空闲连接数。
    pub fn max_idle_connections_per_host(mut self, max: usize) -> Result<Self, HttpError> {
        // 只控制连接池保留数量，不把此值解释为并发请求上限。
        if !(1..=64).contains(&max) {
            return Err(HttpError::InvalidConfig {
                field: "max_idle_connections_per_host",
            });
        }
        self.max_idle_connections_per_host = Some(max);
        Ok(self)
    }

    /// 设置空闲连接保留时间。
    pub fn idle_connection_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        // 保留时间有界，防止无期限维持空闲连接。
        if !(Duration::from_secs(1)..=Duration::from_secs(60 * 60)).contains(&timeout) {
            return Err(HttpError::InvalidConfig {
                field: "idle_connection_timeout",
            });
        }
        self.idle_connection_timeout = Some(timeout);
        Ok(self)
    }

    /// 设置默认重试策略。
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// 设置默认去重策略。
    pub fn deduplication_policy(mut self, policy: DeduplicationPolicy) -> Self {
        self.deduplication_policy = Some(policy);
        self
    }

    /// 完成并校验配置。
    ///
    /// 所有 builder 字段都是可选的。未设置时使用有限默认值：请求总超时 30 秒、连接
    /// 超时 10 秒、请求/响应体上限 1 MiB、空闲连接超时 60 秒，以及包含首次请求在内的
    /// 3 次最大网络尝试。未设置 `base_url` 时只允许执行绝对 URL。若显式设置的
    /// `connect_timeout` 大于 `request_timeout`，返回 `InvalidConfig { field: "connect_timeout" }`。
    pub fn build(self) -> Result<HttpConfig, HttpError> {
        // 默认连接预算随较短的请求预算收缩；调用方显式提供的冲突值则报错。
        let request_timeout = self.request_timeout.unwrap_or(Duration::from_secs(30));
        let connect_timeout = self
            .connect_timeout
            .unwrap_or_else(|| Duration::from_secs(10).min(request_timeout));
        if connect_timeout > request_timeout {
            return Err(HttpError::InvalidConfig {
                field: "connect_timeout",
            });
        }
        // 只有各字段与组合都合法，才发布不可变配置；无基地址仍是有效配置。
        Ok(HttpConfig {
            base_url: self.base_url,
            default_headers: self.default_headers,
            request_timeout,
            connect_timeout,
            max_request_body_bytes: self.max_request_body_bytes.unwrap_or(1024 * 1024),
            max_response_body_bytes: self.max_response_body_bytes.unwrap_or(1024 * 1024),
            max_idle_connections_per_host: self.max_idle_connections_per_host.unwrap_or(8),
            idle_connection_timeout: self
                .idle_connection_timeout
                .unwrap_or(Duration::from_secs(60)),
            retry_policy: self.retry_policy.unwrap_or_default(),
            deduplication_policy: self.deduplication_policy.unwrap_or_default(),
        })
    }
}

/// 检查大于零且不超过 1 小时的网络预算，错误仅包含固定字段名。
fn validate_timeout(value: Duration, field: &'static str) -> Result<(), HttpError> {
    if value.is_zero() || value > Duration::from_secs(60 * 60) {
        return Err(HttpError::InvalidConfig { field });
    }
    Ok(())
}

/// 检查请求/响应正文预算位于 1 字节至 16 MiB。
fn validate_byte_limit(value: usize, field: &'static str) -> Result<(), HttpError> {
    if !(1..=MAX_REQUEST_OR_RESPONSE_BYTES).contains(&value) {
        return Err(HttpError::InvalidConfig { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HttpConfig, HttpError};

    #[test]
    fn regression_base_url_rejects_raw_input_before_normalization() {
        for url in [
            "",
            "https://exa\nmple.com/",
            "\rhttps://example.com/",
            "https://example.com/\tpath",
        ] {
            assert!(
                matches!(
                    HttpConfig::builder().base_url(url),
                    Err(HttpError::InvalidUrl)
                ),
                "accepted {url:?}"
            );
        }
        let too_long = format!("{}https://example.com/", "\t".repeat(8192));
        assert!(matches!(
            HttpConfig::builder().base_url(too_long),
            Err(HttpError::InvalidUrl)
        ));
    }
}
