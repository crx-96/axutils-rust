//! HTTP 方法和请求构造。

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use url::Url;

use super::config::DeduplicationPolicy;
use super::headers::HttpHeaders;
use super::retry::RetryPolicy;
use super::HttpError;

const MAX_URL_BYTES: usize = 8 * 1024;
const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;

/// HTTP 请求方法。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum HttpMethod {
    /// GET。
    Get,
    /// HEAD。
    Head,
    /// POST。
    Post,
    /// PUT。
    Put,
    /// PATCH。
    Patch,
    /// DELETE。
    Delete,
    /// OPTIONS。
    Options,
    /// TRACE。
    Trace,
    /// CONNECT。
    Connect,
    /// 经过 token 校验的自定义方法。
    Custom(String),
}

impl HttpMethod {
    /// 构造自定义 HTTP 方法。
    pub fn custom(value: impl AsRef<str>) -> Result<Self, HttpError> {
        let value = value.as_ref();
        if value.is_empty() || !value.as_bytes().iter().copied().all(is_token_byte) {
            return Err(HttpError::InvalidRequest { field: "method" });
        }
        Ok(match value {
            "GET" => Self::Get,
            "HEAD" => Self::Head,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" => Self::Delete,
            "OPTIONS" => Self::Options,
            "TRACE" => Self::Trace,
            "CONNECT" => Self::Connect,
            _ => Self::Custom(value.to_owned()),
        })
    }

    /// 返回线上的方法名。
    pub fn as_str(&self) -> &str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Trace => "TRACE",
            Self::Connect => "CONNECT",
            Self::Custom(value) => value,
        }
    }

    /// 返回是否属于默认允许重试的安全方法集合。
    pub(crate) fn is_idempotent_safe(&self) -> bool {
        matches!(self, Self::Get | Self::Head | Self::Options)
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = HttpError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::custom(value)
    }
}

/// HTTP 请求。
#[derive(Clone)]
pub struct HttpRequest {
    target: RequestTarget,
    method: HttpMethod,
    headers: HttpHeaders,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
    retry_policy: Option<RetryPolicy>,
    deduplication_policy: Option<DeduplicationPolicy>,
}

#[derive(Clone)]
enum RequestTarget {
    Absolute(Url),
    Relative(String),
}

impl HttpRequest {
    /// 使用方法和 URL 创建请求。
    pub fn new(method: HttpMethod, url: impl AsRef<str>) -> Result<Self, HttpError> {
        Ok(Self {
            target: parse_target(url.as_ref())?,
            method,
            headers: HttpHeaders::new(),
            body: None,
            timeout: None,
            retry_policy: None,
            deduplication_policy: None,
        })
    }

    /// 创建请求 builder。
    pub fn builder() -> HttpRequestBuilder {
        HttpRequestBuilder::default()
    }

    /// 设置一个请求 Header。
    pub fn with_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        self.headers.set(name, value)?;
        Ok(self)
    }

    /// 追加一个请求 Header。
    pub fn append_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        self.headers.append(name, value)?;
        Ok(self)
    }

    /// 设置请求体。
    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Result<Self, HttpError> {
        let body = body.into();
        if body.len() > MAX_REQUEST_BODY_BYTES {
            return Err(HttpError::RequestBodyTooLarge {
                limit: MAX_REQUEST_BODY_BYTES,
            });
        }
        self.body = Some(body);
        Ok(self)
    }

    /// 设置请求总时间预算。
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60 * 60) {
            return Err(HttpError::InvalidRequest { field: "timeout" });
        }
        self.timeout = Some(timeout);
        Ok(self)
    }

    /// 覆盖该请求使用的重试策略。
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// 覆盖该请求使用的去重策略。
    pub fn with_deduplication_policy(mut self, policy: DeduplicationPolicy) -> Self {
        self.deduplication_policy = Some(policy);
        self
    }

    /// 返回请求方法。
    pub fn method(&self) -> &HttpMethod {
        &self.method
    }

    /// 返回原始 URL 或相对路径。
    pub fn url(&self) -> &str {
        match &self.target {
            RequestTarget::Absolute(url) => url.as_str(),
            RequestTarget::Relative(value) => value,
        }
    }

    /// 返回请求 Header。
    pub fn headers(&self) -> &HttpHeaders {
        &self.headers
    }

    /// 返回请求体。
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// 返回请求级时间预算。
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// 返回请求级重试策略。
    pub fn retry_policy(&self) -> Option<&RetryPolicy> {
        self.retry_policy.as_ref()
    }

    /// 返回请求级去重策略。
    pub fn deduplication_policy(&self) -> Option<&DeduplicationPolicy> {
        self.deduplication_policy.as_ref()
    }

    pub(crate) fn resolve(&self, base_url: Option<&Url>) -> Result<Url, HttpError> {
        let resolved = match &self.target {
            RequestTarget::Absolute(url) => url.clone(),
            RequestTarget::Relative(value) => base_url
                .ok_or(HttpError::InvalidRequest { field: "base_url" })?
                .join(value)
                .map_err(|_| HttpError::InvalidUrl)?,
        };
        validate_absolute_url(&resolved)?;
        Ok(resolved)
    }
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &"<redacted>")
            .field("headers", &self.headers)
            .field("body_len", &self.body.as_ref().map(Vec::len))
            .field("timeout", &self.timeout)
            .field("retry_policy", &self.retry_policy)
            .field("deduplication_policy", &self.deduplication_policy)
            .finish()
    }
}

