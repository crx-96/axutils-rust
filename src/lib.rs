//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认不启用第三方依赖，因此 `TimeUtils` 可以直接使用。需要正则校验工具时，
//! 通过 `regex` feature 显式启用 `RegUtils`：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["regex"] }
//! ```

pub mod utils;

#[cfg(feature = "regex")]
pub use utils::reg_utils;

#[cfg(feature = "regex")]
pub use utils::RegUtils;

pub use utils::time_utils;
pub use utils::TimeUtils;
