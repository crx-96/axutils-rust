//! 配置读取与解析错误。

use std::{fmt, io, path::PathBuf};

/// 配置文件读取、格式识别与解析过程中可能发生的错误。
///
/// 错误绝不回显配置文件的原始内容、解析出的值或原始出错行文本；文件路径、键名、格式名
/// 和资源上限用于定位问题，可能出现在错误中，但配置值本身永远不会。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigError {
    /// 打开或读取配置文件失败。
    Io {
        /// 调用方传入的文件路径。
        path: PathBuf,
        /// 底层 I/O 错误分类。
        kind: io::ErrorKind,
    },
    /// 文件大小超过 [`crate::config::ConfigLoader`] 配置的上限。
    FileTooLarge {
        /// 调用方传入的文件路径。
        path: PathBuf,
        /// 生效的字节数上限。
        limit: usize,
    },
    /// `.env` 插值后的累计内容超过 [`crate::config::ConfigLoader`] 配置的字节上限。
    ExpandedValueTooLarge {
        /// 生效的字节数上限。
        limit: usize,
    },
    /// 文件内容不是合法 UTF-8。
    NotUtf8 {
        /// 调用方传入的文件路径。
        path: PathBuf,
    },
    /// 无法从文件扩展名推断配置格式。
    UnknownExtension,
    /// 扩展名对应的格式已知，但其解析后端对应的 feature 未启用。
    FormatNotEnabled {
        /// 识别出的扩展名（小写）。
        extension: String,
    },
    /// 解析失败；不包含出错行的原始文本。
    Parse {
        /// 格式名称，例如 `"json"`、`"yaml"`。
        format: &'static str,
        /// 出错的一基行号（可用时）。
        line: Option<usize>,
        /// 出错的一基列号（可用时）。
        column: Option<usize>,
    },
    /// 嵌套深度超过 [`crate::config::ConfigLoader`] 配置的上限。
    DepthLimitExceeded {
        /// 生效的深度上限。
        limit: usize,
    },
    /// 同一作用域内出现重复键。
    DuplicateKey {
        /// 重复的键名。
        key: String,
    },
    /// `.env` 插值引用了一个既不在文件中、也未在进程环境变量中定义的变量。
    UndefinedVariable {
        /// 被引用但未定义的变量名。
        key: String,
        /// 引用发生的一基行号。
        line: usize,
    },
    /// 整数超出 `i64` 可表示范围。
    ValueOutOfRange {
        /// 触发该错误的键名；不是任何表字段的顶层标量时可能为空字符串。
        key: String,
    },
    /// 有类型反序列化时，值的运行时类型与目标字段的期望类型不匹配。
    TypeMismatch {
        /// 触发该错误的键名。
        key: String,
        /// 期望的类型名称。
        expected: &'static str,
    },
    /// 调用方传入的资源上限参数超出允许范围。
    InvalidLimit,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, kind } => {
                write!(
                    formatter,
                    "failed to read config file {}: {kind}",
                    path.display()
                )
            }
            Self::FileTooLarge { path, limit } => write!(
                formatter,
                "config file {} exceeds the {limit}-byte size limit",
                path.display()
            ),
            Self::ExpandedValueTooLarge { limit } => write!(
                formatter,
                "expanded env config exceeds the {limit}-byte size limit"
            ),
            Self::NotUtf8 { path } => {
                write!(
                    formatter,
                    "config file {} is not valid UTF-8",
                    path.display()
                )
            }
            Self::UnknownExtension => {
                formatter.write_str("could not infer a config format from the file extension")
            }
            Self::FormatNotEnabled { extension } => write!(
                formatter,
                "config format for extension `{extension}` requires enabling its backend feature"
            ),
            Self::Parse {
                format,
                line,
                column,
            } => match (line, column) {
                (Some(line), Some(column)) => write!(
                    formatter,
                    "failed to parse {format} config at line {line}, column {column}"
                ),
                (Some(line), None) => {
                    write!(formatter, "failed to parse {format} config at line {line}")
                }
                _ => write!(formatter, "failed to parse {format} config"),
            },
            Self::DepthLimitExceeded { limit } => write!(
                formatter,
                "config nesting depth exceeds the configured limit of {limit}"
            ),
            Self::DuplicateKey { key } => write!(formatter, "duplicate config key `{key}`"),
            Self::UndefinedVariable { key, line } => write!(
                formatter,
                "undefined variable `{key}` referenced at line {line}"
            ),
            Self::ValueOutOfRange { key } => {
                write!(formatter, "value for key `{key}` is out of range")
            }
            Self::TypeMismatch { key, expected } => {
                write!(formatter, "value for key `{key}` is not a valid {expected}")
            }
            Self::InvalidLimit => formatter.write_str("the provided resource limit is invalid"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// 计算字节偏移量在文本中对应的一基行号与列号（按 Unicode 标量值计数）。
///
/// 仅用于将后端返回的字节 span 转换为定位信息；不读取或返回偏移量之外的文本内容。
#[cfg(feature = "toml")]
pub(crate) fn line_column_at(text: &str, byte_offset: usize) -> (usize, usize) {
    let mut offset = byte_offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let mut line = 1usize;
    let mut line_start = 0usize;

    for (index, byte) in text.as_bytes()[..offset].iter().enumerate() {
        if *byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }

    let column = text[line_start..offset].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::ConfigError;
    use std::path::PathBuf;

    #[cfg(feature = "toml")]
    #[test]
    fn line_column_at_counts_unicode_scalars_not_bytes() {
        use super::line_column_at;

        let text = "a=1\nb=你好\nc=3";
        assert_eq!(line_column_at(text, 0), (1, 1));
        assert_eq!(line_column_at(text, 4), (2, 1));
        // "你" starts at byte 6 on line 2 (b'b','=' take bytes 4,5), and is 3 bytes in UTF-8.
        let second_line_third_char_offset = 4 + "b=".len() + "你".len();
        assert_eq!(line_column_at(text, second_line_third_char_offset), (2, 4));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn line_column_at_clamps_out_of_range_offsets() {
        use super::line_column_at;

        let text = "abc";
        assert_eq!(line_column_at(text, 100), (1, 4));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn line_column_at_clamps_offsets_inside_utf8_scalars() {
        use super::line_column_at;

        let text = "a=你";
        assert_eq!(line_column_at(text, 3), (1, 3));
    }

    #[test]
    fn error_display_never_echoes_config_values() {
        let secret = "s3cr3t-value-that-must-not-appear";
        let errors = [
            ConfigError::Io {
                path: PathBuf::from("config.toml"),
                kind: std::io::ErrorKind::NotFound,
            },
            ConfigError::FileTooLarge {
                path: PathBuf::from("config.toml"),
                limit: 1024,
            },
            ConfigError::ExpandedValueTooLarge { limit: 1024 },
            ConfigError::NotUtf8 {
                path: PathBuf::from("config.toml"),
            },
            ConfigError::UnknownExtension,
            ConfigError::FormatNotEnabled {
                extension: "yaml".to_owned(),
            },
            ConfigError::Parse {
                format: "json",
                line: Some(3),
                column: Some(5),
            },
            ConfigError::DepthLimitExceeded { limit: 64 },
            ConfigError::DuplicateKey {
                key: "password".to_owned(),
            },
            ConfigError::UndefinedVariable {
                key: "TOKEN".to_owned(),
                line: 2,
            },
            ConfigError::ValueOutOfRange {
                key: "count".to_owned(),
            },
            ConfigError::TypeMismatch {
                key: "enabled".to_owned(),
                expected: "bool",
            },
            ConfigError::InvalidLimit,
        ];

        for error in errors {
            let display = error.to_string();
            let debug = format!("{error:?}");
            assert!(!display.contains(secret));
            assert!(!debug.contains(secret));
        }
    }
}
