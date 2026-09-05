use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use axutils::{
    fs::{FsChunkProcessor, FsError, FsTransferError, FsTransferOptions, FsTransferStats},
    utils::FsUtils,
};

#[cfg(feature = "fs-async")]
use axutils::fs::FsAsyncChunkProcessor;
#[cfg(feature = "fs-async")]
use tokio::time::timeout;

#[cfg(any(feature = "fs-temp", feature = "fs-temp-async"))]
use axutils::fs::{FsTempConfig, FsTempError};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: Option<PathBuf>,
}

impl TempDir {
    fn new() -> Self {
        for _ in 0..100 {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("axutils-fs-test-{}-{counter}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path: Some(path) },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create temporary directory: {error}"),
            }
        }
        panic!("failed to acquire an exclusive temporary directory");
    }

    fn path(&self) -> &Path {
        self.path.as_deref().expect("temporary directory is active")
    }

    fn cleanup(mut self) {
        let path = self.path.take().expect("temporary directory is active");
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "test environment cleanup failed for {}: {error}",
                path.display()
            ),
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let Some(path) = self.path.take() else {
            return;
        };
        if let Err(error) = fs::remove_dir_all(&path) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!(
                    "test environment cleanup failed for {}: {error}",
                    path.display()
                );
            }
        }
    }
}

fn assert_io_kind(error: FsError, operation: &str, kind: io::ErrorKind) {
    assert!(matches!(
        error,
        FsError::Io {
            operation: actual,
            kind: actual_kind,
            ..
        } if actual == operation && actual_kind == kind
    ));
}

fn max_valid_bytes() -> usize {
    usize::try_from(u64::MAX - 1).map_or(usize::MAX - 1, |value| value.min(usize::MAX - 1))
}

struct IdentityProcessor;

impl FsChunkProcessor for IdentityProcessor {
    type Error = std::convert::Infallible;

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Ok(chunk)
    }
}

struct UppercaseProcessor;

impl FsChunkProcessor for UppercaseProcessor {
    type Error = std::convert::Infallible;

    fn process(&mut self, mut chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        chunk.make_ascii_uppercase();
        Ok(chunk)
    }
}

struct DuplicateProcessor;

impl FsChunkProcessor for DuplicateProcessor {
    type Error = std::convert::Infallible;

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        let mut output = Vec::with_capacity(chunk.len() * 2);
        output.extend_from_slice(&chunk);
        output.extend_from_slice(&chunk);
        Ok(output)
    }
}

