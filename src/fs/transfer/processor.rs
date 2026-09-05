/// 同步文件块处理器。
///
/// 处理器按源文件顺序串行接收拥有所有权的输入块；返回的输出块会在通过可选上限检查后
/// 写入目标。处理器可以保存跨块状态，但库不会替它限制处理器自己的额外分配。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsChunkProcessor;
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

#[cfg(feature = "fs-async")]
use std::future::Future;

#[cfg(feature = "fs-async")]
/// Tokio 异步文件块处理器。
///
/// 处理器按源文件顺序串行接收拥有所有权的输入块；关联 future 可以跨 `await` 借用处理器
/// 和当前块。库不为每个块创建后台任务，也不创建 Tokio runtime。
///
/// # Examples
///
/// ```
/// use axutils::fs::FsAsyncChunkProcessor;
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
