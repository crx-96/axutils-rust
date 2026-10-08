//! Tokio 文件系统执行，拥有调用入口复制的路径与内容。

use super::{
    ensure_regular_file, io_error, pair_io_error, read_budget, validate_copy_paths,
    validate_max_entries, FsError, OP_APPEND, OP_COPY_FILE, OP_CREATE_DIR, OP_CREATE_DIR_ALL,
    OP_CREATE_FILE, OP_IS_DIR, OP_IS_FILE, OP_LIST_DIR, OP_METADATA, OP_MOVE_PATH, OP_READ_BYTES,
    OP_READ_TO_STRING, OP_REMOVE_DIR, OP_REMOVE_DIR_ALL, OP_REMOVE_FILE, OP_SYMLINK_METADATA,
    OP_TRY_EXISTS, OP_WRITE,
};
use std::{fs, io, path::PathBuf};
use tokio::{
    fs::{self as async_fs, File as AsyncFile, OpenOptions as AsyncOpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::Handle,
};

/// 确认首次执行位于调用方 Tokio context，缺少时在 I/O 前返回稳定错误。
pub(crate) fn ensure_runtime() -> Result<(), FsError> {
    Handle::try_current()
        .map(|_| ())
        .map_err(|_| FsError::RuntimeRequired)
}

/// 在调用方 runtime 中跟随最终链接查询目标是否存在，仅将 NotFound 映射为 false。
pub(crate) async fn try_exists_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match async_fs::metadata(&path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_TRY_EXISTS, &path, &error)),
    }
}

/// 在调用方 runtime 中跟随最终链接检查普通文件类型，仅将 NotFound 映射为 false。
pub(crate) async fn is_file_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match async_fs::metadata(&path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_FILE, &path, &error)),
    }
}

/// 在调用方 runtime 中跟随最终链接检查目录类型，仅将 NotFound 映射为 false。
pub(crate) async fn is_dir_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match async_fs::metadata(&path).await {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_DIR, &path, &error)),
    }
}

/// 在调用方 runtime 中读取跟随最终链接的元数据，保留失败的操作分类。
pub(crate) async fn metadata_async(path: PathBuf) -> Result<fs::Metadata, FsError> {
    ensure_runtime()?;
    async_fs::metadata(&path)
        .await
        .map_err(|error| io_error(OP_METADATA, &path, &error))
}

/// 在调用方 runtime 中读取最终路径项本身的元数据，不跟随该链接。
pub(crate) async fn symlink_metadata_async(path: PathBuf) -> Result<fs::Metadata, FsError> {
    ensure_runtime()?;
    async_fs::symlink_metadata(&path)
        .await
        .map_err(|error| io_error(OP_SYMLINK_METADATA, &path, &error))
}

/// 在调用方 runtime 中通过 create_new 独占创建空文件，拒绝覆盖已有路径。
pub(crate) async fn create_file_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    AsyncOpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map(|_| ())
        .map_err(|error| io_error(OP_CREATE_FILE, &path, &error))
}

/// 在调用方 runtime 中创建单层目录，不隐式创建缺失的父目录。
pub(crate) async fn create_dir_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::create_dir(&path)
        .await
        .map_err(|error| io_error(OP_CREATE_DIR, &path, &error))
}

/// 在调用方 runtime 中按底层语义递归创建目录，失败时可能保留已创建部分。
pub(crate) async fn create_dir_all_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::create_dir_all(&path)
        .await
        .map_err(|error| io_error(OP_CREATE_DIR_ALL, &path, &error))
}

/// 在调用方 runtime 中限制直接子项数量，读取到超额项即失败且不返回部分列表。
pub(crate) async fn list_dir_async(
    path: PathBuf,
    max_entries: usize,
) -> Result<Vec<PathBuf>, FsError> {
    validate_max_entries(max_entries)?;
    ensure_runtime()?;

    // 边读取边计数；取消时只丢弃本次列表，不产生文件修改。
    let mut entries = async_fs::read_dir(&path)
        .await
        .map_err(|error| io_error(OP_LIST_DIR, &path, &error))?;
    let mut paths = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| io_error(OP_LIST_DIR, &path, &error))?
    {
        if paths.len() == max_entries {
            return Err(FsError::DirectoryEntriesTooMany {
                path,
                limit: max_entries,
            });
        }
        paths.push(entry.path());
    }
    Ok(paths)
}

/// 在调用方 runtime 中删除单个文件或文件链接，错误不静默吞掉。
pub(crate) async fn remove_file_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::remove_file(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_FILE, &path, &error))
}

