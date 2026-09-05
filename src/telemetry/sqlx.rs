use std::time::{Duration, Instant};

use crate::sqlx::{SqlxClient, SqlxError, SqlxTransportErrorKind};

pub(crate) struct ConnectMetadata {
    pub(crate) driver: &'static str,
    pub(crate) sqlite_memory: bool,
    pub(crate) max_connections: u32,
    pub(crate) min_connections: u32,
    pub(crate) acquire_timeout: Duration,
    pub(crate) max_rows: usize,
}

pub(crate) fn record_connect(
    metadata: ConnectMetadata,
    result: &Result<SqlxClient, SqlxError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    let acquire_timeout_ms = super::duration_to_ms(metadata.acquire_timeout);
    match result {
        Ok(_) => ::tracing::debug!(
            target: "axutils::sqlx",
            operation = "connect",
            driver = metadata.driver,
            sqlite_memory = metadata.sqlite_memory,
            max_connections = metadata.max_connections,
            min_connections = metadata.min_connections,
            acquire_timeout_ms,
            max_rows = metadata.max_rows,
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::sqlx",
            operation = "connect",
            driver = metadata.driver,
            sqlite_memory = metadata.sqlite_memory,
            max_connections = metadata.max_connections,
            min_connections = metadata.min_connections,
            acquire_timeout_ms,
            max_rows = metadata.max_rows,
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

pub(crate) fn record_event<T>(
    operation: &'static str,
    driver: &'static str,
    rows: usize,
    max_rows: usize,
    result: &Result<T, SqlxError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(_) => ::tracing::debug!(
            target: "axutils::sqlx",
            operation,
            driver,
            rows,
            max_rows,
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::sqlx",
            operation,
            driver,
            rows,
            max_rows,
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

fn error_kind(error: &SqlxError) -> &'static str {
    match error {
        SqlxError::InvalidConfig { .. } => "invalid_config",
        SqlxError::RuntimeRequired => "runtime_required",
        SqlxError::NotInitialized => "not_initialized",
        SqlxError::AlreadyInitialized => "already_initialized",
        SqlxError::RowNotFound => "row_not_found",
        SqlxError::RowLimitExceeded { .. } => "row_limit_exceeded",
        SqlxError::PoolAcquireTimeout => "pool_acquire_timeout",
        SqlxError::PoolClosed => "pool_closed",
        SqlxError::TransactionFailed => "transaction_failed",
        SqlxError::Transport(kind) => match kind {
            SqlxTransportErrorKind::Connection => "connection",
            SqlxTransportErrorKind::Timeout => "timeout",
            SqlxTransportErrorKind::Protocol => "protocol",
            SqlxTransportErrorKind::Server => "server",
            SqlxTransportErrorKind::Network => "network",
            SqlxTransportErrorKind::Decode => "decode",
            SqlxTransportErrorKind::Encode => "encode",
            SqlxTransportErrorKind::Tls => "tls",
            SqlxTransportErrorKind::Other => "transport_other",
        },
    }
}

pub(crate) fn record_client_init(result: &Result<(), SqlxError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::sqlx",
            operation = "client_init",
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::sqlx",
            operation = "client_init",
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

pub(crate) fn record_init_cleanup(error: &SqlxError, started: Instant) {
    ::tracing::warn!(
        target: "axutils::sqlx",
        operation = "client_init_cleanup",
        outcome = "error",
        error_kind = error_kind(error),
        duration_ms = super::duration_ms(started),
    );
}
