//! 受限、可配置的 HTTP 客户端。
//!
//! `http` feature 提供同步 API；同时启用 `tokio` feature 时追加异步 API。客户端默认关闭
//! 系统代理、自动重定向、压缩和隐式重试，并对 URL、Header、请求体和响应体实施大小限制。
//! 配置 builder 的字段均可省略；未设置 `base_url` 时只接受绝对 HTTP/HTTPS URL，配置的基
//! 地址不会覆盖请求自身的绝对 URL。默认总超时为 30 秒、连接超时为 10 秒，重试策略默认
//! 最多进行 3 次网络尝试（包括首次请求），设置为 1 可禁用自动重试。

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
