//! HTTP 客户端配置与去重策略。

use std::fmt;
use std::time::Duration;

use url::Url;

use super::headers::HttpHeaders;
use super::request::validate_absolute_url;
use super::retry::RetryPolicy;
use super::HttpError;

const MAX_REQUEST_OR_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// single-flight 的合并模式。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeduplicationMode {
    /// 不合并请求。
    Disabled,
    /// 只合并当前正在执行的相同请求。
    InFlight,
    /// 合并正在执行的请求，并在成功响应上保留显式 TTL 缓存。
    WithCompletedTtl,
}

/// HTTP 请求去重和短期完成缓存策略。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeduplicationPolicy {
    mode: DeduplicationMode,
    ttl: Duration,
    max_inflight_keys: usize,
    max_completed_entries: usize,
    max_cached_body_bytes: usize,
}

impl Default for DeduplicationPolicy {
    fn default() -> Self {
        Self {
            mode: DeduplicationMode::InFlight,
            ttl: Duration::ZERO,
            max_inflight_keys: 1024,
            max_completed_entries: 128,
            max_cached_body_bytes: 8 * 1024 * 1024,
        }
    }
}

impl DeduplicationPolicy {
    /// 禁用请求去重。
    pub fn disabled() -> Self {
        Self {
            mode: DeduplicationMode::Disabled,
            ..Self::default()
        }
    }

    /// 创建只合并 in-flight 请求的策略。
    pub fn in_flight(max_inflight_keys: usize) -> Result<Self, HttpError> {
        validate_key_limit(max_inflight_keys)?;
        Ok(Self {
            mode: DeduplicationMode::InFlight,
            max_inflight_keys,
            ..Self::default()
        })
    }

    /// 创建带完成缓存 TTL 的策略。
    pub fn with_completed_ttl(
        ttl: Duration,
        max_inflight_keys: usize,
        max_completed_entries: usize,
        max_cached_body_bytes: usize,
    ) -> Result<Self, HttpError> {
        validate_key_limit(max_inflight_keys)?;
        if ttl.is_zero() || ttl > Duration::from_secs(60 * 60) {
            return Err(HttpError::InvalidConfig {
                field: "deduplication_ttl",
            });
        }
        if !(1..=1024).contains(&max_completed_entries) {
            return Err(HttpError::InvalidConfig {
                field: "max_completed_entries",
            });
        }
        if !(1..=64 * 1024 * 1024).contains(&max_cached_body_bytes) {
            return Err(HttpError::InvalidConfig {
                field: "max_cached_body_bytes",
            });
        }
        Ok(Self {
            mode: DeduplicationMode::WithCompletedTtl,
            ttl,
            max_inflight_keys,
            max_completed_entries,
            max_cached_body_bytes,
        })
    }

    /// 返回去重模式。
    pub fn mode(&self) -> DeduplicationMode {
        self.mode
    }

    /// 返回完成缓存 TTL；仅 `WithCompletedTtl` 模式生效。
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// 返回允许同时追踪的 in-flight key 数量。
    pub fn max_inflight_keys(&self) -> usize {
        self.max_inflight_keys
    }

    /// 返回完成缓存最大条目数。
    pub fn max_completed_entries(&self) -> usize {
        self.max_completed_entries
    }

    /// 返回完成缓存允许占用的响应体总字节数。
    pub fn max_cached_body_bytes(&self) -> usize {
        self.max_cached_body_bytes
    }

    /// 返回是否开启请求去重。
    pub fn is_enabled(&self) -> bool {
        self.mode != DeduplicationMode::Disabled
    }

    /// 返回是否开启成功响应缓存。
    pub fn cache_enabled(&self) -> bool {
        self.mode == DeduplicationMode::WithCompletedTtl && !self.ttl.is_zero()
    }
}

/// HTTP 客户端配置。
#[derive(Clone, Eq, PartialEq)]
pub struct HttpConfig {
    base_url: Option<Url>,
    default_headers: HttpHeaders,
    request_timeout: Duration,
    connect_timeout: Duration,
    max_request_body_bytes: usize,
    max_response_body_bytes: usize,
    max_idle_connections_per_host: usize,
    idle_connection_timeout: Duration,
    retry_policy: RetryPolicy,
    deduplication_policy: DeduplicationPolicy,
}

impl Default for HttpConfig {
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

    pub(crate) fn base_url_ref(&self) -> Option<&Url> {
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
    base_url: Option<Url>,
    default_headers: HttpHeaders,
    request_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    max_request_body_bytes: Option<usize>,
    max_response_body_bytes: Option<usize>,
    max_idle_connections_per_host: Option<usize>,
    idle_connection_timeout: Option<Duration>,
    retry_policy: Option<RetryPolicy>,
    deduplication_policy: Option<DeduplicationPolicy>,
}

impl fmt::Debug for HttpConfigBuilder {
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
    pub fn base_url(mut self, value: impl AsRef<str>) -> Result<Self, HttpError> {
        let url = Url::parse(value.as_ref()).map_err(|_| HttpError::InvalidUrl)?;
        validate_absolute_url(&url)?;
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
        self.default_headers.set(name, value)?;
        Ok(self)
    }

    /// 设置请求总时间预算。
    pub fn request_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        validate_timeout(timeout, "request_timeout")?;
        self.request_timeout = Some(timeout);
        Ok(self)
    }

    /// 设置连接建立时间预算。
    pub fn connect_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        validate_timeout(timeout, "connect_timeout")?;
        self.connect_timeout = Some(timeout);
        Ok(self)
    }

    /// 设置请求体上限。
    pub fn max_request_body_bytes(mut self, limit: usize) -> Result<Self, HttpError> {
        validate_byte_limit(limit, "max_request_body_bytes")?;
        self.max_request_body_bytes = Some(limit);
        Ok(self)
    }

    /// 设置响应体上限。
    pub fn max_response_body_bytes(mut self, limit: usize) -> Result<Self, HttpError> {
        validate_byte_limit(limit, "max_response_body_bytes")?;
        self.max_response_body_bytes = Some(limit);
        Ok(self)
    }

    /// 设置每个主机允许保留的最大空闲连接数。
    pub fn max_idle_connections_per_host(mut self, max: usize) -> Result<Self, HttpError> {
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
    pub fn build(self) -> Result<HttpConfig, HttpError> {
        let request_timeout = self.request_timeout.unwrap_or(Duration::from_secs(30));
        let connect_timeout = self
            .connect_timeout
            .unwrap_or_else(|| Duration::from_secs(10).min(request_timeout));
        if connect_timeout > request_timeout {
            return Err(HttpError::InvalidConfig {
                field: "connect_timeout",
            });
        }
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

fn validate_key_limit(value: usize) -> Result<(), HttpError> {
    if !(1..=4096).contains(&value) {
        return Err(HttpError::InvalidConfig {
            field: "max_inflight_keys",
        });
    }
    Ok(())
}

fn validate_timeout(value: Duration, field: &'static str) -> Result<(), HttpError> {
    if value.is_zero() || value > Duration::from_secs(60 * 60) {
        return Err(HttpError::InvalidConfig { field });
    }
    Ok(())
}

fn validate_byte_limit(value: usize, field: &'static str) -> Result<(), HttpError> {
    if !(1..=MAX_REQUEST_OR_RESPONSE_BYTES).contains(&value) {
        return Err(HttpError::InvalidConfig { field });
    }
    Ok(())
}
