//! HTTP 客户端的一次初始化进程级入口。

use std::sync::OnceLock;

use crate::http::{HttpClient, HttpConfig, HttpError, HttpRequest, HttpResponse};

#[cfg(feature = "serde")]
use crate::http::HttpRequestOptions;

static HTTP_CLIENT: OnceLock<HttpClient> = OnceLock::new();

/// HTTP 全局客户端入口。
pub struct HttpUtils;

impl HttpUtils {
    /// 初始化全局 HTTP 客户端。
    ///
    /// 客户端会在成功构造后才写入 `OnceLock`；配置错误不会消耗初始化机会。
    pub fn init(config: HttpConfig) -> Result<(), HttpError> {
        let client = HttpClient::new(config)?;
        HTTP_CLIENT
            .set(client)
            .map_err(|_| HttpError::AlreadyInitialized)
    }

    /// 返回全局客户端是否已经初始化。
    pub fn is_initialized() -> bool {
        HTTP_CLIENT.get().is_some()
    }

    pub(crate) fn client() -> Result<&'static HttpClient, HttpError> {
        HTTP_CLIENT.get().ok_or(HttpError::NotInitialized)
    }

    /// 使用全局客户端同步执行请求。
    pub fn execute(request: HttpRequest) -> Result<HttpResponse, HttpError> {
        Self::client()?.execute(request)
    }

    /// 使用全局客户端异步执行请求。
    #[cfg(all(feature = "http", feature = "tokio"))]
    pub async fn execute_async(request: HttpRequest) -> Result<HttpResponse, HttpError> {
        Self::client()?.execute_async(request).await
    }
}

#[cfg(feature = "serde")]
impl HttpUtils {
    /// 使用全局客户端发送 GET JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::get("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn get<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.get(url, query, options)
    }

    /// 使用全局客户端发送 POST JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::post("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn post<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.post(url, body, options)
    }

    /// 使用全局客户端发送 DELETE JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::delete("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn delete<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.delete(url, query, options)
    }

    /// 使用全局客户端发送 PATCH JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::patch("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn patch<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.patch(url, body, options)
    }

    /// 使用全局客户端发送 PUT JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::put("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn put<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.put(url, body, options)
    }

    /// 使用全局客户端发送 OPTIONS JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::options("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn options<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.options(url, query, options)
    }

    /// 使用全局客户端发送 HEAD JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _: std::collections::BTreeMap<String, bool> =
    ///     axutils::HttpUtils::head("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn head<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.head(url, query, options)
    }

    /// 使用全局客户端发送 GET 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::get_bytes("https://example.com/image", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn get_bytes<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.get_bytes(url, query, options)
    }

    /// 使用全局客户端发送 POST JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::post_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn post_bytes<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.post_bytes(url, body, options)
    }

    /// 使用全局客户端发送 DELETE 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::delete_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn delete_bytes<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.delete_bytes(url, query, options)
    }

    /// 使用全局客户端发送 PATCH JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::patch_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn patch_bytes<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.patch_bytes(url, body, options)
    }

    /// 使用全局客户端发送 PUT JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::put_bytes("https://example.com/items/42", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn put_bytes<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.put_bytes(url, body, options)
    }

    /// 使用全局客户端发送 OPTIONS 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::options_bytes("https://example.com/items", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn options_bytes<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.options_bytes(url, query, options)
    }

    /// 使用全局客户端发送 HEAD 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// let _ = axutils::HttpUtils::head_bytes("https://example.com/health", None::<()>, None)?;
    /// # Ok::<(), axutils::HttpError>(())
    /// ~~~
    pub fn head_bytes<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.head_bytes(url, query, options)
    }
}

#[cfg(all(feature = "serde", feature = "tokio"))]
impl HttpUtils {
    /// 使用全局客户端异步发送 GET JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::get_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn get_async<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.get_async(url, query, options).await
    }

    /// 使用全局客户端异步发送 POST JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::post_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn post_async<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.post_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 DELETE JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::delete_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn delete_async<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.delete_async(url, query, options).await
    }

    /// 使用全局客户端异步发送 PATCH JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::patch_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn patch_async<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.patch_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 PUT JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::put_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn put_async<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.put_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 OPTIONS JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::options_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn options_async<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.options_async(url, query, options).await
    }

    /// 使用全局客户端异步发送 HEAD JSON 请求。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _: std::collections::BTreeMap<String, bool> =
    ///         axutils::HttpUtils::head_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn head_async<T: serde::de::DeserializeOwned, Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<T, HttpError> {
        Self::client()?.head_async(url, query, options).await
    }

    /// 使用全局客户端异步发送 GET 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::get_bytes_async("https://example.com/image", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn get_bytes_async<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.get_bytes_async(url, query, options).await
    }

    /// 使用全局客户端异步发送 POST JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::post_bytes_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn post_bytes_async<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.post_bytes_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 DELETE 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::delete_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn delete_bytes_async<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?
            .delete_bytes_async(url, query, options)
            .await
    }

    /// 使用全局客户端异步发送 PATCH JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::patch_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn patch_bytes_async<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.patch_bytes_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 PUT JSON 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::put_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn put_bytes_async<B: serde::Serialize>(
        url: impl AsRef<str>,
        body: Option<B>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.put_bytes_async(url, body, options).await
    }

    /// 使用全局客户端异步发送 OPTIONS 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::options_bytes_async("https://example.com/items", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn options_bytes_async<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?
            .options_bytes_async(url, query, options)
            .await
    }

    /// 使用全局客户端异步发送 HEAD 请求并返回字节。
    ///
    /// # Examples
    ///
    /// ~~~rust,no_run
    /// #[tokio::main]
    /// async fn main() -> Result<(), axutils::HttpError> {
    ///     let _ = axutils::HttpUtils::head_bytes_async("https://example.com/health", None::<()>, None).await?;
    ///     Ok(())
    /// }
    /// ~~~
    pub async fn head_bytes_async<Q: serde::Serialize>(
        url: impl AsRef<str>,
        query: Option<Q>,
        options: Option<HttpRequestOptions>,
    ) -> Result<Vec<u8>, HttpError> {
        Self::client()?.head_bytes_async(url, query, options).await
    }
}

impl std::fmt::Debug for HttpUtils {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpUtils")
            .field("initialized", &Self::is_initialized())
            .finish()
    }
}
