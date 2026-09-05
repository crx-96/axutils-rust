//! 受限的 SMTP 邮件发送能力。
//!
//! 该模块只在启用 `email` feature 后导出。同步发送可单独使用 `email`；异步发送方法还
//! 要求启用 `email-async` feature。构造客户端不要求 Tokio runtime；首次异步发送必须在调用
//! 方已有的 Tokio runtime 中执行，且本模块不会替调用方创建 runtime。

mod client;
mod config;
mod error;
pub(crate) mod global;
mod message;

pub use client::EmailClient;
pub use config::{EmailConfig, EmailSecurity};
pub use error::{EmailError, EmailTransportErrorKind};
pub use message::{EmailBody, EmailMessage};
