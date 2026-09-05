//! 已读取配置文本的格式分发与脱敏 tracing。

#[cfg(feature = "tracing")]
use std::time::Instant;

use serde::de::DeserializeOwned;

#[cfg(feature = "tracing")]
use crate::telemetry::config as config_trace;

#[cfg(feature = "config-ini")]
use super::ini;
#[cfg(feature = "config-toml")]
use super::toml;
#[cfg(feature = "config-yaml")]
use super::yaml;
use super::{de, env, json, ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

pub(super) fn parse_value(
    loader: &ConfigLoader,
    text: &str,
    format: ConfigFormat,
) -> Result<ConfigValue, ConfigError> {
    #[cfg(feature = "tracing")]
    let started = Instant::now();
    let result = match format {
        ConfigFormat::Json => json::parse_value(text, loader.max_depth),
        ConfigFormat::Env => env::parse_value(text, loader.env_substitution, loader.max_bytes),
        #[cfg(feature = "config-yaml")]
        ConfigFormat::Yaml => yaml::parse_value(text, loader.max_depth),
        #[cfg(feature = "config-toml")]
        ConfigFormat::Toml => toml::parse_value(text, loader.max_depth),
        #[cfg(feature = "config-ini")]
        ConfigFormat::Ini => ini::parse_value(text, loader.max_depth),
    };
    #[cfg(feature = "tracing")]
    config_trace::record_parse(format, text.len(), &result, started);
    result
}

pub(super) fn parse<T: DeserializeOwned>(
    loader: &ConfigLoader,
    text: &str,
    format: ConfigFormat,
) -> Result<T, ConfigError> {
    #[cfg(feature = "tracing")]
    let started = Instant::now();
    let result = match format {
        ConfigFormat::Json => json::parse(text),
        ConfigFormat::Env => env::parse_value(text, loader.env_substitution, loader.max_bytes)
            .and_then(|value| de::deserialize(&value)),
        #[cfg(feature = "config-yaml")]
        ConfigFormat::Yaml => yaml::parse(text, loader.max_depth),
        #[cfg(feature = "config-toml")]
        ConfigFormat::Toml => toml::parse(text),
        #[cfg(feature = "config-ini")]
        ConfigFormat::Ini => ini::parse(text, loader.max_depth),
    };
    #[cfg(feature = "tracing")]
    config_trace::record_parse(format, text.len(), &result, started);
    result
}
