//! `FsUtils` 使用的文件系统操作实现。

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use super::FsError;

#[cfg(feature = "tokio")]
use tokio::io::AsyncReadExt;

const OP_TRY_EXISTS: &str = "try_exists";
const OP_IS_FILE: &str = "is_file";
const OP_IS_DIR: &str = "is_dir";
const OP_METADATA: &str = "metadata";
const OP_SYMLINK_METADATA: &str = "symlink_metadata";
const OP_CREATE_FILE: &str = "create_file";
const OP_CREATE_DIR: &str = "create_dir";
const OP_CREATE_DIR_ALL: &str = "create_dir_all";
const OP_LIST_DIR: &str = "list_dir";
const OP_REMOVE_FILE: &str = "remove_file";
const OP_REMOVE_DIR: &str = "remove_dir";
const OP_REMOVE_DIR_ALL: &str = "remove_dir_all";
const OP_MOVE_PATH: &str = "move_path";
const OP_COPY_FILE: &str = "copy_file";
const OP_READ_BYTES: &str = "read_bytes";
const OP_READ_TO_STRING: &str = "read_to_string";
const OP_WRITE: &str = "write";
const OP_APPEND: &str = "append";

fn io_error(operation: &'static str, path: &Path, error: &io::Error) -> FsError {
    FsError::Io {
        operation,
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

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

fn validate_max_entries(max_entries: usize) -> Result<(), FsError> {
    if max_entries == usize::MAX {
        Err(FsError::InvalidLimit {
            field: "max_entries",
        })
    } else {
        Ok(())
    }
}

fn read_budget(max_bytes: usize) -> Result<u64, FsError> {
    max_bytes
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(FsError::InvalidLimit { field: "max_bytes" })
}

#[cfg(feature = "tokio")]
fn ensure_runtime() -> Result<(), FsError> {
    tokio::runtime::Handle::try_current()
        .map(|_| ())
        .map_err(|_| FsError::RuntimeRequired)
}

pub(crate) fn try_exists(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_TRY_EXISTS, path, &error)),
    }
}

pub(crate) fn is_file(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_FILE, path, &error)),
    }
}

pub(crate) fn is_dir(path: &Path) -> Result<bool, FsError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_DIR, path, &error)),
    }
}

pub(crate) fn metadata(path: &Path) -> Result<fs::Metadata, FsError> {
    fs::metadata(path).map_err(|error| io_error(OP_METADATA, path, &error))
}

pub(crate) fn symlink_metadata(path: &Path) -> Result<fs::Metadata, FsError> {
    fs::symlink_metadata(path).map_err(|error| io_error(OP_SYMLINK_METADATA, path, &error))
}

pub(crate) fn create_file(path: &Path) -> Result<(), FsError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| io_error(OP_CREATE_FILE, path, &error))
}

pub(crate) fn create_dir(path: &Path) -> Result<(), FsError> {
    fs::create_dir(path).map_err(|error| io_error(OP_CREATE_DIR, path, &error))
}

pub(crate) fn create_dir_all(path: &Path) -> Result<(), FsError> {
    fs::create_dir_all(path).map_err(|error| io_error(OP_CREATE_DIR_ALL, path, &error))
}

pub(crate) fn list_dir(path: &Path, max_entries: usize) -> Result<Vec<PathBuf>, FsError> {
    validate_max_entries(max_entries)?;

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

pub(crate) fn remove_file(path: &Path) -> Result<(), FsError> {
    fs::remove_file(path).map_err(|error| io_error(OP_REMOVE_FILE, path, &error))
}

pub(crate) fn remove_dir(path: &Path) -> Result<(), FsError> {
    fs::remove_dir(path).map_err(|error| io_error(OP_REMOVE_DIR, path, &error))
}

pub(crate) fn remove_dir_all(path: &Path) -> Result<(), FsError> {
    fs::remove_dir_all(path).map_err(|error| io_error(OP_REMOVE_DIR_ALL, path, &error))
}

pub(crate) fn move_path(source: &Path, destination: &Path) -> Result<(), FsError> {
    fs::rename(source, destination)
        .map_err(|error| pair_io_error(OP_MOVE_PATH, source, destination, &error))
}

fn ensure_regular_file(
    operation: &'static str,
    path: &Path,
    metadata: Result<fs::Metadata, io::Error>,
    source: &Path,
    destination: &Path,
    allow_missing: bool,
) -> Result<bool, FsError> {
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

pub(crate) fn copy_file(source: &Path, destination: &Path) -> Result<u64, FsError> {
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

    fs::copy(source, destination)
        .map_err(|error| pair_io_error(OP_COPY_FILE, source, destination, &error))
}

fn read_bytes_with_operation(
    path: &Path,
    max_bytes: usize,
    operation: &'static str,
) -> Result<Vec<u8>, FsError> {
    let budget = read_budget(max_bytes)?;
    let mut file = File::open(path).map_err(|error| io_error(operation, path, &error))?;
    let mut buffer = Vec::new();
    Read::by_ref(&mut file)
        .take(budget)
        .read_to_end(&mut buffer)
        .map_err(|error| io_error(operation, path, &error))?;

    if buffer.len() > max_bytes {
        return Err(FsError::FileTooLarge {
            path: path.to_path_buf(),
            limit: max_bytes,
        });
    }
    Ok(buffer)
}

pub(crate) fn read_bytes(path: &Path, max_bytes: usize) -> Result<Vec<u8>, FsError> {
    read_bytes_with_operation(path, max_bytes, OP_READ_BYTES)
}

pub(crate) fn read_to_string(path: &Path, max_bytes: usize) -> Result<String, FsError> {
    let buffer = read_bytes_with_operation(path, max_bytes, OP_READ_TO_STRING)?;
    String::from_utf8(buffer).map_err(|_| FsError::NotUtf8 {
        path: path.to_path_buf(),
    })
}

pub(crate) fn write(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    fs::write(path, contents).map_err(|error| io_error(OP_WRITE, path, &error))
}

pub(crate) fn append(path: &Path, contents: &[u8]) -> Result<(), FsError> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| io_error(OP_APPEND, path, &error))?;
    file.write_all(contents)
        .map_err(|error| io_error(OP_APPEND, path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn try_exists_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match tokio::fs::metadata(&path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_TRY_EXISTS, &path, &error)),
    }
}

