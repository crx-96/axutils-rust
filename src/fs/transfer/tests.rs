use super::{
    checked::{next_chunks, next_input_bytes, next_output_bytes},
    sync_pipeline::process as process_sync,
    FsChunkProcessor, FsTransferError, FsTransferOptions, FsTransferStats,
};
use crate::fs::FsError;
use std::io::{self, Read, Write};

#[cfg(feature = "fs-async")]
use super::{async_pipeline::process as process_async, FsAsyncChunkProcessor};
#[cfg(feature = "fs-async")]
use std::{
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(feature = "fs-async")]
use tokio::io::{self as async_io, AsyncRead, AsyncWrite, ReadBuf};

struct ShortReader {
    data: Vec<u8>,
    offset: usize,
    max_read: usize,
}

impl Read for ShortReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.offset == self.data.len() {
            return Ok(0);
        }
        let count = self
            .max_read
            .min(buffer.len())
            .min(self.data.len() - self.offset);
        buffer[..count].copy_from_slice(&self.data[self.offset..self.offset + count]);
        self.offset += count;
        Ok(count)
    }
}

struct ShortWriter {
    data: Vec<u8>,
    max_write: usize,
}

impl Write for ShortWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let count = self.max_write.min(buffer.len());
        self.data.extend_from_slice(&buffer[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct Identity;

impl FsChunkProcessor for Identity {
    type Error = ();

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Ok(chunk)
    }
}

struct NoDisplayError;

struct FailingProcessor;

impl FsChunkProcessor for FailingProcessor {
    type Error = NoDisplayError;

    fn process(&mut self, _chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Err(NoDisplayError)
    }
}

struct ZeroWriter;

impl Write for ZeroWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "reader failure",
        ))
    }
}

struct FlushFailWriter;

impl Write for FlushFailWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("flush failure"))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncShortReader {
    data: Vec<u8>,
    offset: usize,
    max_read: usize,
}

#[cfg(feature = "fs-async")]
impl AsyncRead for AsyncShortReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.offset == self.data.len() {
            return Poll::Ready(Ok(()));
        }
        let count = self
            .max_read
            .min(buffer.remaining())
            .min(self.data.len() - self.offset);
        buffer.put_slice(&self.data[self.offset..self.offset + count]);
        self.offset += count;
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncShortWriter {
    data: Vec<u8>,
    max_write: usize,
}

#[cfg(feature = "fs-async")]
impl AsyncWrite for AsyncShortWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let count = self.max_write.min(buffer.len());
        self.data.extend_from_slice(&buffer[..count]);
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncZeroWriter;

#[cfg(feature = "fs-async")]
impl AsyncWrite for AsyncZeroWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(0))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncFailingReader;

#[cfg(feature = "fs-async")]
impl AsyncRead for AsyncFailingReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        _buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "reader failure",
        )))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncFlushFailWriter;

#[cfg(feature = "fs-async")]
impl AsyncWrite for AsyncFlushFailWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Ready(Ok(buffer.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Err(io::Error::other("flush failure")))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncIdentity;

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncIdentity {
    type Error = ();
    type Future<'a>
        = std::future::Ready<Result<Vec<u8>, Self::Error>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
        std::future::ready(Ok(chunk))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncFailingProcessor;

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncFailingProcessor {
    type Error = NoDisplayError;
    type Future<'a>
        = std::future::Ready<Result<Vec<u8>, Self::Error>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, _chunk: Vec<u8>) -> Self::Future<'a> {
        std::future::ready(Err(NoDisplayError))
    }
}

#[cfg(feature = "fs-async")]
struct AsyncPanicProcessor;

#[cfg(feature = "fs-async")]
impl FsAsyncChunkProcessor for AsyncPanicProcessor {
    type Error = ();
    type Future<'a>
        = std::future::Ready<Result<Vec<u8>, Self::Error>>
    where
        Self: 'a;

    fn process<'a>(&'a mut self, _chunk: Vec<u8>) -> Self::Future<'a> {
        panic!("empty input must not call the async processor");
    }
}

struct PanicProcessor;

impl FsChunkProcessor for PanicProcessor {
    type Error = ();

    fn process(&mut self, _chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        panic!("empty input must not call the processor");
    }
}

#[test]
fn fills_short_reads_and_completes_short_writes_in_order() {
    let mut reader = ShortReader {
        data: b"abcdefg".to_vec(),
        offset: 0,
        max_read: 2,
    };
    let mut writer = ShortWriter {
        data: Vec::new(),
        max_write: 1,
    };
    let stats = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        Identity,
    )
    .expect("short I/O should complete");

    assert_eq!(writer.data, b"abcdefg");
    assert_eq!(stats.input_bytes, 7);
    assert_eq!(stats.output_bytes, 7);
    assert_eq!(stats.chunks, 2);
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_core_fills_short_reads_and_completes_short_writes_in_order() {
    let mut reader = AsyncShortReader {
        data: b"abcdefg".to_vec(),
        offset: 0,
        max_read: 2,
    };
    let mut writer = AsyncShortWriter {
        data: Vec::new(),
        max_write: 1,
    };
    let stats = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        AsyncIdentity,
    )
    .await
    .expect("short async I/O should complete");

    assert_eq!(writer.data, b"abcdefg");
    assert_eq!(stats.input_bytes, 7);
    assert_eq!(stats.output_bytes, 7);
    assert_eq!(stats.chunks, 2);
}

