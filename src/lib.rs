//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认启用 `regex` feature。关闭默认 feature 后，可以通过
//! `features = ["regex"]` 按需启用正则校验工具：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", default-features = false, features = ["regex"] }
//! ```

pub mod utils;

#[cfg(feature = "regex")]
pub use utils::reg_utils;

#[cfg(feature = "regex")]
pub use utils::RegUtils;

pub use utils::time_utils;
pub use utils::TimeUtils;
