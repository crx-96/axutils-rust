//! 本地文件系统 I/O 领域模块。
//!
//! [`FsError`] 和 [`crate::utils::FsUtils`] 提供文件、目录、受限读取、浅层列举、复制、移动和删除
//! 能力。同步入口默认可用并执行阻塞 I/O；带 `_async` 后缀的入口只在启用 `fs-async` feature
//! 时提供，使用调用方已有的 Tokio runtime，不创建 runtime、不调用 `block_on`。
//!
//! `FsChunkProcessor`、`FsTransferOptions`、`FsTransferStats` 和 `FsTransferError` 默认可用；
//! `FsAsyncChunkProcessor` 需要 `fs-async`。`FsTempConfig`、`FsTempError` 和 `FsUtilsContext`
//! 需要至少一个独立的 `fs-temp`/`fs-temp-async` feature；同步 wrapper
//! `FsTempFile`/`FsTempDir` 只在 `fs-temp` 下提供，异步 wrapper
//! `FsAsyncTempFile`/`FsAsyncTempDir` 只在 `fs-temp-async` 下提供。领域模型从
//! `axutils::fs` 导出；`FsUtils` 只从 `axutils::utils` 导出。
//!
//! 本模块直接作用于调用方提供的路径，不提供安全根、canonicalize 沙箱、权限修改或抗 TOCTOU
//! 保证。`remove_dir_all` 是不可回滚的破坏性操作；受限读取和目录列举只限制已经读取/观察到的
//! 数据，不保证 FIFO、设备文件或其他特殊文件不会阻塞。
//!
//! # Examples
//!
//! ```
//! use axutils::fs::FsError;
//!
//! fn is_runtime_required(error: FsError) -> bool {
//!     matches!(error, FsError::RuntimeRequired)
//! }
//!
//! let _ = is_runtime_required;
//! ```

mod error;
pub(crate) mod facade;
pub(crate) mod ops;
#[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
pub(crate) mod temp;
pub(crate) mod transfer;

pub use error::FsError;
pub use transfer::{FsChunkProcessor, FsTransferError, FsTransferOptions, FsTransferStats};

#[cfg(feature = "fs-async")]
pub use transfer::FsAsyncChunkProcessor;

#[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
pub use temp::{FsTempConfig, FsTempError, FsUtilsContext};

#[cfg(feature = "fs-temp")]
pub use temp::{FsTempDir, FsTempFile};

#[cfg(feature = "fs-temp-async")]
pub use temp::{FsAsyncTempDir, FsAsyncTempFile};
