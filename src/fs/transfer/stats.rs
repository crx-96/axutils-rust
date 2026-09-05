/// 流式传输成功后的统计信息。
///
/// `input_bytes` 是实际读出的输入字节数，`output_bytes` 是实际写入的输出字节数，
/// `chunks` 是处理成功并完成写入的输入块数；失败结果不附带部分统计。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsTransferStats;
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
