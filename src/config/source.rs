//! 受限的配置文件读取：大小上限、BOM 处理与 UTF-8 校验。
//!
//! 启用 `config-async` 时，异步 helper 使用 Tokio 普通文件 API，并复用同步路径的
//! 错误映射、BOM 处理和 UTF-8 校验。

#[cfg(feature = "tracing")]
use std::time::Instant;
use std::{
    fs::File,
    io::{Error as IoError, Read},
    path::Path,
};

use super::error::ConfigError;

#[cfg(feature = "tracing")]
use crate::telemetry::config as config_trace;
#[cfg(feature = "config-async")]
use tokio::fs::File as AsyncFile;
#[cfg(feature = "config-async")]
use tokio::io::AsyncReadExt;

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// 在 `max_bytes` 上限内读取文件并解码为 UTF-8 字符串（跳过前导 BOM）。
///
/// 使用 `File::open` 加 `Read::take(max_bytes + 1)`，只要实际读到超过 `max_bytes` 就立即
/// 判定为超限；不依赖 `fs::metadata` 报告的大小，避免 TOCTOU 竞争，以及命名管道、`/proc`
/// 等大小汇报为 0 却可能无限输出的特殊文件耗尽内存。
pub(crate) fn read_bounded(path: &Path, max_bytes: usize) -> Result<String, ConfigError> {
    #[cfg(feature = "tracing")]
    let started = Instant::now();
    let result = read_bounded_inner(path, max_bytes);
    #[cfg(feature = "tracing")]
    config_trace::record_read("sync", max_bytes, &result, started);
    result
}

fn read_bounded_inner(path: &Path, max_bytes: usize) -> Result<String, ConfigError> {
    let mut file = File::open(path).map_err(|error| io_error(path, &error))?;

    let mut buffer = Vec::new();
    file.by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|error| io_error(path, &error))?;

    if buffer.len() > max_bytes {
        return Err(ConfigError::FileTooLarge {
            path: path.to_path_buf(),
            limit: max_bytes,
        });
    }

    decode_utf8(buffer, path)
}

/// 在 `max_bytes` 上限内异步读取文件并解码为 UTF-8 字符串（跳过前导 BOM）。
///
/// 读取使用 Tokio 的普通文件 API 和 `take(max_bytes + 1)`，不会把无界文件一次性读入内存。
#[cfg(feature = "config-async")]
pub(crate) async fn read_bounded_async(
    path: &Path,
    max_bytes: usize,
) -> Result<String, ConfigError> {
    #[cfg(feature = "tracing")]
    let started = Instant::now();
    let result = read_bounded_async_inner(path, max_bytes).await;
    #[cfg(feature = "tracing")]
    config_trace::record_read("async", max_bytes, &result, started);
    result
}

#[cfg(feature = "config-async")]
async fn read_bounded_async_inner(path: &Path, max_bytes: usize) -> Result<String, ConfigError> {
    let file = AsyncFile::open(path)
        .await
        .map_err(|error| io_error(path, &error))?;

    let mut buffer = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut buffer)
        .await
        .map_err(|error| io_error(path, &error))?;

    if buffer.len() > max_bytes {
        return Err(ConfigError::FileTooLarge {
            path: path.to_path_buf(),
            limit: max_bytes,
        });
    }

    decode_utf8(buffer, path)
}

fn io_error(path: &Path, error: &IoError) -> ConfigError {
    ConfigError::Io {
        path: path.to_path_buf(),
        kind: error.kind(),
    }
}

