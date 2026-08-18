fn compile_sync_api() {
    use axutils::{FsChunkProcessor, FsError, FsTransferOptions, FsUtils};

    struct Identity;

    impl FsChunkProcessor for Identity {
        type Error = std::convert::Infallible;

        fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            Ok(chunk)
        }
    }

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
    let _ = FsUtils::copy_file_with(
        "source",
        "destination",
        FsTransferOptions::default(),
        Identity,
    );
    let _ = FsUtils::read_bytes("fixture", 1);
    let _ = FsUtils::read_to_string("fixture", 1);
    let _ = FsUtils::write("fixture", b"contents");
    let _ = FsUtils::append("fixture", b"contents");

    let _: axutils::FsError = FsError::RuntimeRequired;
    let _: axutils::fs::FsError = FsError::RuntimeRequired;
    let _: axutils::FsTransferOptions = FsTransferOptions::default();
    let _: axutils::FsTransferStats = Default::default();
    let _: Option<axutils::FsTransferError<std::convert::Infallible>> = None;
    let _: axutils::fs::FsTransferOptions = FsTransferOptions::default();
    let _: axutils::fs::FsTransferStats = Default::default();
    let _: Option<axutils::fs::FsTransferError<std::convert::Infallible>> = None;

    fn assert_domain_processor<C: axutils::fs::FsChunkProcessor>() {}
    assert_domain_processor::<Identity>();
}

#[cfg(any(
    feature = "tempfile-only",
    feature = "tokio-tempfile",
    feature = "tempfile-async",
    feature = "tempfile-both",
    feature = "all"
))]
fn compile_temp_context_api() {
    use axutils::{FsTempConfig, FsUtils};

    let context = FsUtils::with_temp_config(FsTempConfig::default());
    let _ = context.config();
    let _: axutils::FsTempConfig = FsTempConfig::default();
    let _: axutils::FsTempError = axutils::FsTempError::RuntimeRequired;
    let _: axutils::fs::FsTempError = axutils::FsTempError::RuntimeRequired;
    let _: axutils::fs::FsTempConfig = FsTempConfig::default();
    let _: axutils::fs::FsUtilsContext = context;
}

#[cfg(any(
    feature = "tempfile-only",
    feature = "tokio-tempfile",
    feature = "tempfile-both",
    feature = "all"
))]
fn compile_sync_temp_api() {
    use axutils::FsUtils;

    let _ = FsUtils::create_temp_file;
    let _ = FsUtils::create_temp_dir;
    let _: Option<axutils::FsTempFile> = None;
    let _: Option<axutils::FsTempDir> = None;
    let _: Option<axutils::fs::FsTempFile> = None;
    let _: Option<axutils::fs::FsTempDir> = None;
}

#[cfg(any(
    feature = "tokio-only",
    feature = "serde-tokio",
    feature = "tokio-tempfile",
    feature = "tempfile-async",
    feature = "tempfile-both",
    feature = "all"
))]
async fn compile_async_api() {
    use axutils::{FsAsyncChunkProcessor, FsTransferOptions, FsUtils};

    struct Identity;

    impl FsAsyncChunkProcessor for Identity {
        type Error = std::convert::Infallible;
        type Future<'a> = std::future::Ready<Result<Vec<u8>, Self::Error>> where Self: 'a;

        fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
            std::future::ready(Ok(chunk))
        }
    }

    fn assert_domain_processor<C: axutils::fs::FsAsyncChunkProcessor>() {}
    assert_domain_processor::<Identity>();

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
    let _ = FsUtils::copy_file_with_async(
        "source",
        "destination",
        FsTransferOptions::default(),
        Identity,
    ).await;
    let _ = FsUtils::read_bytes_async("fixture", 1).await;
    let _ = FsUtils::read_to_string_async("fixture", 1).await;
    let _ = FsUtils::write_async("fixture", b"contents").await;
    let _ = FsUtils::append_async("fixture", b"contents").await;
}

