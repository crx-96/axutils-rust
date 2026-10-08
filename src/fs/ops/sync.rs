//! 同步文件系统执行；不启动线程或 runtime。

use super::{
    ensure_regular_file, io_error, pair_io_error, read_budget, validate_copy_paths,
    validate_max_entries, FsError, OP_APPEND, OP_COPY_FILE, OP_CREATE_DIR, OP_CREATE_DIR_ALL,
    OP_CREATE_FILE, OP_IS_DIR, OP_IS_FILE, OP_LIST_DIR, OP_METADATA, OP_MOVE_PATH, OP_READ_BYTES,
    OP_READ_TO_STRING, OP_REMOVE_DIR, OP_REMOVE_DIR_ALL, OP_REMOVE_FILE, OP_SYMLINK_METADATA,
    OP_TRY_EXISTS, OP_WRITE,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// 跟随最终链接查询目标是否存在，仅将 NotFound 映射为 false。
pub(crate) fn try_exists(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_TRY_EXISTS, path, &error)),
    }
}

/// 跟随最终链接检查普通文件类型，仅将 NotFound 映射为 false。
pub(crate) fn is_file(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_FILE, path, &error)),
    }
}

/// 跟随最终链接检查目录类型，仅将 NotFound 映射为 false。
pub(crate) fn is_dir(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_DIR, path, &error)),
    }
}

/// 读取跟随最终链接的元数据，保留失败的操作分类。
pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata, FsError> {
    fs::metadata(path).map_err(|error| io_error(OP_METADATA, path, &error))
}

/// 读取最终路径项本身的元数据，不跟随该链接。
pub(crate) fn symlink_metadata(path: &Path) -> Result<fs::Metadata, FsError> {
    fs::symlink_metadata(path).map_err(|error| io_error(OP_SYMLINK_METADATA, path, &error))
}

/// 通过 create_new 独占创建空文件，拒绝覆盖已有路径。
pub(crate) fn create_file(path: &Path) -> Result<(), FsError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| io_error(OP_CREATE_FILE, path, &error))
}

/// 创建单层目录，不隐式创建缺失的父目录。
pub(crate) fn create_dir(path: &Path) -> Result<(), FsError> {
    fs::create_dir(path).map_err(|error| io_error(OP_CREATE_DIR, path, &error))
}

/// 按底层语义递归创建目录，失败时可能保留已创建部分。
pub(crate) fn create_dir_all(path: &Path) -> Result<(), FsError> {
    fs::create_dir_all(path).map_err(|error| io_error(OP_CREATE_DIR_ALL, path, &error))
}

/// 限制直接子项数量，读取到超额项即失败且不返回部分列表。
pub(crate) fn list_dir(path: &Path, max_entries: usize) -> Result<Vec<PathBuf>, FsError> {
    validate_max_entries(max_entries)?;

    // 边读取边计数，避免先收集无界目录内容。
    let entries = fs::read_dir(path).map_err(|error| io_error(OP_LIST_DIR, path, &error))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_error(OP_LIST_DIR, path, &error))?;
        if paths.len() == max_entries {
            return Err(FsError::DirectoryEntriesTooMany {
                path: path.to_path_buf(),
                limit: max_entries,
            });
        }
        paths.push(entry.path());
    }
    Ok(paths)
}

/// 删除单个文件或文件链接，错误不静默吞掉。
pub(crate) fn remove_file(path: &Path) -> Result<(), FsError> {
    fs::remove_file(path).map_err(|error| io_error(OP_REMOVE_FILE, path, &error))
}

/// 删除空目录，将非空或缺失等底层失败交给调用方。
pub(crate) fn remove_dir(path: &Path) -> Result<(), FsError> {
    fs::remove_dir(path).map_err(|error| io_error(OP_REMOVE_DIR, path, &error))
}

/// 递归删除目录，失败后可能保留部分结果且不执行回滚。
pub(crate) fn remove_dir_all(path: &Path) -> Result<(), FsError> {
    fs::remove_dir_all(path).map_err(|error| io_error(OP_REMOVE_DIR_ALL, path, &error))
}

/// 沿用文件系统 rename 语义移动路径，不进行跨设备复制回退。
pub(crate) fn move_path(source: &Path, destination: &Path) -> Result<(), FsError> {
    fs::rename(source, destination)
        .map_err(|error| pair_io_error(OP_MOVE_PATH, source, destination, &error))
}

/// 预检词法路径与普通文件类型后复制，失败可能留下部分目标内容。
pub(crate) fn copy_file(source: &Path, destination: &Path) -> Result<u64, FsError> {
    // 先拒绝自复制，避免平台相关的截断或错误行为。
    validate_copy_paths(source, destination)?;
    // 分别检查最终源和目标项；目标可缺失，链接和特殊文件保持拒绝。
    let source_is_file = ensure_regular_file(
        OP_COPY_FILE,
        source,
        fs::symlink_metadata(source),
        source,
        destination,
        false,
    )?;
    debug_assert!(source_is_file);

    let _destination_exists = ensure_regular_file(
        OP_COPY_FILE,
        destination,
        fs::symlink_metadata(destination),
        source,
        destination,
        true,
    )?;

    // 预检通过后交给标准库复制；两次系统调用之间仍可能发生路径变化。
    fs::copy(source, destination)
        .map_err(|error| pair_io_error(OP_COPY_FILE, source, destination, &error))
}

/// 按上限加一字节读取，超过预算时失败并保留调用入口的操作分类。
fn read_bytes_with_operation(
    path: &Path,
    max_bytes: usize,
    operation: &'static str,
) -> Result<Vec<u8>, FsError> {
    // 预算先验校验；额外一字节用于区分恰好达到上限和真正超限。
    let budget = read_budget(max_bytes)?;
    let mut file = File::open(path).map_err(|error| io_error(operation, path, &error))?;
    let mut buffer = Vec::new();
    Read::by_ref(&mut file)
        .take(budget)
        .read_to_end(&mut buffer)
        .map_err(|error| io_error(operation, path, &error))?;

    // 超限内容不会交给调用方，且读取阶段最多保留预算允许的字节。
    if buffer.len() > max_bytes {
        return Err(FsError::FileTooLarge {
            path: path.to_path_buf(),
            limit: max_bytes,
        });
    }
    Ok(buffer)
}

/// 读取有界原始字节，不解释编码。
pub(crate) fn read_bytes(path: &Path, max_bytes: usize) -> Result<Vec<u8>, FsError> {
    read_bytes_with_operation(path, max_bytes, OP_READ_BYTES)
}

/// 有界读取后执行严格 UTF-8 校验，不进行有损转换。
pub(crate) fn read_to_string(path: &Path, max_bytes: usize) -> Result<String, FsError> {
    let buffer = read_bytes_with_operation(path, max_bytes, OP_READ_TO_STRING)?;
    // 编码错误只留下路径，不将非法原始字节保存在公共错误中。
    String::from_utf8(buffer).map_err(|_| FsError::NotUtf8 {
        path: path.to_path_buf(),
    })
}

/// 创建或截断后写入全部内容，沿用底层非原子写入语义。
pub(crate) fn write(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    fs::write(path, contents).map_err(|error| io_error(OP_WRITE, path, &error))
}

/// 以追加模式写入内容，文件缺失时创建；不保证跨调用的业务原子性。
pub(crate) fn append(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    // 用内核追加模式打开，避免先读取长度再 seek 带来的额外竞态。
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| io_error(OP_APPEND, path, &error))?;
    file.write_all(contents)
        .map_err(|error| io_error(OP_APPEND, path, &error))
}
