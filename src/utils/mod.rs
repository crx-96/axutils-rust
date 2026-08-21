//! `axutils` 的通用工具模块。
//!
//! `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化与字符串脱敏默认可用；
//! `FormatUtils` 的模板能力需要显式同时启用 `serde` 和 `strfmt` 或 `minijinja` feature，并通过
//! `TemplateEngine` 参数显式选择后端。`RandomUtils` 及其相关类型需要 `rand` feature，
//! `RegUtils` 需要 `regex` feature；SMTP 邮件能力需要 `lettre` feature，异步发送还需要
//! 同时启用 `tokio`。配置文件读取能力（`ConfigUtils`）需要 `serde` feature，文件异步入口还
//! 需要同时启用 `tokio`；YAML/TOML/INI 后端分别还需要额外启用 `serde-saphyr`/`toml`/`rust-ini`。
//! `CryptoUtils` 的十六进制编解码与 `TextEncoding::Utf8` 默认可用，Base64/MD5/AES 分别需要
//! `base64`/`md5`/`aes` feature，`encoding_rs` 为 `TextEncoding` 追加 legacy 编码变体。AES
//! 静态入口在 `aes` feature 下需要先初始化一次进程级密钥与模式；多密钥或可控生命周期场景
//! 应使用 `crate::AesCipher` 实例。
//! JWT 的一次初始化全局入口需要 `jwt` feature；它只转发到 `crate::jwt` 的固定 codec。
//! HTTP 客户端需要 `http` feature；异步 HTTP 入口还需要同时启用 `tokio`。
//! Redis 客户端需要 `redis` feature；同步 API 使用惰性连接池和单键租约锁，异步 API 还
//! 需要同时启用 `tokio`，并由调用方提供 runtime。全局 `RedisUtils` 只是连接入口，不维护
//! 进程内锁表；锁 guard 自己拥有客户端 clone。
//! 本地文件系统 I/O 由默认可用的 [`crate::FsUtils`] 提供；同步方法会阻塞当前线程，异步
//! 方法需要同时启用 `tokio` 并由调用方提供 runtime。
//! SQLx 客户端需要同时启用 `sqlx` 与 `tokio` feature；`SqlxClient` 使用 SQLx Any pool，
//! `SqlxUtils` 只成功初始化一次，且由调用方提供 Tokio runtime。
//! `ConvertUtils` 始终提供无状态工具类型；整数、浮点数和 UUID 转换分别需要 `itoa`、
//! `ryu`/`zmij` 和 `uuid` feature。借用型格式化入口使用调用方 buffer，追加型入口直接写入
//! 已有字符串，拥有型入口才创建独立 `String`。
//! `TokioUtils` 需要 `tokio`，只在显式 build/run 时创建 runtime；`AxumUtils` 需要
//! `axum + tokio`，保存进程内唯一默认单次服务。
//! `SchedulerUtils` 需要同时启用 `chrono + chrono_tz + tokio + croner`，注册时由调用方
//! 提供启用了 time driver 的 runtime；它只成功初始化一次，关闭后也不可 reset/replace。
//! 库内结构化事件需要显式启用 `tracing` feature；`logging` feature 额外提供 `LogUtils`。
//! `LogUtils` 使用同步、无 ANSI 的 formatter，初始化成功后不可 reset/replace；文件轮转不
//! 负责历史文件 retention，日志写入可能阻塞产生日志的线程。

#[cfg(feature = "rand")]
pub mod random_utils;

#[cfg(feature = "regex")]
pub mod reg_utils;

pub mod format_utils;
pub mod path_utils;
pub mod time_utils;

#[cfg(feature = "lettre")]
/// SMTP 邮件工具模块。
///
/// 仅在 `lettre` feature 下公开；异步发送入口还需要 `tokio` feature，并由调用方提供
/// Tokio runtime。模块中的配置和错误只应按固定字段分类处理，不把密码或其他凭据写入日志。
pub mod email_utils;

#[cfg(feature = "serde")]
pub mod config_utils;

#[cfg(feature = "jwt")]
/// JWT 编解码和一次初始化的全局工具模块。
///
/// 仅在 `jwt` feature 下公开；全局入口成功初始化后不可替换。错误值提供固定字段、分类和
/// 长度等脱敏元数据，不包含 token、claims 或 key 内容，调用方应按公开变体匹配并保留
/// wildcard 分支。
pub mod jwt_utils;

#[cfg(feature = "http")]
pub mod http_utils;

#[cfg(feature = "redis")]
pub mod redis_utils;

#[cfg(all(feature = "sqlx", feature = "tokio"))]
pub mod sqlx_utils;

#[cfg(feature = "logging")]
pub mod log_utils;

/// 进程内唯一默认 AxumServer 的 OnceLock facade。
#[cfg(all(feature = "axum", feature = "tokio"))]
pub mod axum_utils;

/// 无状态 Tokio runtime、任务、channel 与信号 facade。
#[cfg(feature = "tokio")]
pub mod tokio_utils;

/// 一次初始化的进程级 Tokio 调度器 facade。
#[cfg(all(
    feature = "chrono",
    feature = "chrono_tz",
    feature = "tokio",
    feature = "croner"
))]
pub mod scheduler_utils;

pub mod convert_utils;
pub mod crypto_utils;
pub mod fs_utils;

pub use convert_utils::ConvertUtils;

#[cfg(feature = "rand")]
pub use random_utils::{LetterCase, RandomRangeError, RandomUtils};

#[cfg(feature = "regex")]
pub use reg_utils::RegUtils;

pub use format_utils::FormatUtils;
#[cfg(all(feature = "serde", any(feature = "strfmt", feature = "minijinja")))]
pub use format_utils::TemplateEngine;
pub use path_utils::PathUtils;
pub use time_utils::TimeUtils;

#[cfg(feature = "lettre")]
pub use email_utils::EmailUtils;

#[cfg(feature = "serde")]
pub use config_utils::ConfigUtils;

#[cfg(feature = "jwt")]
pub use jwt_utils::JwtUtils;

#[cfg(feature = "http")]
pub use http_utils::HttpUtils;

#[cfg(feature = "redis")]
pub use redis_utils::RedisUtils;

#[cfg(all(feature = "sqlx", feature = "tokio"))]
pub use sqlx_utils::SqlxUtils;

#[cfg(feature = "logging")]
pub use log_utils::{LogConfig, LogError, LogFileConfig, LogLevel, LogRotation, LogUtils};

#[cfg(all(feature = "axum", feature = "tokio"))]
pub use axum_utils::AxumUtils;

#[cfg(feature = "tokio")]
pub use tokio_utils::TokioUtils;

#[cfg(all(
    feature = "chrono",
    feature = "chrono_tz",
    feature = "tokio",
    feature = "croner"
))]
pub use scheduler_utils::SchedulerUtils;

pub use crypto_utils::CryptoUtils;
pub use fs_utils::FsUtils;
