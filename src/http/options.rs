//! 便捷 HTTP 方法使用的单次调用配置。

use std::fmt;
use std::time::Duration;

use super::headers::HttpHeaders;
#[cfg(feature = "http-json")]
use super::request::HttpRequest;
use super::retry::RetryPolicy;
use super::DeduplicationPolicy;
use super::HttpError;

/// 单次便捷 HTTP 调用的可选配置。
///
/// `None` 表示使用 `HttpClient` 的对应默认值。该类型只覆盖单次调用需要覆盖的配置；
/// 连接池、响应体总上限和基础 URL 等实例级配置仍由 [`super::HttpConfig`] 管理。
#[derive(Clone, Default, Eq, PartialEq)]
pub struct HttpRequestOptions {
    /// 仅本次调用使用的 header 覆盖集合；普通同名项替换客户端默认项。
    headers: HttpHeaders,
    /// 可选总网络预算；None 沿用客户端配置。
    timeout: Option<Duration>,
    /// 可选完整重试策略；None 沿用客户端配置。
    retry_policy: Option<RetryPolicy>,
    /// 可选合并策略；Some 表示调用方对本请求显式选择。
    deduplication_policy: Option<DeduplicationPolicy>,
}

impl HttpRequestOptions {
    /// 创建空的单次调用配置。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() {
    /// let options = HttpRequestOptions::new();
    /// assert!(options.headers().is_empty());
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置单次调用 Header；同名 Header 会在此配置内被替换。
    ///
    /// 普通 Header 应用到客户端时可以覆盖客户端默认值；`Authorization`、`Cookie` 和
    /// `Set-Cookie` 与客户端默认 Header 冲突时，执行会返回
    /// [`HttpError::DuplicateSensitiveHeader`]，不会静默覆盖。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpError, HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() -> Result<(), HttpError> {
    /// let options = HttpRequestOptions::new()
    ///     .with_header("x-request-id", "demo")?;
    /// assert_eq!(options.headers().get("x-request-id"), Some(&b"demo"[..]));
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn with_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        // 交给 header 容器原子替换并校验大小，实际默认值合并在 execute 时进行。
        self.headers.set(name, value)?;
        Ok(self)
    }

    /// 追加单次调用 Header；非敏感 Header 保留重复项顺序。
    ///
    /// `Authorization`、`Cookie` 和 `Set-Cookie` 不能与客户端默认 Header 合并；发生冲突时，
    /// 执行会返回 [`HttpError::DuplicateSensitiveHeader`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpError, HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() -> Result<(), HttpError> {
    /// let options = HttpRequestOptions::new()
    ///     .append_header("accept", "application/json")?;
    /// assert!(options.headers().contains("accept"));
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn append_header(
        mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<Self, HttpError> {
        // 保留普通重复项顺序，敏感重复项由容器立即拒绝。
        self.headers.append(name, value)?;
        Ok(self)
    }

    /// 设置本次调用的总时间预算。
    ///
    /// 时间预算必须大于零且不超过一小时；它覆盖客户端默认的请求超时。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpError, HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() -> Result<(), HttpError> {
    /// let options = HttpRequestOptions::new()
    ///     .with_timeout(std::time::Duration::from_secs(5))?;
    /// assert_eq!(options.timeout(), Some(std::time::Duration::from_secs(5)));
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        // 请求级预算与 HttpRequest 采用相同范围，不允许零值或无界时长。
        if timeout.is_zero() || timeout > Duration::from_secs(60 * 60) {
            return Err(HttpError::InvalidRequest { field: "timeout" });
        }
        self.timeout = Some(timeout);
        Ok(self)
    }

    /// 覆盖本次调用的完整重试策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpRequestOptions, RetryPolicy};
    /// # #[cfg(feature = "http")]
    /// # fn main() {
    /// let options = HttpRequestOptions::new()
    ///     .with_retry_policy(RetryPolicy::new());
    /// assert!(options.retry_policy().is_some());
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// 只覆盖本次调用允许的最大总网络尝试次数，其他重试配置沿用默认策略。
    ///
    /// `max_retries` 包括首次请求；设置为 `1` 表示只发送一次请求，设置为 `3` 最多进行三次
    /// 网络尝试。方法名沿用现有 API 路径，不代表额外重试次数。
    /// 如果本 options 已通过 `with_retry_policy` 设置策略，则保留其余选项；否则以
    /// [`RetryPolicy::default`] 为基础生成完整覆盖，不继承客户端自定义的其他重试选项。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{HttpError, HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() -> Result<(), HttpError> {
    /// let options = HttpRequestOptions::new().with_max_retries(2)?;
    /// assert_eq!(options.retry_policy().unwrap().max_retries(), 2);
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn with_max_retries(mut self, max_retries: u32) -> Result<Self, HttpError> {
        // 修改本 options 已设置的策略；未设置时以 RetryPolicy 默认值为基础生成完整覆盖。
        let policy = self
            .retry_policy
            .take()
            .unwrap_or_default()
            .with_max_retries(max_retries)?;
        self.retry_policy = Some(policy);
        Ok(self)
    }

    /// 覆盖本次调用的去重和完成缓存策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{DeduplicationPolicy, HttpRequestOptions};
    /// # #[cfg(feature = "http")]
    /// # fn main() {
    /// let options = HttpRequestOptions::new()
    ///     .with_deduplication_policy(DeduplicationPolicy::disabled());
    /// assert!(!options
    ///     .deduplication_policy()
    ///     .unwrap()
    ///     .is_enabled());
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn with_deduplication_policy(mut self, policy: DeduplicationPolicy) -> Self {
        self.deduplication_policy = Some(policy);
        self
    }

    /// 返回单次调用 Header。
    ///
    /// # Examples
    ///
    /// ~~~
    /// use axutils::http::{HttpRequestOptions};
    /// let options = HttpRequestOptions::new();
    /// assert!(options.headers().is_empty());
    /// ~~~
    pub fn headers(&self) -> &HttpHeaders {
        &self.headers
    }

    /// 返回本次调用的时间预算覆盖值。
    ///
    /// # Examples
    ///
    /// ~~~rust
    /// use axutils::http::{HttpError, HttpRequestOptions};
    /// let options = HttpRequestOptions::new()
    ///     .with_timeout(std::time::Duration::from_secs(5))?;
    /// assert_eq!(options.timeout(), Some(std::time::Duration::from_secs(5)));
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// 返回本次调用的重试策略覆盖值。
    ///
    /// # Examples
    ///
    /// ~~~
    /// use axutils::http::{HttpRequestOptions};
    /// let options = HttpRequestOptions::new();
    /// assert!(options.retry_policy().is_none());
    /// ~~~
    pub fn retry_policy(&self) -> Option<&RetryPolicy> {
        self.retry_policy.as_ref()
    }

    /// 返回本次调用的去重策略覆盖值。
    ///
    /// # Examples
    ///
    /// ~~~
    /// use axutils::http::{HttpRequestOptions};
    /// let options = HttpRequestOptions::new();
    /// assert!(options.deduplication_policy().is_none());
    /// ~~~
    pub fn deduplication_policy(&self) -> Option<&DeduplicationPolicy> {
        self.deduplication_policy.as_ref()
    }

    #[cfg(feature = "http-json")]
    /// 把已验证的可选项应用到便捷 API 新建的请求，保留重复 header 与显式策略语义。
    pub(super) fn apply_to_request(
        &self,
        mut request: HttpRequest,
    ) -> Result<HttpRequest, HttpError> {
        // 逐项追加以保留原顺序，任何合并错误直接中止构造而不执行网络请求。
        for (name, value) in self.headers.iter() {
            request = request.append_header(name, value)?;
        }
        // 仅覆盖显式配置的选项，None 继续由客户端默认值决定。
        if let Some(timeout) = self.timeout {
            request = request.with_timeout(timeout)?;
        }
        if let Some(policy) = &self.retry_policy {
            request = request.with_retry_policy(policy.clone());
        }
        if let Some(policy) = &self.deduplication_policy {
            request = request.with_deduplication_policy(policy.clone());
        }
        Ok(request)
    }
}

impl fmt::Debug for HttpRequestOptions {
    /// 仅显示预算、策略与 header 统计，不输出任何 header 值。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequestOptions")
            .field("headers", &self.headers)
            .field("timeout", &self.timeout)
            .field("retry_policy", &self.retry_policy)
            .field("deduplication_policy", &self.deduplication_policy)
            .finish()
    }
}