/// 在调用方 runtime 中删除空目录，将非空或缺失等底层失败交给调用方。
pub(crate) async fn remove_dir_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::remove_dir(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_DIR, &path, &error))
}

/// 在调用方 runtime 中递归删除目录，失败后可能保留部分结果且不执行回滚。
pub(crate) async fn remove_dir_all_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::remove_dir_all(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_DIR_ALL, &path, &error))
}

/// 在调用方 runtime 中沿用文件系统 rename 语义移动路径，不进行跨设备复制回退。
pub(crate) async fn move_path_async(source: PathBuf, destination: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::rename(&source, &destination)
        .await
        .map_err(|error| pair_io_error(OP_MOVE_PATH, &source, &destination, &error))
}

/// 在调用方 runtime 中预检词法路径与普通文件类型后复制，失败可能留下部分目标内容。
pub(crate) async fn copy_file_async(source: PathBuf, destination: PathBuf) -> Result<u64, FsError> {
    // 无副作用的词法检查先于 runtime 和 I/O。
    validate_copy_paths(&source, &destination)?;
    ensure_runtime()?;

    // 分别检查最终源和目标项；目标可缺失，链接和特殊文件保持拒绝。
    let source_is_file = ensure_regular_file(
        OP_COPY_FILE,
        &source,
        async_fs::symlink_metadata(&source).await,
        &source,
        &destination,
        false,
    )?;
    debug_assert!(source_is_file);

    let _destination_exists = ensure_regular_file(
        OP_COPY_FILE,
        &destination,
        async_fs::symlink_metadata(&destination).await,
        &source,
        &destination,
        true,
    )?;

    // 预检通过后执行复制；取消并不撤销已经交给 Tokio blocking pool 的文件操作。
    async_fs::copy(&source, &destination)
        .await
        .map_err(|error| pair_io_error(OP_COPY_FILE, &source, &destination, &error))
}

/// 在调用方 runtime 中按上限加一字节读取，超过预算时失败并保留调用入口的操作分类。
async fn read_bytes_with_operation_async(
    path: PathBuf,
    max_bytes: usize,
    operation: &'static str,
) -> Result<Vec<u8>, FsError> {
    // 预算先验校验；额外一字节用于区分恰好达到上限和真正超限。
    let budget = read_budget(max_bytes)?;
    ensure_runtime()?;

    let file = AsyncFile::open(&path)
        .await
        .map_err(|error| io_error(operation, &path, &error))?;
    let mut buffer = Vec::new();
    file.take(budget)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| io_error(operation, &path, &error))?;

    // 超限内容不会交给调用方，且读取阶段最多保留预算允许的字节。
    if buffer.len() > max_bytes {
        return Err(FsError::FileTooLarge {
            path,
            limit: max_bytes,
        });
    }
    Ok(buffer)
}

/// 在调用方 runtime 中读取有界原始字节，不解释编码。
pub(crate) async fn read_bytes_async(path: PathBuf, max_bytes: usize) -> Result<Vec<u8>, FsError> {
    read_bytes_with_operation_async(path, max_bytes, OP_READ_BYTES).await
}

/// 在调用方 runtime 中有界读取后执行严格 UTF-8 校验，不进行有损转换。
pub(crate) async fn read_to_string_async(
    path: PathBuf,
    max_bytes: usize,
) -> Result<String, FsError> {
    let path_for_error = path.clone();
    let buffer = read_bytes_with_operation_async(path, max_bytes, OP_READ_TO_STRING).await?;
    // 保留错误路径副本，编码失败时不将读取内容暴露到错误对象。
    String::from_utf8(buffer).map_err(|_| FsError::NotUtf8 {
        path: path_for_error,
    })
}

/// 在调用方 runtime 中创建或截断后写入全部内容，沿用底层非原子写入语义。
pub(crate) async fn write_async(path: PathBuf, contents: Vec<u8>) -> Result<(), FsError> {
    ensure_runtime()?;
    async_fs::write(&path, contents)
        .await
        .map_err(|error| io_error(OP_WRITE, &path, &error))
}

/// 在调用方 runtime 中以追加模式写入内容，文件缺失时创建；不保证跨调用的业务原子性。
pub(crate) async fn append_async(path: PathBuf, contents: Vec<u8>) -> Result<(), FsError> {
    ensure_runtime()?;
    // 用内核追加模式打开，完成后 flush 以等待 Tokio 的在途写入。
    let mut file = AsyncOpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))?;
    file.write_all(&contents)
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))?;
    file.flush()
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))
}
