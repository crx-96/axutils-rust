pub(crate) fn record_init() {
    ::tracing::debug!(
        target: "axutils::log",
        operation = "log_init",
        outcome = "success",
    );
}
