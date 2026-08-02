//! 统一的配置文件读取能力。
//!
//! 支持 JSON、YAML、TOML、INI 和 `.env`（dotenv）五种常用配置格式；JSON 与 `.env` 随
//! `serde` feature 直接可用，YAML/TOML/INI 分别需要额外启用 `serde-saphyr`/`toml`/`rust-ini`
//! feature。每种格式都提供无类型（[`ConfigValue`]）与有类型（`serde::Deserialize`）两条
//! 读取路径共享同一套文件大小上限与错误语义；JSON/TOML/YAML/INI 的无类型路径以及 YAML/INI
//! 的有类型路径使用本加载器的嵌套深度上限，JSON/TOML 有类型路径使用各自后端的递归保护。
//! YAML 别名回放还固定了有限预算：总回放事件最多 1,000,000 次、单个 anchor 最多展开 10,000
//! 次，回放栈深度不超过配置的嵌套深度上限。
//!
//! 本模块只负责“把一个配置文件安全地读成数据”：不做多文件合并、层叠覆盖、热重载、写回
//! 或 `include`/`import` 之类的指令；`.env` 语法之外的格式不提供插值或表达式能力。

mod de;
mod env;
mod error;
mod format;
mod json;
mod source;
mod value;

#[cfg(feature = "rust-ini")]
mod ini;
#[cfg(feature = "toml")]
mod toml;
#[cfg(feature = "serde-saphyr")]
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
    format_override: Option<ConfigFormat>,
    max_bytes: usize,
    max_depth: usize,
    env_substitution: bool,
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
    /// use axutils::ConfigLoader;
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
    /// use axutils::{ConfigFormat, ConfigLoader};
    ///
    /// let loader = ConfigLoader::new().with_format(ConfigFormat::Json);
    /// let _ = loader;
    /// ```
    #[must_use]
    pub fn with_format(mut self, format: ConfigFormat) -> Self {
        self.format_override = Some(format);
        self
    }

    /// 设置文件大小上限（字节），允许范围为 1 KiB 到 16 MiB（含边界）。
    ///
    /// # Errors
    ///
    /// 超出该范围时返回 [`ConfigError::InvalidLimit`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConfigLoader;
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

    /// 设置嵌套深度上限，允许范围为 1 到 256（含边界）。
    ///
    /// # Errors
    ///
    /// 超出该范围时返回 [`ConfigError::InvalidLimit`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConfigLoader;
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
    /// use axutils::{ConfigFormat, ConfigLoader};
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
    /// use std::io::Write;
    /// use axutils::ConfigLoader;
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-loader-doctest-{}.json", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let value = ConfigLoader::new().load_value(&path).unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load_value(&self, path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError> {
        let path = path.as_ref();
        let format = self.resolve_format(path)?;
        let text = source::read_bounded(path, self.max_bytes)?;
        self.parse_value(&text, format)
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
    /// use std::io::Write;
    /// use axutils::ConfigLoader;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-loader-doctest-load-{}.json", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let config: Config = ConfigLoader::new().load(&path).unwrap();
    /// assert_eq!(config.port, 8080);
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError> {
        let path = path.as_ref();
        let format = self.resolve_format(path)?;
        let text = source::read_bounded(path, self.max_bytes)?;
        self.parse(&text, format)
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
    /// use axutils::{ConfigFormat, ConfigLoader};
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
        match format {
            ConfigFormat::Json => json::parse_value(text, self.max_depth),
            ConfigFormat::Env => env::parse_value(text, self.env_substitution),
            #[cfg(feature = "serde-saphyr")]
            ConfigFormat::Yaml => yaml::parse_value(text, self.max_depth),
            #[cfg(feature = "toml")]
            ConfigFormat::Toml => toml::parse_value(text, self.max_depth),
            #[cfg(feature = "rust-ini")]
            ConfigFormat::Ini => ini::parse_value(text, self.max_depth),
        }
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
    /// use axutils::{ConfigFormat, ConfigLoader};
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
        match format {
            ConfigFormat::Json => json::parse(text),
            ConfigFormat::Env => {
                let value = env::parse_value(text, self.env_substitution)?;
                de::deserialize(&value)
            }
            #[cfg(feature = "serde-saphyr")]
            ConfigFormat::Yaml => yaml::parse(text, self.max_depth),
            #[cfg(feature = "toml")]
            ConfigFormat::Toml => toml::parse(text),
            #[cfg(feature = "rust-ini")]
            ConfigFormat::Ini => ini::parse(text, self.max_depth),
        }
    }

    fn resolve_format(&self, path: &Path) -> Result<ConfigFormat, ConfigError> {
        match self.format_override {
            Some(format) => Ok(format),
            None => ConfigFormat::from_path(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigLoader;
    use crate::{ConfigError, ConfigFormat};

    #[test]
    fn max_bytes_rejects_out_of_range_values() {
        assert!(matches!(
            ConfigLoader::new().with_max_bytes(0),
            Err(ConfigError::InvalidLimit)
        ));
        assert!(matches!(
            ConfigLoader::new().with_max_bytes(16 * 1024 * 1024 + 1),
            Err(ConfigError::InvalidLimit)
        ));
        assert!(ConfigLoader::new().with_max_bytes(1024).is_ok());
        assert!(ConfigLoader::new().with_max_bytes(16 * 1024 * 1024).is_ok());
    }

    #[test]
    fn max_depth_rejects_out_of_range_values() {
        assert!(matches!(
            ConfigLoader::new().with_max_depth(0),
            Err(ConfigError::InvalidLimit)
        ));
        assert!(matches!(
            ConfigLoader::new().with_max_depth(257),
            Err(ConfigError::InvalidLimit)
        ));
        assert!(ConfigLoader::new().with_max_depth(1).is_ok());
        assert!(ConfigLoader::new().with_max_depth(256).is_ok());
    }

    #[test]
    fn parse_value_dispatches_json_and_env() {
        let loader = ConfigLoader::new();
        let json = loader
            .parse_value(r#"{"a": 1}"#, ConfigFormat::Json)
            .expect("json should parse");
        assert_eq!(json.get("a").and_then(|v| v.as_i64()), Some(1));

        let env = loader
            .parse_value("A=1\n", ConfigFormat::Env)
            .expect("env should parse");
        assert_eq!(env.get("A").and_then(|v| v.as_str()), Some("1"));
    }

    #[test]
    fn parse_dispatches_typed_json() {
        #[derive(serde::Deserialize)]
        struct Config {
            a: i64,
        }
        let config: Config = ConfigLoader::new()
            .parse(r#"{"a": 5}"#, ConfigFormat::Json)
            .expect("typed json should parse");
        assert_eq!(config.a, 5);
    }

    #[test]
    fn with_format_overrides_extension_inference_for_load() {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-loader-test-{}-override.txt",
            std::process::id()
        ));
        std::fs::write(&path, r#"{"a": 42}"#).expect("write temp file");

        let value = ConfigLoader::new()
            .with_format(ConfigFormat::Json)
            .load_value(&path)
            .expect("override should force json parsing");
        assert_eq!(value.get("a").and_then(|v| v.as_i64()), Some(42));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_value_reports_unknown_extension_without_override() {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-loader-test-{}-noext",
            std::process::id()
        ));
        std::fs::write(&path, "{}").expect("write temp file");

        let result = ConfigLoader::new().load_value(&path);
        assert!(matches!(result, Err(ConfigError::UnknownExtension)));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_value_enforces_configured_max_bytes() {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-loader-test-{}-too-large.json",
            std::process::id()
        ));
        std::fs::write(&path, format!(r#"{{"a": "{}"}}"#, "x".repeat(2048)))
            .expect("write temp file");

        let result = ConfigLoader::new()
            .with_max_bytes(1024)
            .expect("valid limit")
            .load_value(&path);
        assert!(matches!(
            result,
            Err(ConfigError::FileTooLarge { limit: 1024, .. })
        ));

        let _ = std::fs::remove_file(&path);
    }
}
