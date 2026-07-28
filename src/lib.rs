//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认不启用第三方依赖，因此 `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化
//! 能力可以直接使用。
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

pub use utils::format_utils;
pub use utils::FormatUtils;
