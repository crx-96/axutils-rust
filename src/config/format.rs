//! 配置文件格式识别。

use std::path::Path;

use super::error::ConfigError;

/// 配置文件支持的格式。
///
/// 未启用对应后端 feature 时，该变体不存在于枚举中，也不会出现在公共 API 里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigFormat {
    /// JSON；随 `serde` feature 提供，不需要额外的第三方依赖。
    Json,
    /// `.env`（dotenv）；随 `serde` feature 提供，解析器为本 crate 自实现。
    Env,
    /// YAML；需要额外启用 `serde-saphyr` feature。
    #[cfg(feature = "serde-saphyr")]
    Yaml,
    /// TOML；需要额外启用 `toml` feature。
    #[cfg(feature = "toml")]
    Toml,
    /// INI（`.ini`/`.cfg`/`.conf`）；需要额外启用 `rust-ini` feature。
    #[cfg(feature = "rust-ini")]
    Ini,
}

impl ConfigFormat {
    /// 根据文件路径推断配置格式。
    ///
    /// 文件名为 `.env` 或以 `.env.` 开头（例如 `.env.local`）时识别为 [`ConfigFormat::Env`]，
    /// 优先于扩展名判断。其余情况按扩展名映射：`json` 对应 JSON；`env` 对应 `.env`；
    /// `yaml`/`yml` 对应 YAML；`toml` 对应 TOML；`ini`/`cfg`/`conf` 对应 INI。扩展名和
    /// `.env` 文件名识别均不区分大小写。
    ///
    /// # Errors
    ///
    /// 文件没有可识别的扩展名时返回 [`ConfigError::UnknownExtension`]；扩展名对应已知格式
    /// 但该格式的后端 feature 未启用时返回 [`ConfigError::FormatNotEnabled`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConfigFormat;
    ///
    /// assert_eq!(ConfigFormat::from_path("app.json").unwrap(), ConfigFormat::Json);
    /// assert_eq!(ConfigFormat::from_path(".env").unwrap(), ConfigFormat::Env);
    /// assert_eq!(ConfigFormat::from_path(".env.local").unwrap(), ConfigFormat::Env);
    /// assert!(ConfigFormat::from_path("app.unknownext").is_err());
    /// ```
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            let lower = name.to_ascii_lowercase();
            if lower == ".env" || lower.starts_with(".env.") {
                return Ok(Self::Env);
            }
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);

        match extension.as_deref() {
            Some("json") => Ok(Self::Json),
            Some("env") => Ok(Self::Env),
            #[cfg(feature = "serde-saphyr")]
            Some("yaml" | "yml") => Ok(Self::Yaml),
            #[cfg(not(feature = "serde-saphyr"))]
            Some(extension @ ("yaml" | "yml")) => Err(ConfigError::FormatNotEnabled {
                extension: extension.to_owned(),
            }),
            #[cfg(feature = "toml")]
            Some("toml") => Ok(Self::Toml),
            #[cfg(not(feature = "toml"))]
            Some(extension @ "toml") => Err(ConfigError::FormatNotEnabled {
                extension: extension.to_owned(),
            }),
            #[cfg(feature = "rust-ini")]
            Some("ini" | "cfg" | "conf") => Ok(Self::Ini),
            #[cfg(not(feature = "rust-ini"))]
            Some(extension @ ("ini" | "cfg" | "conf")) => Err(ConfigError::FormatNotEnabled {
                extension: extension.to_owned(),
            }),
            _ => Err(ConfigError::UnknownExtension),
        }
    }

    /// 返回格式的稳定小写名称，例如 `"json"`、`"yaml"`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConfigFormat;
    ///
    /// assert_eq!(ConfigFormat::Json.as_str(), "json");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Env => "env",
            #[cfg(feature = "serde-saphyr")]
            Self::Yaml => "yaml",
            #[cfg(feature = "toml")]
            Self::Toml => "toml",
            #[cfg(feature = "rust-ini")]
            Self::Ini => "ini",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigFormat;
    use crate::ConfigError;

    #[test]
    fn recognizes_json_and_env_case_insensitively() {
        assert_eq!(
            ConfigFormat::from_path("app.JSON").unwrap(),
            ConfigFormat::Json
        );
        assert_eq!(
            ConfigFormat::from_path("app.env").unwrap(),
            ConfigFormat::Env
        );
        assert_eq!(ConfigFormat::from_path(".ENV").unwrap(), ConfigFormat::Env);
        assert_eq!(
            ConfigFormat::from_path(".env.production").unwrap(),
            ConfigFormat::Env
        );
        assert_eq!(
            ConfigFormat::from_path(".Env.Local").unwrap(),
            ConfigFormat::Env
        );
    }

    #[test]
    fn rejects_unknown_extension() {
        assert!(matches!(
            ConfigFormat::from_path("app.unknownext"),
            Err(ConfigError::UnknownExtension)
        ));
        assert!(matches!(
            ConfigFormat::from_path("app"),
            Err(ConfigError::UnknownExtension)
        ));
    }

    #[cfg(feature = "serde-saphyr")]
    #[test]
    fn recognizes_yaml_when_backend_enabled() {
        assert_eq!(
            ConfigFormat::from_path("app.yaml").unwrap(),
            ConfigFormat::Yaml
        );
        assert_eq!(
            ConfigFormat::from_path("app.YML").unwrap(),
            ConfigFormat::Yaml
        );
        assert_eq!(ConfigFormat::Yaml.as_str(), "yaml");
    }

    #[cfg(not(feature = "serde-saphyr"))]
    #[test]
    fn reports_yaml_as_not_enabled() {
        assert!(matches!(
            ConfigFormat::from_path("app.yaml"),
            Err(ConfigError::FormatNotEnabled { extension }) if extension == "yaml"
        ));
    }

    #[cfg(feature = "toml")]
    #[test]
    fn recognizes_toml_when_backend_enabled() {
        assert_eq!(
            ConfigFormat::from_path("app.toml").unwrap(),
            ConfigFormat::Toml
        );
        assert_eq!(ConfigFormat::Toml.as_str(), "toml");
    }

    #[cfg(not(feature = "toml"))]
    #[test]
    fn reports_toml_as_not_enabled() {
        assert!(matches!(
            ConfigFormat::from_path("app.toml"),
            Err(ConfigError::FormatNotEnabled { extension }) if extension == "toml"
        ));
    }

    #[cfg(feature = "rust-ini")]
    #[test]
    fn recognizes_ini_variants_when_backend_enabled() {
        for name in ["app.ini", "app.CFG", "app.conf"] {
            assert_eq!(ConfigFormat::from_path(name).unwrap(), ConfigFormat::Ini);
        }
        assert_eq!(ConfigFormat::Ini.as_str(), "ini");
    }

    #[cfg(not(feature = "rust-ini"))]
    #[test]
    fn reports_ini_as_not_enabled() {
        for name in ["app.ini", "app.cfg", "app.conf"] {
            assert!(matches!(
                ConfigFormat::from_path(name),
                Err(ConfigError::FormatNotEnabled { .. })
            ));
        }
    }
}
