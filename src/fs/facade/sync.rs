//! `FsUtils` 的同步基础、传输和临时资源入口。

use std::path::{Path, PathBuf};

use super::super::{
    ops, transfer, FsChunkProcessor, FsError, FsTransferError, FsTransferOptions, FsTransferStats,
};
use super::FsUtils;

#[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
use super::super::{temp, FsTempConfig, FsUtilsContext};

#[cfg(feature = "fs-temp")]
use super::super::{FsTempDir, FsTempError, FsTempFile};

impl FsUtils {
    /// 查询路径是否存在；目标不存在返回 `Ok(false)`，其他 I/O 错误返回
    /// [`FsError::Io`]（operation token 为 `try_exists`）。
    ///
    /// 该方法跟随符号链接，坏链接按目标不存在处理；它不应被用作删除授权检查。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::utils::FsUtils;
    /// let _exists = FsUtils::try_exists("example.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _is_file = FsUtils::is_file("example.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _is_dir = FsUtils::is_dir("example-dir")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _metadata = FsUtils::metadata("example.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _metadata = FsUtils::symlink_metadata("example.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::create_file("new-file")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::create_dir("new-dir")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::create_dir_all("parent/child")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _children = FsUtils::list_dir("example-dir", 100)?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::remove_file("example.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::remove_dir("empty-dir")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::remove_dir_all("temporary-tree")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::move_path("source", "destination")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _bytes = FsUtils::copy_file("source.txt", "destination.txt")?;
    /// # Ok::<(), axutils::fs::FsError>(())
    /// ```
    pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(
        source: P,
        destination: Q,
    ) -> Result<u64, FsError> {
        ops::copy_file(source.as_ref(), destination.as_ref())
    }

    /// 按块读取普通源文件，经处理器转换后流式写入目标文件。
    ///
    /// `chunk_size` 必须在 1 KiB 到 16 MiB 之间；默认值为 64 KiB。处理器按串行顺序收到
    /// 拥有所有权的 `Vec<u8>`，返回的 `Vec<u8>` 会在通过可选累计输出上限检查后写入目标。
    /// 源和已存在的目标最终路径项必须是普通文件；目标会被截断，目标父目录不会自动创建。
    /// 预检只使用词法路径比较和 `symlink_metadata`，不提供 canonicalize、硬链接别名检测、
    /// 原子替换或抗 TOCTOU 保证；错误或取消可能留下部分目标内容。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{fs::{FsChunkProcessor, FsTransferOptions}, utils::FsUtils};
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
    /// let _stats = FsUtils::copy_file_with(
    ///     "source.bin",
    ///     "destination.bin",
    ///     FsTransferOptions::default(),
    ///     Identity,
    /// )?;
    /// # Ok::<(), axutils::fs::FsTransferError<std::convert::Infallible>>(())
    /// ```
    pub fn copy_file_with<P, Q, C>(
        source: P,
        destination: Q,
        options: FsTransferOptions,
        processor: C,
    ) -> Result<FsTransferStats, FsTransferError<C::Error>>
    where
        P: AsRef<Path>,
        Q: AsRef<Path>,
        C: FsChunkProcessor,
    {
        transfer::copy_file_with(
            source.as_ref().to_path_buf(),
            destination.as_ref().to_path_buf(),
            options,
            processor,
        )
    }

    #[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
    /// 使用显式配置创建一个持有临时目录策略的 `FsUtilsContext`。
    ///
    /// 构造 context 不访问文件系统，也不改变进程级临时目录；指定父目录在实际创建时必须
    /// 已经存在。`FsUtils` 本身仍是兼容既有调用方的 unit struct。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
    /// {
    ///     use axutils::{fs::FsTempConfig, utils::FsUtils};
    ///
    ///     let context = FsUtils::with_temp_config(
    ///         FsTempConfig::new().with_prefix("axutils-").with_suffix(".tmp"),
    ///     );
    ///     assert_eq!(context.config().prefix.as_deref(), Some("axutils-"));
    /// }
    /// ```
    pub fn with_temp_config(config: FsTempConfig) -> FsUtilsContext {
        temp::context(config)
    }

    #[cfg(feature = "fs-temp")]
    /// 使用默认配置创建一个同步拥有型命名临时文件。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// fn example() -> Result<(), axutils::fs::FsTempError> {
    ///     let file = axutils::utils::FsUtils::create_temp_file()?;
    ///     let path = file.path().to_path_buf();
    ///     file.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_file() -> Result<FsTempFile, FsTempError> {
        temp::create_temp_file(&FsTempConfig::default())
    }

    #[cfg(feature = "fs-temp")]
    /// 使用默认配置创建一个同步拥有型命名临时目录。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # #[cfg(feature = "fs-temp")]
    /// fn example() -> Result<(), axutils::fs::FsTempError> {
    ///     let directory = axutils::utils::FsUtils::create_temp_dir()?;
    ///     let path = directory.path().to_path_buf();
    ///     directory.close()?;
    ///     assert!(!path.exists());
    ///     Ok(())
    /// }
    /// ```
    pub fn create_temp_dir() -> Result<FsTempDir, FsTempError> {
        temp::create_temp_dir(&FsTempConfig::default())
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
    /// use axutils::utils::FsUtils;
    /// let _bytes = FsUtils::read_bytes("example.bin", 1024)?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// let _text = FsUtils::read_to_string("example.txt", 1024)?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::write("example.txt", b"content")?;
    /// # Ok::<(), axutils::fs::FsError>(())
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
    /// use axutils::utils::FsUtils;
    /// FsUtils::append("example.log", b"line\n")?;
    /// # Ok::<(), axutils::fs::FsError>(())
    /// ```
    pub fn append<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), FsError> {
        ops::append(path.as_ref(), contents.as_ref())
    }
}
