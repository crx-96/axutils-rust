//! 可选的同步/异步 RAII 临时文件能力。

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

#[cfg(feature = "tempfile-async")]
use std::future::Future;

/// 临时文件/目录创建配置。
///
/// 配置本身只保存拥有型数据，不在构造时访问文件系统、创建对象、访问网络或修改进程级
/// 临时目录。指定的 `directory` 必须在创建时已经存在；本类型不会替调用方创建配置目录。
/// `prefix` 和 `suffix` 只接受单个文件名片段，不能包含 `/`、`\` 或 NUL；空白字符允许，
/// 但不会被裁剪。
///
/// # Examples
///
/// ```
/// use axutils::FsTempConfig;
///
/// let config = FsTempConfig::new()
///     .with_prefix("axutils-")
///     .with_suffix(".tmp");
/// assert_eq!(config.prefix.as_deref(), Some("axutils-"));
/// assert_eq!(config.suffix.as_deref(), Some(".tmp"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FsTempConfig {
    /// 临时对象的父目录；为 `None` 时使用后端的系统临时目录。
    pub directory: Option<PathBuf>,
    /// 临时对象名的前缀。
    pub prefix: Option<String>,
    /// 临时对象名的后缀。
    pub suffix: Option<String>,
}

impl FsTempConfig {
    /// 创建使用后端默认目录、命名规则和自动清理的配置。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FsTempConfig;
    ///
    /// let config = FsTempConfig::new();
    /// assert!(config.directory.is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置已存在的临时对象父目录。
    ///
    /// 创建临时对象时会检查该目录；此方法本身不访问文件系统，也不会创建目录。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FsTempConfig;
    ///
    /// let config = FsTempConfig::new().with_directory("fixtures");
    /// assert_eq!(config.directory.as_deref().and_then(|path| path.to_str()), Some("fixtures"));
    /// ```
    #[must_use]
    pub fn with_directory<P: Into<PathBuf>>(mut self, directory: P) -> Self {
        self.directory = Some(directory.into());
        self
    }

    /// 设置文件名/目录名前缀。
    ///
    /// 路径分隔符和 NUL 会在创建时被拒绝；该 builder 不执行校验或 I/O。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FsTempConfig;
    ///
    /// let config = FsTempConfig::new().with_prefix("upload-");
    /// assert_eq!(config.prefix.as_deref(), Some("upload-"));
    /// ```
    #[must_use]
    pub fn with_prefix<S: Into<String>>(mut self, prefix: S) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// 设置文件名/目录名后缀。
    ///
    /// 路径分隔符和 NUL 会在创建时被拒绝；该 builder 不执行校验或 I/O。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FsTempConfig;
    ///
    /// let config = FsTempConfig::new().with_suffix(".part");
    /// assert_eq!(config.suffix.as_deref(), Some(".part"));
    /// ```
    #[must_use]
    pub fn with_suffix<S: Into<String>>(mut self, suffix: S) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    fn validate_affixes(&self) -> Result<(), FsTempError> {
        for (field, value) in [
            ("prefix", self.prefix.as_deref()),
            ("suffix", self.suffix.as_deref()),
        ] {
            if value.is_some_and(|value| {
                value
                    .chars()
                    .any(|character| character == '\0' || character == '/' || character == '\\')
            }) {
                return Err(FsTempError::InvalidConfig { field });
            }
        }
        Ok(())
    }

    fn backend_directory(&self) -> PathBuf {
        self.directory.clone().unwrap_or_else(std::env::temp_dir)
    }

    #[cfg(feature = "tempfile")]
    fn validate_directory(&self, operation: &'static str) -> Result<(), FsTempError> {
        let Some(directory) = self.directory.as_deref() else {
            return Ok(());
        };
        match std::fs::metadata(directory) {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => Err(FsTempError::Create {
                operation,
                path: directory.to_path_buf(),
                kind: io::ErrorKind::NotADirectory,
            }),
            Err(error) => Err(FsTempError::Create {
                operation,
                path: directory.to_path_buf(),
                kind: error.kind(),
            }),
        }
    }

