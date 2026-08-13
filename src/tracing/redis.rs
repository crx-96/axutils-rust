use std::time::Instant;

use crate::redis::RedisError;

pub(crate) fn record_command<T>(
    mode: &'static str,
    backend: &'static str,
    result: &Result<T, RedisError>,
    connection_discarded: bool,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(_) => ::tracing::debug!(
            target: "axutils::redis",
            operation = "command",
            mode,
            backend,
            outcome = "success",
            connection_discarded,
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::redis",
            operation = "command",
            mode,
            backend,
            outcome = "error",
            error_kind = error_kind(error),
            connection_discarded,
            duration_ms,
        ),
    }
}

#[cfg(feature = "tokio")]
pub(crate) fn record_connection(
    operation: &'static str,
    backend: &'static str,
    outcome: &'static str,
    error: Option<&RedisError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match error {
        Some(error) => ::tracing::warn!(
            target: "axutils::redis",
            operation,
            backend,
            outcome,
            error_kind = error_kind(error),
            duration_ms,
        ),
        None => ::tracing::debug!(
            target: "axutils::redis",
            operation,
            backend,
            outcome,
            duration_ms,
        ),
    }
}

fn error_kind(error: &RedisError) -> &'static str {
    match error {
        RedisError::InvalidConfig { .. } => "invalid_config",
        RedisError::InvalidKey => "invalid_key",
        RedisError::InvalidField => "invalid_field",
        RedisError::ValueTooLarge { .. } => "value_too_large",
        RedisError::ResponseTooLarge { .. } => "response_too_large",
        RedisError::CollectionTooLarge { .. } => "collection_too_large",
        RedisError::Serialize => "serialize",
        RedisError::Deserialize => "deserialize",
        RedisError::Transport(kind) => match kind {
            crate::redis::RedisTransportErrorKind::Connection => "connection",
            crate::redis::RedisTransportErrorKind::Authentication => "authentication",
            crate::redis::RedisTransportErrorKind::Timeout => "timeout",
            crate::redis::RedisTransportErrorKind::Protocol => "protocol",
            crate::redis::RedisTransportErrorKind::Server => "server",
            crate::redis::RedisTransportErrorKind::Network => "network",
            crate::redis::RedisTransportErrorKind::Other => "transport_other",
        },
        RedisError::Pool => "pool",
        RedisError::Timeout => "timeout",
        RedisError::RuntimeRequired => "runtime_required",
        RedisError::TransactionFailed => "transaction_failed",
        RedisError::UnsupportedMode => "unsupported_mode",
        RedisError::CrossSlot => "cross_slot",
        RedisError::NotInitialized => "not_initialized",
        RedisError::AlreadyInitialized => "already_initialized",
    }
}

pub(crate) fn record_client_init(result: &Result<(), RedisError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::redis",
            operation = "client_init",
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::redis",
            operation = "client_init",
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}
