//! 受限的配置文件读取：大小上限、BOM 处理与 UTF-8 校验。

use std::{fs::File, io::Read, path::Path};

use super::error::ConfigError;

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// 在 `max_bytes` 上限内读取文件并解码为 UTF-8 字符串（跳过前导 BOM）。
///
/// 使用 `File::open` 加 `Read::take(max_bytes + 1)`，只要实际读到超过 `max_bytes` 就立即
/// 判定为超限；不依赖 `fs::metadata` 报告的大小，避免 TOCTOU 竞争，以及命名管道、`/proc`
/// 等大小汇报为 0 却可能无限输出的特殊文件耗尽内存。
pub(crate) fn read_bounded(path: &Path, max_bytes: usize) -> Result<String, ConfigError> {
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

fn io_error(path: &Path, error: &std::io::Error) -> ConfigError {
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
    use super::read_bounded;
    use crate::ConfigError;
    use std::io::Write;

    fn write_temp_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-source-test-{}-{name}",
            std::process::id()
        ));
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(contents).expect("write temp file");
        path
    }

    #[test]
    fn reads_file_within_limit_and_strips_bom() {
        let path = write_temp_file("bom.txt", b"\xEF\xBB\xBFhello");
        let text = read_bounded(&path, 1024).expect("read within limit");
        assert_eq!(text, "hello");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_file_exceeding_limit_without_reading_it_fully() {
        let path = write_temp_file("too-large.txt", &[b'a'; 20]);
        let result = read_bounded(&path, 10);
        assert!(matches!(
            result,
            Err(ConfigError::FileTooLarge { limit: 10, .. })
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_file_exactly_at_limit() {
        let path = write_temp_file("exact.txt", &[b'a'; 10]);
        let text = read_bounded(&path, 10).expect("read exactly at limit");
        assert_eq!(text.len(), 10);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_non_utf8_content() {
        let path = write_temp_file("invalid-utf8.bin", &[0xFF, 0xFE, 0xFD]);
        let result = read_bounded(&path, 1024);
        assert!(matches!(result, Err(ConfigError::NotUtf8 { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reports_io_error_for_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-source-test-{}-missing.txt",
            std::process::id()
        ));
        let result = read_bounded(&path, 1024);
        assert!(matches!(
            result,
            Err(ConfigError::Io {
                kind: std::io::ErrorKind::NotFound,
                ..
            })
        ));
    }
}
