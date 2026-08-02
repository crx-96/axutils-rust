//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认不启用第三方依赖，因此 `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化
//! 能力可以直接使用。
//! 需要发送 SMTP 邮件时，显式启用 `lettre` feature；它提供强制 SMTPS/STARTTLS、连接池、
//! 多实例 `EmailClient` 和一次初始化的全局 `EmailUtils`。如果还要使用异步发送，必须
//! 同时启用 `lettre` 与 `tokio` feature，异步调用方需要自行运行在 Tokio runtime 中。
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["lettre"] }
//! # 异步邮件改为 features = ["lettre", "tokio"]，并由调用方提供 Tokio runtime。
//! ```
//! 需要随机工具时，
//! 通过 `rand` feature 显式启用 `RandomUtils`；需要邮箱和中国大陆手机号码校验时，
//! 通过 `regex` feature 显式启用 `RegUtils`；`is_phone` 还需要同时启用独立的
//! `libphonenumber` feature：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["regex"] }
//! ```

//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
//! ```

//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["rand"] }
//! ```
//!
//! `FormatUtils` 的运行时模板能力需要用户显式启用 `serde` 和一个后端 feature：`strfmt`
//! 使用 `{name}` 语法并只支持扁平顶层变量；`minijinja` 使用 `{{ name }}` 语法，支持嵌套
//! 字段、数组、条件和循环。后端 feature 不会自动启用 `serde`；同时启用两个后端时，请调用
//! 带后缀的方法以明确选择模板语法：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["serde", "minijinja"] }
//! ```
//!
//! `TimeUtils` 的日期格式化能力分别由 `chrono`、`time` 和 `jiff` feature 提供。三个
//! feature 相互独立；只启用一个后端时可以使用无后缀方法，同时启用多个后端时应调用带
//! 后缀的方法以明确日期类型。日期默认模板为 `yyyy-MM-dd`，含时间值默认模板为
//! `yyyy-MM-dd HH:mm:ss`；带偏移方法的 `offset: Option<TimeZoneOffset>` 传入 `None` 时
//! 使用 `+08:00`。格式化采用本 crate 的统一模板：`yyyy`、`MM`、`dd`、`HH`、`mm`、
//! `ss`、`SSS` 与固定偏移专用的 `XXX`。
//!
//! 需要读取配置文件时，通过 `serde` feature 显式启用 `ConfigLoader`/`ConfigUtils`，提供
//! JSON 与自实现 `.env`（dotenv）读取；YAML、TOML、INI 分别需要额外启用
//! `serde-saphyr`、`toml`、`rust-ini` feature。每种格式都提供无类型 [`ConfigValue`]（点号
//! 路径访问）与有类型 `serde::Deserialize` 两条读取路径；文件大小上限统一，JSON/TOML/YAML/INI
//! 的无类型路径以及 YAML/INI 的有类型路径使用配置的嵌套深度上限，JSON/TOML 有类型路径使用
//! 各自后端的递归保护；错误不回显配置文件内容：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["serde", "serde-saphyr", "toml", "rust-ini"] }
//! ```

mod time;
pub mod utils;

#[cfg(feature = "regex")]
pub use utils::reg_utils;

#[cfg(feature = "regex")]
pub use utils::RegUtils;

#[cfg(feature = "rand")]
pub use utils::random_utils;

#[cfg(feature = "rand")]
pub use utils::{LetterCase, RandomRangeError, RandomUtils};

pub use utils::path_utils;
pub use utils::PathUtils;

pub use utils::time_utils;
pub use utils::TimeUtils;

pub use time::{
    TimeFormatError, TimeFormatToken, TimeValueKind, TimeZoneOffset, TimeZoneOffsetError,
};

pub use utils::format_utils;
pub use utils::FormatUtils;

#[cfg(feature = "lettre")]
pub mod email;

#[cfg(feature = "lettre")]
pub use email::{
    EmailBody, EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity,
    EmailTransportErrorKind,
};

#[cfg(feature = "lettre")]
pub use utils::EmailUtils;

#[cfg(feature = "serde")]
pub mod config;

#[cfg(feature = "serde")]
pub use config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

#[cfg(feature = "serde")]
pub use utils::ConfigUtils;
