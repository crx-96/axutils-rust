fn compile_sync_api() {
    use axutils::{FsError, FsUtils};

    let _ = FsUtils::try_exists("fixture");
    let _ = axutils::utils::FsUtils::is_file("fixture");
    let _ = axutils::utils::fs_utils::FsUtils::is_dir("fixture");
    let _ = FsUtils::metadata("fixture");
    let _ = FsUtils::symlink_metadata("fixture");
    let _ = FsUtils::create_file("fixture");
    let _ = FsUtils::create_dir("fixture");
    let _ = FsUtils::create_dir_all("fixture");
    let _ = FsUtils::list_dir("fixture", 1);
    let _ = FsUtils::remove_file("fixture");
    let _ = FsUtils::remove_dir("fixture");
    let _ = FsUtils::remove_dir_all("fixture");
    let _ = FsUtils::move_path("source", "destination");
    let _ = FsUtils::copy_file("source", "destination");
    let _ = FsUtils::read_bytes("fixture", 1);
    let _ = FsUtils::read_to_string("fixture", 1);
    let _ = FsUtils::write("fixture", b"contents");
    let _ = FsUtils::append("fixture", b"contents");

    let _: axutils::FsError = FsError::RuntimeRequired;
    let _: axutils::fs::FsError = FsError::RuntimeRequired;
}

#[cfg(any(feature = "tokio-only", feature = "serde-tokio"))]
async fn compile_async_api() {
    use axutils::FsUtils;

    let _ = FsUtils::try_exists_async("fixture").await;
    let _ = FsUtils::is_file_async("fixture").await;
    let _ = FsUtils::is_dir_async("fixture").await;
    let _ = FsUtils::metadata_async("fixture").await;
    let _ = FsUtils::symlink_metadata_async("fixture").await;
    let _ = FsUtils::create_file_async("fixture").await;
    let _ = FsUtils::create_dir_async("fixture").await;
    let _ = FsUtils::create_dir_all_async("fixture").await;
    let _ = FsUtils::list_dir_async("fixture", 1).await;
    let _ = FsUtils::remove_file_async("fixture").await;
    let _ = FsUtils::remove_dir_async("fixture").await;
    let _ = FsUtils::remove_dir_all_async("fixture").await;
    let _ = FsUtils::move_path_async("source", "destination").await;
    let _ = FsUtils::copy_file_async("source", "destination").await;
    let _ = FsUtils::read_bytes_async("fixture", 1).await;
    let _ = FsUtils::read_to_string_async("fixture", 1).await;
    let _ = FsUtils::write_async("fixture", b"contents").await;
    let _ = FsUtils::append_async("fixture", b"contents").await;
}

#[cfg(any(
    feature = "tokio-only",
    feature = "serde-tokio",
    feature = "serde-only"
))]
fn main() {
    compile_sync_api();
    #[cfg(any(feature = "tokio-only", feature = "serde-tokio"))]
    let _ = compile_async_api;
}

#[cfg(feature = "negative-no-domain-fs-operation")]
fn main() {
    let _ = axutils::fs::read_bytes;
}

#[cfg(feature = "negative-no-tokio-async")]
fn main() {
    let _ = axutils::FsUtils::try_exists_async;
    let _ = axutils::FsUtils::is_file_async;
    let _ = axutils::FsUtils::is_dir_async;
    let _ = axutils::FsUtils::metadata_async;
    let _ = axutils::FsUtils::symlink_metadata_async;
    let _ = axutils::FsUtils::create_file_async;
    let _ = axutils::FsUtils::create_dir_async;
    let _ = axutils::FsUtils::create_dir_all_async;
    let _ = axutils::FsUtils::list_dir_async;
    let _ = axutils::FsUtils::remove_file_async;
    let _ = axutils::FsUtils::remove_dir_async;
    let _ = axutils::FsUtils::remove_dir_all_async;
    let _ = axutils::FsUtils::move_path_async;
    let _ = axutils::FsUtils::copy_file_async;
    let _ = axutils::FsUtils::read_bytes_async;
    let _ = axutils::FsUtils::read_to_string_async;
    let _ = axutils::FsUtils::write_async;
    let _ = axutils::FsUtils::append_async;
}

#[cfg(feature = "negative-no-domain-fs-utils")]
fn main() {
    let _ = axutils::fs::FsUtils;
}

#[cfg(feature = "negative-no-root-fs-utils-module")]
fn main() {
    let _ = axutils::fs_utils::FsUtils;
}

#[cfg(feature = "negative-no-utils-fs-error")]
fn main() {
    let _: axutils::utils::FsError = axutils::FsError::RuntimeRequired;
}

#[cfg(feature = "negative-no-nested-fs-error")]
fn main() {
    let _: axutils::utils::fs_utils::FsError = axutils::FsError::RuntimeRequired;
}

#[cfg(not(any(
    feature = "tokio-only",
    feature = "serde-tokio",
    feature = "serde-only",
    feature = "negative-no-tokio-async",
    feature = "negative-no-domain-fs-utils",
    feature = "negative-no-domain-fs-operation",
    feature = "negative-no-root-fs-utils-module",
    feature = "negative-no-utils-fs-error",
    feature = "negative-no-nested-fs-error",
)))]
fn main() {
    compile_sync_api();
}
