//! 文件系统执行入口及同步/异步共用的纯校验和错误映射。

#[cfg(feature = "fs-async")]
mod asynchronous;
mod sync;

#[cfg(feature = "fs-async")]
pub(crate) use asynchronous::{
    append_async, copy_file_async, create_dir_all_async, create_dir_async, create_file_async,
    ensure_runtime, is_dir_async, is_file_async, list_dir_async, metadata_async, move_path_async,
    read_bytes_async, read_to_string_async, remove_dir_all_async, remove_dir_async,
    remove_file_async, symlink_metadata_async, try_exists_async, write_async,
};
pub(crate) use sync::{
    append, copy_file, create_dir, create_dir_all, create_file, is_dir, is_file, list_dir,
    metadata, move_path, read_bytes, read_to_string, remove_dir, remove_dir_all, remove_file,
    symlink_metadata, try_exists, write,
};

use super::FsError;
use std::{fs, io, path::Path};

/// try_exists 操作在公开错误中的稳定分类标记。
const OP_TRY_EXISTS: &str = "try_exists";
/// is_file 操作在公开错误中的稳定分类标记。
const OP_IS_FILE: &str = "is_file";
/// is_dir 操作在公开错误中的稳定分类标记。
const OP_IS_DIR: &str = "is_dir";
/// metadata 操作在公开错误中的稳定分类标记。
const OP_METADATA: &str = "metadata";
/// symlink_metadata 操作在公开错误中的稳定分类标记。
const OP_SYMLINK_METADATA: &str = "symlink_metadata";
/// create_file 操作在公开错误中的稳定分类标记。
const OP_CREATE_FILE: &str = "create_file";
/// create_dir 操作在公开错误中的稳定分类标记。
const OP_CREATE_DIR: &str = "create_dir";
/// create_dir_all 操作在公开错误中的稳定分类标记。
const OP_CREATE_DIR_ALL: &str = "create_dir_all";
/// list_dir 操作在公开错误中的稳定分类标记。
const OP_LIST_DIR: &str = "list_dir";
/// remove_file 操作在公开错误中的稳定分类标记。
const OP_REMOVE_FILE: &str = "remove_file";
/// remove_dir 操作在公开错误中的稳定分类标记。
const OP_REMOVE_DIR: &str = "remove_dir";
/// remove_dir_all 操作在公开错误中的稳定分类标记。
const OP_REMOVE_DIR_ALL: &str = "remove_dir_all";
/// move_path 操作在公开错误中的稳定分类标记。
const OP_MOVE_PATH: &str = "move_path";
/// copy_file 操作在公开错误中的稳定分类标记。
const OP_COPY_FILE: &str = "copy_file";
/// read_bytes 操作在公开错误中的稳定分类标记。
const OP_READ_BYTES: &str = "read_bytes";
/// read_to_string 操作在公开错误中的稳定分类标记。
const OP_READ_TO_STRING: &str = "read_to_string";
/// write 操作在公开错误中的稳定分类标记。
const OP_WRITE: &str = "write";
/// append 操作在公开错误中的稳定分类标记。
const OP_APPEND: &str = "append";

/// 将单路径 I/O 失败压缩为稳定操作、调用方路径与错误分类，不保存后端文本。
fn io_error(operation: &'static str, path: &Path, error: &io::Error) -> FsError {
    FsError::Io {
        operation,
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

/// 保留复制或移动两端路径与稳定错误分类，不传播原始操作系统消息。
fn pair_io_error(
    operation: &'static str,
    source: &Path,
    destination: &Path,
    error: &io::Error,
) -> FsError {
    FsError::PairIo {
        operation,
        source: source.to_path_buf(),
        destination: destination.to_path_buf(),
        kind: error.kind(),
    }
}

/// 拒绝无法保留额外观察余量的条目上界；零表示只接受空目录。
fn validate_max_entries(max_entries: usize) -> Result<(), FsError> {
    if max_entries == usize::MAX {
        Err(FsError::InvalidLimit {
            field: "max_entries",
        })
    } else {
        Ok(())
    }
}

/// 为读取上限保留一个额外探测字节，并检查 usize/u64 的可表示范围。
fn read_budget(max_bytes: usize) -> Result<u64, FsError> {
    // 先在平台 usize 内保留探测字节，再转换为 Read::take 接受的 u64 上限。
    max_bytes
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(FsError::InvalidLimit { field: "max_bytes" })
}

/// 校验最终路径项为普通文件，按调用方策略允许目标不存在；不提供抗竞态保证。
fn ensure_regular_file(
    operation: &'static str,
    path: &Path,
    metadata: Result<fs::Metadata, io::Error>,
    source: &Path,
    destination: &Path,
    allow_missing: bool,
) -> Result<bool, FsError> {
    // 使用 symlink_metadata 的最终项类型；仅允许目标缺失，其他错误保留源/目标角色。
    match metadata {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(FsError::UnsupportedEntry {
            operation,
            path: path.to_path_buf(),
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound && allow_missing => Ok(false),
        Err(error) => Err(pair_io_error(operation, source, destination, &error)),
    }
}

/// 在任何复制副作用前拒绝相同词法路径；不检测硬链接或规范化路径别名。
fn validate_copy_paths(source: &Path, destination: &Path) -> Result<(), FsError> {
    // 路径比较不访问磁盘；无法证明别名不同的输入仍遵循底层复制语义。
    if source == destination {
        return Err(FsError::PairIo {
            operation: OP_COPY_FILE,
            source: source.to_path_buf(),
            destination: destination.to_path_buf(),
            kind: io::ErrorKind::InvalidInput,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_budget, validate_max_entries, FsError};

    #[test]
    fn validates_limits_without_io() {
        assert_eq!(read_budget(0), Ok(1));
        assert_eq!(
            read_budget(usize::MAX),
            Err(FsError::InvalidLimit { field: "max_bytes" })
        );
        assert_eq!(
            validate_max_entries(usize::MAX),
            Err(FsError::InvalidLimit {
                field: "max_entries"
            })
        );
        assert_eq!(validate_max_entries(0), Ok(()));
    }
}