#[cfg(any(feature = "tempfile-async", feature = "tempfile-both", feature = "all"))]
async fn compile_async_temp_api() {
    use axutils::FsUtils;

    let _ = FsUtils::create_temp_file_async().await;
    let _ = FsUtils::create_temp_dir_async().await;
    let _: Option<axutils::FsAsyncTempFile> = None;
    let _: Option<axutils::FsAsyncTempDir> = None;
    let _: Option<axutils::fs::FsAsyncTempFile> = None;
    let _: Option<axutils::fs::FsAsyncTempDir> = None;
}

#[cfg(any(
    feature = "tokio-only",
    feature = "serde-tokio",
    feature = "tokio-tempfile",
    feature = "serde-only",
    feature = "tempfile-only",
    feature = "tempfile-async",
    feature = "tempfile-both",
    feature = "all"
))]
fn main() {
    compile_sync_api();
    #[cfg(any(
        feature = "tempfile-only",
        feature = "tokio-tempfile",
        feature = "tempfile-async",
        feature = "tempfile-both",
        feature = "all"
    ))]
    compile_temp_context_api();
    #[cfg(any(
        feature = "tempfile-only",
        feature = "tokio-tempfile",
        feature = "tempfile-both",
        feature = "all"
    ))]
    compile_sync_temp_api();
    #[cfg(any(
        feature = "tokio-only",
        feature = "serde-tokio",
        feature = "tokio-tempfile",
        feature = "tempfile-async",
        feature = "tempfile-both",
        feature = "all"
    ))]
    let _ = compile_async_api;
    #[cfg(any(feature = "tempfile-async", feature = "tempfile-both", feature = "all"))]
    let _ = compile_async_temp_api;
}

#[cfg(feature = "negative-no-domain-fs-operation")]
fn main() {
    let _ = axutils::fs::read_bytes;
}

#[cfg(feature = "negative-no-domain-fs-transfer")]
fn main() {
    let _ = axutils::fs::copy_file_with;
}

#[cfg(feature = "negative-no-utils-fs-transfer")]
fn main() {
    let _: axutils::utils::FsTransferOptions = axutils::FsTransferOptions::default();
}

#[cfg(feature = "negative-no-utils-fs-temp")]
fn main() {
    let _: axutils::utils::FsTempConfig = axutils::FsTempConfig::default();
}

#[cfg(feature = "negative-tokio-no-tempfile")]
fn main() {
    let _ = axutils::FsUtils::create_temp_file;
}

#[cfg(feature = "negative-no-tempfile-sync")]
fn main() {
    let _ = axutils::FsUtils::create_temp_file;
}

#[cfg(feature = "negative-no-tempfile-async")]
fn main() {
    let _ = axutils::FsUtils::create_temp_file_async;
}

#[cfg(feature = "negative-tempfile-only-async")]
fn main() {
    let _ = axutils::FsUtils::create_temp_file_async;
}

#[cfg(feature = "negative-tempfile-async-sync")]
fn main() {
    let _ = axutils::FsUtils::create_temp_file;
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
    let _ = axutils::FsUtils::copy_file_with_async;
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
    feature = "tempfile-only",
    feature = "tokio-tempfile",
    feature = "tempfile-async",
    feature = "tempfile-both",
    feature = "all",
    feature = "negative-no-tokio-async",
    feature = "negative-tokio-no-tempfile",
    feature = "negative-no-tempfile-sync",
    feature = "negative-no-tempfile-async",
    feature = "negative-tempfile-only-async",
    feature = "negative-tempfile-async-sync",
    feature = "negative-no-domain-fs-utils",
    feature = "negative-no-domain-fs-operation",
    feature = "negative-no-domain-fs-transfer",
    feature = "negative-no-utils-fs-transfer",
    feature = "negative-no-utils-fs-temp",
    feature = "negative-no-root-fs-utils-module",
    feature = "negative-no-utils-fs-error",
    feature = "negative-no-nested-fs-error",
)))]
fn main() {
    compile_sync_api();
}
