//! HTTP 响应类型。

use std::fmt;
use std::sync::Arc;

use super::headers::HttpHeaders;
use super::HttpError;

/// 已读取并受大小限制的 HTTP 响应。
#[derive(Clone)]
pub struct HttpResponse {
    status: u16,
    headers: HttpHeaders,
    body: Arc<[u8]>,
    attempts: u32,
}

impl HttpResponse {
    pub(crate) fn new(status: u16, headers: HttpHeaders, body: Vec<u8>, attempts: u32) -> Self {
        Self {
            status,
            headers,
            body: Arc::from(body),
            attempts,
        }
    }

    /// 返回 HTTP 状态码。
    pub fn status(&self) -> u16 {
        self.status
    }

    /// 返回状态码是否处于 2xx 成功范围。
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// 返回响应 Header。
    pub fn headers(&self) -> &HttpHeaders {
        &self.headers
    }

    /// 返回第一个同名响应 Header。
    pub fn header(&self, name: impl AsRef<[u8]>) -> Option<&[u8]> {
        self.headers.get(name)
    }

    /// 返回响应体字节。
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// 消费响应并返回拥有型响应体字节。
    ///
    /// 该方法适合字节快捷 API；响应头、状态码和尝试次数在消费后不再保留。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "http")]
    /// # fn main() -> Result<(), axutils::HttpError> {
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let response = client.execute(axutils::HttpRequest::new(
    ///     axutils::HttpMethod::Get,
    ///     "https://example.com/bytes",
    /// )?)?;
    /// let bytes: Vec<u8> = response.into_body();
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "http"))]
    /// # fn main() {}
    /// ```
    pub fn into_body(self) -> Vec<u8> {
        self.body.to_vec()
    }

    /// 将响应体按严格 UTF-8 解码。
    pub fn text(&self) -> Result<&str, HttpError> {
        std::str::from_utf8(&self.body).map_err(|_| HttpError::InvalidUtf8)
    }

    /// 返回实际网络尝试次数。
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// 将响应体按 JSON 反序列化为调用方类型。
    ///
    /// 该方法需要同时启用 `http` 与 `serde` feature；解析失败只返回稳定的
    /// [`HttpError::JsonDeserialize`]，不会暴露 Serde 的原始错误文本。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(all(feature = "http", feature = "serde"))]
    /// # fn main() -> Result<(), axutils::HttpError> {
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Health {
    ///     ok: bool,
    /// }
    ///
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let response = client.execute(axutils::HttpRequest::new(
    ///     axutils::HttpMethod::Get,
    ///     "https://example.com/health",
    /// )?)?;
    /// let health: Health = response.json()?;
    /// assert!(health.ok);
    /// # Ok(())
    /// # }
    /// # #[cfg(not(all(feature = "http", feature = "serde")))]
    /// # fn main() {}
    /// ```
    #[cfg(feature = "serde")]
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, HttpError> {
        serde_json::from_slice(&self.body).map_err(|_| HttpError::JsonDeserialize)
    }
}

impl fmt::Debug for HttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body_len", &self.body.len())
            .field("attempts", &self.attempts)
            .finish()
    }
}
