//! 同步 Serde HTTP 便捷方法。

use serde::{de::DeserializeOwned, Serialize};

use super::super::options::HttpRequestOptions;
use super::super::{HttpClient, HttpError, HttpMethod};
use super::shared;

impl HttpClient {
    /// 发送 GET JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.get("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn get<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Get, url, query, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 POST JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.post("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Post, url, body, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 DELETE JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.delete("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn delete<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Delete, url, query, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 PATCH JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.patch("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn patch<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Patch, url, body, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 PUT JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.put("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Put, url, body, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 OPTIONS JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.options("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn options<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Options, url, query, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 HEAD 并尝试按 JSON 解码；query 会被编码并追加到 URL。
    ///
    /// 为保持现有 API，本方法仍要求 JSON 响应体。合规的 HEAD 响应没有正文，因此会返回
    /// [`HttpError::JsonDeserialize`]；需要状态码或响应头时使用 [`Self::execute`] 和
    /// [`HttpMethod::Head`]，只需要空响应体时使用 [`Self::head_bytes`]。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let result = client.head::<std::collections::BTreeMap<String, bool>, _>(
    ///     "https://example.com/health", None::<()>, None,
    /// );
    /// assert!(matches!(result, Err(HttpError::JsonDeserialize)));
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn head<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Head, url, query, options, true)?;
        shared::decode_json(self.execute(request)?)
    }

    /// 发送 GET 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.get_bytes("https://example.com/image", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn get_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Get, url, query, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 POST JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.post_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn post_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Post, url, body, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 DELETE 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.delete_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn delete_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Delete, url, query, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 PATCH JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.patch_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn patch_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Patch, url, body, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 PUT JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.put_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn put_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先序列化 JSON 正文并校验请求选项，再进入客户端的统一执行策略。
        let request = shared::build_body_request(HttpMethod::Put, url, body, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 OPTIONS 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.options_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn options_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Options, url, query, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }

    /// 发送 HEAD 请求；合规的响应正文为空，返回空字节向量。
    ///
    /// 需要状态码或响应头时使用 [`Self::execute`] 和 [`HttpMethod::Head`]。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// let client = HttpClient::new(HttpConfig::default())?;
    /// let _ = client.head_bytes("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), HttpError>(())
    /// ~~~
    pub fn head_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        // 先编码 query 并应用单次选项，再进入客户端的统一执行策略。
        let request = shared::build_query_request(HttpMethod::Head, url, query, options, false)?;
        shared::decode_bytes(self.execute(request)?)
    }
}