#[test]
fn processor_error_is_retained_without_display_bounds() {
    let mut reader = io::Cursor::new(b"input".to_vec());
    let mut writer = Vec::new();
    let result = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        FailingProcessor,
    );

    assert!(matches!(
        result,
        Err(FsTransferError::Processor {
            error: NoDisplayError,
            ..
        })
    ));
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_processor_error_is_retained_without_display_bounds() {
    let mut reader = AsyncShortReader {
        data: b"input".to_vec(),
        offset: 0,
        max_read: usize::MAX,
    };
    let mut writer = AsyncShortWriter {
        data: Vec::new(),
        max_write: 1,
    };
    let result = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        AsyncFailingProcessor,
    )
    .await;

    assert!(matches!(
        result,
        Err(FsTransferError::Processor {
            error: NoDisplayError,
            ..
        })
    ));
}

#[test]
fn write_zero_is_reported_as_destination_io() {
    let mut reader = io::Cursor::new(b"input".to_vec());
    let mut writer = ZeroWriter;
    let result = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        Identity,
    );

    assert!(matches!(
        result,
        Err(FsTransferError::DestinationIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::WriteZero,
                ..
            }
        })
    ));
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_write_zero_is_reported_as_destination_io() {
    let mut reader = AsyncShortReader {
        data: b"input".to_vec(),
        offset: 0,
        max_read: usize::MAX,
    };
    let mut writer = AsyncZeroWriter;
    let result = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        AsyncIdentity,
    )
    .await;

    assert!(matches!(
        result,
        Err(FsTransferError::DestinationIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::WriteZero,
                ..
            }
        })
    ));
}

#[test]
fn source_read_and_destination_flush_failures_keep_error_roles() {
    let mut reader = FailingReader;
    let mut writer = Vec::new();
    let result = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        Identity,
    );
    assert!(matches!(
        result,
        Err(FsTransferError::SourceIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::PermissionDenied,
                ..
            }
        })
    ));

    let mut reader = io::Cursor::new(b"input".to_vec());
    let mut writer = FlushFailWriter;
    let result = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        Identity,
    );
    assert!(matches!(
        result,
        Err(FsTransferError::DestinationIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::Other,
                ..
            }
        })
    ));
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_source_read_and_destination_flush_failures_keep_error_roles() {
    let mut reader = AsyncFailingReader;
    let mut writer = AsyncShortWriter {
        data: Vec::new(),
        max_write: 1,
    };
    let result = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        AsyncIdentity,
    )
    .await;
    assert!(matches!(
        result,
        Err(FsTransferError::SourceIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::PermissionDenied,
                ..
            }
        })
    ));

    let mut reader = AsyncShortReader {
        data: b"input".to_vec(),
        offset: 0,
        max_read: usize::MAX,
    };
    let mut writer = AsyncFlushFailWriter;
    let result = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: None,
        },
        AsyncIdentity,
    )
    .await;
    assert!(matches!(
        result,
        Err(FsTransferError::DestinationIo {
            error: FsError::Io {
                operation: "copy_file_with",
                kind: io::ErrorKind::Other,
                ..
            }
        })
    ));
}

#[test]
fn empty_input_does_not_call_the_processor() {
    let mut reader = io::Cursor::new(Vec::<u8>::new());
    let mut writer = Vec::new();
    let stats = process_sync(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: Some(0),
        },
        PanicProcessor,
    )
    .expect("empty input should succeed");
    assert_eq!(stats, FsTransferStats::default());
}

#[cfg(feature = "fs-async")]
#[tokio::test]
async fn async_empty_input_does_not_call_the_processor() {
    let mut reader = async_io::empty();
    let mut writer = AsyncShortWriter {
        data: Vec::new(),
        max_write: 1,
    };
    let stats = process_async(
        &mut reader,
        &mut writer,
        Path::new("source"),
        Path::new("destination"),
        FsTransferOptions {
            chunk_size: 4,
            max_output_bytes: Some(0),
        },
        AsyncPanicProcessor,
    )
    .await
    .expect("empty async input should succeed");
    assert_eq!(stats, FsTransferStats::default());
}

#[test]
fn checked_counters_distinguish_overflow_and_limit() {
    assert!(matches!(next_input_bytes::<()>(0, 4), Ok(4)));
    assert!(matches!(
        next_input_bytes::<()>(u64::MAX, 1),
        Err(FsTransferError::InputSizeOverflow)
    ));
    assert!(matches!(next_output_bytes::<()>(0, 0, Some(0)), Ok(0)));
    assert!(matches!(
        next_output_bytes::<()>(0, 1, Some(0)),
        Err(FsTransferError::OutputLimitExceeded {
            limit: 0,
            observed: 1
        })
    ));
    assert!(matches!(
        next_output_bytes::<()>(u64::MAX, 1, None),
        Err(FsTransferError::OutputSizeOverflow)
    ));
    assert!(matches!(
        next_chunks::<()>(u64::MAX),
        Err(FsTransferError::ChunkCountOverflow)
    ));
}

use std::path::Path;
