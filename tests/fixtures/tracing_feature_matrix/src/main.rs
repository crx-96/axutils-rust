#[cfg(feature = "tracing")]
fn main() {
    tracing::info!(target: "axutils::fixture", operation = "compile");
    let _ = axutils::PathUtils;
}

#[cfg(feature = "logging")]
fn main() {
    use axutils::{LogConfig, LogFileConfig, LogLevel, LogRotation, LogUtils};

    let config = LogConfig::new()
        .with_stdout(false)
        .with_level(LogLevel::Debug)
        .with_file(LogFileConfig::new("fixture.log").with_rotation(LogRotation::Never));
    let _init: fn(LogConfig) -> Result<(), axutils::LogError> = LogUtils::init;
    let _state: fn() -> bool = LogUtils::is_initialized;
    let _utils_init: fn(axutils::utils::LogConfig) -> Result<(), axutils::utils::LogError> =
        axutils::utils::LogUtils::init;
    let _module_init: fn(axutils::utils::log_utils::LogConfig) -> Result<
        (),
        axutils::utils::log_utils::LogError,
    > = axutils::utils::log_utils::LogUtils::init;
    LogUtils::init(LogConfig::default()).expect("logging fixture init");
    assert!(LogUtils::is_initialized());
    let _ = config;
}

#[cfg(feature = "direct-tracing")]
fn main() {
    let _ = tracing::Level::INFO;
    let _ = axutils::PathUtils;
}

#[cfg(feature = "negative-tracing-root")]
fn main() {
    let _ = axutils::LogUtils;
}

#[cfg(feature = "negative-none-root")]
fn main() {
    let _ = axutils::LogUtils;
}

#[cfg(feature = "negative-none-config")]
fn main() {
    let _ = axutils::LogConfig::new();
}

#[cfg(feature = "negative-tracing-config")]
fn main() {
    let _ = axutils::LogConfig::new();
}

#[cfg(feature = "negative-tracing-utils")]
fn main() {
    let _ = axutils::utils::LogUtils;
}

#[cfg(feature = "negative-tracing-module")]
fn main() {
    let _ = axutils::utils::log_utils::LogUtils;
}

#[cfg(feature = "negative-no-root-module")]
fn main() {
    let _ = axutils::log_utils::LogUtils;
}

#[cfg(not(any(
    feature = "tracing",
    feature = "logging",
    feature = "direct-tracing",
    feature = "negative-none-root",
    feature = "negative-none-config",
    feature = "negative-tracing-root",
    feature = "negative-tracing-config",
    feature = "negative-tracing-utils",
    feature = "negative-tracing-module",
    feature = "negative-no-root-module"
)))]
fn main() {}
