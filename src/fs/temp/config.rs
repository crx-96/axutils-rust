use std::{fmt, io, path::PathBuf};

#[cfg(feature = "fs-temp-async")]
use std::future::Future;
#[cfg(feature = "fs-temp-async")]
use tokio::fs as async_fs;

#[cfg(feature = "fs-temp-async")]
use super::{asynchronous as async_temp, FsAsyncTempDir, FsAsyncTempFile};
#[cfg(feature = "fs-temp")]
use super::{sync as sync_temp, FsTempDir, FsTempFile};

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
/// use axutils::fs::FsTempConfig;
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
    /// use axutils::fs::FsTempConfig;
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
    /// use axutils::fs::FsTempConfig;
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
    /// use axutils::fs::FsTempConfig;
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
    /// use axutils::fs::FsTempConfig;
    ///
    /// let config = FsTempConfig::new().with_suffix(".part");
    /// assert_eq!(config.suffix.as_deref(), Some(".part"));
    /// ```
    #[must_use]
    pub fn with_suffix<S: Into<String>>(mut self, suffix: S) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    pub(super) fn validate_affixes(&self) -> Result<(), FsTempError> {
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

    pub(super) fn backend_directory(&self) -> PathBuf {
        self.directory.clone().unwrap_or_else(std::env::temp_dir)
    }

    #[cfg(feature = "fs-temp")]
    pub(super) fn validate_directory(&self, operation: &'static str) -> Result<(), FsTempError> {
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

    #[cfg(feature = "fs-temp-async")]
    pub(super) async fn validate_directory_async(
        &self,
        operation: &'static str,
    ) -> Result<(), FsTempError> {
        let Some(directory) = self.directory.as_deref() else {
            return Ok(());
        };
        match async_fs::metadata(directory).await {
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
/// use axutils::fs::FsTempError;
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
/// use axutils::{fs::FsTempConfig, utils::FsUtils};
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
    /// use axutils::{fs::FsTempConfig, utils::FsUtils};
    ///
    /// let context = FsUtils::with_temp_config(FsTempConfig::new().with_suffix(".tmp"));
    /// assert_eq!(context.config().suffix.as_deref(), Some(".tmp"));
    /// ```
    #[must_use]
    pub fn config(&self) -> &FsTempConfig {
        &self.config
    }

    #[cfg(feature = "fs-temp")]
    /// 使用 context 配置创建一个拥有所有权的同步命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::{FsTempConfig, FsTempError}, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
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
        sync_temp::create_temp_file(&self.config)
    }

    #[cfg(feature = "fs-temp")]
    /// 使用 context 配置创建一个拥有所有权的同步命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// use axutils::{fs::{FsTempConfig, FsTempError}, utils::FsUtils};
    ///
    /// fn example() -> Result<(), FsTempError> {
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
        sync_temp::create_temp_dir(&self.config)
    }

    #[cfg(feature = "fs-temp-async")]
    /// 使用 context 配置创建一个拥有所有权的异步命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::{FsTempConfig, FsTempError}, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
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
        async move { async_temp::create_temp_file_async(config).await }
    }

    #[cfg(feature = "fs-temp-async")]
    /// 使用 context 配置创建一个拥有所有权的异步命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::{FsTempConfig, FsTempError}, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
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
        async move { async_temp::create_temp_dir_async(config).await }
    }
}

pub(crate) fn context(config: FsTempConfig) -> FsUtilsContext {
    FsUtilsContext { config }
}
