//! 基于 Serde 的 JSON 请求与响应便捷方法。

use serde::{de::DeserializeOwned, Serialize};
use url::{form_urlencoded, Url};

use super::options::HttpRequestOptions;
use super::{HttpClient, HttpError, HttpMethod, HttpRequest, HttpResponse};

fn build_query_request<Q: Serialize>(
    method: HttpMethod,
    url: impl AsRef<str>,
    query: Option<Q>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    build_request(
        method,
        append_query(url.as_ref(), query)?,
        None,
        options,
        json_response,
    )
}

fn build_body_request<B: Serialize>(
    method: HttpMethod,
    url: impl AsRef<str>,
    body: Option<B>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    let body = body
        .map(|body| serde_json::to_vec(&body).map_err(|_| HttpError::JsonSerialize))
        .transpose()?;
    build_request(
        method,
        url.as_ref().to_owned(),
        body,
        options,
        json_response,
    )
}

fn build_request(
    method: HttpMethod,
    url: String,
    body: Option<Vec<u8>>,
    options: Option<HttpRequestOptions>,
    json_response: bool,
) -> Result<HttpRequest, HttpError> {
    let has_body = body.is_some();
    let mut request = HttpRequest::new(method, url)?;
    if let Some(body) = body {
        request = request.with_body(body)?;
    }
    if let Some(options) = options {
        request = options.apply_to_request(request)?;
    }
    if has_body && !request.headers().contains("content-type") {
        request = request.with_header("content-type", "application/json")?;
    }
    if json_response && !request.headers().contains("accept") {
        request = request.with_header("accept", "application/json")?;
    }
    Ok(request)
}

fn append_query<Q: Serialize>(url: &str, query: Option<Q>) -> Result<String, HttpError> {
    let Some(query) = query else {
        return Ok(url.to_owned());
    };
    let encoded = serde_urlencoded::to_string(&query).map_err(|_| HttpError::QuerySerialize)?;
    if encoded.is_empty() {
        return Ok(url.to_owned());
    }

    if let Ok(mut parsed) = Url::parse(url) {
        {
            let mut pairs = parsed.query_pairs_mut();
            for (key, value) in form_urlencoded::parse(encoded.as_bytes()) {
                pairs.append_pair(&key, &value);
            }
        }
        return Ok(parsed.into());
    }

    if url.contains('#') {
        return Err(HttpError::InvalidUrl);
    }
    let separator = if url.contains('?') {
        if url.ends_with('?') || url.ends_with('&') {
            ""
        } else {
            "&"
        }
    } else {
        "?"
    };
    Ok(format!("{url}{separator}{encoded}"))
}

fn decode_json<T: DeserializeOwned>(response: HttpResponse) -> Result<T, HttpError> {
    response.json()
}

fn decode_bytes(response: HttpResponse) -> Result<Vec<u8>, HttpError> {
    Ok(response.into_body())
}

impl HttpClient {
    /// 发送 GET JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.get("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn get<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_query_request(HttpMethod::Get, url, query, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 POST JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.post("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_body_request(HttpMethod::Post, url, body, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 DELETE JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.delete("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn delete<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_query_request(HttpMethod::Delete, url, query, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 PATCH JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.patch("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn patch<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_body_request(HttpMethod::Patch, url, body, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 PUT JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.put("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_body_request(HttpMethod::Put, url, body, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 OPTIONS JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.options("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn options<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_query_request(HttpMethod::Options, url, query, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 HEAD JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     client.head("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn head<T: DeserializeOwned, Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        let request = build_query_request(HttpMethod::Head, url, query, options, true)?;
        decode_json(self.execute(request)?)
    }

    /// 发送 GET 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.get_bytes("https://example.com/image", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn get_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_query_request(HttpMethod::Get, url, query, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 POST JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.post_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn post_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_body_request(HttpMethod::Post, url, body, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 DELETE 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.delete_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn delete_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_query_request(HttpMethod::Delete, url, query, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 PATCH JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.patch_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn patch_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_body_request(HttpMethod::Patch, url, body, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 PUT JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.put_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn put_bytes<B: Serialize>(
        &self,
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_body_request(HttpMethod::Put, url, body, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 OPTIONS 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.options_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn options_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_query_request(HttpMethod::Options, url, query, options, false)?;
        decode_bytes(self.execute(request)?)
    }

    /// 发送 HEAD 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    /// let _ = client.head_bytes("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn head_bytes<Q: Serialize>(
        &self,
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        let request = build_query_request(HttpMethod::Head, url, query, options, false)?;
        decode_bytes(self.execute(request)?)
    }
}

#[cfg(feature = "tokio")]
impl HttpClient {
    /// 异步发送 GET JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Get, url, query, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 POST JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Post, url, body, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 DELETE JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Delete, url, query, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 PATCH JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Patch, url, body, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 PUT JSON 请求；body 会被序列化为 JSON。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Put, url, body, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 OPTIONS JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Options, url, query, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 HEAD JSON 请求；query 会被编码并追加到 URL。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Head, url, query, options, true)?;
        decode_json(self.execute_async(request).await?)
    }

    /// 异步发送 GET 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Get, url, query, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 POST JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Post, url, body, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 DELETE 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Delete, url, query, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 PATCH JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Patch, url, body, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 PUT JSON 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_body_request(HttpMethod::Put, url, body, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 OPTIONS 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Options, url, query, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }

    /// 异步发送 HEAD 请求并返回原始响应体字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
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
        let request = build_query_request(HttpMethod::Head, url, query, options, false)?;
        decode_bytes(self.execute_async(request).await?)
    }
}
