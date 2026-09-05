//! 统一的配置文件读取能力。
//!
//! 支持 JSON、YAML、TOML、INI 和 `.env`（dotenv）五种常用配置格式；JSON 与 `.env` 随
//! `config` feature 直接可用，YAML/TOML/INI 分别需要额外启用
//! `config-yaml`/`config-toml`/`config-ini`。每种格式都提供无类型（[`ConfigValue`]）与有类型
//! （`serde::Deserialize`）两条
//! 读取路径共享同一套文件大小上限与错误语义；JSON/TOML/YAML/INI 的无类型路径以及 YAML/INI
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
pub use value::ConfigValue;

use std::path::Path;

use serde::de::DeserializeOwned;

const DEFAULT_MAX_BYTES: usize = 1024 * 1024;
const MIN_MAX_BYTES: usize = 1024;
const MAX_MAX_BYTES: usize = 16 * 1024 * 1024;

const DEFAULT_MAX_DEPTH: usize = 64;
const MIN_MAX_DEPTH: usize = 1;
const MAX_MAX_DEPTH: usize = 256;

/// 配置文件读取器：持有大小/深度上限、格式覆盖和 `.env` 环境回退开关。
///
/// 所有方法都是纯函数式的构建者（consuming builder），不持有文件句柄或缓存；同一个
/// `ConfigLoader` 可以安全地重复用于多次读取。
pub struct ConfigLoader {
    pub(super) format_override: Option<ConfigFormat>,
    pub(super) max_bytes: usize,
    pub(super) max_depth: usize,
    pub(super) env_substitution: bool,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    /// 创建默认配置：文件大小上限 1 MiB，嵌套深度上限 64，`.env` 插值允许回退到进程环境变量，
    /// 格式按扩展名自动推断。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigLoader;
    ///
    /// let loader = ConfigLoader::new();
    /// let _ = loader;
    /// ```
    pub fn new() -> Self {
        Self {
            format_override: None,
            max_bytes: DEFAULT_MAX_BYTES,
            max_depth: DEFAULT_MAX_DEPTH,
            env_substitution: true,
        }
    }

    /// 显式指定格式，覆盖 [`load_value`](ConfigLoader::load_value)/[`load`](ConfigLoader::load)
    /// 的扩展名推断；不影响 [`parse_value`](ConfigLoader::parse_value)/[`parse`](ConfigLoader::parse)
    /// （它们的格式始终由调用方显式传入）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::{ConfigFormat, ConfigLoader};
    ///
    /// let loader = ConfigLoader::new().with_format(ConfigFormat::Json);
    /// let _ = loader;
    /// ```
    #[must_use]
    pub fn with_format(mut self, format: ConfigFormat) -> Self {
        self.format_override = Some(format);
        self
    }

    /// 设置文件与 `.env` 插值后累计内容的大小上限（字节），允许范围为 1 KiB 到 16 MiB
    ///（含边界）。
    ///
    /// # Errors
    ///
    /// 超出该范围时返回 [`ConfigError::InvalidLimit`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigLoader;
    ///
    /// let loader = ConfigLoader::new().with_max_bytes(64 * 1024).unwrap();
    /// let _ = loader;
    /// assert!(ConfigLoader::new().with_max_bytes(0).is_err());
    /// ```
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Result<Self, ConfigError> {
        if !(MIN_MAX_BYTES..=MAX_MAX_BYTES).contains(&max_bytes) {
            return Err(ConfigError::InvalidLimit);
        }
        self.max_bytes = max_bytes;
        Ok(self)
    }

    /// 设置嵌套深度上限，允许范围为 1 到 256（含边界）。JSON 无类型解析会严格使用此预算；
    /// JSON/TOML 有类型解析仍使用各自后端的递归保护。
    ///
    /// # Errors
    ///
    /// 超出该范围时返回 [`ConfigError::InvalidLimit`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigLoader;
    ///
    /// let loader = ConfigLoader::new().with_max_depth(8).unwrap();
    /// let _ = loader;
    /// assert!(ConfigLoader::new().with_max_depth(0).is_err());
    /// ```
    pub fn with_max_depth(mut self, max_depth: usize) -> Result<Self, ConfigError> {
        if !(MIN_MAX_DEPTH..=MAX_MAX_DEPTH).contains(&max_depth) {
            return Err(ConfigError::InvalidLimit);
        }
        self.max_depth = max_depth;
        Ok(self)
    }

