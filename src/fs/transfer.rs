//! 文件流式传输领域类型和实现。

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[cfg(feature = "tokio")]
use std::future::Future;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::FsError;

const OP_COPY_FILE_WITH: &str = "copy_file_with";
const MIN_CHUNK_SIZE: usize = 1024;
const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// 流式文件传输的配置。
///
/// `chunk_size` 是处理器每次收到的输入块的最大大小，而不是底层单次 `read` 调用的大小。
/// 实现会填充当前块，因此偶发的短读不会改变普通文件的块边界。库只限制当前输入块和
/// 当前处理结果；处理器自身的额外分配、磁盘容量和进程总内存不受此类型控制。
///
/// # Examples
///
/// ```
/// use axutils::FsTransferOptions;
///
/// let options = FsTransferOptions {
///     chunk_size: 128 * 1024,
///     max_output_bytes: Some(8 * 1024 * 1024),
/// };
/// assert_eq!(options.chunk_size, 128 * 1024);
/// assert_eq!(options.max_output_bytes, Some(8 * 1024 * 1024));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsTransferOptions {
    /// 每个输入块的最大字节数，默认值为 64 KiB，允许范围为 1 KiB 到 16 MiB。
    pub chunk_size: usize,
    /// 可选的累计输出字节上限；超限的当前块不会写入目标。
    pub max_output_bytes: Option<u64>,
}

impl Default for FsTransferOptions {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024,
            max_output_bytes: None,
        }
    }
}

impl FsTransferOptions {
    fn validate<E>(&self) -> Result<(), FsTransferError<E>> {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&self.chunk_size) {
            return Err(FsTransferError::InvalidOptions {
                field: "chunk_size",
            });
        }
        Ok(())
    }
}

/// 流式传输成功后的统计信息。
///
/// `input_bytes` 是实际读出的输入字节数，`output_bytes` 是实际写入的输出字节数，
/// `chunks` 是处理成功并完成写入的输入块数；失败结果不附带部分统计。
///
/// # Examples
///
/// ```
/// use axutils::FsTransferStats;
///
/// let stats = FsTransferStats {
///     input_bytes: 2048,
///     output_bytes: 4096,
///     chunks: 2,
/// };
/// assert_eq!(stats.output_bytes, stats.input_bytes * 2);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FsTransferStats {
    /// 从源文件实际读出的字节数。
    pub input_bytes: u64,
    /// 实际写入目标文件的字节数。
    pub output_bytes: u64,
    /// 处理器成功调用并且对应输出已完整写入的块数。
    pub chunks: u64,
}

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
/// use axutils::FsTransferError;
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

/// 同步文件块处理器。
///
/// 处理器按源文件顺序串行接收拥有所有权的输入块；返回的输出块会在通过可选上限检查后
/// 写入目标。处理器可以保存跨块状态，但库不会替它限制处理器自己的额外分配。
///
/// # Examples
///
/// ```
/// use axutils::FsChunkProcessor;
///
/// struct Identity;
/// impl FsChunkProcessor for Identity {
///     type Error = std::convert::Infallible;
///
///     fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
///         Ok(chunk)
///     }
/// }
///
/// let mut processor = Identity;
/// assert_eq!(processor.process(b"chunk".to_vec()).unwrap(), b"chunk");
/// ```
pub trait FsChunkProcessor {
    /// 处理器错误类型。
    type Error;

    /// 处理一个拥有所有权的输入块并返回要写入目标的输出块。
    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
}

#[cfg(feature = "tokio")]
/// Tokio 异步文件块处理器。
///
/// 处理器按源文件顺序串行接收拥有所有权的输入块；关联 future 可以跨 `await` 借用处理器
/// 和当前块。库不为每个块创建后台任务，也不创建 Tokio runtime。
///
/// # Examples
///
/// ```
/// use axutils::FsAsyncChunkProcessor;
///
/// struct Identity;
/// impl FsAsyncChunkProcessor for Identity {
///     type Error = std::convert::Infallible;
///     type Future<'a> = std::future::Ready<Result<Vec<u8>, Self::Error>> where Self: 'a;
///
///     fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
///         std::future::ready(Ok(chunk))
///     }
/// }
///
/// # let _ = Identity;
/// ```
pub trait FsAsyncChunkProcessor {
    /// 处理器错误类型。
    type Error;
    /// 跨 `await` 持有当前输入块的关联 future。
    type Future<'a>: Future<Output = Result<Vec<u8>, Self::Error>> + 'a
    where
        Self: 'a;

    /// 异步处理一个拥有所有权的输入块。
    fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a>;
}