#[cfg(feature = "tokio")]
pub(crate) async fn is_file_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match tokio::fs::metadata(&path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_FILE, &path, &error)),
    }
}

#[cfg(feature = "tokio")]
pub(crate) async fn is_dir_async(path: PathBuf) -> Result<bool, FsError> {
    ensure_runtime()?;
    match tokio::fs::metadata(&path).await {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(OP_IS_DIR, &path, &error)),
    }
}

#[cfg(feature = "tokio")]
pub(crate) async fn metadata_async(path: PathBuf) -> Result<fs::Metadata, FsError> {
    ensure_runtime()?;
    tokio::fs::metadata(&path)
        .await
        .map_err(|error| io_error(OP_METADATA, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn symlink_metadata_async(path: PathBuf) -> Result<fs::Metadata, FsError> {
    ensure_runtime()?;
    tokio::fs::symlink_metadata(&path)
        .await
        .map_err(|error| io_error(OP_SYMLINK_METADATA, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn create_file_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map(|_| ())
        .map_err(|error| io_error(OP_CREATE_FILE, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn create_dir_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::create_dir(&path)
        .await
        .map_err(|error| io_error(OP_CREATE_DIR, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn create_dir_all_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::create_dir_all(&path)
        .await
        .map_err(|error| io_error(OP_CREATE_DIR_ALL, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn list_dir_async(
    path: PathBuf,
    max_entries: usize,
) -> Result<Vec<PathBuf>, FsError> {
    validate_max_entries(max_entries)?;
    ensure_runtime()?;

    let mut entries = tokio::fs::read_dir(&path)
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

#[cfg(feature = "tokio")]
pub(crate) async fn remove_file_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::remove_file(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_FILE, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn remove_dir_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::remove_dir(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_DIR, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn remove_dir_all_async(path: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::remove_dir_all(&path)
        .await
        .map_err(|error| io_error(OP_REMOVE_DIR_ALL, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn move_path_async(source: PathBuf, destination: PathBuf) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::rename(&source, &destination)
        .await
        .map_err(|error| pair_io_error(OP_MOVE_PATH, &source, &destination, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn copy_file_async(source: PathBuf, destination: PathBuf) -> Result<u64, FsError> {
    ensure_runtime()?;

    let source_is_file = ensure_regular_file(
        OP_COPY_FILE,
        &source,
        tokio::fs::symlink_metadata(&source).await,
        &source,
        &destination,
        false,
    )?;
    debug_assert!(source_is_file);

    let _destination_exists = ensure_regular_file(
        OP_COPY_FILE,
        &destination,
        tokio::fs::symlink_metadata(&destination).await,
        &source,
        &destination,
        true,
    )?;

    tokio::fs::copy(&source, &destination)
        .await
        .map_err(|error| pair_io_error(OP_COPY_FILE, &source, &destination, &error))
}

#[cfg(feature = "tokio")]
async fn read_bytes_with_operation_async(
    path: PathBuf,
    max_bytes: usize,
    operation: &'static str,
) -> Result<Vec<u8>, FsError> {
    let budget = read_budget(max_bytes)?;
    ensure_runtime()?;

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|error| io_error(operation, &path, &error))?;
    let mut buffer = Vec::new();
    tokio::io::AsyncReadExt::take(file, budget)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| io_error(operation, &path, &error))?;

    if buffer.len() > max_bytes {
        return Err(FsError::FileTooLarge {
            path,
            limit: max_bytes,
        });
    }
    Ok(buffer)
}

#[cfg(feature = "tokio")]
pub(crate) async fn read_bytes_async(path: PathBuf, max_bytes: usize) -> Result<Vec<u8>, FsError> {
    read_bytes_with_operation_async(path, max_bytes, OP_READ_BYTES).await
}

#[cfg(feature = "tokio")]
pub(crate) async fn read_to_string_async(
    path: PathBuf,
    max_bytes: usize,
) -> Result<String, FsError> {
    let path_for_error = path.clone();
    let buffer = read_bytes_with_operation_async(path, max_bytes, OP_READ_TO_STRING).await?;
    String::from_utf8(buffer).map_err(|_| FsError::NotUtf8 {
        path: path_for_error,
    })
}

#[cfg(feature = "tokio")]
pub(crate) async fn write_async(path: PathBuf, contents: Vec<u8>) -> Result<(), FsError> {
    ensure_runtime()?;
    tokio::fs::write(&path, contents)
        .await
        .map_err(|error| io_error(OP_WRITE, &path, &error))
}

#[cfg(feature = "tokio")]
pub(crate) async fn append_async(path: PathBuf, contents: Vec<u8>) -> Result<(), FsError> {
    ensure_runtime()?;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))?;
    tokio::io::AsyncWriteExt::write_all(&mut file, &contents)
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))?;
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|error| io_error(OP_APPEND, &path, &error))
}

#[cfg(test)]
mod tests {
    use super::{read_budget, validate_max_entries};
    use crate::FsError;

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
