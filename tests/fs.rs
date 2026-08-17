use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use axutils::{FsError, FsUtils};

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

    let _: axutils::fs::FsError = FsError::RuntimeRequired;
    let _: axutils::utils::FsUtils = axutils::utils::FsUtils;
    let _: axutils::utils::fs_utils::FsUtils = axutils::utils::fs_utils::FsUtils;
    assert_same_fs_utils(axutils::utils::fs_utils::FsUtils);
    assert_same_fs_error(axutils::fs::FsError::RuntimeRequired);
    temp.cleanup();
}

fn assert_same_fs_utils(_: axutils::FsUtils) {}

fn assert_same_fs_error(_: axutils::FsError) {}

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

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
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

#[cfg(feature = "tokio")]
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
