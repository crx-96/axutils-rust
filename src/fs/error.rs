//! 文件系统操作错误。

use std::{fmt, io, path::PathBuf};

/// 文件系统工具的脱敏错误分类。
///
/// 错误只保留稳定的操作 token、调用方路径、`io::ErrorKind` 或固定限制信息；不会保存或
/// 回显底层错误文本、文件内容、权限详情或其他敏感数据。该枚举为非穷尽枚举，调用方匹配
/// 时必须保留 wildcard。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsError;
///
/// fn classify(error: FsError) -> &'static str {
///     match error {
///         FsError::InvalidLimit { .. } => "invalid-limit",
///         _ => "other",
///     }
/// }
///
/// let _ = classify;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FsError {
    /// 单路径操作打开、读取、创建、写入、列举或删除失败。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::read_bytes("missing-file", 1).unwrap_err();
    /// assert!(matches!(error, FsError::Io { operation: "read_bytes", .. }));
    /// ```
    Io {
        /// 稳定的小写操作 token。
        operation: &'static str,
        /// 调用方传入的路径。
        path: PathBuf,
        /// 底层 I/O 错误分类。
        kind: io::ErrorKind,
    },
    /// 双路径移动或复制操作失败。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::copy_file("missing-source", "destination").unwrap_err();
    /// assert!(matches!(error, FsError::PairIo { operation: "copy_file", .. }));
    /// ```
    PairIo {
        /// 稳定的小写操作 token。
        operation: &'static str,
        /// 源路径。
        source: PathBuf,
        /// 目标路径。
        destination: PathBuf,
        /// 底层 I/O 错误分类。
        kind: io::ErrorKind,
    },
    /// 文件内容不是严格合法的 UTF-8。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::read_to_string("binary-file", 1024).unwrap_err();
    /// if let FsError::NotUtf8 { path } = error {
    ///     let _ = path;
    /// }
    /// ```
    NotUtf8 {
        /// 调用方传入的路径。
        path: PathBuf,
    },
    /// 文件实际读到的字节数超过调用方上限。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::read_bytes("example.bin", 0).unwrap_err();
    /// assert!(matches!(error, FsError::FileTooLarge { limit: 0, .. }));
    /// ```
    FileTooLarge {
        /// 调用方传入的路径。
        path: PathBuf,
        /// 生效的字节上限。
        limit: usize,
    },
    /// 目录实际观察到的直接子项数超过调用方上限。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::list_dir("example-dir", 0).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     FsError::DirectoryEntriesTooMany { limit: 0, .. }
    /// ));
    /// ```
    DirectoryEntriesTooMany {
        /// 调用方传入的目录路径。
        path: PathBuf,
        /// 生效的直接子项上限。
        limit: usize,
    },
    /// 调用方传入的限制会导致计数溢出。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::read_bytes("unused", usize::MAX).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     FsError::InvalidLimit { field: "max_bytes" }
    /// ));
    /// ```
    InvalidLimit {
        /// 发生溢出的参数名，例如 `"max_bytes"` 或 `"max_entries"`。
        field: &'static str,
    },
    /// `copy_file` 的最终源或目标路径项不是普通文件。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// let error = FsUtils::copy_file("directory", "destination").unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     FsError::UnsupportedEntry { operation: "copy_file", .. }
    /// ));
    /// ```
    UnsupportedEntry {
        /// 稳定的小写操作 token。
        operation: &'static str,
        /// 被拒绝的最终路径项。
        path: PathBuf,
    },
    /// 异步入口首次执行时不在 Tokio runtime 中。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::fs::FsError;
    ///
    /// fn is_runtime_required(error: FsError) -> bool {
    ///     matches!(error, FsError::RuntimeRequired)
    /// }
    ///
    /// let _ = is_runtime_required;
    /// ```
    RuntimeRequired,
}

impl fmt::Display for FsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                kind,
            } => write!(
                formatter,
                "filesystem operation `{operation}` failed for {}: {kind}",
                path.display()
            ),
            Self::PairIo {
                operation,
                source,
                destination,
                kind,
            } => write!(
                formatter,
                "filesystem operation `{operation}` failed from {} to {}: {kind}",
                source.display(),
                destination.display(),
            ),
            Self::NotUtf8 { path } => {
                write!(formatter, "file {} is not valid UTF-8", path.display())
            }
            Self::FileTooLarge { path, limit } => write!(
                formatter,
                "file {} exceeds the {limit}-byte limit",
                path.display()
            ),
            Self::DirectoryEntriesTooMany { path, limit } => write!(
                formatter,
                "directory {} exceeds the {limit}-entry limit",
                path.display()
            ),
            Self::InvalidLimit { field } => {
                write!(formatter, "invalid filesystem limit `{field}`")
            }
            Self::UnsupportedEntry { operation, path } => write!(
                formatter,
                "filesystem operation `{operation}` does not support {}",
                path.display()
            ),
            Self::RuntimeRequired => formatter.write_str("a Tokio runtime is required"),
        }
    }
}

impl std::error::Error for FsError {}

#[cfg(test)]
mod tests {
    use super::FsError;
    use std::{io, path::PathBuf};

    #[test]
    fn error_display_does_not_include_raw_io_text_or_content() {
        let error = FsError::Io {
            operation: "read_bytes",
            path: PathBuf::from("sentinel-path"),
            kind: io::ErrorKind::PermissionDenied,
        };
        let display = error.to_string();
        assert!(display.contains("read_bytes"));
        assert!(display.contains("sentinel-path"));
        assert!(!display.contains("secret-content"));
        assert!(!display.contains("raw operating-system detail"));
    }

    #[test]
    fn every_error_variant_has_redacted_display_and_debug() {
        let path = PathBuf::from("sentinel-path");
        let errors = [
            FsError::Io {
                operation: "read_bytes",
                path: path.clone(),
                kind: io::ErrorKind::PermissionDenied,
            },
            FsError::PairIo {
                operation: "copy_file",
                source: path.clone(),
                destination: PathBuf::from("sentinel-destination"),
                kind: io::ErrorKind::NotFound,
            },
            FsError::NotUtf8 { path: path.clone() },
            FsError::FileTooLarge {
                path: path.clone(),
                limit: 1,
            },
            FsError::DirectoryEntriesTooMany {
                path: path.clone(),
                limit: 1,
            },
            FsError::InvalidLimit { field: "max_bytes" },
            FsError::UnsupportedEntry {
                operation: "copy_file",
                path,
            },
            FsError::RuntimeRequired,
        ];

        for error in errors {
            let display = error.to_string();
            let debug = format!("{error:?}");
            for rendered in [display, debug] {
                assert!(!rendered.contains("secret-content"));
                assert!(!rendered.contains("raw operating-system detail"));
            }
        }
    }

    #[test]
    fn runtime_error_is_stable_and_matches() {
        assert_eq!(
            FsError::RuntimeRequired.to_string(),
            "a Tokio runtime is required"
        );
        assert_eq!(FsError::RuntimeRequired, FsError::RuntimeRequired);
    }
}
