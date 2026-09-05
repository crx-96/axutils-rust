//! Axum HTTP/1 服务组装、单次运行状态与协作式关闭。
//!
//! 本模块只在 crate 启用 `axum` feature 时以 `axutils::axum` 公开；基础 API 负责在内存中组装路由和配置，并由调用方提供
//! Tokio runtime；只有 `AxumServer` 的 `serve*` 入口会绑定或使用 TCP listener 并产生网络 I/O。
//! 服务只支持 HTTP/1，不提供 TLS、HTTP/2、强制 drain deadline 或可信代理 CIDR 验证。
//! 配置错误和服务状态错误以 `AxumError` 返回，不会把请求内容或 provider 原始错误写入错误文本。
//!
//! Middleware 按能力 feature 精确开放：`axum-tower` 提供并发限制，`axum-tower-http` 提供 CORS、
//! request ID、service timeout、请求体限制和 panic 捕获，`axum-tower-http + tracing` 提供 HTTP trace，
//! `axum-governor` 提供按 peer IP 或未经验证的转发 header 限流。构造 builder 或安装 layer
//! 均不访问网络；实际请求处理会修改响应、记录事件或维护 provider 内部限流状态。
//!
//! # Examples
//!
//! ```rust,no_run
//! # use axutils::axum::*;
//! use axutils::axum::AxumApp;
//!
//! let server = AxumApp::new().into_server_builder().build()?;
//! assert_eq!(server.config().max_body_bytes(), 1024 * 1024);
//! # Ok::<(), AxumError>(())
//! ```

mod app;
mod config;
mod error;
pub(crate) mod global;
mod middleware;
mod server;
mod shutdown;

pub use app::AxumApp;
pub use config::AxumConfig;
pub use error::AxumError;
#[cfg(feature = "axum-tower-http")]
pub use middleware::{AxumCorsConfig, AxumCorsOrigin, AxumTimeoutStatus};
pub use server::{AxumServer, AxumServerBuilder};
pub use shutdown::{AxumServeOutcome, AxumShutdownHandle, AxumShutdownReason};
