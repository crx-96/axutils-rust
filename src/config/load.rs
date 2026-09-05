//! 同步与异步文件读取后对 `ConfigLoader` 解析入口的组合。

use std::path::Path;

use serde::de::DeserializeOwned;

use super::{parse, source, ConfigError, ConfigLoader, ConfigValue};

pub(super) fn load_value(loader: &ConfigLoader, path: &Path) -> Result<ConfigValue, ConfigError> {
    let format = loader.resolve_format(path)?;
    let text = source::read_bounded(path, loader.max_bytes)?;
    parse::parse_value(loader, &text, format)
}

pub(super) fn load<T: DeserializeOwned>(
    loader: &ConfigLoader,
    path: &Path,
) -> Result<T, ConfigError> {
    let format = loader.resolve_format(path)?;
    let text = source::read_bounded(path, loader.max_bytes)?;
    parse::parse(loader, &text, format)
}

#[cfg(feature = "config-async")]
pub(super) async fn load_value_async(
    loader: &ConfigLoader,
    path: &Path,
) -> Result<ConfigValue, ConfigError> {
    let (format, text) = read_file_async(loader, path).await?;
    parse::parse_value(loader, &text, format)
}

#[cfg(feature = "config-async")]
pub(super) async fn load_async<T: DeserializeOwned>(
    loader: &ConfigLoader,
    path: &Path,
) -> Result<T, ConfigError> {
    let (format, text) = read_file_async(loader, path).await?;
    parse::parse(loader, &text, format)
}

#[cfg(feature = "config-async")]
async fn read_file_async(
    loader: &ConfigLoader,
    path: &Path,
) -> Result<(super::ConfigFormat, String), ConfigError> {
    let format = loader.resolve_format(path)?;
    let text = source::read_bounded_async(path, loader.max_bytes).await?;
    Ok((format, text))
}
