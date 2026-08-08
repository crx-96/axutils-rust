//! 受限、可配置的 HTTP 客户端。
//!
//! `http` feature 提供同步 API；同时启用 `tokio` feature 时追加异步 API。客户端默认关闭
//! 系统代理、自动重定向、压缩和隐式重试，并对 URL、Header、请求体和响应体实施大小限制。

mod client;
mod coalesce;
mod config;
mod error;
mod headers;
mod options;
mod request;
mod response;
mod retry;

#[cfg(feature = "serde")]
mod serde_api;

pub use client::HttpClient;
pub use config::{DeduplicationMode, DeduplicationPolicy, HttpConfig, HttpConfigBuilder};
pub use error::{HttpError, HttpTransportErrorKind};
pub use headers::HttpHeaders;
pub use options::HttpRequestOptions;
pub use request::{HttpMethod, HttpRequest, HttpRequestBuilder};
pub use response::HttpResponse;
pub use retry::RetryPolicy;