    #[cfg(feature = "tempfile-async")]
    async fn validate_directory_async(&self, operation: &'static str) -> Result<(), FsTempError> {
        let Some(directory) = self.directory.as_deref() else {
            return Ok(());
        };
        match tokio::fs::metadata(directory).await {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => Err(FsTempError::Create {
                operation,
                path: directory.to_path_buf(),
                kind: io::ErrorKind::NotADirectory,
            }),
            Err(error) => Err(FsTempError::Create {
                operation,
                path: directory.to_path_buf(),
                kind: error.kind(),
            }),
        }
    }
}

/// 临时文件能力的脱敏错误分类。
///
/// 后端错误不会直接出现在公共签名中；这里只保留稳定操作 token、路径和 `io::ErrorKind`。
/// 析构期间的清理错误无法返回给调用方；需要观察清理结果时使用包装对象的 `close`。
/// `Access` 和 `Cleanup` 是为后续句柄/错误报告能力保留的分类，当前第一版不会构造它们；
/// 当前可观察的创建、关闭、配置和 runtime 错误分别使用 `Create`、`Close`、`InvalidConfig`
/// 和 `RuntimeRequired`。
///
/// # Examples
///
/// ```
/// use axutils::FsTempError;
///
/// fn category(error: &FsTempError) -> &'static str {
///     match error {
///         FsTempError::InvalidConfig { .. } => "config",
///         FsTempError::Create { .. } => "create",
///         FsTempError::Close { .. } => "close",
///         FsTempError::RuntimeRequired => "runtime",
///         FsTempError::Access { .. } | FsTempError::Cleanup { .. } => "reserved",
///         _ => "future-error-variant",
///     }
/// }
///
/// assert_eq!(category(&FsTempError::RuntimeRequired), "runtime");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FsTempError {
    /// 配置字段不符合本 crate 的命名约束。
    InvalidConfig {
        /// 无效字段名。
        field: &'static str,
    },
    /// 创建临时文件或目录失败。
    Create {
        /// 稳定操作 token。
        operation: &'static str,
        /// 配置目录或后端报告的相关路径。
        path: PathBuf,
        /// 底层 I/O 分类。
        kind: io::ErrorKind,
    },
    /// 访问或打开临时文件失败。
    Access {
        /// 稳定操作 token。
        operation: &'static str,
        /// 相关路径。
        path: PathBuf,
        /// 底层 I/O 分类。
        kind: io::ErrorKind,
    },
    /// 显式同步关闭/删除临时对象失败。
    Close {
        /// 稳定操作 token。
        operation: &'static str,
        /// 临时对象路径。
        path: PathBuf,
        /// 底层 I/O 分类。
        kind: io::ErrorKind,
    },
    /// 保留给未来可观察清理操作的错误分类；当前 `Drop` 不返回该错误。
    Cleanup {
        /// 稳定操作 token。
        operation: &'static str,
        /// 临时对象路径。
        path: PathBuf,
        /// 底层 I/O 分类。
        kind: io::ErrorKind,
    },
    /// 异步临时文件入口首次 poll 时不在 Tokio runtime 中。
    RuntimeRequired,
}

impl fmt::Display for FsTempError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid temporary-file configuration `{field}`")
            }
            Self::Create {
                operation,
                path,
                kind,
            } => write!(
                formatter,
                "temporary-file operation `{operation}` failed for {}: {kind}",
                path.display()
            ),
            Self::Access {
                operation,
                path,
                kind,
            } => write!(
                formatter,
                "temporary-file access operation `{operation}` failed for {}: {kind}",
                path.display()
            ),
            Self::Close {
                operation,
                path,
                kind,
            } => write!(
                formatter,
                "temporary-file close operation `{operation}` failed for {}: {kind}",
                path.display()
            ),
            Self::Cleanup {
                operation,
                path,
                kind,
            } => write!(
                formatter,
                "temporary-file cleanup operation `{operation}` failed for {}: {kind}",
                path.display()
            ),
            Self::RuntimeRequired => formatter.write_str("a Tokio runtime is required"),
        }
    }
}

impl std::error::Error for FsTempError {}

/// 保留临时目录配置的有状态 facade。
///
/// `FsUtils` 仍然是无字段 unit struct；此类型只拥有一份配置，不使用全局可变状态，也不会
/// 影响其他任务或其他库的临时目录。配置目录在实际创建时检查，构造 context 本身不做 I/O。
///
/// # Examples
///
/// ```
/// use axutils::{FsTempConfig, FsUtils};
///
/// let context = FsUtils::with_temp_config(FsTempConfig::new().with_prefix("job-"));
/// assert_eq!(context.config().prefix.as_deref(), Some("job-"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsUtilsContext {
    config: FsTempConfig,
}