    /// 设置 `.env` 的 `${VAR}` 插值是否允许在文件中找不到键时回退到进程环境变量。
    ///
    /// 默认允许回退；关闭后插值只能引用文件内已解析的键，本 crate 完全不读取进程环境变量。
    /// 该开关只影响 `.env` 插值，不影响其他格式，也从不向进程环境变量写入任何内容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::{ConfigFormat, ConfigLoader};
    ///
    /// let loader = ConfigLoader::new().with_env_substitution(false);
    /// let error = loader.parse_value("A=\"${UNDEFINED}\"\n", ConfigFormat::Env);
    /// assert!(error.is_err());
    /// ```
    #[must_use]
    pub fn with_env_substitution(mut self, enabled: bool) -> Self {
        self.env_substitution = enabled;
        self
    }

    /// 从磁盘读取配置文件，解析为无类型的 [`ConfigValue`]。
    ///
    /// # Errors
    ///
    /// 文件无法打开或读取、超过大小上限、不是合法 UTF-8、格式无法识别或未启用、内容不合法、
    /// 超过深度上限，均返回对应的 [`ConfigError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{env, fs::{self, File}, io::Write, process};
    /// use axutils::config::ConfigLoader;
    ///
    /// let mut path = env::temp_dir();
    /// path.push(format!("axutils-config-loader-doctest-{}.json", process::id()));
    /// File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let value = ConfigLoader::new().load_value(&path).unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// fs::remove_file(&path).ok();
    /// ```
    pub fn load_value(&self, path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError> {
        load::load_value(self, path.as_ref())
    }

    /// 在 Tokio runtime 中异步读取配置文件，解析为无类型的 [`ConfigValue`]。
    ///
    /// 该方法仅在启用 `config-async` feature 时提供。文件读取使用 Tokio 的普通文件
    /// API 和受限的 `take(上限 + 1)`，格式推断、显式格式覆盖、文件大小、UTF-8/BOM、深度、
    /// `.env` 回退和错误语义均沿用 [`load_value`](ConfigLoader::load_value)。crate 不创建
    /// runtime、不调用 `block_on`，解析在当前异步任务中同步执行，较大的配置仍可能占用 Tokio
    /// worker；需要隔离解析 CPU 时请由调用方自行决定 `spawn_blocking` 和并发限制。每个并发
    /// 调用独立占用最多约 `max_bytes + 1` 字节读取缓冲区，crate 不新增全局并发或内存配额；
    /// 调用方需自行限制路径来源、任务数和总内存。配置文件可能包含凭据，不要直接记录整个
    /// [`ConfigValue`] 或其错误上下文。
    ///
    /// # Errors
    ///
    /// 与 [`load_value`](ConfigLoader::load_value) 相同：打开/读取失败、超出大小上限、UTF-8、
    /// 格式、解析和深度错误均返回对应的 [`ConfigError`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::config::{ConfigError, ConfigLoader};
    ///
    /// async fn example() -> Result<(), ConfigError> {
    ///     let value = ConfigLoader::new()
    ///         .load_value_async("app.json")
    ///         .await?;
    ///     let _ = value;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "config-async")]
    pub async fn load_value_async(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ConfigValue, ConfigError> {
        load::load_value_async(self, path.as_ref()).await
    }

    /// 从磁盘读取配置文件，直接反序列化为调用方类型 `T`。
    ///
    /// # Errors
    ///
    /// 与 [`load_value`](ConfigLoader::load_value) 相同，另外当值的运行时类型与 `T` 的字段
    /// 不匹配时返回 [`ConfigError::TypeMismatch`]。JSON/TOML 的有类型路径由后端直接反序列化，
    /// 因此其嵌套深度由后端自身保护；YAML/INI 的有类型路径使用本加载器配置的深度上限。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::{env, fs::{self, File}, io::Write, process};
    /// use axutils::config::ConfigLoader;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let mut path = env::temp_dir();
    /// path.push(format!("axutils-config-loader-doctest-load-{}.json", process::id()));
    /// File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let config: Config = ConfigLoader::new().load(&path).unwrap();
    /// assert_eq!(config.port, 8080);
    /// fs::remove_file(&path).ok();
    /// ```
    pub fn load<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        load::load(self, path.as_ref())
    }

    /// 在 Tokio runtime 中异步读取配置文件，并反序列化为调用方类型 `T`。
    ///
    /// 该方法仅在启用 `config-async` feature 时提供。文件读取使用 Tokio 的普通文件
    /// API 和受限的 `take(上限 + 1)`；格式推断、显式格式覆盖、文件大小、UTF-8/BOM、深度、
    /// `.env` 回退和错误语义均沿用 [`load`](ConfigLoader::load)。crate 不创建 runtime、不调用
    /// `block_on`，解析在当前异步任务中同步执行，较大的配置仍可能占用 Tokio worker；需要隔离
    /// 解析 CPU 时请由调用方自行决定 `spawn_blocking` 和并发限制。每个并发调用独立占用最多约
    /// `max_bytes + 1` 字节读取缓冲区，crate 不新增全局并发或内存配额；调用方需自行限制路径
    /// 来源、任务数和总内存。配置文件可能包含凭据，不要直接记录反序列化结果。
    ///
    /// # Errors
    ///
    /// 与 [`load`](ConfigLoader::load) 相同；有类型反序列化错误按对应格式同步后端的既有映射
    /// 返回，例如 [`ConfigError::Parse`] 或 [`ConfigError::TypeMismatch`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::config::{ConfigError, ConfigLoader};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// async fn example() -> Result<(), ConfigError> {
    ///     let config: AppConfig = ConfigLoader::new().load_async("app.json").await?;
    ///     let _ = config;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(feature = "config-async")]
    pub async fn load_async<T: DeserializeOwned>(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<T, ConfigError> {
        load::load_async(self, path.as_ref()).await
    }

    /// 把内存中的文本按指定格式解析为无类型的 [`ConfigValue`]。
    ///
    /// 不做文件大小校验（文本已在调用方内存中），但仍受本加载器配置的深度上限约束。
    ///
    /// # Errors
    ///
    /// 内容不合法或超过深度上限时返回对应的 [`ConfigError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::{ConfigFormat, ConfigLoader};
    ///
    /// let value = ConfigLoader::new()
    ///     .parse_value(r#"{"port": 8080}"#, ConfigFormat::Json)
    ///     .unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// ```
    pub fn parse_value(
        &self,
        text: &str,
        format: ConfigFormat,
    ) -> Result<ConfigValue, ConfigError> {
        parse::parse_value(self, text, format)
    }

    /// 把内存中的文本按指定格式直接反序列化为调用方类型 `T`。
    ///
    /// # Errors
    ///
    /// 与 [`parse_value`](ConfigLoader::parse_value) 相同，另外当值的运行时类型与 `T` 的字段
    /// 不匹配时返回 [`ConfigError::TypeMismatch`]。JSON/TOML 的有类型路径由后端直接反序列化，
    /// 因此其嵌套深度由后端自身保护；YAML/INI 的有类型路径使用本加载器配置的深度上限。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::{ConfigFormat, ConfigLoader};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let config: Config = ConfigLoader::new()
    ///     .parse(r#"{"port": 8080}"#, ConfigFormat::Json)
    ///     .unwrap();
    /// assert_eq!(config.port, 8080);
    /// ```
    pub fn parse<T: DeserializeOwned>(
        &self,
        text: &str,
        format: ConfigFormat,
    ) -> Result<T, ConfigError> {
        parse::parse(self, text, format)
    }

    pub(super) fn resolve_format(&self, path: &Path) -> Result<ConfigFormat, ConfigError> {
        match self.format_override {
            Some(format) => Ok(format),
            None => ConfigFormat::from_path(path),
        }
    }
}

#[cfg(test)]
mod tests;
