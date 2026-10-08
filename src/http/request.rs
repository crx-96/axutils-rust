//! HTTP 方法和请求构造。

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use url::Url;

use super::headers::HttpHeaders;
use super::retry::RetryPolicy;
use super::DeduplicationPolicy;
use super::HttpError;

/// 原始 URL 输入与最终规范化绝对 URL 各自的字节上限。
const MAX_URL_BYTES: usize = 8 * 1024;
/// 请求模型可持有的正文绝对字节上限；客户端还可施加更小预算。
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
        // 只接受非空 token；标准大写方法收敛到固定变体以复用其安全重试语义。
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
    pub(super) fn is_idempotent_safe(&self) -> bool {
        matches!(self, Self::Get | Self::Head | Self::Options)
    }
}

impl fmt::Display for HttpMethod {
    /// 输出线上的方法 token；自定义方法保持调用方提供的大小写。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for HttpMethod {
    /// 方法解析失败时返回不含原始输入的请求字段错误。
    type Err = HttpError;

    /// 通过同一 token 校验入口解析标准或自定义方法。
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::custom(value)
    }
}

/// HTTP 请求。
#[derive(Clone)]
pub struct HttpRequest {
    /// 已验证的绝对地址或等待客户端基地址解析的相对地址。
    target: RequestTarget,
    /// HTTP 方法；Custom 的公开字段在传输构造时再次验证。
    method: HttpMethod,
    /// 仅本请求的有界 header，执行前与客户端默认项合并。
    headers: HttpHeaders,
    /// 可选正文，构造上限 16 MiB；execute 还应用客户端更小的预算。
    body: Option<Vec<u8>>,
    /// 可选总网络预算；None 沿用客户端配置。
    timeout: Option<Duration>,
    /// 可选完整重试策略；None 沿用客户端配置。
    retry_policy: Option<RetryPolicy>,
    /// 可选合并策略；Some 表示对本请求显式选择。
    deduplication_policy: Option<DeduplicationPolicy>,
}

/// 请求地址的已解析状态；相对地址需要在 execute 时结合客户端配置解析。
#[derive(Clone)]
enum RequestTarget {
    /// 已校验的完整 HTTP(S) URL。
    Absolute(Url),
    /// 已检查原始大小与控制字符、等待基地址解析的文本。
    Relative(String),
}

impl HttpRequest {
    /// 使用方法和 URL 创建请求。
    pub fn new(method: HttpMethod, url: impl AsRef<str>) -> Result<Self, HttpError> {
        // 地址在构造时检查，默认不携带正文或请求级策略覆盖。
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
        // 接管正文后立即检查模型绝对上限；execute 会再应用客户端预算。
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
        // 预算必须为有限正值，避免传输 deadline 溢出或零时长隐式语义。
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
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest};
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health").unwrap();
    /// assert_eq!(request.method(), &HttpMethod::Get);
    /// ```
    pub fn method(&self) -> &HttpMethod {
        &self.method
    }

    /// 返回原始 URL 或相对路径。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest};
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health?full=1").unwrap();
    /// assert_eq!(request.url(), "/health?full=1");
    /// ```
    pub fn url(&self) -> &str {
        match &self.target {
            RequestTarget::Absolute(url) => url.as_str(),
            RequestTarget::Relative(value) => value,
        }
    }

    /// 返回请求 Header。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest};
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health")
    ///     .unwrap()
    ///     .with_header("x-request-id", "demo")
    ///     .unwrap();
    /// assert_eq!(request.headers().get("x-request-id"), Some(&b"demo"[..]));
    /// ```
    pub fn headers(&self) -> &HttpHeaders {
        &self.headers
    }

    /// 返回请求体。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest};
    ///
    /// let request = HttpRequest::new(HttpMethod::Post, "/events")
    ///     .unwrap()
    ///     .with_body(b"payload".to_vec())
    ///     .unwrap();
    /// assert_eq!(request.body(), Some(&b"payload"[..]));
    /// ```
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// 消费请求并转移正文所有权，避免请求准备阶段再复制正文。
    pub(super) fn into_body(self) -> Option<Vec<u8>> {
        self.body
    }

    /// 返回请求级时间预算。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest};
    /// use std::time::Duration;
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health")
    ///     .unwrap()
    ///     .with_timeout(Duration::from_secs(2))
    ///     .unwrap();
    /// assert_eq!(request.timeout(), Some(Duration::from_secs(2)));
    /// ```
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// 返回请求级重试策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpMethod, HttpRequest, RetryPolicy};
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health")
    ///     .unwrap()
    ///     .with_retry_policy(RetryPolicy::new());
    /// assert!(request.retry_policy().is_some());
    /// ```
    pub fn retry_policy(&self) -> Option<&RetryPolicy> {
        self.retry_policy.as_ref()
    }

