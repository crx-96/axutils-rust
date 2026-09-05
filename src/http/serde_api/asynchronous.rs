//! 异步 Serde HTTP 便捷方法。

#[cfg(feature = "http-async")]
use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "http-async")]
use super::super::options::HttpRequestOptions;
#[cfg(feature = "http-async")]
use super::super::{HttpClient, HttpError, HttpMethod};
#[cfg(feature = "http-async")]
use super::shared;

#[cfg(feature = "http-async")]
impl HttpClient {
    /// 异步发送 GET JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.get_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn get_async<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_query_request(HttpMethod::Get, url, query, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 POST JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.post_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn post_async<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_body_request(HttpMethod::Post, url, body, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 DELETE JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.delete_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn delete_async<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_query_request(HttpMethod::Delete, url, query, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 PATCH JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.patch_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn patch_async<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_body_request(HttpMethod::Patch, url, body, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 PUT JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.put_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn put_async<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_body_request(HttpMethod::Put, url, body, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 OPTIONS JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.options_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn options_async<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_query_request(HttpMethod::Options, url, query, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 HEAD JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         client.head_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn head_async<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = shared::build_query_request(HttpMethod::Head, url, query, options, true)?;
        shared::decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 GET 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.get_bytes_async("https://example.com/image", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn get_bytes_async<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_query_request(HttpMethod::Get, url, query, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 POST JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.post_bytes_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn post_bytes_async<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_body_request(HttpMethod::Post, url, body, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 DELETE 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.delete_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn delete_bytes_async<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_query_request(HttpMethod::Delete, url, query, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 PATCH JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.patch_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn patch_bytes_async<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_body_request(HttpMethod::Patch, url, body, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 PUT JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.put_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn put_bytes_async<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_body_request(HttpMethod::Put, url, body, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 OPTIONS 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.options_bytes_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn options_bytes_async<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_query_request(HttpMethod::Options, url, query, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 HEAD 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// use axutils::http::{HttpClient, HttpConfig, HttpError};
    /// #[tokio::main]
    /// async fn main() -> Result<(), HttpError> {
    ///     let client = HttpClient::new(HttpConfig::default())?;
    ///     let _ = client.head_bytes_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn head_bytes_async<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = shared::build_query_request(HttpMethod::Head, url, query, options, false)?;
        shared::decode_bytes(self.execute_async(request).await?)
    }
}
