//! 便利工具 façade 的统一公共入口。
//!
//! 叶子实现模块保持私有；调用方应从本模块导入 `XxxUtils`。领域 Client、配置、错误和
//! 数据模型则从对应的 `axutils::<domain>` 模块导入。

mod convert_utils;
mod crypto_utils;
mod format_utils;
mod fs_utils;
mod path_utils;
mod time_utils;

#[cfg(feature = "rand")]
mod random_utils;
#[cfg(feature = "regex")]
mod reg_utils;

#[cfg(feature = "axum")]
mod axum_utils;
#[cfg(feature = "config")]
mod config_utils;
#[cfg(feature = "email")]
mod email_utils;
#[cfg(feature = "http")]
mod http_utils;
#[cfg(feature = "jwt")]
mod jwt_utils;
#[cfg(feature = "logging")]
mod log_utils;
#[cfg(feature = "redis")]
mod redis_utils;
#[cfg(feature = "scheduler")]
mod scheduler_utils;
#[cfg(any(
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
mod sqlx_utils;
#[cfg(feature = "tokio")]
mod tokio_utils;

pub use convert_utils::ConvertUtils;
pub use crypto_utils::CryptoUtils;
pub use format_utils::FormatUtils;
pub use fs_utils::FsUtils;
pub use path_utils::PathUtils;
pub use time_utils::TimeUtils;

#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
pub use format_utils::TemplateEngine;
#[cfg(feature = "rand")]
pub use random_utils::{LetterCase, RandomRangeError, RandomUtils};
#[cfg(feature = "regex")]
pub use reg_utils::RegUtils;

#[cfg(feature = "axum")]
pub use axum_utils::AxumUtils;
#[cfg(feature = "config")]
pub use config_utils::ConfigUtils;
#[cfg(feature = "email")]
pub use email_utils::EmailUtils;
#[cfg(feature = "http")]
pub use http_utils::HttpUtils;
#[cfg(feature = "jwt")]
pub use jwt_utils::JwtUtils;
#[cfg(feature = "logging")]
pub use log_utils::LogUtils;
#[cfg(feature = "redis")]
pub use redis_utils::RedisUtils;
#[cfg(feature = "scheduler")]
pub use scheduler_utils::SchedulerUtils;
#[cfg(any(
    feature = "sqlx",
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
))]
pub use sqlx_utils::SqlxUtils;
#[cfg(feature = "tokio")]
pub use tokio_utils::TokioUtils;
