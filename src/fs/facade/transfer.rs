//! `FsUtils` 的异步传输和异步临时资源入口。

#[cfg(any(feature = "fs-async", feature = "fs-temp-async"))]
use std::future::Future;
#[cfg(feature = "fs-async")]
use std::path::Path;

#[cfg(feature = "fs-async")]
use super::super::{
    ops, transfer, FsAsyncChunkProcessor, FsError, FsTransferError, FsTransferOptions,
    FsTransferStats,
};
#[cfg(feature = "fs-temp-async")]
use super::super::{temp, FsAsyncTempDir, FsAsyncTempFile, FsTempConfig, FsTempError};
use super::FsUtils;

impl FsUtils {
    /// 在 Tokio runtime 中异步复制普通文件并返回字节数。
    ///
    /// 仅在 `fs-async` feature 下提供；目录、链接和其他非普通最终路径项在无竞态预检时被拒绝，
    /// 返回 [`FsError::UnsupportedEntry`]；其他源/目标错误返回 [`FsError::PairIo`]，预检不提供
    /// 抗 TOCTOU 保证；源/目标错误的 operation token 为 `copy_file`；无 runtime 时首次 poll
    /// 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{fs::FsError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsError> {
    ///     let _ = FsUtils::copy_file_async("source.txt", "destination.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn copy_file_async<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> impl Future<Output = Result<u64, FsError>> + 'static {
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        async move { ops::copy_file_async(source, destination).await }
    }

    /// 在调用方 Tokio runtime 中按块异步读取、处理并写入普通文件。
    ///
    /// 处理器调用保持串行，不会为每个块创建任务，也不会在库内创建 runtime 或调用
    /// `block_on`。路径、配置和处理器会被 future 持有；future 被取消时目标可能只包含部分
    /// 结果。无 runtime 时首次 poll 返回 [`FsTransferError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-async")]
    /// use axutils::{fs::{FsAsyncChunkProcessor, FsTransferError, FsTransferOptions}, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTransferError<std::convert::Infallible>> {
    ///
    ///     struct Identity;
    ///     impl FsAsyncChunkProcessor for Identity {
    ///         type Error = std::convert::Infallible;
    ///         type Future<'a> = std::future::Ready<Result<Vec<u8>, Self::Error>> where Self: 'a;
    ///
    ///         fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
    ///             std::future::ready(Ok(chunk))
    ///         }
    ///     }
    ///
    ///     let _stats = FsUtils::copy_file_with_async(
    ///         "source.bin",
    ///         "destination.bin",
    ///         FsTransferOptions::default(),
    ///         Identity,
    ///     ).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn copy_file_with_async<P, Q, C>(
        source: P,
        destination: Q,
        options: FsTransferOptions,
        processor: C,
    ) -> impl Future<Output = Result<FsTransferStats, FsTransferError<C::Error>>> + 'static
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
        C: FsAsyncChunkProcessor + 'static,
        C::Error: 'static,
    {
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        async move { transfer::copy_file_with_async(source, destination, options, processor).await }
    }

    #[cfg(feature = "fs-temp-async")]
    /// 使用默认配置创建一个异步拥有型命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let file = FsUtils::create_temp_file_async().await?;
    ///     let path = file.path().to_path_buf();
    ///     file.drop_async().await;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_file_async(
    ) -> impl Future<Output = Result<FsAsyncTempFile, FsTempError>> + 'static {
        temp::create_temp_file_async(FsTempConfig::default())
    }

    #[cfg(feature = "fs-temp-async")]
    /// 使用默认配置创建一个异步拥有型命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp-async")]
    /// use axutils::{fs::FsTempError, utils::FsUtils};
    ///
    /// async fn example() -> Result<(), FsTempError> {
    ///     let directory = FsUtils::create_temp_dir_async().await?;
    ///     let path = directory.path().to_path_buf();
    ///     directory.drop_async().await;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_dir_async(
    ) -> impl Future<Output = Result<FsAsyncTempDir, FsTempError>> + 'static {
        temp::create_temp_dir_async(FsTempConfig::default())
    }
}
