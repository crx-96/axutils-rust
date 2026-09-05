use std::time::Instant;

use crate::email::{EmailError, EmailTransportErrorKind};

pub(crate) fn record_send(mode: &'static str, result: &Result<(), EmailError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::email",
            operation = "send",
            mode,
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::email",
            operation = "send",
            mode,
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

#[cfg(feature = "email-async")]
pub(crate) fn record_transport_init(result: Result<(), &EmailError>) {
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::email",
            operation = "transport_init",
            outcome = "success",
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::email",
            operation = "transport_init",
            outcome = "error",
            error_kind = error_kind(error),
        ),
    }
}

pub(crate) fn record_client_init(result: &Result<(), EmailError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::email",
            operation = "client_init",
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::email",
            operation = "client_init",
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

fn error_kind(error: &EmailError) -> &'static str {
    match error {
        EmailError::InvalidConfig { .. } => "invalid_config",
        EmailError::InvalidMessage { .. } => "invalid_message",
        EmailError::InvalidRecipient { .. } => "invalid_recipient",
        EmailError::MessageBuild => "message_build",
        EmailError::Transport(kind) => match kind {
            EmailTransportErrorKind::Connection => "connection",
            EmailTransportErrorKind::Tls => "tls",
            EmailTransportErrorKind::Authentication => "authentication",
            EmailTransportErrorKind::Timeout => "timeout",
            EmailTransportErrorKind::SmtpResponse => "smtp_response",
            EmailTransportErrorKind::Network => "network",
            EmailTransportErrorKind::Client => "client",
            EmailTransportErrorKind::Shutdown => "shutdown",
        },
        EmailError::NotInitialized => "not_initialized",
        EmailError::AlreadyInitialized => "already_initialized",
    }
}
