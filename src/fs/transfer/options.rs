use super::error::FsTransferError;

pub(super) const MIN_CHUNK_SIZE: usize = 1024;
pub(super) const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// 流式文件传输的配置。
///
/// `chunk_size` 是处理器每次收到的输入块的最大大小，而不是底层单次 `read` 调用的大小。
/// 实现会填充当前块，因此偶发的短读不会改变普通文件的块边界。库只限制当前输入块和
/// 当前处理结果；处理器自身的额外分配、磁盘容量和进程总内存不受此类型控制。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsTransferOptions;
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
    pub(super) fn validate<E>(&self) -> Result<(), FsTransferError<E>> {
        if !(MIN_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&self.chunk_size) {
            return Err(FsTransferError::InvalidOptions {
                field: "chunk_size",
            });
        }
        Ok(())
    }
}
