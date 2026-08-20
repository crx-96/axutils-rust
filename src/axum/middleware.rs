//! Axum middleware 的 provider 适配。
//!
//! 此模块为 crate 内部组装层，不作为公共模块导出。`tower-http` 只编译对应的通用 HTTP
//! middleware；`tower_governor` 只编译限流适配。HTTP trace 还在 `tower-http` 适配内部精确要求
//! `tracing`，不会因单独启用 `tracing` 或 `tower-http` 而出现。

#[cfg(feature = "tower-http")]
mod tower_http_support;
#[cfg(feature = "tower-http")]
pub use tower_http_support::*;

#[cfg(feature = "tower_governor")]
mod governor_support;