#[cfg(feature = "fs-async")]
struct AsyncIdentityProcessor;

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncIdentityProcessor {
    type Error = std::convert::Infallible;
    type Future<'a>
        = std::future::Ready<Result<Vec<u8>, Self::Error>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
        std::future::ready(Ok(chunk))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncDuplicateProcessor;

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncDuplicateProcessor {
    type Error = std::convert::Infallible;
    type Future<'a>
        = std::future::Ready<Result<Vec<u8>, Self::Error>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
        let mut output = Vec::with_capacity(chunk.len() * 2);
        output.extend_from_slice(&chunk);
        output.extend_from_slice(&chunk);
        std::future::ready(Ok(output))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncCancelAfterFirst {
    processed: usize,
}

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncCancelAfterFirst {
    type Error = std::convert::Infallible;
    type Future<'a>
        = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Self::Error>> + 'a>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
        self.processed += 1;
        if self.processed == 1 {
            Box::pin(std::future::ready(Ok(chunk)))
        } else {
            Box::pin(std::future::pending())
        }
    }
}

#[test]
fn public_paths_cover_all_sync_methods() {
    let temp = TempDir::new();
    let root = temp.path().to_path_buf();
    let nested = root.join("nested");
    let tree = root.join("tree").join("child");
    let file = root.join("file.txt");
    let empty = root.join("empty.txt");
    let copy = root.join("copy.txt");
    let moved = root.join("moved.txt");

    assert!(!FsUtils::try_exists(&file).expect("query should succeed"));
    assert!(!FsUtils::is_file(&file).expect("file query should succeed"));
    assert!(!FsUtils::is_dir(&file).expect("directory query should succeed"));
    FsUtils::create_dir(&nested).expect("create directory");
    FsUtils::create_dir_all(&tree).expect("create directory tree");
    FsUtils::create_file(&empty).expect("create empty file");
    FsUtils::write(&file, b"hello").expect("write file");
    assert!(FsUtils::is_file(&file).expect("file query"));
    assert!(!FsUtils::is_dir(&file).expect("directory query"));
    assert!(FsUtils::is_dir(&nested).expect("directory query"));
    let _ = FsUtils::metadata(&file).expect("metadata");
    let _ = FsUtils::symlink_metadata(&file).expect("symlink metadata");
    assert!(FsUtils::try_exists(&file).expect("query after write"));
    assert_eq!(FsUtils::read_bytes(&file, 5).expect("read bytes"), b"hello");
    assert_eq!(
        FsUtils::read_to_string(&file, 5).expect("read text"),
        "hello"
    );
    let children = FsUtils::list_dir(&root, 20).expect("list directory");
    assert!(children.iter().any(|child| child == &file));
    assert!(children.iter().any(|child| child == &root.join("tree")));
    assert!(!children.iter().any(|child| child == &tree));
    FsUtils::write(&copy, b"old").expect("write existing copy target");
    assert_eq!(FsUtils::copy_file(&file, &copy).expect("copy file"), 5);
    assert_eq!(
        FsUtils::read_bytes(&copy, 5).expect("read copied file"),
        b"hello"
    );
    FsUtils::move_path(&copy, &moved).expect("move file");
    assert!(!FsUtils::try_exists(&copy).expect("source query after move"));
    FsUtils::append(&moved, b"!").expect("append file");
    assert_eq!(
        FsUtils::read_bytes(&moved, 6).expect("read appended file"),
        b"hello!"
    );
    FsUtils::remove_file(&moved).expect("remove moved file");
    assert!(!FsUtils::try_exists(&moved).expect("moved file should be absent"));
    FsUtils::remove_file(&empty).expect("remove empty file");
    assert!(!FsUtils::try_exists(&empty).expect("empty file should be absent"));
    FsUtils::remove_dir(&nested).expect("remove empty directory");
    assert!(!FsUtils::try_exists(&nested).expect("nested directory should be absent"));
    FsUtils::remove_dir_all(root.join("tree")).expect("remove directory tree");
    assert!(!FsUtils::try_exists(root.join("tree")).expect("directory tree should be absent"));

    let _: FsError = FsError::RuntimeRequired;
    let _: FsUtils = FsUtils;
    temp.cleanup();
}

#[test]
fn sync_limits_errors_and_overwrite_contracts_are_explicit() {
    let temp = TempDir::new();
    let root = temp.path();
    let file = root.join("file.bin");
    FsUtils::write(&file, b"abc").expect("write fixture");
    let empty = root.join("empty.bin");
    FsUtils::create_file(&empty).expect("create empty file");
    assert_eq!(FsUtils::read_bytes(&empty, 0), Ok(Vec::new()));
    assert_eq!(FsUtils::read_to_string(&empty, 0), Ok(String::new()));
    let max_bytes = max_valid_bytes();
    assert!(FsUtils::read_bytes(&empty, max_bytes).is_ok());
    assert_eq!(
        FsUtils::read_bytes(root.join("missing-max-bytes"), max_bytes + 1),
        Err(FsError::InvalidLimit { field: "max_bytes" })
    );

    let duplicate = FsUtils::create_file(&file).expect_err("create_file must not truncate");
    assert_io_kind(duplicate, "create_file", io::ErrorKind::AlreadyExists);
    assert_eq!(
        FsUtils::read_bytes(&file, 3).expect("read unchanged file"),
        b"abc"
    );
    assert!(matches!(
        FsUtils::read_bytes(&file, 2),
        Err(FsError::FileTooLarge { limit: 2, .. })
    ));
    assert!(matches!(
        FsUtils::read_to_string(&file, 2),
        Err(FsError::FileTooLarge { limit: 2, .. })
    ));
    assert_eq!(
        FsUtils::read_bytes(&file, 0).unwrap_err(),
        FsError::FileTooLarge {
            path: file.clone(),
            limit: 0,
        }
    );
    assert_eq!(
        FsUtils::read_bytes(root.join("empty.bin"), usize::MAX),
        Err(FsError::InvalidLimit { field: "max_bytes" })
    );
    assert_eq!(
        FsUtils::list_dir(root.join("missing"), usize::MAX),
        Err(FsError::InvalidLimit {
            field: "max_entries",
        })
    );
    assert!(matches!(
        FsUtils::read_bytes(root.join("missing"), 10),
        Err(FsError::Io {
            operation: "read_bytes",
            kind: io::ErrorKind::NotFound,
            ..
        })
    ));
    assert!(matches!(
        FsUtils::remove_file(root.join("missing")),
        Err(FsError::Io {
            operation: "remove_file",
            kind: io::ErrorKind::NotFound,
            ..
        })
    ));

    let invalid_utf8 = root.join("invalid.bin");
    let mut writer = fs::File::create(&invalid_utf8).expect("create invalid UTF-8 fixture");
    writer
        .write_all(&[0xff, 0xfe])
        .expect("write invalid UTF-8 fixture");
    assert_eq!(
        FsUtils::read_to_string(&invalid_utf8, 2),
        Err(FsError::NotUtf8 { path: invalid_utf8 })
    );

    let first = root.join("first");
    let second = root.join("second");
    let empty_dir = root.join("empty-dir");
    let exact_dir = root.join("exact-dir");
    let exact_entry = exact_dir.join("entry");
    FsUtils::create_dir(&empty_dir).expect("create empty directory");
    FsUtils::create_dir(&exact_dir).expect("create exact-limit directory");
    FsUtils::create_file(&exact_entry).expect("create exact-limit entry");
    assert_eq!(FsUtils::list_dir(&empty_dir, 0), Ok(Vec::new()));
    assert_eq!(
        FsUtils::list_dir(&empty_dir, usize::MAX - 1),
        Ok(Vec::new())
    );
    assert_eq!(
        FsUtils::list_dir(&exact_dir, 1),
        Ok(vec![exact_entry.clone()])
    );
    FsUtils::create_file(&first).expect("create first entry");
    FsUtils::create_file(&second).expect("create second entry");
    assert!(matches!(
        FsUtils::list_dir(root, 0),
        Err(FsError::DirectoryEntriesTooMany { limit: 0, .. })
    ));
    assert!(matches!(
        FsUtils::list_dir(root, 1),
        Err(FsError::DirectoryEntriesTooMany { limit: 1, .. })
    ));
    assert!(FsUtils::list_dir(root, usize::MAX - 1).is_ok());

    let overwrite = root.join("overwrite.txt");
    FsUtils::write(&overwrite, b"abcdef").expect("write long file");
    FsUtils::write(&overwrite, b"xy").expect("truncate file");
    assert_eq!(
        FsUtils::read_bytes(&overwrite, 2).expect("read truncated file"),
        b"xy"
    );
    let appended = root.join("appended.txt");
    FsUtils::append(&appended, b"created").expect("create by append");
    assert_eq!(
        FsUtils::read_bytes(&appended, 7).expect("read created append"),
        b"created"
    );
    temp.cleanup();
}

#[test]
fn sync_error_tokens_cover_single_path_operations() {
    let temp = TempDir::new();
    let root = temp.path();
    let missing = root.join("missing");
    let missing_file = root.join("missing-parent").join("file");

    assert_io_kind(
        FsUtils::metadata(&missing).expect_err("metadata should fail for a missing path"),
        "metadata",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::symlink_metadata(&missing)
            .expect_err("symlink_metadata should fail for a missing path"),
        "symlink_metadata",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::create_dir(&missing_file)
            .expect_err("create_dir should fail for a missing parent"),
        "create_dir",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::list_dir(&missing, 1).expect_err("list_dir should fail for a missing path"),
        "list_dir",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::read_bytes(&missing, 1).expect_err("read_bytes should fail for a missing path"),
        "read_bytes",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::read_to_string(&missing, 1)
            .expect_err("read_to_string should fail for a missing path"),
        "read_to_string",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::write(&missing_file, b"data").expect_err("write should fail for a missing parent"),
        "write",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::append(&missing_file, b"data")
            .expect_err("append should fail for a missing parent"),
        "append",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::remove_dir(&missing).expect_err("remove_dir should fail for a missing path"),
        "remove_dir",
        io::ErrorKind::NotFound,
    );
    assert_io_kind(
        FsUtils::remove_dir_all(&missing)
            .expect_err("remove_dir_all should fail for a missing path"),
        "remove_dir_all",
        io::ErrorKind::NotFound,
    );

    let blocker = root.join("blocker");
    FsUtils::write(&blocker, b"file").expect("write blocker");
    let blocked_child = blocker.join("child");
    let blocked_error = FsUtils::create_dir_all(&blocked_child)
        .expect_err("create_dir_all should fail below a regular file");
    assert!(matches!(
        blocked_error,
        FsError::Io { operation, .. } if operation == "create_dir_all"
    ));

    temp.cleanup();
}

#[test]
fn sync_create_dir_all_allows_same_path_concurrency() {
    use std::{sync::Arc, thread};

    let temp = TempDir::new();
    let target = Arc::new(temp.path().join("concurrent/a/b"));
    let handles = (0..4)
        .map(|_| {
            let target = Arc::clone(&target);
            thread::spawn(move || FsUtils::create_dir_all(&*target))
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle
            .join()
            .expect("create_dir_all worker should join")
            .expect("same-path create_dir_all should succeed");
    }
    assert!(target.is_dir());
    temp.cleanup();
}

#[test]
fn sync_copy_rejects_directories_and_symlinks() {
    let temp = TempDir::new();
    let root = temp.path();
    let source = root.join("source.txt");
    let directory = root.join("directory");
    let destination = root.join("destination.txt");
    FsUtils::write(&source, b"source").expect("write source");
    FsUtils::create_dir(&directory).expect("create directory");

    let missing_source = root.join("missing-source.txt");
    assert_eq!(
        FsUtils::copy_file(&missing_source, &destination),
        Err(FsError::PairIo {
            operation: "copy_file",
            source: missing_source.clone(),
            destination: destination.clone(),
            kind: io::ErrorKind::NotFound,
        })
    );
    let missing_parent_destination = root.join("missing-parent").join("destination.txt");
    assert_eq!(
        FsUtils::copy_file(&source, &missing_parent_destination),
        Err(FsError::PairIo {
            operation: "copy_file",
            source: source.clone(),
            destination: missing_parent_destination.clone(),
            kind: io::ErrorKind::NotFound,
        })
    );
    let missing_move_source = root.join("missing-move-source.txt");
    let move_destination = root.join("move-destination.txt");
    assert_eq!(
        FsUtils::move_path(&missing_move_source, &move_destination),
        Err(FsError::PairIo {
            operation: "move_path",
            source: missing_move_source,
            destination: move_destination,
            kind: io::ErrorKind::NotFound,
        })
    );

    assert_eq!(
        FsUtils::copy_file(&directory, &destination),
        Err(FsError::UnsupportedEntry {
            operation: "copy_file",
            path: directory.clone(),
        })
    );
    let destination_directory = root.join("destination-directory");
    FsUtils::create_dir(&destination_directory).expect("create destination directory");
    assert_eq!(
        FsUtils::copy_file(&source, &destination_directory),
        Err(FsError::UnsupportedEntry {
            operation: "copy_file",
            path: destination_directory.clone(),
        })
    );

    let link = root.join("source-link");
    match create_file_symlink(&source, &link) {
        Ok(()) => {
            let duplicate = FsUtils::create_file(&link).expect_err("create_file must reject link");
            assert_io_kind(duplicate, "create_file", io::ErrorKind::AlreadyExists);
            assert_eq!(
                FsUtils::copy_file(&link, &destination),
                Err(FsError::UnsupportedEntry {
                    operation: "copy_file",
                    path: link.clone(),
                })
            );
            let destination_link = root.join("destination-link");
            create_file_symlink(&source, &destination_link).expect("create destination symlink");
            assert_eq!(
                FsUtils::copy_file(&source, &destination_link),
                Err(FsError::UnsupportedEntry {
                    operation: "copy_file",
                    path: destination_link,
                })
            );
        }
        Err(error) if symlink_permission_unavailable(&error) => {
            eprintln!("skipping symlink copy assertion: symlink permission is unavailable");
        }
        Err(error) => panic!("failed to create symlink fixture: {error}"),
    }
    temp.cleanup();
}

#[test]
fn sync_remove_dir_all_does_not_remove_final_symlink_target() {
    let temp = TempDir::new();
    let root = temp.path();
    let outside = root.join("outside");
    let link = root.join("outside-link");
    FsUtils::create_dir(&outside).expect("create outside directory");
    let marker = outside.join("marker.txt");
    FsUtils::write(&marker, b"keep").expect("write marker");

    match create_dir_symlink(&outside, &link) {
        Ok(()) => {
            assert!(FsUtils::metadata(&link)
                .expect("metadata should follow directory symlink")
                .is_dir());
            assert!(FsUtils::symlink_metadata(&link)
                .expect("symlink_metadata should inspect link itself")
                .file_type()
                .is_symlink());
            FsUtils::remove_dir_all(&link).expect("remove final symlink");
            assert!(outside.is_dir(), "symlink removal must not remove target");
            assert!(
                marker.is_file(),
                "symlink removal must not remove target contents"
            );
            assert_io_kind(
                FsUtils::symlink_metadata(&link).expect_err("final symlink should be removed"),
                "symlink_metadata",
                io::ErrorKind::NotFound,
            );
        }
        Err(error) if symlink_permission_unavailable(&error) => {
            eprintln!(
                "skipping final symlink deletion assertion: symlink permission is unavailable"
            );
        }
        Err(error) => panic!("failed to create directory symlink fixture: {error}"),
    }
    temp.cleanup();
}

fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "filesystem symlinks are unsupported on this target",
        ))
    }
}

fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "filesystem symlinks are unsupported on this target",
        ))
    }
}

fn symlink_permission_unavailable(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied
        || error.kind() == io::ErrorKind::Unsupported
        || (cfg!(windows) && error.raw_os_error() == Some(1314))
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn public_paths_cover_all_async_methods() {
    let temp = TempDir::new();
    let root = temp.path().to_path_buf();
    let nested = root.join("nested");
    let tree = root.join("tree").join("child");
    let file = root.join("file.txt");
    let empty = root.join("empty.txt");
    let copy = root.join("copy.txt");
    let moved = root.join("moved.txt");

    assert!(!FsUtils::try_exists_async(&file).await.expect("query"));
    assert!(!FsUtils::is_file_async(&file).await.expect("file query"));
    assert!(!FsUtils::is_dir_async(&file).await.expect("directory query"));
    FsUtils::create_dir_async(&nested)
        .await
        .expect("create dir");
    FsUtils::create_dir_all_async(&tree)
        .await
        .expect("create dir tree");
    FsUtils::create_file_async(&empty)
        .await
        .expect("create empty file");
    assert_eq!(FsUtils::read_bytes_async(&empty, 0).await, Ok(Vec::new()));
    assert_eq!(
        FsUtils::read_to_string_async(&empty, 0).await,
        Ok(String::new())
    );
    let max_bytes = max_valid_bytes();
    assert!(FsUtils::read_bytes_async(&empty, max_bytes).await.is_ok());
    assert_eq!(
        FsUtils::read_bytes_async(root.join("missing-max-bytes"), max_bytes + 1).await,
        Err(FsError::InvalidLimit { field: "max_bytes" })
    );
    FsUtils::write_async(&file, b"hello")
        .await
        .expect("write file");
    assert!(FsUtils::is_file_async(&file).await.expect("file query"));
    assert!(!FsUtils::is_dir_async(&file).await.expect("directory query"));
    assert!(FsUtils::is_dir_async(&nested)
        .await
        .expect("directory query"));
    let _ = FsUtils::metadata_async(&file).await.expect("metadata");
    let _ = FsUtils::symlink_metadata_async(&file)
        .await
        .expect("symlink metadata");
    assert!(FsUtils::try_exists_async(&file)
        .await
        .expect("query after write"));
    assert_eq!(
        FsUtils::read_bytes_async(&file, 5)
            .await
            .expect("read bytes"),
        b"hello"
    );
    assert_eq!(
        FsUtils::read_to_string_async(&file, 5)
            .await
            .expect("read text"),
        "hello"
    );
    let children = FsUtils::list_dir_async(&root, 20).await.expect("list dir");
    assert!(children.iter().any(|child| child == &file));
    assert!(children.iter().any(|child| child == &root.join("tree")));
    assert!(!children.iter().any(|child| child == &tree));
    let empty_dir = root.join("empty-dir");
    let exact_dir = root.join("exact-dir");
    let exact_entry = exact_dir.join("entry");
    FsUtils::create_dir_async(&empty_dir)
        .await
        .expect("create empty directory");
    FsUtils::create_dir_async(&exact_dir)
        .await
        .expect("create exact-limit directory");
    FsUtils::create_file_async(&exact_entry)
        .await
        .expect("create exact-limit entry");
    assert_eq!(FsUtils::list_dir_async(&empty_dir, 0).await, Ok(Vec::new()));
    assert_eq!(
        FsUtils::list_dir_async(&empty_dir, usize::MAX - 1).await,
        Ok(Vec::new())
    );
    assert_eq!(
        FsUtils::list_dir_async(&exact_dir, 1).await,
        Ok(vec![exact_entry.clone()])
    );
    assert!(matches!(
        FsUtils::list_dir_async(&root, 0).await,
        Err(FsError::DirectoryEntriesTooMany { limit: 0, .. })
    ));
    FsUtils::remove_dir_async(&empty_dir)
        .await
        .expect("remove empty directory");
    FsUtils::write_async(&copy, b"old")
        .await
        .expect("write existing copy target");
    assert_eq!(
        FsUtils::copy_file_async(&file, &copy)
            .await
            .expect("copy file"),
        5
    );
    assert_eq!(
        FsUtils::read_bytes_async(&copy, 5)
            .await
            .expect("read copied file"),
        b"hello"
    );
    FsUtils::move_path_async(&copy, &moved)
        .await
        .expect("move file");
    assert!(!FsUtils::try_exists_async(&copy)
        .await
        .expect("source query after move"));
    FsUtils::append_async(&moved, b"!")
        .await
        .expect("append file");
    assert_eq!(
        FsUtils::read_bytes_async(&moved, 6)
            .await
            .expect("read appended file"),
        b"hello!"
    );
    assert!(matches!(
        FsUtils::read_to_string_async(&file, 4).await,
        Err(FsError::FileTooLarge { limit: 4, .. })
    ));
    let overwrite = root.join("overwrite.txt");
    FsUtils::write_async(&overwrite, b"abcdef")
        .await
        .expect("write long file");
    FsUtils::write_async(&overwrite, b"xy")
        .await
        .expect("truncate file");
    assert_eq!(
        FsUtils::read_bytes_async(&overwrite, 2)
            .await
            .expect("read truncated file"),
        b"xy"
    );
    let appended = root.join("appended.txt");
    FsUtils::append_async(&appended, b"created")
        .await
        .expect("create by append");
    assert_eq!(
        FsUtils::read_bytes_async(&appended, 7)
            .await
            .expect("read created append"),
        b"created"
    );
    FsUtils::remove_file_async(&moved)
        .await
        .expect("remove moved file");
    assert!(!FsUtils::try_exists_async(&moved)
        .await
        .expect("moved file should be absent"));
    FsUtils::remove_file_async(&empty)
        .await
        .expect("remove empty file");
    assert!(!FsUtils::try_exists_async(&empty)
        .await
        .expect("empty file should be absent"));
    FsUtils::remove_dir_async(&nested)
        .await
        .expect("remove empty directory");
    assert!(!FsUtils::try_exists_async(&nested)
        .await
        .expect("nested directory should be absent"));
    FsUtils::remove_dir_all_async(root.join("tree"))
        .await
        .expect("remove directory tree");
    assert!(!FsUtils::try_exists_async(root.join("tree"))
        .await
        .expect("directory tree should be absent"));
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_create_dir_all_allows_same_path_concurrency() {
    let temp = TempDir::new();
    let target = temp.path().join("concurrent/a/b");
    let (first, second, third, fourth) = tokio::join!(
        FsUtils::create_dir_all_async(&target),
        FsUtils::create_dir_all_async(&target),
        FsUtils::create_dir_all_async(&target),
        FsUtils::create_dir_all_async(&target),
    );
    for result in [first, second, third, fourth] {
        result.expect("same-path async create_dir_all should succeed");
    }
    assert!(target.is_dir());
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_copy_and_remove_dir_all_respect_final_symlinks() {
    let temp = TempDir::new();
    let root = temp.path();
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    FsUtils::write_async(&source, b"source")
        .await
        .expect("write source");
    let missing_source = root.join("missing-source.txt");
    assert_eq!(
        FsUtils::copy_file_async(&missing_source, &destination).await,
        Err(FsError::PairIo {
            operation: "copy_file",
            source: missing_source.clone(),
            destination: destination.clone(),
            kind: io::ErrorKind::NotFound,
        })
    );
    let missing_parent_destination = root.join("missing-parent").join("destination.txt");
    assert_eq!(
        FsUtils::copy_file_async(&source, &missing_parent_destination).await,
        Err(FsError::PairIo {
            operation: "copy_file",
            source: source.clone(),
            destination: missing_parent_destination.clone(),
            kind: io::ErrorKind::NotFound,
        })
    );
    let missing_move_source = root.join("missing-move-source.txt");
    let move_destination = root.join("move-destination.txt");
    assert_eq!(
        FsUtils::move_path_async(&missing_move_source, &move_destination).await,
        Err(FsError::PairIo {
            operation: "move_path",
            source: missing_move_source,
            destination: move_destination,
            kind: io::ErrorKind::NotFound,
        })
    );
    let source_directory = root.join("source-directory");
    FsUtils::create_dir_async(&source_directory)
        .await
        .expect("create source directory");
    assert_eq!(
        FsUtils::copy_file_async(&source_directory, &destination).await,
        Err(FsError::UnsupportedEntry {
            operation: "copy_file",
            path: source_directory,
        })
    );
    let destination_directory = root.join("destination-directory");
    FsUtils::create_dir_async(&destination_directory)
        .await
        .expect("create destination directory");
    assert_eq!(
        FsUtils::copy_file_async(&source, &destination_directory).await,
        Err(FsError::UnsupportedEntry {
            operation: "copy_file",
            path: destination_directory,
        })
    );

    let file_link = root.join("source-link");
    match create_file_symlink(&source, &file_link) {
        Ok(()) => {
            assert_eq!(
                FsUtils::copy_file_async(&file_link, &destination).await,
                Err(FsError::UnsupportedEntry {
                    operation: "copy_file",
                    path: file_link.clone(),
                })
            );
            FsUtils::remove_file_async(&file_link)
                .await
                .expect("remove file symlink");
            let destination_link = root.join("destination-link");
            create_file_symlink(&source, &destination_link).expect("create destination symlink");
            assert_eq!(
                FsUtils::copy_file_async(&source, &destination_link).await,
                Err(FsError::UnsupportedEntry {
                    operation: "copy_file",
                    path: destination_link,
                })
            );
        }
        Err(error) if symlink_permission_unavailable(&error) => {
            eprintln!("skipping async symlink copy assertion: symlink permission is unavailable");
        }
        Err(error) => panic!("failed to create async symlink fixture: {error}"),
    }

    let outside = root.join("outside");
    let marker = outside.join("marker.txt");
    FsUtils::create_dir_async(&outside)
        .await
        .expect("create outside directory");
    FsUtils::write_async(&marker, b"keep")
        .await
        .expect("write marker");
    let dir_link = root.join("outside-link");
    match create_dir_symlink(&outside, &dir_link) {
        Ok(()) => {
            assert!(FsUtils::metadata_async(&dir_link)
                .await
                .expect("metadata should follow directory symlink")
                .is_dir());
            assert!(FsUtils::symlink_metadata_async(&dir_link)
                .await
                .expect("symlink_metadata should inspect link itself")
                .file_type()
                .is_symlink());
            FsUtils::remove_dir_all_async(&dir_link)
                .await
                .expect("remove directory symlink");
            assert!(outside.is_dir(), "symlink removal must not remove target");
            assert!(
                marker.is_file(),
                "symlink removal must preserve target contents"
            );
            assert_io_kind(
                FsUtils::symlink_metadata_async(&dir_link)
                    .await
                    .expect_err("final symlink should be removed"),
                "symlink_metadata",
                io::ErrorKind::NotFound,
            );
        }
        Err(error) if symlink_permission_unavailable(&error) => {
            eprintln!(
                "skipping async final symlink deletion assertion: symlink permission is unavailable"
            );
        }
        Err(error) => panic!("failed to create async directory symlink fixture: {error}"),
    }
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_facade_owns_arguments_before_returning_future() {
    let temp = TempDir::new();
    let path = temp.path().join("owned.txt");
    let verify_path = path.clone();
    let contents = b"owned".to_vec();
    let future = FsUtils::write_async(&path, &contents);
    drop(path);
    drop(contents);

    tokio::spawn(future)
        .await
        .expect("owned future should join")
        .expect("owned future should write");
    assert_eq!(
        FsUtils::read_bytes_async(verify_path, 5)
            .await
            .expect("read owned write"),
        b"owned"
    );
    let path_only = temp.path().join("path-only.txt");
    let path_only_future = FsUtils::try_exists_async(&path_only);
    drop(path_only);
    assert!(!tokio::spawn(path_only_future)
        .await
        .expect("spawn path-only future")
        .expect("query path-only future"));
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[test]
fn async_limits_are_checked_before_runtime() {
    use std::{future::Future, pin::pin, task::Context};

    fn poll_once<F: Future>(future: F) -> std::task::Poll<F::Output> {
        let mut future = pin!(future);
        let mut context = Context::from_waker(std::task::Waker::noop());
        future.as_mut().poll(&mut context)
    }

    macro_rules! assert_runtime_required {
        ($future:expr) => {
            assert!(matches!(
                poll_once($future),
                std::task::Poll::Ready(Err(FsError::RuntimeRequired))
            ));
        };
    }

    assert!(matches!(
        poll_once(FsUtils::read_bytes_async("missing", usize::MAX)),
        std::task::Poll::Ready(Err(FsError::InvalidLimit { field: "max_bytes" }))
    ));
    assert!(matches!(
        poll_once(FsUtils::list_dir_async("missing", usize::MAX)),
        std::task::Poll::Ready(Err(FsError::InvalidLimit {
            field: "max_entries"
        }))
    ));
    assert_runtime_required!(FsUtils::try_exists_async("missing"));
    assert_runtime_required!(FsUtils::is_file_async("missing"));
    assert_runtime_required!(FsUtils::is_dir_async("missing"));
    assert_runtime_required!(FsUtils::metadata_async("missing"));
    assert_runtime_required!(FsUtils::symlink_metadata_async("missing"));
    assert_runtime_required!(FsUtils::create_file_async("missing"));
    assert_runtime_required!(FsUtils::create_dir_async("missing"));
    assert_runtime_required!(FsUtils::create_dir_all_async("missing"));
    assert_runtime_required!(FsUtils::list_dir_async("missing", 0));
    assert_runtime_required!(FsUtils::remove_file_async("missing"));
    assert_runtime_required!(FsUtils::remove_dir_async("missing"));
    assert_runtime_required!(FsUtils::remove_dir_all_async("missing"));
    assert_runtime_required!(FsUtils::move_path_async("missing", "destination"));
    assert_runtime_required!(FsUtils::copy_file_async("missing", "destination"));
    assert_runtime_required!(FsUtils::read_bytes_async("missing", 0));
    assert_runtime_required!(FsUtils::read_to_string_async("missing", 0));
    assert_runtime_required!(FsUtils::write_async("missing", b"data"));
    assert_runtime_required!(FsUtils::append_async("missing", b"data"));
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_limit_and_utf8_semantics_match_sync() {
    let temp = TempDir::new();
    let path = temp.path().join("data");
    FsUtils::write_async(&path, b"abc")
        .await
        .expect("write data");
    assert!(matches!(
        FsUtils::read_bytes_async(&path, 2).await,
        Err(FsError::FileTooLarge { limit: 2, .. })
    ));
    FsUtils::write_async(&path, &[0xff, 0xfe])
        .await
        .expect("write invalid bytes");
    assert!(matches!(
        FsUtils::read_to_string_async(&path, 2).await,
        Err(FsError::NotUtf8 { .. })
    ));
    temp.cleanup();
}

#[test]
fn sync_stream_transfer_processes_serial_chunks_and_reports_stats() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    let input = (0..2050)
        .map(|index| b'a' + u8::try_from(index % 26).expect("modulo fits in u8"))
        .collect::<Vec<_>>();
    FsUtils::write(&source, &input).expect("write transfer source");
    FsUtils::write(&destination, b"stale destination").expect("write transfer destination");

    let stats = FsUtils::copy_file_with(
        &source,
        &destination,
        FsTransferOptions {
            chunk_size: 1024,
            max_output_bytes: None,
        },
        UppercaseProcessor,
    )
    .expect("stream transfer should succeed");

    let expected = input
        .iter()
        .map(|byte| byte.to_ascii_uppercase())
        .collect::<Vec<_>>();
    assert_eq!(
        stats,
        FsTransferStats {
            input_bytes: 2050,
            output_bytes: 2050,
            chunks: 3,
        }
    );
    assert_eq!(FsUtils::read_bytes(&destination, 2050), Ok(expected));
    temp.cleanup();
}

#[test]
fn sync_stream_transfer_checks_options_and_output_limit_before_writing() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    FsUtils::write(&source, [b'x'; 1024]).expect("write transfer source");
    FsUtils::write(&destination, b"stale destination").expect("write transfer destination");

    assert!(matches!(
        FsUtils::copy_file_with(
            temp.path().join("missing"),
            &destination,
            FsTransferOptions {
                chunk_size: 1,
                max_output_bytes: None,
            },
            IdentityProcessor,
        ),
        Err(FsTransferError::InvalidOptions {
            field: "chunk_size"
        })
    ));
    assert!(matches!(
        FsUtils::copy_file_with(
            &source,
            &source,
            FsTransferOptions::default(),
            IdentityProcessor,
        ),
        Err(FsTransferError::SameFile { .. })
    ));

    let source_directory = temp.path().join("source-directory");
    FsUtils::create_dir(&source_directory).expect("create stream source directory");
    assert!(matches!(
        FsUtils::copy_file_with(
            &source_directory,
            &destination,
            FsTransferOptions::default(),
            IdentityProcessor,
        ),
        Err(FsTransferError::SourceIo {
            error: FsError::UnsupportedEntry {
                operation: "copy_file_with",
                path,
            }
        }) if path == source_directory
    ));

    let destination_directory = temp.path().join("destination-directory");
    FsUtils::create_dir(&destination_directory).expect("create stream destination directory");
    assert!(matches!(
        FsUtils::copy_file_with(
            &source,
            &destination_directory,
            FsTransferOptions::default(),
            IdentityProcessor,
        ),
        Err(FsTransferError::DestinationIo {
            error: FsError::UnsupportedEntry {
                operation: "copy_file_with",
                path,
            }
        }) if path == destination_directory
    ));

    let result = FsUtils::copy_file_with(
        &source,
        &destination,
        FsTransferOptions {
            chunk_size: 1024,
            max_output_bytes: Some(1024),
        },
        DuplicateProcessor,
    );
    assert!(matches!(
        result,
        Err(FsTransferError::OutputLimitExceeded {
            limit: 1024,
            observed: 2048
        })
    ));
    assert_eq!(
        FsUtils::read_bytes(&destination, 0),
        Ok(Vec::new()),
        "the rejected chunk must not be written"
    );
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_stream_transfer_owns_arguments_and_uses_caller_runtime() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    FsUtils::write(&source, b"async transfer").expect("write transfer source");
    let expected_destination = destination.clone();
    let future = FsUtils::copy_file_with_async(
        &source,
        &destination,
        FsTransferOptions::default(),
        AsyncIdentityProcessor,
    );
    drop(source);
    drop(destination);

    let stats = tokio::spawn(future)
        .await
        .expect("transfer task should join")
        .expect("async stream transfer should succeed");
    assert_eq!(stats.input_bytes, 14);
    assert_eq!(stats.output_bytes, 14);
    assert_eq!(stats.chunks, 1);
    assert_eq!(
        FsUtils::read_bytes_async(&expected_destination, 14).await,
        Ok(b"async transfer".to_vec())
    );
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_stream_transfer_processes_multiple_chunks_and_preserves_prefix_on_limit() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    let input = (0..2050)
        .map(|index| b'a' + u8::try_from(index % 26).expect("modulo fits in u8"))
        .collect::<Vec<_>>();
    FsUtils::write(&source, &input).expect("write transfer source");
    FsUtils::write(&destination, b"stale destination").expect("write transfer destination");

    let stats = FsUtils::copy_file_with_async(
        &source,
        &destination,
        FsTransferOptions {
            chunk_size: 1024,
            max_output_bytes: None,
        },
        AsyncDuplicateProcessor,
    )
    .await
    .expect("multi-chunk async transfer should succeed");
    let expected = input
        .chunks(1024)
        .flat_map(|chunk| chunk.iter().copied().chain(chunk.iter().copied()))
        .collect::<Vec<_>>();
    assert_eq!(stats.input_bytes, 2050);
    assert_eq!(stats.output_bytes, 4100);
    assert_eq!(stats.chunks, 3);
    assert_eq!(FsUtils::read_bytes(&destination, 4100), Ok(expected));

    FsUtils::write(&destination, b"stale destination").expect("reset destination");
    let result = FsUtils::copy_file_with_async(
        &source,
        &destination,
        FsTransferOptions {
            chunk_size: 1024,
            max_output_bytes: Some(2050),
        },
        AsyncDuplicateProcessor,
    )
    .await;
    assert!(matches!(
        result,
        Err(FsTransferError::OutputLimitExceeded {
            limit: 2050,
            observed: 4096
        })
    ));
    assert_eq!(
        FsUtils::metadata(&destination)
            .expect("read partial destination metadata")
            .len(),
        2048
    );
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_stream_transfer_rejects_non_regular_source_and_destination() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    FsUtils::write(&source, b"source").expect("write stream source");

    let source_directory = temp.path().join("source-directory");
    FsUtils::create_dir(&source_directory).expect("create async stream source directory");
    assert!(matches!(
        FsUtils::copy_file_with_async(
            &source_directory,
            &destination,
            FsTransferOptions::default(),
            AsyncIdentityProcessor,
        )
        .await,
        Err(FsTransferError::SourceIo {
            error: FsError::UnsupportedEntry {
                operation: "copy_file_with",
                path,
            }
        }) if path == source_directory
    ));

    let destination_directory = temp.path().join("destination-directory");
    FsUtils::create_dir(&destination_directory).expect("create async stream destination directory");
    assert!(matches!(
        FsUtils::copy_file_with_async(
            &source,
            &destination_directory,
            FsTransferOptions::default(),
            AsyncIdentityProcessor,
        )
        .await,
        Err(FsTransferError::DestinationIo {
            error: FsError::UnsupportedEntry {
                operation: "copy_file_with",
                path,
            }
        }) if path == destination_directory
    ));

    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_stream_transfer_cancellation_keeps_written_prefix() {
    let temp = TempDir::new();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    FsUtils::write(&source, [b'x'; 2050]).expect("write cancellation source");

    let result = timeout(
        std::time::Duration::from_millis(20),
        FsUtils::copy_file_with_async(
            &source,
            &destination,
            FsTransferOptions {
                chunk_size: 1024,
                max_output_bytes: None,
            },
            AsyncCancelAfterFirst { processed: 0 },
        ),
    )
    .await;
    assert!(
        result.is_err(),
        "the second processor future should remain pending"
    );
    assert_eq!(
        FsUtils::metadata(&destination)
            .expect("partial destination should remain after cancellation")
            .len(),
        1024
    );
    temp.cleanup();
}

#[cfg(feature = "fs-async")]
#[test]
fn async_stream_transfer_checks_validation_before_runtime() {
    use std::{future::Future, pin::pin, task::Context};

    fn poll_once<F: Future>(future: F) -> std::task::Poll<F::Output> {
        let mut future = pin!(future);
        let mut context = Context::from_waker(std::task::Waker::noop());
        future.as_mut().poll(&mut context)
    }

    assert!(matches!(
        poll_once(FsUtils::copy_file_with_async(
            "missing",
            "destination",
            FsTransferOptions {
                chunk_size: 1,
                max_output_bytes: None,
            },
            AsyncIdentityProcessor,
        )),
        std::task::Poll::Ready(Err(FsTransferError::InvalidOptions {
            field: "chunk_size"
        }))
    ));
    assert!(matches!(
        poll_once(FsUtils::copy_file_with_async(
            "same",
            "same",
            FsTransferOptions::default(),
            AsyncIdentityProcessor,
        )),
        std::task::Poll::Ready(Err(FsTransferError::SameFile { .. }))
    ));
    assert!(matches!(
        poll_once(FsUtils::copy_file_with_async(
            "missing",
            "destination",
            FsTransferOptions::default(),
            AsyncIdentityProcessor,
        )),
        std::task::Poll::Ready(Err(FsTransferError::RuntimeRequired))
    ));
}

#[cfg(feature = "fs-temp")]
#[test]
fn sync_temp_context_owns_configuration_and_close_reports_cleanup() {
    let temp = TempDir::new();
    let config = FsTempConfig::default()
        .with_directory(temp.path())
        .with_prefix("axutils-")
        .with_suffix(".fixture");
    let context = FsUtils::with_temp_config(config.clone());
    assert_eq!(context.config(), &config);

    let mut file = context
        .create_temp_file()
        .expect("create configured temp file");
    assert!(file.path().starts_with(temp.path()));
    assert!(file.path().file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.starts_with("axutils-") && name.ends_with(".fixture")
    }));
    file.as_file_mut()
        .write_all(b"temporary contents")
        .expect("write temporary file");
    let file_path = file.path().to_path_buf();
    file.close().expect("close should remove temporary file");
    assert!(!file_path.exists());

    let first = context
        .create_temp_file()
        .expect("create first independent temp file");
    let second = context
        .create_temp_file()
        .expect("create second independent temp file");
    let first_path = first.path().to_path_buf();
    let second_path = second.path().to_path_buf();
    assert_ne!(first_path, second_path);
    drop(first);
    drop(second);
    assert!(!first_path.exists());
    assert!(!second_path.exists());

    let directory = context
        .create_temp_dir()
        .expect("create configured temp dir");
    let directory_path = directory.path().to_path_buf();
    directory
        .close()
        .expect("close should remove temporary dir");
    assert!(!directory_path.exists());

    let missing_parent = temp.path().join("does-not-exist");
    let result = FsUtils::with_temp_config(FsTempConfig::default().with_directory(&missing_parent))
        .create_temp_file();
    assert!(matches!(
        result,
        Err(FsTempError::Create {
            kind: io::ErrorKind::NotFound,
            ..
        })
    ));
    assert!(!missing_parent.exists());

    let invalid = FsUtils::with_temp_config(FsTempConfig::default().with_prefix("bad/name"))
        .create_temp_file();
    assert!(matches!(
        invalid,
        Err(FsTempError::InvalidConfig { field: "prefix" })
    ));
    temp.cleanup();
}

#[cfg(all(feature = "fs-temp-async", feature = "fs-async"))]
#[tokio::test]
async fn async_temp_context_supports_drop_async_and_close() {
    let temp = TempDir::new();
    let context = FsUtils::with_temp_config(
        FsTempConfig::default()
            .with_directory(temp.path())
            .with_prefix("axutils-async-"),
    );

    let file = context
        .create_temp_file_async()
        .await
        .expect("create configured async temp file");
    let file_path = file.path().to_path_buf();
    let copied_path = temp.path().join("copied-temp-file.bin");
    FsUtils::write_async(file.path(), b"temporary contents")
        .await
        .expect("write async temporary file");
    FsUtils::copy_file_async(file.path(), &copied_path)
        .await
        .expect("copy async temporary file");
    assert_eq!(
        FsUtils::read_bytes_async(&copied_path, 18).await,
        Ok(b"temporary contents".to_vec())
    );
    file.drop_async().await;
    assert!(!file_path.exists());
    assert!(copied_path.is_file());

    let directory = context
        .create_temp_dir_async()
        .await
        .expect("create configured async temp dir");
    let directory_path = directory.path().to_path_buf();
    FsUtils::write_async(directory.path().join("payload.bin"), b"payload")
        .await
        .expect("write async temporary directory payload");
    directory
        .close()
        .expect("sync close should remove async temp dir");
    assert!(!directory_path.exists());

    let static_file = FsUtils::create_temp_file_async()
        .await
        .expect("create default async temp file");
    let static_file_path = static_file.path().to_path_buf();
    static_file.drop_async().await;
    assert!(!static_file_path.exists());
    temp.cleanup();
}

#[cfg(feature = "fs-temp-async")]
#[tokio::test]
async fn async_temp_drop_cancellation_uses_backend_drop_fallback() {
    use std::{future::Future, pin::Pin, task::Context};

    let temp = TempDir::new();
    let file = FsUtils::create_temp_file_async()
        .await
        .expect("create cancellable async temp file");
    let file_path = file.path().to_path_buf();
    let mut future = Pin::from(Box::new(file.drop_async()));
    let mut context = Context::from_waker(std::task::Waker::noop());
    let _ = future.as_mut().poll(&mut context);
    drop(future);
    assert!(
        !file_path.exists(),
        "cancelled file cleanup should use Drop fallback"
    );

    let directory = FsUtils::create_temp_dir_async()
        .await
        .expect("create cancellable async temp dir");
    let directory_path = directory.path().to_path_buf();
    FsUtils::write_async(directory.path().join("payload.bin"), b"payload")
        .await
        .expect("write cancellable directory payload");
    let mut future = Pin::from(Box::new(directory.drop_async()));
    let mut context = Context::from_waker(std::task::Waker::noop());
    let _ = future.as_mut().poll(&mut context);
    drop(future);
    assert!(
        !directory_path.exists(),
        "cancelled directory cleanup should use Drop fallback"
    );
    temp.cleanup();
}

#[cfg(feature = "fs-temp-async")]
#[test]
fn async_temp_creation_requires_runtime_before_filesystem_access() {
    use std::{future::Future, pin::pin, task::Context};

    fn poll_once<F: Future>(future: F) -> std::task::Poll<F::Output> {
        let mut future = pin!(future);
        let mut context = Context::from_waker(std::task::Waker::noop());
        future.as_mut().poll(&mut context)
    }

    assert!(matches!(
        poll_once(FsUtils::create_temp_file_async()),
        std::task::Poll::Ready(Err(FsTempError::RuntimeRequired))
    ));
}
