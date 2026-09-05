//! 显式 Tokio runtime、任务与关闭信号工具。
mod config;
mod error;
pub(crate) mod facade;
mod shutdown;
#[cfg(feature = "task-group")]
mod tasks;
pub use config::{TokioConfig, TokioRuntimeFlavor};
pub use error::TokioError;
pub use shutdown::{wait_for_shutdown, TokioShutdownReason};
#[cfg(feature = "task-group")]
pub use tasks::TokioTaskGroup;
