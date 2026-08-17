//! `axutils` 的通用工具模块。
//!
//! `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化默认可用；`FormatUtils` 的
//! 模板能力需要显式同时启用 `serde` 和 `strfmt` 或 `minijinja` feature，并通过
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
pub mod email_utils;

#[cfg(feature = "serde")]
pub mod config_utils;

#[cfg(feature = "jwt")]
pub mod jwt_utils;

#[cfg(feature = "http")]
pub mod http_utils;

#[cfg(feature = "redis")]
pub mod redis_utils;

#[cfg(all(feature = "sqlx", feature = "tokio"))]
pub mod sqlx_utils;

#[cfg(feature = "logging")]
pub mod log_utils;

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

pub use crypto_utils::CryptoUtils;
pub use fs_utils::FsUtils;
