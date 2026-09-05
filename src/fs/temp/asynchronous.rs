use std::{io, path::Path};

use async_tempfile::{
    Error as AsyncTempError, TempDir as BackendTempDir, TempFile as BackendTempFile,
};
#[cfg(feature = "fs-temp-async")]
use tokio::runtime::Handle;

use super::{FsTempConfig, FsTempError};

#[cfg(feature = "fs-temp-async")]
/// Tokio 异步拥有型命名临时文件包装。
///
/// 显式调用 [`FsAsyncTempFile::drop_async`] 时使用后端的异步删除路径并尽力清理；后端
/// 不返回删除错误。包装对象被隐式 `Drop`、或清理 future 被取消时，后端仍会使用同步
/// `Drop` 作为后备，因此异步任务应优先完整等待 `drop_async`，不要把取消路径当成非阻塞
/// 或绝对成功的清理保证。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "fs-temp-async")]
/// use axutils::{fs::FsTempError, utils::FsUtils};
///
/// async fn example() -> Result<(), FsTempError> {
///     let file = FsUtils::create_temp_file_async().await?;
///     file.drop_async().await;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsAsyncTempFile {
    inner: BackendTempFile,
}

#[cfg(feature = "fs-temp-async")]
impl FsAsyncTempFile {
    /// 返回当前临时文件路径。
    ///
    /// 路径只在包装对象仍然存活且仍拥有临时文件时有效。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file_async().await?;
    ///     let _path = file.path().to_path_buf();
    ///     file.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.file_path().as_path()
    }

    /// 在调用方 runtime 中异步删除临时文件。
    ///
    /// 后端方法不返回删除错误；正常完成时使用调用方 runtime 的异步删除。future 被取消或
    /// panic 时，后端的同步 `Drop` 仍会尝试删除，因此该后备路径可能在 runtime worker 上
    /// 执行同步文件系统调用；调用方应优先完整等待本方法，并且不能把取消视为清理成功。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file_async().await?;
    ///     file.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn drop_async(self) {
        self.inner.drop_async().await;
    }

    /// 同步关闭并删除临时文件，保留删除错误。
    ///
    /// 该方法可能阻塞当前线程；异步上下文优先使用 [`Self::drop_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file_async().await?;
    ///     file.close()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn close(self) -> Result<(), FsTempError> {
        let path = self.inner.file_path().to_path_buf();
        self.inner.close().map_err(|error| FsTempError::Close {
            operation: "close_temp_file_async",
            path,
            kind: error.kind(),
        })
    }
}

#[cfg(feature = "fs-temp-async")]
/// Tokio 异步拥有型命名临时目录包装。
///
/// `drop_async` 正常完成时使用后端异步递归删除；隐式 `Drop` 或取消路径使用后端同步
/// 删除作为后备，可能在 runtime worker 上执行同步递归删除。需要观察删除错误时使用
/// 可能阻塞当前线程的 `close`。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "fs-temp-async")]
/// use axutils::{fs::FsTempError, utils::FsUtils};
///
/// async fn example() -> Result<(), FsTempError> {
///     let directory = FsUtils::create_temp_dir_async().await?;
///     directory.drop_async().await;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsAsyncTempDir {
    inner: BackendTempDir,
}

#[cfg(feature = "fs-temp-async")]
impl FsAsyncTempDir {
    /// 返回当前临时目录路径。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir_async().await?;
    ///     let _path = directory.path().to_path_buf();
    ///     directory.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.dir_path().as_path()
    }

    /// 在调用方 runtime 中异步递归删除临时目录。
    ///
    /// 后端方法不返回删除错误；正常完成时使用调用方 runtime 的异步递归删除。future 被
    /// 取消或 panic 时，后端同步 `Drop` 仍会尝试递归删除，可能在 runtime worker 上执行
    /// 同步文件系统调用；调用方不能把取消视为清理成功。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir_async().await?;
    ///     directory.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn drop_async(self) {
        self.inner.drop_async().await;
    }

    /// 同步关闭并递归删除临时目录，保留删除错误。
    ///
    /// 该方法可能阻塞当前线程；异步上下文优先使用 [`Self::drop_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir_async().await?;
    ///     directory.close()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn close(self) -> Result<(), FsTempError> {
        let path = self.inner.dir_path().to_path_buf();
        self.inner.close().map_err(|error| FsTempError::Close {
            operation: "close_temp_dir_async",
            path,
            kind: error.kind(),
        })
    }
}

#[cfg(feature = "fs-temp-async")]
fn ensure_runtime() -> Result<(), FsTempError> {
    Handle::try_current()
        .map(|_| ())
        .map_err(|_| FsTempError::RuntimeRequired)
}

#[cfg(feature = "fs-temp-async")]
fn async_error_kind(error: AsyncTempError) -> io::ErrorKind {
    match error {
        AsyncTempError::InvalidDirectory => io::ErrorKind::NotADirectory,
        AsyncTempError::InvalidFile => io::ErrorKind::InvalidInput,
        AsyncTempError::InvalidAffix => io::ErrorKind::InvalidInput,
        AsyncTempError::Io(error) => error.kind(),
    }
}

#[cfg(feature = "fs-temp-async")]
pub(crate) async fn create_temp_file_async(
    config: FsTempConfig,
) -> Result<FsAsyncTempFile, FsTempError> {
    const OPERATION: &str = "create_temp_file_async";
    config.validate_affixes()?;
    ensure_runtime()?;
    config.validate_directory_async(OPERATION).await?;

    let directory = config.backend_directory();
    let mut builder = BackendTempFile::builder();
    if let Some(prefix) = config.prefix {
        builder = builder.prefix(prefix);
    }
    if let Some(suffix) = config.suffix {
        builder = builder.suffix(suffix);
    }
    if config.directory.is_some() {
        builder = builder.dir(directory.clone());
    }
    builder
        .create()
        .await
        .map(|inner| FsAsyncTempFile { inner })
        .map_err(|error| FsTempError::Create {
            operation: OPERATION,
            path: directory,
            kind: async_error_kind(error),
        })
}

#[cfg(feature = "fs-temp-async")]
pub(crate) async fn create_temp_dir_async(
    config: FsTempConfig,
) -> Result<FsAsyncTempDir, FsTempError> {
    const OPERATION: &str = "create_temp_dir_async";
    config.validate_affixes()?;
    ensure_runtime()?;
    config.validate_directory_async(OPERATION).await?;

    let directory = config.backend_directory();
    let mut builder = BackendTempDir::builder();
    if let Some(prefix) = config.prefix {
        builder = builder.prefix(prefix);
    }
    if let Some(suffix) = config.suffix {
        builder = builder.suffix(suffix);
    }
    if config.directory.is_some() {
        builder = builder.dir(directory.clone());
    }
    builder
        .create()
        .await
        .map(|inner| FsAsyncTempDir { inner })
        .map_err(|error| FsTempError::Create {
            operation: OPERATION,
            path: directory,
            kind: async_error_kind(error),
        })
}
