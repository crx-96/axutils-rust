//! `axutils` 的通用工具模块。
//!
//! `PathUtils`、`TimeUtils` 和 `FormatUtils` 默认可用；`RandomUtils` 及其相关类型需要
//! `rand` feature，`RegUtils` 需要 `regex` feature。

#[cfg(feature = "rand")]
pub mod random_utils;

#[cfg(feature = "regex")]
pub mod reg_utils;

pub mod format_utils;
pub mod path_utils;
pub mod time_utils;

#[cfg(feature = "rand")]
pub use random_utils::{LetterCase, RandomRangeError, RandomUtils};

#[cfg(feature = "regex")]
pub use reg_utils::RegUtils;

pub use format_utils::FormatUtils;
pub use path_utils::PathUtils;
pub use time_utils::TimeUtils;