impl FsUtilsContext {
    /// 返回 context 持有的配置引用。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{FsTempConfig, FsUtils};
    ///
    /// let context = FsUtils::with_temp_config(FsTempConfig::new().with_suffix(".tmp"));
    /// assert_eq!(context.config().suffix.as_deref(), Some(".tmp"));
    /// ```
    #[must_use]
    pub fn config(&self) -> &FsTempConfig {
        &self.config
    }

    #[cfg(feature = "tempfile")]
    /// 使用 context 配置创建一个拥有所有权的同步命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     use axutils::{FsTempConfig, FsUtils};
    ///
    ///     let context = FsUtils::with_temp_config(FsTempConfig::new());
    ///     let file = context.create_temp_file()?;
    ///     let path = file.path().to_path_buf();
    ///     file.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_file(&self) -> Result<FsTempFile, FsTempError> {
        create_temp_file(&self.config)
    }

    #[cfg(feature = "tempfile")]
    /// 使用 context 配置创建一个拥有所有权的同步命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     use axutils::{FsTempConfig, FsUtils};
    ///
    ///     let context = FsUtils::with_temp_config(FsTempConfig::new());
    ///     let directory = context.create_temp_dir()?;
    ///     let path = directory.path().to_path_buf();
    ///     directory.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_dir(&self) -> Result<FsTempDir, FsTempError> {
        create_temp_dir(&self.config)
    }

    #[cfg(feature = "tempfile-async")]
    /// 使用 context 配置创建一个拥有所有权的异步命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     use axutils::{FsTempConfig, FsUtils};
    ///
    ///     let context = FsUtils::with_temp_config(FsTempConfig::new());
    ///     let file = context.create_temp_file_async().await?;
    ///     file.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_file_async(
        &self,
    ) -> impl Future<Output = Result<FsAsyncTempFile, FsTempError>> + 'static {
        let config = self.config.clone();
        async move { create_temp_file_async(config).await }
    }

    #[cfg(feature = "tempfile-async")]
    /// 使用 context 配置创建一个拥有所有权的异步命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     use axutils::{FsTempConfig, FsUtils};
    ///
    ///     let context = FsUtils::with_temp_config(FsTempConfig::new());
    ///     let directory = context.create_temp_dir_async().await?;
    ///     directory.drop_async().await;
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_dir_async(
        &self,
    ) -> impl Future<Output = Result<FsAsyncTempDir, FsTempError>> + 'static {
        let config = self.config.clone();
        async move { create_temp_dir_async(config).await }
    }
}

#[cfg(feature = "tempfile")]
/// 同步拥有型命名临时文件包装。
///
/// 包装对象持有底层文件句柄和清理所有权；离开作用域时会尝试删除临时文件。需要观察
/// 删除错误时调用 [`FsTempFile::close`]，而不是只依赖 `Drop`。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "tempfile")]
/// fn example() -> Result<(), axutils::FsTempError> {
///     let file = axutils::FsUtils::create_temp_file()?;
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

