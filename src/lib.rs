//! `axutils` 是一个默认零第三方依赖、按能力 feature 启用的 Rust 工具库。
//!
//! 公共 API 以领域模块为规范入口：客户端、配置、错误与模型位于
//! `axutils::<domain>`，工具 façade 位于 `axutils::utils`。crate 根不重导出领域类型，
//! 以便导入路径始终保留来源。
//!
//! ```
//! use axutils::{
//!     fs::FsError,
//!     utils::{FsUtils, PathUtils},
//! };
//!
//! let path = PathUtils::join(["tmp", "example"]);
//! let _ = (path, FsUtils::try_exists("."), Option::<FsError>::None);
//! ```
//!
//! 默认 feature 为空。异步、网络、数据库、配置、模板和加密后端均通过各自的能力
//! feature 显式启用；启用通用 `tokio` 只提供 Tokio 工具，不会隐式开放其他领域的异步 API。

#[cfg(feature = "tracing")]
mod telemetry;

pub mod convert;
pub mod crypto;
pub mod fs;
pub mod time;
pub mod utils;

#[cfg(feature = "config")]
pub mod config;

#[cfg(feature = "email")]
pub mod email;

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "jwt")]
pub mod jwt;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(any(
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
pub mod sqlx;

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "scheduler")]
pub mod scheduler;

#[cfg(feature = "axum")]
pub mod axum;

#[cfg(feature = "logging")]
pub mod logging;
