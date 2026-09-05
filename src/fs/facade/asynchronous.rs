//! `FsUtils` 的一般异步文件系统入口。

#[cfg(feature = "fs-async")]
use std::{
    future::Future,
    path::{Path, PathBuf},
};

#[cfg(feature = "fs-async")]
use super::super::{ops, FsError};
use super::FsUtils;

impl FsUtils {
    /// 在 Tokio runtime 中异步查询路径是否存在。
    ///
    /// 仅在 `fs-async` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `try_exists`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-async")]
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::try_exists_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn try_exists_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::try_exists_async(path).await }
    }

    /// 在 Tokio runtime 中异步查询路径是否为普通文件。
    ///
    /// 仅在 `fs-async` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `is_file`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::is_file_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn is_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::is_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步查询路径是否为目录。
    ///
    /// 仅在 `fs-async` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `is_dir`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::is_dir_async("example-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn is_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::is_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步获取跟随符号链接的元数据。
    ///
    /// 仅在 `fs-async` feature 下提供；返回标准库 [`std::fs::Metadata`]，I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `metadata`），无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::metadata_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn metadata_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::metadata_async(path).await }
    }

    /// 在 Tokio runtime 中异步获取最终路径项自身的元数据。
    ///
    /// 仅在 `fs-async` feature 下提供，不跟随符号链接；I/O 错误返回 [`FsError::Io`]（operation
    /// token 为 `symlink_metadata`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::symlink_metadata_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn symlink_metadata_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::symlink_metadata_async(path).await }
    }

    /// 在 Tokio runtime 中异步创建一个不覆盖已有目标的空文件。
    ///
    /// 仅在 `fs-async` feature 下提供；目标已存在、父目录缺失或其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `create_file`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::create_file_async("new-file").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn create_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步创建最后一级目录。
    ///
    /// 仅在 `fs-async` feature 下提供；不会自动创建父目录，底层失败返回 [`FsError::Io`]（operation
    /// token 为 `create_dir`）；无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::create_dir_async("new-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn create_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步递归创建目录。
    ///
    /// 仅在 `fs-async` feature 下提供；已有目录是幂等成功，同名文件、权限、组件类型或其他
    /// 底层失败返回 [`FsError::Io`]（operation token 为 `create_dir_all`）；无 runtime 时首次
    /// poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::create_dir_all_async("parent/child").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn create_dir_all_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_dir_all_async(path).await }
    }

    /// 在 Tokio runtime 中异步列出目录直接子项，并在观察到第 `max_entries + 1` 项时停止。
    ///
    /// 仅在 `fs-async` feature 下提供；只列直接子项且不保证排序，观察到第 `max_entries + 1`
    /// 项时返回 [`FsError::DirectoryEntriesTooMany`]。无效限制在任何 I/O 和 runtime 检查前
    /// 返回 [`FsError::InvalidLimit`]；有效限制但无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]，其他 I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `list_dir`）。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::list_dir_async("example-dir", 100).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn list_dir_async<P: AsRef<Path>>(
        path: P,
        max_entries: usize,
    ) -> impl Future<Output = Result<Vec<PathBuf>, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::list_dir_async(path, max_entries).await }
    }

    /// 在 Tokio runtime 中异步删除文件或文件类符号链接。
    ///
    /// 仅在 `fs-async` feature 下提供；缺失目标不会静默成功，I/O 错误返回 [`FsError::Io`]（operation
    /// token 为 `remove_file`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::remove_file_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn remove_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步删除空目录。
    ///
    /// 仅在 `fs-async` feature 下提供；非空目录、文件、链接或缺失目标按 [`FsError::Io`] 返回
    /// （operation token 为 `remove_dir`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::remove_dir_async("empty-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn remove_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步递归删除目录树及目录自身。
    ///
    /// 仅在 `fs-async` feature 下提供；I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `remove_dir_all`），操作不可回滚，取消或错误可能留下部分结果；无 runtime 时首次 poll
    /// 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::remove_dir_all_async("temporary-tree").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn remove_dir_all_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_dir_all_async(path).await }
    }

    /// 在 Tokio runtime 中异步直接执行 `rename` 移动文件或目录。
    ///
    /// 仅在 `fs-async` feature 下提供；跨设备时不会执行 copy-delete fallback，源/目标错误返回
    /// [`FsError::PairIo`]（operation token 为 `move_path`），无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::move_path_async("source", "destination").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn move_path_async<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        async move { ops::move_path_async(source, destination).await }
    }

    /// 在 Tokio runtime 中异步受限读取二进制内容。
    ///
    /// 仅在 `fs-async` feature 下提供；无效 `max_bytes` 在首次 poll 时先返回
    /// [`FsError::InvalidLimit`]，实际内容超限返回 [`FsError::FileTooLarge`]，其他 I/O 错误
    /// 返回 [`FsError::Io`]（operation token 为 `read_bytes`）；有效限制但无 runtime 时返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::read_bytes_async("example.bin", 1024).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn read_bytes_async<P: AsRef<Path>>(
        path: P,
        max_bytes: usize,
    ) -> impl Future<Output = Result<Vec<u8>, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::read_bytes_async(path, max_bytes).await }
    }

    /// 在 Tokio runtime 中异步受限读取并严格解码 UTF-8。
    ///
    /// 仅在 `fs-async` feature 下提供；不剥离 BOM，不替换非法字节；限制超出返回
    /// [`FsError::FileTooLarge`]，非法 UTF-8 返回 [`FsError::NotUtf8`]，无效限制返回
    /// [`FsError::InvalidLimit`]，其他 I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `read_to_string`）；有效限制但无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     let _ = axutils::utils::FsUtils::read_to_string_async("example.txt", 1024).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn read_to_string_async<P: AsRef<Path>>(
        path: P,
        max_bytes: usize,
    ) -> impl Future<Output = Result<String, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::read_to_string_async(path, max_bytes).await }
    }

    /// 在 Tokio runtime 中异步创建或截断文件并写入内容。
    ///
    /// 仅在 `fs-async` feature 下提供；不会自动创建父目录或保证原子更新，I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `write`），异步取消可能留下部分结果；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::write_async("example.txt", b"content").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn write_async<P: AsRef<Path>, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        let contents = contents.as_ref().to_vec();
        async move { ops::write_async(path, contents).await }
    }

    /// 在 Tokio runtime 中异步追加内容，必要时创建目标文件。
    ///
    /// 仅在 `fs-async` feature 下提供；不承诺跨任务记录级原子性，I/O 错误返回 [`FsError::Io`]
    /// （operation token 为 `append`）；无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::fs::FsError> {
    ///     axutils::utils::FsUtils::append_async("example.log", b"line\\n").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "fs-async")]
    pub fn append_async<P: AsRef<Path>, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        let contents = contents.as_ref().to_vec();
        async move { ops::append_async(path, contents).await }
    }
}
