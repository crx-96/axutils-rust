//! 配置文件读取的静态便捷入口。

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

/// 配置文件读取的无状态静态入口，等价于使用默认 [`ConfigLoader`]。
///
/// 不引入全局单例、缓存或可变全局状态；需要自定义大小/深度上限或关闭 `.env` 环境回退时，
/// 请使用 [`ConfigUtils::loader`] 获取一个可配置的 [`ConfigLoader`]。
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfigUtils;

impl ConfigUtils {
    /// 按扩展名推断格式，从磁盘读取配置文件为无类型的 [`ConfigValue`]。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_value`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Write;
    /// use axutils::ConfigUtils;
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-utils-doctest-{}.json", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let value = ConfigUtils::load_value(&path).unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load_value(path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError> {
        ConfigLoader::new().load_value(path)
    }

    /// 按扩展名推断格式，从磁盘读取配置文件并反序列化为调用方类型 `T`。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Write;
    /// use axutils::ConfigUtils;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-utils-doctest-load-{}.json", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let config: Config = ConfigUtils::load(&path).unwrap();
    /// assert_eq!(config.port, 8080);
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError> {
        ConfigLoader::new().load(path)
    }

    /// 使用显式指定的格式（忽略扩展名推断），从磁盘读取配置文件为无类型的 [`ConfigValue`]。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_value`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Write;
    /// use axutils::{ConfigFormat, ConfigUtils};
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-utils-doctest-explicit-{}.txt", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let value = ConfigUtils::load_value_as(&path, ConfigFormat::Json).unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load_value_as(
        path: impl AsRef<Path>,
        format: ConfigFormat,
    ) -> Result<ConfigValue, ConfigError> {
        ConfigLoader::new().with_format(format).load_value(path)
    }

    /// 使用显式指定的格式（忽略扩展名推断），从磁盘读取配置文件并反序列化为调用方类型 `T`。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::io::Write;
    /// use axutils::{ConfigFormat, ConfigUtils};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let mut path = std::env::temp_dir();
    /// path.push(format!("axutils-config-utils-doctest-explicit-typed-{}.txt", std::process::id()));
    /// std::fs::File::create(&path)
    ///     .unwrap()
    ///     .write_all(br#"{"port": 8080}"#)
    ///     .unwrap();
    ///
    /// let config: Config = ConfigUtils::load_as(&path, ConfigFormat::Json).unwrap();
    /// assert_eq!(config.port, 8080);
    /// std::fs::remove_file(&path).ok();
    /// ```
    pub fn load_as<T: DeserializeOwned>(
        path: impl AsRef<Path>,
        format: ConfigFormat,
    ) -> Result<T, ConfigError> {
        ConfigLoader::new().with_format(format).load(path)
    }

    /// 把内存中的文本按指定格式解析为无类型的 [`ConfigValue`]。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::parse_value`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConfigFormat, ConfigUtils};
    ///
    /// let value = ConfigUtils::parse_value(r#"{"port": 8080}"#, ConfigFormat::Json).unwrap();
    /// assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
    /// ```
    pub fn parse_value(text: &str, format: ConfigFormat) -> Result<ConfigValue, ConfigError> {
        ConfigLoader::new().parse_value(text, format)
    }

    /// 把内存中的文本按指定格式直接反序列化为调用方类型 `T`。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::parse`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConfigFormat, ConfigUtils};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config {
    ///     port: u16,
    /// }
    ///
    /// let config: Config = ConfigUtils::parse(r#"{"port": 8080}"#, ConfigFormat::Json).unwrap();
    /// assert_eq!(config.port, 8080);
    /// ```
    pub fn parse<T: DeserializeOwned>(text: &str, format: ConfigFormat) -> Result<T, ConfigError> {
        ConfigLoader::new().parse(text, format)
    }

    /// 获取一个默认配置的 [`ConfigLoader`]，用于自定义大小/深度上限或关闭 `.env` 环境回退。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConfigUtils;
    ///
    /// let loader = ConfigUtils::loader().with_max_depth(8).unwrap();
    /// let _ = loader;
    /// ```
    pub fn loader() -> ConfigLoader {
        ConfigLoader::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigUtils;
    use crate::ConfigFormat;

    #[test]
    fn parse_value_and_parse_delegate_to_default_loader() {
        let value = ConfigUtils::parse_value(r#"{"a": 1}"#, ConfigFormat::Json)
            .expect("parse_value should succeed");
        assert_eq!(value.get("a").and_then(|v| v.as_i64()), Some(1));

        #[derive(serde::Deserialize)]
        struct Config {
            a: i64,
        }
        let config: Config =
            ConfigUtils::parse(r#"{"a": 2}"#, ConfigFormat::Json).expect("parse should succeed");
        assert_eq!(config.a, 2);
    }

    #[test]
    fn load_value_as_and_load_as_use_explicit_format() {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-utils-test-{}-explicit",
            std::process::id()
        ));
        std::fs::write(&path, r#"{"a": 7}"#).expect("write temp file");

        let value = ConfigUtils::load_value_as(&path, ConfigFormat::Json)
            .expect("load_value_as should succeed");
        assert_eq!(value.get("a").and_then(|v| v.as_i64()), Some(7));

        #[derive(serde::Deserialize)]
        struct Config {
            a: i64,
        }
        let config: Config =
            ConfigUtils::load_as(&path, ConfigFormat::Json).expect("load_as should succeed");
        assert_eq!(config.a, 7);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn loader_returns_an_independently_configurable_loader() {
        let loader = ConfigUtils::loader()
            .with_max_depth(2)
            .expect("valid depth");
        let error = loader
            .parse_value(r#"{"a": {"b": {"c": 1}}}"#, ConfigFormat::Json)
            .expect_err("depth 3 should exceed budget 2");
        assert!(matches!(
            error,
            crate::ConfigError::DepthLimitExceeded { limit: 2 }
        ));
    }
}
