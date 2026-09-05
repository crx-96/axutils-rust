use std::time::Instant;

use crate::config::{ConfigError, ConfigFormat};

pub(crate) fn record_read(
    mode: &'static str,
    max_bytes: usize,
    result: &Result<String, ConfigError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(text) => ::tracing::debug!(
            target: "axutils::config",
            operation = "read",
            mode,
            outcome = "success",
            bytes = text.len(),
            max_bytes,
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::config",
            operation = "read",
            mode,
            outcome = "error",
            error_kind = error_kind(error),
            max_bytes,
            duration_ms,
        ),
    }
}

pub(crate) fn record_parse<T>(
    format: ConfigFormat,
    bytes: usize,
    result: &Result<T, ConfigError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(_) => ::tracing::debug!(
            target: "axutils::config",
            operation = "parse",
            format = format.as_str(),
            outcome = "success",
            bytes,
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::config",
            operation = "parse",
            format = format.as_str(),
            outcome = "error",
            error_kind = error_kind(error),
            bytes,
            duration_ms,
        ),
    }
}

fn error_kind(error: &ConfigError) -> &'static str {
    match error {
        ConfigError::Io { .. } => "io",
        ConfigError::FileTooLarge { .. } => "file_too_large",
        ConfigError::ExpandedValueTooLarge { .. } => "expanded_value_too_large",
        ConfigError::NotUtf8 { .. } => "not_utf8",
        ConfigError::UnknownExtension => "unknown_extension",
        ConfigError::FormatNotEnabled { .. } => "format_not_enabled",
        ConfigError::Parse { .. } => "parse",
        ConfigError::DepthLimitExceeded { .. } => "depth_limit_exceeded",
        ConfigError::DuplicateKey { .. } => "duplicate_key",
        ConfigError::UndefinedVariable { .. } => "undefined_variable",
        ConfigError::ValueOutOfRange { .. } => "value_out_of_range",
        ConfigError::TypeMismatch { .. } => "type_mismatch",
        ConfigError::InvalidLimit => "invalid_limit",
    }
}
