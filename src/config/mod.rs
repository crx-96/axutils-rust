//! 统一的配置文件读取能力。
//!
//! 支持 JSON、YAML、TOML、INI 和 `.env`（dotenv）五种常用配置格式；JSON 与 `.env` 随
//! `config` feature 直接可用，YAML/TOML/INI 分别需要额外启用
//! `config-yaml`/`config-toml`/`config-ini`。每种格式都提供无类型（[`ConfigValue`]）与有类型
//! （`serde::Deserialize`）两条读取路径。两条路径共享同一套文件大小上限与错误语义；
//! JSON/TOML/YAML/INI 的无类型路径以及 YAML/INI
//! 的有类型路径使用本加载器的嵌套深度上限；JSON 无类型路径关闭后端较小的默认递归限制后
//! 使用本加载器的 1..=256 深度预算，JSON/TOML 有类型路径使用各自后端的递归保护。
//! YAML 别名回放还固定了有限预算：总回放事件最多 1,000,000 次、单个 anchor 最多展开 10,000
//! 次，回放栈深度不超过配置的嵌套深度上限。
//! 启用 `config-async` feature 后，`ConfigLoader` 还提供异步文件读取入口；该入口只异步化文件
//! I/O，不创建 Tokio runtime，也不把解析阶段自动移到其他线程。
//!
//! 本模块只负责“把一个配置文件安全地读成数据”：不做多文件合并、层叠覆盖、热重载、写回
//! 或 `include`/`import` 之类的指令；`.env` 语法之外的格式不提供插值或表达式能力。

mod de;
mod env;
mod error;
pub(crate) mod facade;
mod format;
mod json;
mod load;
mod loader;
mod parse;
mod source;
mod value;

#[cfg(feature = "config-ini")]
mod ini;
#[cfg(feature = "config-toml")]
mod toml;
#[cfg(feature = "config-yaml")]
mod yaml;

pub use error::ConfigError;
pub use format::ConfigFormat;
pub use loader::ConfigLoader;
pub use value::ConfigValue;

#[cfg(test)]
mod tests;
