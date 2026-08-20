//! 显式 Tokio runtime、任务与关闭信号工具。
mod config;
mod error;
mod shutdown;
#[cfg(feature = "tokio-util")]
mod tasks;
pub use config::{TokioConfig, TokioRuntimeFlavor};
pub use error::TokioError;
pub use shutdown::{wait_for_shutdown, TokioShutdownReason};
#[cfg(feature = "tokio-util")]
pub use tasks::TokioTaskGroup;
