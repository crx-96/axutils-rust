//! 库内 tracing 埋点共用的非公开适配层。

#[cfg(feature = "logging")]
pub(crate) mod application;

#[cfg(feature = "config")]
pub(crate) mod config;
#[cfg(feature = "aes")]
pub(crate) mod crypto;
#[cfg(feature = "email")]
pub(crate) mod email;
#[cfg(feature = "http")]
pub(crate) mod http;
#[cfg(feature = "jwt")]
pub(crate) mod jwt;
#[cfg(feature = "redis")]
pub(crate) mod redis;
#[cfg(any(
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
pub(crate) mod sqlx;

#[cfg(any(
    feature = "http",
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
use std::time::Duration;
#[cfg(any(
    feature = "aes",
    feature = "config",
    feature = "http",
    feature = "jwt",
    feature = "email",
    feature = "redis",
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
use std::time::Instant;

/// 将耗时转换为不会溢出的毫秒数。
#[cfg(any(
    feature = "aes",
    feature = "config",
    feature = "http",
    feature = "jwt",
    feature = "email",
    feature = "redis",
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
pub(crate) fn duration_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u64::MAX as u128) as u64
}

/// 将持续时间转换为不会溢出的毫秒数。
#[cfg(any(
    feature = "http",
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
pub(crate) fn duration_to_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}
