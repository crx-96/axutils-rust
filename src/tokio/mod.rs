//! 显式 Tokio runtime、任务与关闭信号工具。
mod config;
mod error;
pub(crate) mod facade;
mod shutdown;
mod task_guard;
#[cfg(feature = "task-group")]
mod tasks;
pub use config::{TokioConfig, TokioRuntimeFlavor};
pub use error::TokioError;
pub use shutdown::{wait_for_shutdown, TokioShutdownReason};
pub use task_guard::TokioTaskGuard;
#[cfg(feature = "task-group")]
pub use tasks::TokioTaskGroup;