/// [`HttpRequest`] 的 builder。
#[derive(Default)]
pub struct HttpRequestBuilder {
    method: Option<HttpMethod>,
    url: Option<String>,
    headers: HttpHeaders,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
    retry_policy: Option<RetryPolicy>,
    deduplication_policy: Option<DeduplicationPolicy>,
}

impl fmt::Debug for HttpRequestBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequestBuilder")
            .field("method", &self.method)
            .field("url_configured", &self.url.is_some())
            .field("headers", &self.headers)
            .field("body_len", &self.body.as_ref().map(Vec::len))
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl HttpRequestBuilder {
    /// 设置方法。
    pub fn method(mut self, method: HttpMethod) -> Self {
        self.method = Some(method);
        self
    }

    /// 设置 URL 或相对路径。
    pub fn url(mut self, url: impl AsRef<str>) -> Self {
        self.url = Some(url.as_ref().to_owned());
        self
    }

    /// 设置 Header。
    pub fn header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        self.headers.set(name, value)?;
        Ok(self)
    }

    /// 追加 Header。
    pub fn append_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        self.headers.append(name, value)?;
        Ok(self)
    }

    /// 设置请求体。
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Result<Self, HttpError> {
        let body = body.into();
        if body.len() > MAX_REQUEST_BODY_BYTES {
            return Err(HttpError::RequestBodyTooLarge {
                limit: MAX_REQUEST_BODY_BYTES,
            });
        }
        self.body = Some(body);
        Ok(self)
    }

    /// 设置请求总时间预算。
    pub fn timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        if timeout.is_zero() || timeout > Duration::from_secs(60 * 60) {
            return Err(HttpError::InvalidRequest { field: "timeout" });
        }
        self.timeout = Some(timeout);
        Ok(self)
    }

    /// 设置请求级重试策略。
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// 设置请求级去重策略。
    pub fn deduplication_policy(mut self, policy: DeduplicationPolicy) -> Self {
        self.deduplication_policy = Some(policy);
        self
    }

    /// 构造请求。
    pub fn build(self) -> Result<HttpRequest, HttpError> {
        HttpRequest::new(
            self.method
                .ok_or(HttpError::InvalidRequest { field: "method" })?,
            self.url.ok_or(HttpError::InvalidRequest { field: "url" })?,
        )
        .map(|mut request| {
            request.headers = self.headers;
            request.body = self.body;
            request.timeout = self.timeout;
            request.retry_policy = self.retry_policy;
            request.deduplication_policy = self.deduplication_policy;
            request
        })
    }
}

fn parse_target(value: &str) -> Result<RequestTarget, HttpError> {
    if value.is_empty()
        || value.len() > MAX_URL_BYTES
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(HttpError::InvalidUrl);
    }
    match Url::parse(value) {
        Ok(url) => {
            validate_absolute_url(&url)?;
            Ok(RequestTarget::Absolute(url))
        }
        Err(_) if value.starts_with("//") || value.contains('\\') || value.contains("://") => {
            Err(HttpError::InvalidUrl)
        }
        Err(_) => Ok(RequestTarget::Relative(value.to_owned())),
    }
}

pub(crate) fn validate_absolute_url(url: &Url) -> Result<(), HttpError> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.password().is_some()
        || !url.username().is_empty()
        || url.fragment().is_some()
        || url.as_str().len() > MAX_URL_BYTES
    {
        return Err(HttpError::InvalidUrl);
    }
    if let Some((_, rest)) = url.as_str().split_once("//") {
        let authority = rest
            .split_once('/')
            .map(|(authority, _)| authority)
            .unwrap_or(rest);
        if authority.contains('@') {
            return Err(HttpError::InvalidUrl);
        }
    }
    Ok(())
}

fn is_token_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9'
            | b'a'..=b'z'
            | b'A'..=b'Z'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
    )
}
