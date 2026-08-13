use std::time::{Duration, Instant};

use crate::http::{HttpError, HttpMethod, HttpResponse};

pub(crate) fn record_completion(
    mode: &'static str,
    result: &Result<HttpResponse, HttpError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(response) if response.is_success() => {
            ::tracing::debug!(
                target: "axutils::http",
                operation = "request_complete",
                mode,
                outcome = "success",
                status = response.status(),
                attempts = response.attempts(),
                duration_ms,
            );
        }
        Ok(response) => {
            ::tracing::warn!(
                target: "axutils::http",
                operation = "request_complete",
                mode,
                outcome = "http_status",
                status = response.status(),
                attempts = response.attempts(),
                duration_ms,
            );
        }
        Err(error) => {
            ::tracing::warn!(
                target: "axutils::http",
                operation = "request_complete",
                mode,
                outcome = "error",
                error_kind = error_kind(error),
                duration_ms,
            );
        }
    }
}

pub(crate) fn record_dispatch(mode: &'static str, method: &HttpMethod, outcome: &'static str) {
    ::tracing::debug!(
        target: "axutils::http",
        operation = "request_dispatch",
        mode,
        method = method_label(method),
        outcome,
    );
}

pub(crate) fn record_retry(
    mode: &'static str,
    retry_number: u32,
    attempts: u32,
    delay: Duration,
    outcome: &'static str,
) {
    ::tracing::debug!(
        target: "axutils::http",
        operation = "request_retry",
        mode,
        retry_number,
        attempts,
        delay_ms = super::duration_to_ms(delay),
        outcome,
    );
}

pub(crate) fn record_client_init(result: &Result<(), HttpError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::http",
            operation = "client_init",
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::http",
            operation = "client_init",
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

fn error_kind(error: &HttpError) -> &'static str {
    match error {
        HttpError::InvalidConfig { .. } => "invalid_config",
        HttpError::InvalidRequest { .. } => "invalid_request",
        HttpError::InvalidUrl => "invalid_url",
        HttpError::InvalidHeaderName => "invalid_header_name",
        HttpError::InvalidHeaderValue => "invalid_header_value",
        HttpError::HeaderLimitExceeded => "header_limit_exceeded",
        HttpError::DuplicateSensitiveHeader => "duplicate_sensitive_header",
        HttpError::RequestBodyTooLarge { .. } => "request_body_too_large",
        HttpError::ResponseTooLarge { .. } => "response_too_large",
        HttpError::InvalidUtf8 => "invalid_utf8",
        HttpError::JsonSerialize => "json_serialize",
        HttpError::QuerySerialize => "query_serialize",
        HttpError::JsonDeserialize => "json_deserialize",
        HttpError::Transport { kind, .. } => match kind {
            crate::http::HttpTransportErrorKind::Connection => "connection",
            crate::http::HttpTransportErrorKind::Timeout => "timeout",
            crate::http::HttpTransportErrorKind::Tls => "tls",
            crate::http::HttpTransportErrorKind::Protocol => "protocol",
            crate::http::HttpTransportErrorKind::Other => "transport_other",
        },
        HttpError::RuntimeRequired => "runtime_required",
        HttpError::BlockingInAsyncRuntime => "blocking_in_async_runtime",
        HttpError::NotInitialized => "not_initialized",
        HttpError::AlreadyInitialized => "already_initialized",
        HttpError::CoalescedRequestCancelled => "coalesced_request_cancelled",
        HttpError::CoalescedWaitTimeout => "coalesced_wait_timeout",
        HttpError::ClientBuild => "client_build",
    }
}

fn method_label(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Options => "OPTIONS",
        HttpMethod::Trace => "TRACE",
        HttpMethod::Connect => "CONNECT",
        HttpMethod::Custom(_) => "custom",
    }
}
