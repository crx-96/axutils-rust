//! 配置文件读取的静态便捷入口。
//!
//! 同时启用 `serde` 与 `tokio` feature 后，文件读取入口还提供异步版本；异步版本要求调用方
//! 已经运行在 Tokio runtime 中，不创建 runtime 或调用 `block_on`。

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

    /// 按扩展名推断格式，异步读取配置文件为无类型的 [`ConfigValue`]。
    ///
    /// 该方法仅在同时启用 `serde` 与 `tokio` feature 时提供，调用方必须在已有 Tokio runtime
    /// 中等待它。文件大小、UTF-8/BOM、格式、深度、`.env` 回退和错误语义沿用默认
    /// [`ConfigLoader::load_value_async`]；crate 不创建 runtime 或调用 `block_on`，解析阶段仍在
    /// 当前异步任务中同步执行。每个并发调用独立占用最多约文件大小上限加 1 字节的读取缓冲区，
    /// crate 不新增全局并发或内存配额；调用方需自行限制路径来源、任务数和总内存。配置文件
    /// 可能包含凭据，不要直接记录整个配置值。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_value_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::ConfigUtils;
    ///
    /// async fn example() -> Result<(), axutils::ConfigError> {
    ///     let value = ConfigUtils::load_value_async("app.json").await?;
    ///     let _ = value;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(feature = "serde", feature = "tokio"))]
    pub async fn load_value_async(path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError> {
        ConfigLoader::new().load_value_async(path).await
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

    /// 按扩展名推断格式，异步读取配置文件并反序列化为调用方类型 `T`。
    ///
    /// 该方法仅在同时启用 `serde` 与 `tokio` feature 时提供，调用方必须在已有 Tokio runtime
    /// 中等待它。文件大小、UTF-8/BOM、格式、深度、`.env` 回退和错误语义沿用默认
    /// [`ConfigLoader::load_async`]；crate 不创建 runtime 或调用 `block_on`，解析阶段仍在当前
    /// 异步任务中同步执行。每个并发调用独立占用最多约文件大小上限加 1 字节的读取缓冲区，crate
    /// 不新增全局并发或内存配额；调用方需自行限制路径来源、任务数和总内存。配置文件可能包含
    /// 凭据，不要直接记录反序列化结果。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::ConfigUtils;
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// async fn example() -> Result<(), axutils::ConfigError> {
    ///     let config: AppConfig = ConfigUtils::load_async("app.json").await?;
    ///     let _ = config;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(feature = "serde", feature = "tokio"))]
    pub async fn load_async<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError> {
        ConfigLoader::new().load_async(path).await
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

    /// 使用显式指定的格式异步读取配置文件为无类型的 [`ConfigValue`]。
    ///
    /// 该方法仅在同时启用 `serde` 与 `tokio` feature 时提供，显式格式会覆盖扩展名推断；调用方
    /// 必须在已有 Tokio runtime 中等待它。文件大小、UTF-8/BOM、深度、`.env` 回退和错误语义沿用
    /// [`ConfigLoader::load_value_async`]；crate 不创建 runtime 或调用 `block_on`，解析阶段仍在
    /// 当前异步任务中同步执行。每个并发调用独立占用最多约文件大小上限加 1 字节的读取缓冲区，
    /// crate 不新增全局并发或内存配额；调用方需自行限制路径来源、任务数和总内存。配置文件
    /// 可能包含凭据，不要直接记录整个配置值。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_value_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{ConfigFormat, ConfigUtils};
    ///
    /// async fn example() -> Result<(), axutils::ConfigError> {
    ///     let value = ConfigUtils::load_value_as_async("app.conf", ConfigFormat::Json).await?;
    ///     let _ = value;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(feature = "serde", feature = "tokio"))]
    pub async fn load_value_as_async(
        path: impl AsRef<Path>,
        format: ConfigFormat,
    ) -> Result<ConfigValue, ConfigError> {
        ConfigLoader::new()
            .with_format(format)
            .load_value_async(path)
            .await
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

    /// 使用显式指定的格式异步读取配置文件，并反序列化为调用方类型 `T`。
    ///
    /// 该方法仅在同时启用 `serde` 与 `tokio` feature 时提供，显式格式会覆盖扩展名推断；调用方
    /// 必须在已有 Tokio runtime 中等待它。文件大小、UTF-8/BOM、深度、`.env` 回退和错误语义沿用
    /// [`ConfigLoader::load_async`]；crate 不创建 runtime 或调用 `block_on`，解析阶段仍在当前
    /// 异步任务中同步执行。每个并发调用独立占用最多约文件大小上限加 1 字节的读取缓冲区，crate
    /// 不新增全局并发或内存配额；调用方需自行限制路径来源、任务数和总内存。配置文件可能包含
    /// 凭据，不要直接记录反序列化结果。
    ///
    /// # Errors
    ///
    /// 见 [`ConfigLoader::load_async`]；有类型反序列化错误按对应格式同步后端的既有映射返回，
    /// 例如 [`ConfigError::Parse`] 或 [`ConfigError::TypeMismatch`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{ConfigFormat, ConfigUtils};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// async fn example() -> Result<(), axutils::ConfigError> {
    ///     let config: AppConfig =
    ///         ConfigUtils::load_as_async("app.conf", ConfigFormat::Json).await?;
    ///     let _ = config;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(all(feature = "serde", feature = "tokio"))]
    pub async fn load_as_async<T: DeserializeOwned>(
        path: impl AsRef<Path>,
        format: ConfigFormat,
    ) -> Result<T, ConfigError> {
        ConfigLoader::new()
            .with_format(format)
            .load_async(path)
            .await
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