fn decode_utf8(mut buffer: Vec<u8>, path: &Path) -> Result<String, ConfigError> {
    if buffer.starts_with(&UTF8_BOM) {
        buffer.drain(..UTF8_BOM.len());
    }
    String::from_utf8(buffer).map_err(|_| ConfigError::NotUtf8 {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        fs::{self, File},
        io::{ErrorKind, Write},
        path::PathBuf,
        process,
    };

    use super as config_source;
    use crate::config::ConfigError;

    fn write_temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "axutils-config-source-test-{}-{name}",
            process::id()
        ));
        let mut file = File::create(&path).expect("create temp file");
        file.write_all(contents).expect("write temp file");
        path
    }

    #[test]
    fn reads_file_within_limit_and_strips_bom() {
        let path = write_temp_file("bom.txt", b"\xEF\xBB\xBFhello");
        let text = config_source::read_bounded(&path, 1024).expect("read within limit");
        assert_eq!(text, "hello");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_file_exceeding_limit_without_reading_it_fully() {
        let path = write_temp_file("too-large.txt", &[b'a'; 20]);
        let result = config_source::read_bounded(&path, 10);
        assert!(matches!(
            result,
            Err(ConfigError::FileTooLarge { limit: 10, .. })
        ));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_file_exactly_at_limit() {
        let path = write_temp_file("exact.txt", &[b'a'; 10]);
        let text = config_source::read_bounded(&path, 10).expect("read exactly at limit");
        assert_eq!(text.len(), 10);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_non_utf8_content() {
        let path = write_temp_file("invalid-utf8.bin", &[0xFF, 0xFE, 0xFD]);
        let result = config_source::read_bounded(&path, 1024);
        assert!(matches!(result, Err(ConfigError::NotUtf8 { .. })));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn reports_io_error_for_missing_file() {
        let path = env::temp_dir().join(format!(
            "axutils-config-source-test-{}-missing.txt",
            process::id()
        ));
        let result = config_source::read_bounded(&path, 1024);
        assert!(matches!(
            result,
            Err(ConfigError::Io {
                kind: ErrorKind::NotFound,
                ..
            })
        ));
    }

    #[cfg(feature = "config-async")]
    #[tokio::test]
    async fn async_reads_file_within_limit_and_strips_bom() {
        let path = write_temp_file("async-bom.txt", b"\xEF\xBB\xBFhello");
        let text = config_source::read_bounded_async(&path, 1024)
            .await
            .expect("async read within limit");
        assert_eq!(text, "hello");
        let _ = fs::remove_file(&path);
    }

    #[cfg(feature = "config-async")]
    #[tokio::test]
    async fn async_rejects_file_exceeding_limit() {
        let path = write_temp_file("async-too-large.txt", &[b'a'; 20]);
        let result = config_source::read_bounded_async(&path, 10).await;
        assert!(matches!(
            result,
            Err(ConfigError::FileTooLarge { limit: 10, .. })
        ));
        let _ = fs::remove_file(&path);
    }

    #[cfg(feature = "config-async")]
    #[tokio::test]
    async fn async_accepts_file_exactly_at_limit() {
        let path = write_temp_file("async-exact.txt", &[b'a'; 10]);
        let text = config_source::read_bounded_async(&path, 10)
            .await
            .expect("async read exactly at limit");
        assert_eq!(text.len(), 10);
        let _ = fs::remove_file(&path);
    }

    #[cfg(feature = "config-async")]
    #[tokio::test]
    async fn async_rejects_non_utf8_content() {
        let path = write_temp_file("async-invalid-utf8.bin", &[0xFF, 0xFE, 0xFD]);
        let result = config_source::read_bounded_async(&path, 1024).await;
        assert!(matches!(result, Err(ConfigError::NotUtf8 { .. })));
        let _ = fs::remove_file(&path);
    }

    #[cfg(feature = "config-async")]
    #[tokio::test]
    async fn async_reports_io_error_for_missing_file() {
        let path = env::temp_dir().join(format!(
            "axutils-config-source-test-{}-async-missing.txt",
            process::id()
        ));
        let result = config_source::read_bounded_async(&path, 1024).await;
        assert!(matches!(
            result,
            Err(ConfigError::Io {
                kind: ErrorKind::NotFound,
                ..
            })
        ));
    }
}
