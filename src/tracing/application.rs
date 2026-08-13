use std::fmt;

pub(crate) fn record_init() {
    ::tracing::debug!(
        target: "axutils::log",
        operation = "log_init",
        outcome = "success",
    );
}

pub(crate) fn trace(message: impl fmt::Display) {
    ::tracing::trace!(target: "axutils::log", message = %message);
}

pub(crate) fn debug(message: impl fmt::Display) {
    ::tracing::debug!(target: "axutils::log", message = %message);
}

pub(crate) fn info(message: impl fmt::Display) {
    ::tracing::info!(target: "axutils::log", message = %message);
}

pub(crate) fn warn(message: impl fmt::Display) {
    ::tracing::warn!(target: "axutils::log", message = %message);
}

pub(crate) fn error(message: impl fmt::Display) {
    ::tracing::error!(target: "axutils::log", message = %message);
}
