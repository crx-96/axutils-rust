use std::{fs, io, path::Path};

use crate::fs::FsError;

use super::{error::FsTransferError, OP_COPY_FILE_WITH};

pub(super) fn source_io<E>(path: &Path, error: io::Error) -> FsTransferError<E> {
    FsTransferError::SourceIo {
        error: FsError::Io {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
            kind: error.kind(),
        },
    }
}

pub(super) fn destination_io<E>(path: &Path, error: io::Error) -> FsTransferError<E> {
    FsTransferError::DestinationIo {
        error: FsError::Io {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
            kind: error.kind(),
        },
    }
}

fn unsupported_destination_entry<E>(path: &Path) -> FsTransferError<E> {
    FsTransferError::DestinationIo {
        error: FsError::UnsupportedEntry {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
        },
    }
}

fn unsupported_source_entry<E>(path: &Path) -> FsTransferError<E> {
    FsTransferError::SourceIo {
        error: FsError::UnsupportedEntry {
            operation: OP_COPY_FILE_WITH,
            path: path.to_path_buf(),
        },
    }
}

pub(super) fn validate_regular_metadata<E>(
    metadata: io::Result<fs::Metadata>,
    path: &Path,
    source: bool,
    allow_missing: bool,
) -> Result<(), FsTransferError<E>> {
    match metadata {
        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
        Ok(_) if source => Err(unsupported_source_entry(path)),
        Ok(_) => Err(unsupported_destination_entry(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound && allow_missing => Ok(()),
        Err(error) if source => Err(source_io(path, error)),
        Err(error) => Err(destination_io(path, error)),
    }
}

pub(super) fn next_input_bytes<E>(
    current: u64,
    chunk_len: usize,
) -> Result<u64, FsTransferError<E>> {
    let chunk_len = u64::try_from(chunk_len).map_err(|_| FsTransferError::InputSizeOverflow)?;
    current
        .checked_add(chunk_len)
        .ok_or(FsTransferError::InputSizeOverflow)
}

pub(super) fn next_output_bytes<E>(
    current: u64,
    output_len: usize,
    max_output_bytes: Option<u64>,
) -> Result<u64, FsTransferError<E>> {
    let output_len = u64::try_from(output_len).map_err(|_| FsTransferError::OutputSizeOverflow)?;
    let observed = current
        .checked_add(output_len)
        .ok_or(FsTransferError::OutputSizeOverflow)?;
    if let Some(limit) = max_output_bytes {
        if observed > limit {
            return Err(FsTransferError::OutputLimitExceeded { limit, observed });
        }
    }
    Ok(observed)
}

pub(super) fn next_chunks<E>(current: u64) -> Result<u64, FsTransferError<E>> {
    current
        .checked_add(1)
        .ok_or(FsTransferError::ChunkCountOverflow)
}
