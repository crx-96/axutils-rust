//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认不启用第三方依赖，因此 `PathUtils` 和 `TimeUtils` 可以直接使用。需要邮箱和中国大陆手机号码
//! 校验时，通过 `regex` feature 显式启用 `RegUtils`；`is_phone` 还需要同时启用独立的
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

pub mod utils;

#[cfg(feature = "regex")]
pub use utils::reg_utils;

#[cfg(feature = "regex")]
pub use utils::RegUtils;

pub use utils::path_utils;
pub use utils::PathUtils;

pub use utils::time_utils;
pub use utils::TimeUtils;
