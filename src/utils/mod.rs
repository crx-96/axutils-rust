//! `axutils` 的通用工具模块。
//!
//! `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化默认可用；`FormatUtils` 的
//! 模板能力需要显式同时启用 `serde` 和 `strfmt` 或 `minijinja` feature，并通过
//! `TemplateEngine` 参数显式选择后端。`RandomUtils` 及其相关类型需要 `rand` feature，
//! `RegUtils` 需要 `regex` feature；SMTP 邮件能力需要 `lettre` feature，异步发送还需要
//! 同时启用 `tokio`。配置文件读取能力（`ConfigUtils`）需要 `serde` feature，YAML/TOML/INI
//! 后端分别还需要额外启用 `serde-saphyr`/`toml`/`rust-ini`。

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