#[cfg(feature = "tempfile")]
impl FsTempFile {
    /// 返回当前临时文件路径。
    ///
    /// 返回的路径只在包装对象仍持有所有权时有效；对象被关闭或析构后不应继续使用。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file()?;
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
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file()?;
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
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), Box<dyn std::error::Error>> {
    ///     use std::io::Write;
    ///
    ///     let mut file = axutils::FsUtils::create_temp_file()?;
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
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file()?;
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

#[cfg(feature = "tempfile")]
/// 同步拥有型命名临时目录包装。
///
/// 包装对象拥有临时目录及其内容的清理责任；`close` 会递归删除临时目录自身，不会删除
/// `FsTempConfig::directory` 指定的父目录。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "tempfile")]
/// fn example() -> Result<(), axutils::FsTempError> {
///     let directory = axutils::FsUtils::create_temp_dir()?;
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

#[cfg(feature = "tempfile")]
impl FsTempDir {
    /// 返回当前临时目录路径。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     let directory = axutils::FsUtils::create_temp_dir()?;
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
    /// # #[cfg(feature = "tempfile")]
    /// fn example() -> Result<(), axutils::FsTempError> {
    ///     let directory = axutils::FsUtils::create_temp_dir()?;
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

#[cfg(feature = "tempfile-async")]
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
/// # #[cfg(feature = "tempfile-async")]
/// async fn example() -> Result<(), axutils::FsTempError> {
///     let file = axutils::FsUtils::create_temp_file_async().await?;
///     file.drop_async().await;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsAsyncTempFile {
    inner: async_tempfile::TempFile,
}

#[cfg(feature = "tempfile-async")]
impl FsAsyncTempFile {
    /// 返回当前临时文件路径。
    ///
    /// 路径只在包装对象仍然存活且仍拥有临时文件时有效。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file_async().await?;
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
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file_async().await?;
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
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let file = axutils::FsUtils::create_temp_file_async().await?;
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

#[cfg(feature = "tempfile-async")]
/// Tokio 异步拥有型命名临时目录包装。
///
/// `drop_async` 正常完成时使用后端异步递归删除；隐式 `Drop` 或取消路径使用后端同步
/// 删除作为后备，可能在 runtime worker 上执行同步递归删除。需要观察删除错误时使用
/// 可能阻塞当前线程的 `close`。
///
/// # Examples
///
/// ```no_run
/// # #[cfg(feature = "tempfile-async")]
/// async fn example() -> Result<(), axutils::FsTempError> {
///     let directory = axutils::FsUtils::create_temp_dir_async().await?;
///     directory.drop_async().await;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct FsAsyncTempDir {
    inner: async_tempfile::TempDir,
}

#[cfg(feature = "tempfile-async")]
impl FsAsyncTempDir {
    /// 返回当前临时目录路径。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let directory = axutils::FsUtils::create_temp_dir_async().await?;
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
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let directory = axutils::FsUtils::create_temp_dir_async().await?;
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
    /// # #[cfg(feature = "tempfile-async")]
    /// async fn example() -> Result<(), axutils::FsTempError> {
    ///     let directory = axutils::FsUtils::create_temp_dir_async().await?;
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

#[cfg(feature = "tempfile")]
pub(crate) fn create_temp_file(config: &FsTempConfig) -> Result<FsTempFile, FsTempError> {
    const OPERATION: &str = "create_temp_file";
    config.validate_affixes()?;
    config.validate_directory(OPERATION)?;

    let mut builder = tempfile::Builder::new();
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

#[cfg(feature = "tempfile")]
pub(crate) fn create_temp_dir(config: &FsTempConfig) -> Result<FsTempDir, FsTempError> {
    const OPERATION: &str = "create_temp_dir";
    config.validate_affixes()?;
    config.validate_directory(OPERATION)?;

    let mut builder = tempfile::Builder::new();
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

#[cfg(feature = "tempfile-async")]
fn ensure_runtime() -> Result<(), FsTempError> {
    tokio::runtime::Handle::try_current()
        .map(|_| ())
        .map_err(|_| FsTempError::RuntimeRequired)
}

#[cfg(feature = "tempfile-async")]
fn async_error_kind(error: async_tempfile::Error) -> io::ErrorKind {
    match error {
        async_tempfile::Error::InvalidDirectory => io::ErrorKind::NotADirectory,
        async_tempfile::Error::InvalidFile => io::ErrorKind::InvalidInput,
        async_tempfile::Error::InvalidAffix => io::ErrorKind::InvalidInput,
        async_tempfile::Error::Io(error) => error.kind(),
    }
}

#[cfg(feature = "tempfile-async")]
pub(crate) async fn create_temp_file_async(
    config: FsTempConfig,
) -> Result<FsAsyncTempFile, FsTempError> {
    const OPERATION: &str = "create_temp_file_async";
    config.validate_affixes()?;
    ensure_runtime()?;
    config.validate_directory_async(OPERATION).await?;

    let directory = config.backend_directory();
    let mut builder = async_tempfile::TempFile::builder();
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

#[cfg(feature = "tempfile-async")]
pub(crate) async fn create_temp_dir_async(
    config: FsTempConfig,
) -> Result<FsAsyncTempDir, FsTempError> {
    const OPERATION: &str = "create_temp_dir_async";
    config.validate_affixes()?;
    ensure_runtime()?;
    config.validate_directory_async(OPERATION).await?;

    let directory = config.backend_directory();
    let mut builder = async_tempfile::TempDir::builder();
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

pub(crate) fn context(config: FsTempConfig) -> FsUtilsContext {
    FsUtilsContext { config }
}