fn source_io<E>(path: &Path, error: io::Error) -> FsTransferError<E> {
    FsTransferError::SourceIo {
        error: FsError::Io {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
            kind: error.kind(),
        },
    }
}

fn destination_io<E>(path: &Path, error: io::Error) -> FsTransferError<E> {
    FsTransferError::DestinationIo {
        error: FsError::Io {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
            kind: error.kind(),
        },
    }
}

fn unsupported_entry<E>(path: &Path) -> FsTransferError<E> {
    FsTransferError::DestinationIo {
        error: FsError::UnsupportedEntry {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
        },
    }
}

fn unsupported_source_entry<E>(path: &Path) -> FsTransferError<E> {
    FsTransferError::SourceIo {
        error: FsError::UnsupportedEntry {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
        },
    }
}

fn validate_regular_metadata<E>(
    metadata: io::Result<fs::Metadata>,
    path: &Path,
    source: bool,
    allow_missing: bool,
) -> Result<(), FsTransferError<E>> {
    match metadata {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) if source => Err(unsupported_source_entry(path)),
        Ok(_) => Err(unsupported_entry(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound && allow_missing => Ok(()),
        Err(error) if source => Err(source_io(path, error)),
        Err(error) => Err(destination_io(path, error)),
    }
}

fn read_chunk<R: Read>(reader: &mut R, chunk_size: usize) -> io::Result<Option<Vec<u8>>> {
    let mut chunk = vec![0; chunk_size];
    let mut filled = 0;
    while filled < chunk_size {
        match reader.read(&mut chunk[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => {
                chunk.truncate(filled);
                return Ok(Some(chunk));
            }
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(Some(chunk))
}

#[cfg(feature = "tokio")]
async fn read_chunk_async<R: AsyncRead + Unpin>(
    reader: &mut R,
    chunk_size: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut chunk = vec![0; chunk_size];
    let mut filled = 0;
    while filled < chunk_size {
        match reader.read(&mut chunk[filled..]).await {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => {
                chunk.truncate(filled);
                return Ok(Some(chunk));
            }
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(Some(chunk))
}

fn next_input_bytes<E>(current: u64, chunk_len: usize) -> Result<u64, FsTransferError<E>> {
    let chunk_len = u64::try_from(chunk_len).map_err(|_| FsTransferError::InputSizeOverflow)?;
    current
        .checked_add(chunk_len)
        .ok_or(FsTransferError::InputSizeOverflow)
}

fn next_output_bytes<E>(
    current: u64,
    output_len: usize,
    max_output_bytes: Option<u64>,
) -> Result<u64, FsTransferError<E>> {
    let output_len = u64::try_from(output_len).map_err(|_| FsTransferError::OutputSizeOverflow)?;
    let observed = current
        .checked_add(output_len)
        .ok_or(FsTransferError::OutputSizeOverflow)?;
    if let Some(limit) = max_output_bytes {
        if observed > limit {
            return Err(FsTransferError::OutputLimitExceeded { limit, observed });
        }
    }
    Ok(observed)
}

fn next_chunks<E>(current: u64) -> Result<u64, FsTransferError<E>> {
    current
        .checked_add(1)
        .ok_or(FsTransferError::ChunkCountOverflow)
}

fn process_sync<R, W, C>(
    reader: &mut R,
    writer: &mut W,
    source: &Path,
    destination: &Path,
    options: FsTransferOptions,
    mut processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    R: Read,
    W: Write,
    C: FsChunkProcessor,
{
    let mut stats = FsTransferStats::default();
    loop {
        let Some(chunk) =
            read_chunk(reader, options.chunk_size).map_err(|error| source_io(source, error))?
        else {
            break;
        };

        let next_input = next_input_bytes(stats.input_bytes, chunk.len())?;
        let output = processor
            .process(chunk)
            .map_err(|error| FsTransferError::Processor {
                error,
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
            })?;
        let next_output =
            next_output_bytes(stats.output_bytes, output.len(), options.max_output_bytes)?;
        let next_chunk_count = next_chunks(stats.chunks)?;

        if !output.is_empty() {
            writer
                .write_all(&output)
                .map_err(|error| destination_io(destination, error))?;
        }
        stats.input_bytes = next_input;
        stats.output_bytes = next_output;
        stats.chunks = next_chunk_count;
    }

    writer
        .flush()
        .map_err(|error| destination_io(destination, error))?;
    Ok(stats)
}

#[cfg(feature = "tokio")]
async fn process_async<R, W, C>(
    reader: &mut R,
    writer: &mut W,
    source: &Path,
    destination: &Path,
    options: FsTransferOptions,
    mut processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    C: FsAsyncChunkProcessor,
{
    let mut stats = FsTransferStats::default();
    loop {
        let Some(chunk) = read_chunk_async(reader, options.chunk_size)
            .await
            .map_err(|error| source_io(source, error))?
        else {
            break;
        };

        let next_input = next_input_bytes(stats.input_bytes, chunk.len())?;
        let output =
            processor
                .process(chunk)
                .await
                .map_err(|error| FsTransferError::Processor {
                    error,
                    source: source.to_path_buf(),
                    destination: destination.to_path_buf(),
                })?;
        let next_output =
            next_output_bytes(stats.output_bytes, output.len(), options.max_output_bytes)?;
        let next_chunk_count = next_chunks(stats.chunks)?;

        if !output.is_empty() {
            writer
                .write_all(&output)
                .await
                .map_err(|error| destination_io(destination, error))?;
        }
        stats.input_bytes = next_input;
        stats.output_bytes = next_output;
        stats.chunks = next_chunk_count;
    }

    writer
        .flush()
        .await
        .map_err(|error| destination_io(destination, error))?;
    Ok(stats)
}

pub(crate) fn copy_file_with<C>(
    source: PathBuf,
    destination: PathBuf,
    options: FsTransferOptions,
    processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    C: FsChunkProcessor,
{
    options.validate()?;
    if source == destination {
        return Err(FsTransferError::SameFile {
            source,
            destination,
        });
    }

    validate_regular_metadata(fs::symlink_metadata(&source), &source, true, false)?;
    validate_regular_metadata(
        fs::symlink_metadata(&destination),
        &destination,
        false,
        true,
    )?;

    let mut source_file = File::open(&source).map_err(|error| source_io(&source, error))?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&destination)
        .map_err(|error| destination_io(&destination, error))?;

    process_sync(
        &mut source_file,
        &mut destination_file,
        &source,
        &destination,
        options,
        processor,
    )
}

#[cfg(feature = "tokio")]
pub(crate) async fn copy_file_with_async<C>(
    source: PathBuf,
    destination: PathBuf,
    options: FsTransferOptions,
    processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    C: FsAsyncChunkProcessor,
{
    options.validate()?;
    if source == destination {
        return Err(FsTransferError::SameFile {
            source,
            destination,
        });
    }
    super::ops::ensure_runtime().map_err(|_| FsTransferError::RuntimeRequired)?;

    validate_regular_metadata(
        tokio::fs::symlink_metadata(&source).await,
        &source,
        true,
        false,
    )?;
    validate_regular_metadata(
        tokio::fs::symlink_metadata(&destination).await,
        &destination,
        false,
        true,
    )?;

    let mut source_file = tokio::fs::File::open(&source)
        .await
        .map_err(|error| source_io(&source, error))?;
    let mut destination_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&destination)
        .await
        .map_err(|error| destination_io(&destination, error))?;

    process_async(
        &mut source_file,
        &mut destination_file,
        &source,
        &destination,
        options,
        processor,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        next_chunks, next_input_bytes, next_output_bytes, process_sync, FsChunkProcessor,
        FsTransferError, FsTransferOptions,
    };
    use std::io::{self, Read, Write};

    #[cfg(feature = "tokio")]
    use super::{process_async, FsAsyncChunkProcessor};
    #[cfg(feature = "tokio")]
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };
    #[cfg(feature = "tokio")]
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    struct ShortReader {
        data: Vec<u8>,
        offset: usize,
        max_read: usize,
    }

    impl Read for ShortReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.offset == self.data.len() {
                return Ok(0);
            }
            let count = self
                .max_read
                .min(buffer.len())
                .min(self.data.len() - self.offset);
            buffer[..count].copy_from_slice(&self.data[self.offset..self.offset + count]);
            self.offset += count;
            Ok(count)
        }
    }

    struct ShortWriter {
        data: Vec<u8>,
        max_write: usize,
    }

    impl Write for ShortWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            let count = self.max_write.min(buffer.len());
            self.data.extend_from_slice(&buffer[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Identity;

    impl FsChunkProcessor for Identity {
        type Error = ();

        fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            Ok(chunk)
        }
    }

    struct NoDisplayError;

    struct FailingProcessor;

    impl FsChunkProcessor for FailingProcessor {
        type Error = NoDisplayError;

        fn process(&mut self, _chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            Err(NoDisplayError)
        }
    }

    struct ZeroWriter;

    impl Write for ZeroWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Ok(0)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailingReader;

    impl Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "reader failure",
            ))
        }
    }

    struct FlushFailWriter;

    impl Write for FlushFailWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("flush failure"))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncShortReader {
        data: Vec<u8>,
        offset: usize,
        max_read: usize,
    }

    #[cfg(feature = "tokio")]
    impl AsyncRead for AsyncShortReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if self.offset == self.data.len() {
                return Poll::Ready(Ok(()));
            }
            let count = self
                .max_read
                .min(buffer.remaining())
                .min(self.data.len() - self.offset);
            buffer.put_slice(&self.data[self.offset..self.offset + count]);
            self.offset += count;
            Poll::Ready(Ok(()))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncShortWriter {
        data: Vec<u8>,
        max_write: usize,
    }

    #[cfg(feature = "tokio")]
    impl AsyncWrite for AsyncShortWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            let count = self.max_write.min(buffer.len());
            self.data.extend_from_slice(&buffer[..count]);
            Poll::Ready(Ok(count))
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncZeroWriter;

    #[cfg(feature = "tokio")]
    impl AsyncWrite for AsyncZeroWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(0))
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncFailingReader;

    #[cfg(feature = "tokio")]
    impl AsyncRead for AsyncFailingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "reader failure",
            )))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncFlushFailWriter;

    #[cfg(feature = "tokio")]
    impl AsyncWrite for AsyncFlushFailWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Ok(buffer.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::other("flush failure")))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncIdentity;

    #[cfg(feature = "tokio")]
    impl FsAsyncChunkProcessor for AsyncIdentity {
        type Error = ();
        type Future<'a>
            = std::future::Ready<Result<Vec<u8>, Self::Error>>
        where
            Self: 'a;

        fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
            std::future::ready(Ok(chunk))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncFailingProcessor;

    #[cfg(feature = "tokio")]
    impl FsAsyncChunkProcessor for AsyncFailingProcessor {
        type Error = NoDisplayError;
        type Future<'a>
            = std::future::Ready<Result<Vec<u8>, Self::Error>>
        where
            Self: 'a;

        fn process<'a>(&'a mut self, _chunk: Vec<u8>) -> Self::Future<'a> {
            std::future::ready(Err(NoDisplayError))
        }
    }

    #[cfg(feature = "tokio")]
    struct AsyncPanicProcessor;

    #[cfg(feature = "tokio")]
    impl FsAsyncChunkProcessor for AsyncPanicProcessor {
        type Error = ();
        type Future<'a>
            = std::future::Ready<Result<Vec<u8>, Self::Error>>
        where
            Self: 'a;

        fn process<'a>(&'a mut self, _chunk: Vec<u8>) -> Self::Future<'a> {
            panic!("empty input must not call the async processor");
        }
    }

    struct PanicProcessor;

    impl FsChunkProcessor for PanicProcessor {
        type Error = ();

        fn process(&mut self, _chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            panic!("empty input must not call the processor");
        }
    }

    #[test]
    fn fills_short_reads_and_completes_short_writes_in_order() {
        let mut reader = ShortReader {
            data: b"abcdefg".to_vec(),
            offset: 0,
            max_read: 2,
        };
        let mut writer = ShortWriter {
            data: Vec::new(),
            max_write: 1,
        };
        let stats = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            Identity,
        )
        .expect("short I/O should complete");

        assert_eq!(writer.data, b"abcdefg");
        assert_eq!(stats.input_bytes, 7);
        assert_eq!(stats.output_bytes, 7);
        assert_eq!(stats.chunks, 2);
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn async_core_fills_short_reads_and_completes_short_writes_in_order() {
        let mut reader = AsyncShortReader {
            data: b"abcdefg".to_vec(),
            offset: 0,
            max_read: 2,
        };
        let mut writer = AsyncShortWriter {
            data: Vec::new(),
            max_write: 1,
        };
        let stats = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            AsyncIdentity,
        )
        .await
        .expect("short async I/O should complete");

        assert_eq!(writer.data, b"abcdefg");
        assert_eq!(stats.input_bytes, 7);
        assert_eq!(stats.output_bytes, 7);
        assert_eq!(stats.chunks, 2);
    }

    #[test]
    fn processor_error_is_retained_without_display_bounds() {
        let mut reader = io::Cursor::new(b"input".to_vec());
        let mut writer = Vec::new();
        let result = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            FailingProcessor,
        );

        assert!(matches!(
            result,
            Err(FsTransferError::Processor {
                error: NoDisplayError,
                ..
            })
        ));
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn async_processor_error_is_retained_without_display_bounds() {
        let mut reader = AsyncShortReader {
            data: b"input".to_vec(),
            offset: 0,
            max_read: usize::MAX,
        };
        let mut writer = AsyncShortWriter {
            data: Vec::new(),
            max_write: 1,
        };
        let result = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            AsyncFailingProcessor,
        )
        .await;

        assert!(matches!(
            result,
            Err(FsTransferError::Processor {
                error: NoDisplayError,
                ..
            })
        ));
    }

    #[test]
    fn write_zero_is_reported_as_destination_io() {
        let mut reader = io::Cursor::new(b"input".to_vec());
        let mut writer = ZeroWriter;
        let result = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            Identity,
        );

        assert!(matches!(
            result,
            Err(FsTransferError::DestinationIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::WriteZero,
                    ..
                }
            })
        ));
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn async_write_zero_is_reported_as_destination_io() {
        let mut reader = AsyncShortReader {
            data: b"input".to_vec(),
            offset: 0,
            max_read: usize::MAX,
        };
        let mut writer = AsyncZeroWriter;
        let result = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            AsyncIdentity,
        )
        .await;

        assert!(matches!(
            result,
            Err(FsTransferError::DestinationIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::WriteZero,
                    ..
                }
            })
        ));
    }

    #[test]
    fn source_read_and_destination_flush_failures_keep_error_roles() {
        let mut reader = FailingReader;
        let mut writer = Vec::new();
        let result = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            Identity,
        );
        assert!(matches!(
            result,
            Err(FsTransferError::SourceIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::PermissionDenied,
                    ..
                }
            })
        ));

        let mut reader = io::Cursor::new(b"input".to_vec());
        let mut writer = FlushFailWriter;
        let result = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            Identity,
        );
        assert!(matches!(
            result,
            Err(FsTransferError::DestinationIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::Other,
                    ..
                }
            })
        ));
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn async_source_read_and_destination_flush_failures_keep_error_roles() {
        let mut reader = AsyncFailingReader;
        let mut writer = AsyncShortWriter {
            data: Vec::new(),
            max_write: 1,
        };
        let result = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            AsyncIdentity,
        )
        .await;
        assert!(matches!(
            result,
            Err(FsTransferError::SourceIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::PermissionDenied,
                    ..
                }
            })
        ));

        let mut reader = AsyncShortReader {
            data: b"input".to_vec(),
            offset: 0,
            max_read: usize::MAX,
        };
        let mut writer = AsyncFlushFailWriter;
        let result = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: None,
            },
            AsyncIdentity,
        )
        .await;
        assert!(matches!(
            result,
            Err(FsTransferError::DestinationIo {
                error: crate::FsError::Io {
                    operation: "copy_file_with",
                    kind: io::ErrorKind::Other,
                    ..
                }
            })
        ));
    }

    #[test]
    fn empty_input_does_not_call_the_processor() {
        let mut reader = io::Cursor::new(Vec::<u8>::new());
        let mut writer = Vec::new();
        let stats = process_sync(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: Some(0),
            },
            PanicProcessor,
        )
        .expect("empty input should succeed");
        assert_eq!(stats, super::FsTransferStats::default());
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn async_empty_input_does_not_call_the_processor() {
        let mut reader = tokio::io::empty();
        let mut writer = AsyncShortWriter {
            data: Vec::new(),
            max_write: 1,
        };
        let stats = process_async(
            &mut reader,
            &mut writer,
            Path::new("source"),
            Path::new("destination"),
            FsTransferOptions {
                chunk_size: 4,
                max_output_bytes: Some(0),
            },
            AsyncPanicProcessor,
        )
        .await
        .expect("empty async input should succeed");
        assert_eq!(stats, super::FsTransferStats::default());
    }

    #[test]
    fn checked_counters_distinguish_overflow_and_limit() {
        assert!(matches!(next_input_bytes::<()>(0, 4), Ok(4)));
        assert!(matches!(
            next_input_bytes::<()>(u64::MAX, 1),
            Err(FsTransferError::InputSizeOverflow)
        ));
        assert!(matches!(next_output_bytes::<()>(0, 0, Some(0)), Ok(0)));
        assert!(matches!(
            next_output_bytes::<()>(0, 1, Some(0)),
            Err(FsTransferError::OutputLimitExceeded {
                limit: 0,
                observed: 1
            })
        ));
        assert!(matches!(
            next_output_bytes::<()>(u64::MAX, 1, None),
            Err(FsTransferError::OutputSizeOverflow)
        ));
        assert!(matches!(
            next_chunks::<()>(u64::MAX),
            Err(FsTransferError::ChunkCountOverflow)
        ));
    }

    use std::path::Path;
}
