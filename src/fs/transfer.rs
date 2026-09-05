//! 文件流式传输领域类型和实现。

mod checked;
mod error;
mod options;
mod processor;
mod stats;
mod sync_pipeline;

#[cfg(feature = "fs-async")]
mod async_pipeline;

#[cfg(test)]
#[path = "transfer/tests.rs"]
mod tests;

use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
};

#[cfg(feature = "fs-async")]
use super::ops;
#[cfg(feature = "fs-async")]
use tokio::fs::{self as async_fs, File as AsyncFile, OpenOptions as AsyncOpenOptions};

pub use error::FsTransferError;
pub use options::FsTransferOptions;
pub use processor::FsChunkProcessor;
pub use stats::FsTransferStats;

#[cfg(feature = "fs-async")]
pub use processor::FsAsyncChunkProcessor;

const OP_COPY_FILE_WITH: &str = "copy_file_with";

pub(crate) fn copy_file_with<C>(
    source: PathBuf,
    destination: PathBuf,
    options: FsTransferOptions,
    processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    C: FsChunkProcessor,
{
    options.validate()?;
    if source == destination {
        return Err(FsTransferError::SameFile {
            source,
            destination,
        });
    }

    checked::validate_regular_metadata(fs::symlink_metadata(&source), &source, true, false)?;
    checked::validate_regular_metadata(
        fs::symlink_metadata(&destination),
        &destination,
        false,
        true,
    )?;

    let mut source_file =
        File::open(&source).map_err(|error| checked::source_io(&source, error))?;
    let mut destination_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&destination)
        .map_err(|error| checked::destination_io(&destination, error))?;

    sync_pipeline::process(
        &mut source_file,
        &mut destination_file,
        &source,
        &destination,
        options,
        processor,
    )
}

#[cfg(feature = "fs-async")]
pub(crate) async fn copy_file_with_async<C>(
    source: PathBuf,
    destination: PathBuf,
    options: FsTransferOptions,
    processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    C: FsAsyncChunkProcessor,
{
    options.validate()?;
    if source == destination {
        return Err(FsTransferError::SameFile {
            source,
            destination,
        });
    }
    ops::ensure_runtime().map_err(|_| FsTransferError::RuntimeRequired)?;

    checked::validate_regular_metadata(
        async_fs::symlink_metadata(&source).await,
        &source,
        true,
        false,
    )?;
    checked::validate_regular_metadata(
        async_fs::symlink_metadata(&destination).await,
        &destination,
        false,
        true,
    )?;

    let mut source_file = AsyncFile::open(&source)
        .await
        .map_err(|error| checked::source_io(&source, error))?;
    let mut destination_file = AsyncOpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&destination)
        .await
        .map_err(|error| checked::destination_io(&destination, error))?;

    async_pipeline::process(
        &mut source_file,
        &mut destination_file,
        &source,
        &destination,
        options,
        processor,
    )
    .await
}
