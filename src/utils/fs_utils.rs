//! `FsUtils` 无状态文件系统 facade。

#[cfg(feature = "tokio")]
use std::future::Future;
use std::path::{Path, PathBuf};

use crate::{fs::ops, fs::FsError};

/// 本地文件系统操作的无状态静态入口。
///
/// `FsUtils` 不保存句柄、根目录、权限上下文、缓存或全局状态。同步方法默认可用且会阻塞
/// 当前线程；带 `_async` 后缀的方法只在启用 `tokio` feature 时提供，是在调用时复制路径/内容
/// 并返回 owned future 的工厂函数，要求调用方持有 Tokio runtime。该库不会创建 runtime、调用
/// `block_on` 或把路径检查当作沙箱/授权保证。
///
/// # Examples
///
/// ```
/// use axutils::FsUtils;
///
/// let _tool = FsUtils;
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct FsUtils;

impl FsUtils {
    /// 查询路径是否存在；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `try_exists`）。
    ///
    /// 该方法跟随符号链接，坏链接按目标不存在处理；它不应被用作删除授权检查。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _exists = FsUtils::try_exists("example.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn try_exists<P: AsRef<Path>>(path: P) -> Result<bool, FsError> {
        ops::try_exists(path.as_ref())
    }

    /// 查询路径是否为普通文件；目标不存在返回 `Ok(false)`。
    ///
    /// 该方法跟随符号链接，权限或其他 I/O 错误不会被吞掉；错误的 operation token 为
    /// `is_file`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _is_file = FsUtils::is_file("example.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn is_file<P: AsRef<Path>>(path: P) -> Result<bool, FsError> {
        ops::is_file(path.as_ref())
    }

    /// 查询路径是否为目录；目标不存在返回 `Ok(false)`。
    ///
    /// 该方法跟随符号链接，权限或其他 I/O 错误不会被吞掉；错误的 operation token 为
    /// `is_dir`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _is_dir = FsUtils::is_dir("example-dir")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn is_dir<P: AsRef<Path>>(path: P) -> Result<bool, FsError> {
        ops::is_dir(path.as_ref())
    }

    /// 获取跟随符号链接的文件系统元数据。
    ///
    /// 返回标准库 [`std::fs::Metadata`]；I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `metadata`）。该方法不执行权限、沙箱或大小安全判断。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _metadata = FsUtils::metadata("example.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn metadata<P: AsRef<Path>>(path: P) -> Result<std::fs::Metadata, FsError> {
        ops::metadata(path.as_ref())
    }

    /// 获取最终路径项自身的元数据，不跟随符号链接；I/O 错误返回 [`FsError::Io`]（operation
    /// token 为 `symlink_metadata`）。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _metadata = FsUtils::symlink_metadata("example.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> Result<std::fs::Metadata, FsError> {
        ops::symlink_metadata(path.as_ref())
    }

    /// 创建一个空文件，使用 `create_new` 语义，不覆盖已有文件、目录或链接。
    ///
    /// 父目录必须已经存在；目标已存在时返回 [`FsError::Io`] 并保留底层 `AlreadyExists`
    /// 分类，operation token 为 `create_file`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::create_file("new-file")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn create_file<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::create_file(path.as_ref())
    }

    /// 只创建最后一级目录，不自动创建缺失的父目录。
    ///
    /// 目标已存在、父目录缺失或类型不匹配时返回 [`FsError::Io`]，operation token 为
    /// `create_dir`，并保留底层错误分类。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::create_dir("new-dir")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn create_dir<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::create_dir(path.as_ref())
    }

    /// 递归创建缺失的目录；已有目录是幂等成功。
    ///
    /// 同名文件、权限错误、组件类型不匹配或其他底层失败会返回对应的错误分类；同一目标的
    /// 并发创建允许按底层语义成功，创建过程非原子，失败可能留下部分父目录，调用方不应把它
    /// 当作事务操作；底层错误返回 [`FsError::Io`]，operation token 为 `create_dir_all`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::create_dir_all("parent/child")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::create_dir_all(path.as_ref())
    }

    /// 列出目录的直接子项，返回 `PathBuf`，不递归且不承诺排序。
    ///
    /// `max_entries` 为 0 是有效限制；读取到第 `max_entries + 1` 个子项时返回
    /// [`FsError::DirectoryEntriesTooMany`]。`usize::MAX` 在任何文件系统 I/O 前返回
    /// [`FsError::InvalidLimit`]；其他 I/O 错误返回 [`FsError::Io`]，operation token 为
    /// `list_dir`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _children = FsUtils::list_dir("example-dir", 100)?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn list_dir<P: AsRef<Path>>(path: P, max_entries: usize) -> Result<Vec<PathBuf>, FsError> {
        ops::list_dir(path.as_ref(), max_entries)
    }

    /// 删除文件或文件类符号链接，不对缺失目标静默成功。
    ///
    /// 传入目录时返回 [`FsError::Io`]；删除符号链接只删除链接本身，operation token 为
    /// `remove_file`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::remove_file("example.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn remove_file<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::remove_file(path.as_ref())
    }

    /// 删除空目录，不递归删除其内容。
    ///
    /// 非空目录、文件、链接或缺失目标按 [`FsError::Io`] 返回，operation token 为
    /// `remove_dir`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::remove_dir("empty-dir")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn remove_dir<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::remove_dir(path.as_ref())
    }

    /// 递归删除目录树及目录自身。
    ///
    /// 这是直接映射标准库/Tokio 的破坏性、非事务性操作；失败或取消后可能已经部分删除，
    /// 不提供回滚、最大深度或全局条目预算；I/O 错误返回 [`FsError::Io`]，operation token 为
    /// `remove_dir_all`。调用方应只对受信路径使用它。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::remove_dir_all("temporary-tree")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> Result<(), FsError> {
        ops::remove_dir_all(path.as_ref())
    }

    /// 直接执行 `rename` 移动文件或目录。
    ///
    /// 同一文件系统内通常具有操作系统提供的原子性；跨设备时保留底层错误，不执行
    /// copy-delete fallback。源/目标错误返回 [`FsError::PairIo`]，operation token 为
    /// `move_path`；目标冲突语义依平台而异。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::move_path("source", "destination")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn move_path<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> Result<(), FsError> {
        ops::move_path(source.as_ref(), destination.as_ref())
    }

    /// 复制一个普通文件并返回复制的字节数。
    ///
    /// 源和已存在的目标最终路径项必须是普通文件；目录、符号链接和其他非普通文件在无
    /// 竞态预检时返回 [`FsError::UnsupportedEntry`]。预检不提供抗 TOCTOU 保证，也不创建
    /// 目标父目录；其他源/目标错误返回 [`FsError::PairIo`]，operation token 为 `copy_file`；
    /// 失败或取消可能留下部分目标文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _bytes = FsUtils::copy_file("source.txt", "destination.txt")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> Result<u64, FsError> {
        ops::copy_file(source.as_ref(), destination.as_ref())
    }

    /// 在 `max_bytes` 上限内流式读取二进制内容。
    ///
    /// 内部使用 `open + take(max_bytes + 1)`，先通过 checked addition 和 `u64` 转换验证预算，
    /// 不依赖 metadata 作为唯一防线。0 是有效上限；实际读到超限字节时返回
    /// [`FsError::FileTooLarge`]；无效上限返回 [`FsError::InvalidLimit`]，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `read_bytes`），特殊文件仍可能阻塞。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _bytes = FsUtils::read_bytes("example.bin", 1024)?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn read_bytes<P: AsRef<Path>>(path: P, max_bytes: usize) -> Result<Vec<u8>, FsError> {
        ops::read_bytes(path.as_ref(), max_bytes)
    }

    /// 在 `max_bytes` 上限内读取并严格解码为 UTF-8 字符串。
    ///
    /// 不剥离 BOM、不替换非法字节；非法 UTF-8 返回 [`FsError::NotUtf8`]，大小超限返回
    /// [`FsError::FileTooLarge`]；无效上限返回 [`FsError::InvalidLimit`]，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `read_to_string`）。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// let _text = FsUtils::read_to_string("example.txt", 1024)?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn read_to_string<P: AsRef<Path>>(path: P, max_bytes: usize) -> Result<String, FsError> {
        ops::read_to_string(path.as_ref(), max_bytes)
    }

    /// 创建或截断目标文件并写完 `contents`。
    ///
    /// 不自动创建父目录、不保证原子更新；异常或并发可能留下部分文件。空内容也会创建或
    /// 截断文件；I/O 错误返回 [`FsError::Io`]，operation token 为 `write`。它采用普通打开语义，
    /// 可能跟随目标最终项的符号链接。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::write("example.txt", b"content")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), FsError> {
        ops::write(path.as_ref(), contents.as_ref())
    }

    /// 以追加模式创建（若不存在）并写入 `contents`。
    ///
    /// 不承诺多进程或多任务下的记录级原子性，也不保证异常后目标内容完整。它采用普通打开
    /// 语义，可能跟随目标最终项的符号链接；I/O 错误返回 [`FsError::Io`]，operation token 为
    /// `append`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::FsUtils;
    /// FsUtils::append("example.log", b"line\n")?;
    /// # Ok::<(), axutils::FsError>(())
    /// ```
    pub fn append<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), FsError> {
        ops::append(path.as_ref(), contents.as_ref())
    }

    /// 在 Tokio runtime 中异步查询路径是否存在。
    ///
    /// 仅在 `tokio` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `try_exists`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "tokio")]
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::try_exists_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn try_exists_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::try_exists_async(path).await }
    }

    /// 在 Tokio runtime 中异步查询路径是否为普通文件。
    ///
    /// 仅在 `tokio` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `is_file`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::is_file_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn is_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::is_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步查询路径是否为目录。
    ///
    /// 仅在 `tokio` feature 下提供；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `is_dir`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::is_dir_async("example-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn is_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<bool, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::is_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步获取跟随符号链接的元数据。
    ///
    /// 仅在 `tokio` feature 下提供；返回标准库 [`std::fs::Metadata`]，I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `metadata`），无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::metadata_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn metadata_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::metadata_async(path).await }
    }

    /// 在 Tokio runtime 中异步获取最终路径项自身的元数据。
    ///
    /// 仅在 `tokio` feature 下提供，不跟随符号链接；I/O 错误返回 [`FsError::Io`]（operation
    /// token 为 `symlink_metadata`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::symlink_metadata_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn symlink_metadata_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::symlink_metadata_async(path).await }
    }

    /// 在 Tokio runtime 中异步创建一个不覆盖已有目标的空文件。
    ///
    /// 仅在 `tokio` feature 下提供；目标已存在、父目录缺失或其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `create_file`）；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::create_file_async("new-file").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn create_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步创建最后一级目录。
    ///
    /// 仅在 `tokio` feature 下提供；不会自动创建父目录，底层失败返回 [`FsError::Io`]（operation
    /// token 为 `create_dir`）；无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::create_dir_async("new-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn create_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步递归创建目录。
    ///
    /// 仅在 `tokio` feature 下提供；已有目录是幂等成功，同名文件、权限、组件类型或其他
    /// 底层失败返回 [`FsError::Io`]（operation token 为 `create_dir_all`）；无 runtime 时首次
    /// poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::create_dir_all_async("parent/child").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn create_dir_all_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::create_dir_all_async(path).await }
    }

    /// 在 Tokio runtime 中异步列出目录直接子项，并在观察到第 `max_entries + 1` 项时停止。
    ///
    /// 仅在 `tokio` feature 下提供；只列直接子项且不保证排序，观察到第 `max_entries + 1`
    /// 项时返回 [`FsError::DirectoryEntriesTooMany`]。无效限制在任何 I/O 和 runtime 检查前
    /// 返回 [`FsError::InvalidLimit`]；有效限制但无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]，其他 I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `list_dir`）。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::list_dir_async("example-dir", 100).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn list_dir_async<P: AsRef<Path>>(
        path: P,
        max_entries: usize,
    ) -> impl Future<Output = Result<Vec<PathBuf>, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::list_dir_async(path, max_entries).await }
    }

    /// 在 Tokio runtime 中异步删除文件或文件类符号链接。
    ///
    /// 仅在 `tokio` feature 下提供；缺失目标不会静默成功，I/O 错误返回 [`FsError::Io`]（operation
    /// token 为 `remove_file`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::remove_file_async("example.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn remove_file_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_file_async(path).await }
    }

    /// 在 Tokio runtime 中异步删除空目录。
    ///
    /// 仅在 `tokio` feature 下提供；非空目录、文件、链接或缺失目标按 [`FsError::Io`] 返回
    /// （operation token 为 `remove_dir`），无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::remove_dir_async("empty-dir").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn remove_dir_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_dir_async(path).await }
    }

    /// 在 Tokio runtime 中异步递归删除目录树及目录自身。
    ///
    /// 仅在 `tokio` feature 下提供；I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `remove_dir_all`），操作不可回滚，取消或错误可能留下部分结果；无 runtime 时首次 poll
    /// 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::remove_dir_all_async("temporary-tree").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn remove_dir_all_async<P: AsRef<Path>>(
        path: P,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::remove_dir_all_async(path).await }
    }

    /// 在 Tokio runtime 中异步直接执行 `rename` 移动文件或目录。
    ///
    /// 仅在 `tokio` feature 下提供；跨设备时不会执行 copy-delete fallback，源/目标错误返回
    /// [`FsError::PairIo`]（operation token 为 `move_path`），无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::move_path_async("source", "destination").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn move_path_async<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        async move { ops::move_path_async(source, destination).await }
    }

    /// 在 Tokio runtime 中异步复制普通文件并返回字节数。
    ///
    /// 仅在 `tokio` feature 下提供；目录、链接和其他非普通最终路径项在无竞态预检时被拒绝，
    /// 返回 [`FsError::UnsupportedEntry`]；其他源/目标错误返回 [`FsError::PairIo`]，预检不提供
    /// 抗 TOCTOU 保证；源/目标错误的 operation token 为 `copy_file`；无 runtime 时首次 poll
    /// 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::copy_file_async("source.txt", "destination.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn copy_file_async<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> impl Future<Output = Result<u64, FsError>> + 'static {
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        async move { ops::copy_file_async(source, destination).await }
    }

    /// 在 Tokio runtime 中异步受限读取二进制内容。
    ///
    /// 仅在 `tokio` feature 下提供；无效 `max_bytes` 在首次 poll 时先返回
    /// [`FsError::InvalidLimit`]，实际内容超限返回 [`FsError::FileTooLarge`]，其他 I/O 错误
    /// 返回 [`FsError::Io`]（operation token 为 `read_bytes`）；有效限制但无 runtime 时返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::read_bytes_async("example.bin", 1024).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn read_bytes_async<P: AsRef<Path>>(
        path: P,
        max_bytes: usize,
    ) -> impl Future<Output = Result<Vec<u8>, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::read_bytes_async(path, max_bytes).await }
    }

    /// 在 Tokio runtime 中异步受限读取并严格解码 UTF-8。
    ///
    /// 仅在 `tokio` feature 下提供；不剥离 BOM，不替换非法字节；限制超出返回
    /// [`FsError::FileTooLarge`]，非法 UTF-8 返回 [`FsError::NotUtf8`]，无效限制返回
    /// [`FsError::InvalidLimit`]，其他 I/O 错误返回 [`FsError::Io`]（operation token 为
    /// `read_to_string`）；有效限制但无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     let _ = axutils::FsUtils::read_to_string_async("example.txt", 1024).await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn read_to_string_async<P: AsRef<Path>>(
        path: P,
        max_bytes: usize,
    ) -> impl Future<Output = Result<String, FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        async move { ops::read_to_string_async(path, max_bytes).await }
    }

    /// 在 Tokio runtime 中异步创建或截断文件并写入内容。
    ///
    /// 仅在 `tokio` feature 下提供；不会自动创建父目录或保证原子更新，I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `write`），异步取消可能留下部分结果；无 runtime 时首次 poll 返回
    /// [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::write_async("example.txt", b"content").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
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
    /// 仅在 `tokio` feature 下提供；不承诺跨任务记录级原子性，I/O 错误返回 [`FsError::Io`]
    /// （operation token 为 `append`）；无 runtime 时首次 poll 返回 [`FsError::RuntimeRequired`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// async fn example() -> Result<(), axutils::FsError> {
    ///     axutils::FsUtils::append_async("example.log", b"line\\n").await?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn append_async<P: AsRef<Path>, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> impl Future<Output = Result<(), FsError>> + 'static {
        let path = path.as_ref().to_path_buf();
        let contents = contents.as_ref().to_vec();
        async move { ops::append_async(path, contents).await }
    }
}
