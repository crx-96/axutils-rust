use std::path::Path;

use tempfile::Builder as TempBuilder;

use super::{FsTempConfig, FsTempError};

#[cfg(feature = "fs-temp")]
/// 同步拥有型命名临时文件包装。
///
/// 包装对象持有底层文件句柄和清理所有权；离开作用域时会尝试删除临时文件。需要观察
/// 删除错误时调用 [`FsTempFile::close`]，而不是只依赖 `Drop`。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "fs-temp")]
/// use axutils::{fs::FsTempError, utils::FsUtils};
///
/// fn example() -> Result<(), FsTempError> {
///     let file = FsUtils::create_temp_file()?;
///     let path = file.path().to_path_buf();
///     file.close()?;
///     assert!(!path.exists());
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsTempFile {
    inner: tempfile::NamedTempFile,
}

#[cfg(feature = "fs-temp")]
impl FsTempFile {
    /// 返回当前临时文件路径。
    ///
    /// 返回的路径只在包装对象仍持有所有权时有效；对象被关闭或析构后不应继续使用。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file()?;
    ///     let _path = file.path().to_path_buf();
    ///     file.close()?;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// 返回由包装对象持有的同步文件句柄。
    ///
    /// 句柄的生命周期受 `FsTempFile` 约束；该方法不会转移句柄所有权。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file()?;
    ///     let _metadata = file.as_file().metadata();
    ///     file.close()?;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn as_file(&self) -> &std::fs::File {
        self.inner.as_file()
    }

    /// 返回由包装对象持有的可变同步文件句柄。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///     use std::io::Write;
    ///
    ///     use axutils::utils::FsUtils;
    ///
    ///     let mut file = FsUtils::create_temp_file()?;
    ///     file.as_file_mut().write_all(b"temporary")?;
    ///     file.close()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn as_file_mut(&mut self) -> &mut std::fs::File {
        self.inner.as_file_mut()
    }

    /// 关闭句柄并立即删除临时文件，保留删除错误。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file()?;
    ///     let path = file.path().to_path_buf();
    ///     file.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn close(self) -> Result<(), FsTempError> {
        let path = self.inner.path().to_path_buf();
        self.inner.close().map_err(|error| FsTempError::Close {
            operation: "close_temp_file",
            path,
            kind: error.kind(),
        })
    }
}

#[cfg(feature = "fs-temp")]
/// 同步拥有型命名临时目录包装。
///
/// 包装对象拥有临时目录及其内容的清理责任；`close` 会递归删除临时目录自身，不会删除
/// `FsTempConfig::directory` 指定的父目录。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "fs-temp")]
/// use axutils::{fs::FsTempError, utils::FsUtils};
///
/// fn example() -> Result<(), FsTempError> {
///     let directory = FsUtils::create_temp_dir()?;
///     let path = directory.path().to_path_buf();
///     directory.close()?;
///     assert!(!path.exists());
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsTempDir {
    inner: tempfile::TempDir,
}

#[cfg(feature = "fs-temp")]
impl FsTempDir {
    /// 返回当前临时目录路径。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir()?;
    ///     let _path = directory.path().to_path_buf();
    ///     directory.close()?;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// 关闭并递归删除临时目录，保留删除错误。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir()?;
    ///     let path = directory.path().to_path_buf();
    ///     directory.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn close(self) -> Result<(), FsTempError> {
        let path = self.inner.path().to_path_buf();
        self.inner.close().map_err(|error| FsTempError::Close {
            operation: "close_temp_dir",
            path,
            kind: error.kind(),
        })
    }
}

#[cfg(feature = "fs-temp")]
pub(crate) fn create_temp_file(config: &FsTempConfig) -> Result<FsTempFile, FsTempError> {
    const OPERATION: &str = "create_temp_file";
    config.validate_affixes()?;
    config.validate_directory(OPERATION)?;

    let mut builder = TempBuilder::new();
    if let Some(prefix) = config.prefix.as_deref() {
        builder.prefix(prefix);
    }
    if let Some(suffix) = config.suffix.as_deref() {
        builder.suffix(suffix);
    }

    let directory = config.backend_directory();
    let result = match config.directory.as_deref() {
        Some(directory) => builder.tempfile_in(directory),
        None => builder.tempfile(),
    };
    result
        .map(|inner| FsTempFile { inner })
        .map_err(|error| FsTempError::Create {
            operation: OPERATION,
            path: directory,
            kind: error.kind(),
        })
}

#[cfg(feature = "fs-temp")]
pub(crate) fn create_temp_dir(config: &FsTempConfig) -> Result<FsTempDir, FsTempError> {
    const OPERATION: &str = "create_temp_dir";
    config.validate_affixes()?;
    config.validate_directory(OPERATION)?;

    let mut builder = TempBuilder::new();
    if let Some(prefix) = config.prefix.as_deref() {
        builder.prefix(prefix);
    }
    if let Some(suffix) = config.suffix.as_deref() {
        builder.suffix(suffix);
    }

    let directory = config.backend_directory();
    let result = match config.directory.as_deref() {
        Some(directory) => builder.tempdir_in(directory),
        None => builder.tempdir(),
    };
    result
        .map(|inner| FsTempDir { inner })
        .map_err(|error| FsTempError::Create {
            operation: OPERATION,
            path: directory,
            kind: error.kind(),
        })
}
