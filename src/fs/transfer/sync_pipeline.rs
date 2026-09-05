use std::{
    io::{self, Read, Write},
    path::Path,
};

use super::{
    checked::{destination_io, next_chunks, next_input_bytes, next_output_bytes, source_io},
    error::FsTransferError,
    options::FsTransferOptions,
    processor::FsChunkProcessor,
    stats::FsTransferStats,
};

fn read_chunk<R: Read>(reader: &mut R, chunk_size: usize) -> io::Result<Option<Vec<u8>>> {
    let mut chunk = vec![0; chunk_size];
    let mut filled = 0;
    while filled < chunk_size {
        match reader.read(&mut chunk[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => {
                chunk.truncate(filled);
                return Ok(Some(chunk));
            }
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(Some(chunk))
}

pub(super) fn process<R, W, C>(
    reader: &mut R,
    writer: &mut W,
    source: &Path,
    destination: &Path,
    options: FsTransferOptions,
    mut processor: C,
) -> Result<FsTransferStats, FsTransferError<C::Error>>
where
    R: Read,
    W: Write,
    C: FsChunkProcessor,
{
    let mut stats = FsTransferStats::default();
    loop {
        let Some(chunk) =
            read_chunk(reader, options.chunk_size).map_err(|error| source_io(source, error))?
        else {
            break;
        };

        let next_input = next_input_bytes(stats.input_bytes, chunk.len())?;
        let output = processor
            .process(chunk)
            .map_err(|error| FsTransferError::Processor {
                error,
                source: source.to_path_buf(),
                destination: destination.to_path_buf(),
            })?;
        let next_output =
            next_output_bytes(stats.output_bytes, output.len(), options.max_output_bytes)?;
        let next_chunk_count = next_chunks(stats.chunks)?;

        if !output.is_empty() {
            writer
                .write_all(&output)
                .map_err(|error| destination_io(destination, error))?;
        }
        stats.input_bytes = next_input;
        stats.output_bytes = next_output;
        stats.chunks = next_chunk_count;
    }

    writer
        .flush()
        .map_err(|error| destination_io(destination, error))?;
    Ok(stats)
}
