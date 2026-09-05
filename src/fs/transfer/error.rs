use std::{fmt, path::PathBuf};

use crate::fs::FsError;

/// 流式传输错误。
///
/// 处理器错误 `E` 会以原始类型保留在 [`FsTransferError::Processor`] 中。该枚举为非穷尽
/// 枚举，调用方匹配时必须保留 wildcard。处理器错误不要求实现 `Display`、`Clone` 或 `Eq`；
/// 本类型的 `Display` 只显示稳定的错误类别和路径，不会把处理器错误强制转换成字符串。
/// `FsTransferError<E>` 只有在 `E: std::error::Error + 'static` 时才实现
/// `std::error::Error`；`Display` 本身不要求这些 bound。
///
/// `SourceIo` 表示源文件打开/读取失败，`DestinationIo` 表示目标文件创建、截断、写入或
/// flush 失败，`Processor` 保留处理器原始错误和源/目标路径，`OutputLimitExceeded` 表示
/// 当前结果在写入前超过累计上限，三个 `*Overflow` 变体表示 checked 计数失败，
/// `InvalidOptions` 表示块大小无效，`SameFile` 表示词法路径相同，`RuntimeRequired` 只
/// 由异步入口在首次 poll 时没有调用方 Tokio runtime 返回。I/O 预检不提供 canonicalize、
/// 硬链接别名检测或 TOCTOU 防护；错误或取消可能留下已写出的目标前缀。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsTransferError;
///
/// fn category(error: &FsTransferError<std::convert::Infallible>) -> &'static str {
///     match error {
///         FsTransferError::SourceIo { .. } => "source-io",
///         FsTransferError::DestinationIo { .. } => "destination-io",
///         FsTransferError::Processor { .. } => "processor",
///         FsTransferError::OutputLimitExceeded { .. } => "output-limit",
///         FsTransferError::OutputSizeOverflow
///         | FsTransferError::InputSizeOverflow
///         | FsTransferError::ChunkCountOverflow => "counter-overflow",
///         FsTransferError::InvalidOptions { .. } => "invalid-options",
///         FsTransferError::SameFile { .. } => "same-file",
///         FsTransferError::RuntimeRequired => "runtime",
///         _ => "future-error-variant",
///     }
/// }
///
/// assert_eq!(
///     category(&FsTransferError::<std::convert::Infallible>::RuntimeRequired),
///     "runtime"
/// );
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum FsTransferError<E> {
    /// 打开或读取源文件失败。
    SourceIo {
        /// 已脱敏的底层文件系统错误。
        error: FsError,
    },
    /// 创建、截断、写入或刷新目标文件失败。
    DestinationIo {
        /// 已脱敏的底层文件系统错误。
        error: FsError,
    },
    /// 用户处理器返回错误；目标可能已经包含前序块的部分结果。
    Processor {
        /// 处理器返回的原始错误。
        error: E,
        /// 源路径。
        source: PathBuf,
        /// 目标路径。
        destination: PathBuf,
    },
    /// 当前块会使累计输出超过上限。
    OutputLimitExceeded {
        /// 生效的累计输出上限。
        limit: u64,
        /// 当前块被拒绝前计算出的累计输出。
        observed: u64,
    },
    /// 输出字节数无法用 `u64` 或 checked addition 表示。
    OutputSizeOverflow,
    /// 输入字节数无法用 `u64` 或 checked addition 表示。
    InputSizeOverflow,
    /// 块数量无法用 `u64` 表示。
    ChunkCountOverflow,
    /// 传输参数无效。
    InvalidOptions {
        /// 无效字段名。
        field: &'static str,
    },
    /// 词法路径相等；不会尝试 canonicalize 或识别所有硬链接别名。
    SameFile {
        /// 源路径。
        source: PathBuf,
        /// 目标路径。
        destination: PathBuf,
    },
    /// 异步入口被首次 poll 时不在 Tokio runtime 中。
    RuntimeRequired,
}

impl<E> fmt::Display for FsTransferError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceIo { error } => {
                write!(formatter, "source file transfer I/O failed: {error}")
            }
            Self::DestinationIo { error } => {
                write!(formatter, "destination file transfer I/O failed: {error}")
            }
            Self::Processor {
                source,
                destination,
                ..
            } => write!(
                formatter,
                "file transfer processor failed from {} to {}",
                source.display(),
                destination.display()
            ),
            Self::OutputLimitExceeded { limit, observed } => write!(
                formatter,
                "file transfer output of {observed} bytes exceeds the {limit}-byte limit"
            ),
            Self::OutputSizeOverflow => formatter.write_str("file transfer output size overflowed"),
            Self::InputSizeOverflow => formatter.write_str("file transfer input size overflowed"),
            Self::ChunkCountOverflow => formatter.write_str("file transfer chunk count overflowed"),
            Self::InvalidOptions { field } => {
                write!(formatter, "invalid file transfer option `{field}`")
            }
            Self::SameFile {
                source,
                destination: _,
            } => write!(
                formatter,
                "file transfer source and destination are the same path: {}",
                source.display()
            ),
            Self::RuntimeRequired => formatter.write_str("a Tokio runtime is required"),
        }
    }
}

impl<E> std::error::Error for FsTransferError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceIo { error } | Self::DestinationIo { error } => Some(error),
            Self::Processor { error, .. } => Some(error),
            _ => None,
        }
    }
}
