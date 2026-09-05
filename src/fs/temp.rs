//! 可选的同步/异步 RAII 临时文件能力。

#[cfg(feature = "fs-temp-async")]
mod asynchronous;
mod config;
#[cfg(feature = "fs-temp")]
mod sync;

pub(crate) use config::context;
pub use config::{FsTempConfig, FsTempError, FsUtilsContext};

#[cfg(feature = "fs-temp")]
pub(crate) use sync::{create_temp_dir, create_temp_file};
#[cfg(feature = "fs-temp")]
pub use sync::{FsTempDir, FsTempFile};

#[cfg(feature = "fs-temp-async")]
pub(crate) use asynchronous::{create_temp_dir_async, create_temp_file_async};
#[cfg(feature = "fs-temp-async")]
pub use asynchronous::{FsAsyncTempDir, FsAsyncTempFile};