    /// 返回请求级去重策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{DeduplicationPolicy, HttpMethod, HttpRequest};
    ///
    /// let request = HttpRequest::new(HttpMethod::Get, "/health")
    ///     .unwrap()
    ///     .with_deduplication_policy(DeduplicationPolicy::in_flight(8).unwrap());
    /// assert!(request.deduplication_policy().is_some());
    /// ```
    pub fn deduplication_policy(&self) -> Option<&DeduplicationPolicy> {
        self.deduplication_policy.as_ref()
    }

    /// 保留绝对请求优先级，或使用基地址解析相对请求，再校验最终目标。
    pub(super) fn resolve(&self, base_url: Option<&Url>) -> Result<Url, HttpError> {
        // 相对 URL 缺少基地址时明确失败；join 之后重新检查避免越过最终地址限制。
        let resolved = match &self.target {
            RequestTarget::Absolute(url) => url.clone(),
            RequestTarget::Relative(value) => base_url
                .ok_or(HttpError::InvalidUrl)?
                .join(value)
                .map_err(|_| HttpError::InvalidUrl)?,
        };
        validate_absolute_url(&resolved)?;
        Ok(resolved)
    }
}

impl fmt::Debug for HttpRequest {
    /// 仅显示方法、预算与大小，不回显 URL、header 值或正文。
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
    /// 必填方法，缺失时 build 返回 method 字段错误。
    method: Option<HttpMethod>,
    /// 必填原始 URL 文本，build 时检查语法与大小。
    url: Option<String>,
    /// 已逐项校验的请求 header 集合。
    headers: HttpHeaders,
    /// 可选正文；设置时执行 16 MiB 绝对上限检查。
    body: Option<Vec<u8>>,
    /// 可选请求总预算；None 沿用客户端配置。
    timeout: Option<Duration>,
    /// 可选完整重试策略；None 沿用客户端配置。
    retry_policy: Option<RetryPolicy>,
    /// 可选显式合并策略；None 沿用客户端配置。
    deduplication_policy: Option<DeduplicationPolicy>,
}

impl fmt::Debug for HttpRequestBuilder {
    /// 展示未完成请求的存在标记和大小，不展开敏感地址或正文。
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
        // builder 同样不能暂存超过模型绝对预算的正文。
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
        // 与直接请求入口采用一致的正值、最长一小时约束。
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
        // 先检查必填方法和地址，再把已经验证的各选项整体移入请求。
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

/// 校验原始地址并区分绝对/相对目标，拒绝含混的跨主机相对写法。
fn parse_target(value: &str) -> Result<RequestTarget, HttpError> {
    // parser 规范化之前检查原始输入；绝对地址继续检查 scheme 和敏感 URL 部分。
    validate_raw_url(value)?;
    match Url::parse(value) {
        Ok(url) => {
            validate_absolute_url(&url)?;
            Ok(RequestTarget::Absolute(url))
        }
        // join 会去掉首尾 ASCII 空格；不能让带空格的 network-path 绕过跨主机限制。
        Err(_)
            if value.trim_matches(' ').starts_with("//")
                || value.contains('\\')
                || value.contains("://") =>
        {
            Err(HttpError::InvalidUrl)
        }
        Err(_) => Ok(RequestTarget::Relative(value.to_owned())),
    }
}

/// 在 URL parser 丢弃控制字符或规范化文本前，统一检查输入边界。
pub(super) fn validate_raw_url(value: &str) -> Result<(), HttpError> {
    // 原始输入和最终绝对 URL 分别限长；前者还拒绝 parser 会静默忽略的 TAB、CR、LF。
    if value.is_empty()
        || value.len() > MAX_URL_BYTES
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(HttpError::InvalidUrl);
    }
    // HTTP(S) 的 special-scheme 解析允许省略或混用斜线，并会删除空 userinfo。
    // 因而在规范化前仅检查 authority；路径、query 中的 @ 与相对路径保持合法。
    if let Some((scheme, rest)) = value.trim_matches(' ').split_once(':') {
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            let authority = rest
                .trim_start_matches(['/', '\\'])
                .split(['/', '\\', '?', '#'])
                .next()
                .unwrap_or_default();
            if authority.contains('@') {
                return Err(HttpError::InvalidUrl);
            }
        }
    }
    Ok(())
}

/// 校验最终 HTTP(S) 地址，拒绝用户名、密码、fragment 及超长规范化结果。
pub(super) fn validate_absolute_url(url: &Url) -> Result<(), HttpError> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.password().is_some()
        || !url.username().is_empty()
        || url.fragment().is_some()
        || url.as_str().len() > MAX_URL_BYTES
    {
        return Err(HttpError::InvalidUrl);
    }
    Ok(())
}

/// 判断方法 token 可接受的 ASCII 字节，不依赖字符串错误文本推断合法性。
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
