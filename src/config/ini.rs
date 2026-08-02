//! `rust-ini` 后端：INI 文本到 [`ConfigValue`]，以及经 [`super::de`] 的类型化读取。
//!
//! section 映射为嵌套表，没有 section 的键（rust-ini 的“通用 section”）直接放在顶层；
//! 重复键（同一 section 内）由本 crate 在构建 [`ConfigValue`] 时检测并拒绝，rust-ini 自身
//! 的底层存储保留了重复插入的全部条目，因此可以准确探测。解析选项显式列出，不依赖上游
//! 默认值的隐式变化。

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;

use super::{de, error::ConfigError, value::ConfigValue};

fn parse_options() -> ::ini::ParseOption {
    ::ini::ParseOption {
        enabled_quote: true,
        enabled_escape: true,
        enabled_indented_mutiline_value: false,
        enabled_preserve_key_leading_whitespace: false,
    }
}

pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    let document = ::ini::Ini::load_from_str_opt(text, parse_options()).map_err(map_parse_error)?;

    let mut root: BTreeMap<String, ConfigValue> = BTreeMap::new();
    for (section_name, properties) in document.iter() {
        if section_name.is_some() && max_depth < 2 {
            return Err(ConfigError::DepthLimitExceeded { limit: max_depth });
        }
        let mut table: BTreeMap<String, ConfigValue> = BTreeMap::new();
        for (key, value) in properties.iter() {
            if table.contains_key(key) {
                return Err(ConfigError::DuplicateKey {
                    key: key.to_owned(),
                });
            }
            table.insert(key.to_owned(), ConfigValue::String(value.to_owned()));
        }

        match section_name {
            None => {
                for (key, value) in table {
                    if root.contains_key(&key) {
                        return Err(ConfigError::DuplicateKey { key });
                    }
                    root.insert(key, value);
                }
            }
            Some(name) => {
                if root.contains_key(name) {
                    return Err(ConfigError::DuplicateKey {
                        key: name.to_owned(),
                    });
                }
                root.insert(name.to_owned(), ConfigValue::Table(table));
            }
        }
    }

    Ok(ConfigValue::Table(root))
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str, max_depth: usize) -> Result<T, ConfigError> {
    let value = parse_value(text, max_depth)?;
    de::deserialize(&value)
}

fn map_parse_error(error: ::ini::ParseError) -> ConfigError {
    ConfigError::Parse {
        format: "ini",
        line: Some(error.line),
        column: Some(error.col),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_value};
    use crate::ConfigError;
    use serde::Deserialize;

    #[test]
    fn parses_sections_and_top_level_keys() {
        let text = "top = 1\n[server]\nport = 8080\nhost = example.com\n";
        let value = parse_value(text, 64).expect("parse ini");
        assert_eq!(value.get("top").and_then(|v| v.as_str()), Some("1"));
        assert_eq!(
            value.get("server.port").and_then(|v| v.as_str()),
            Some("8080")
        );
    }

    #[test]
    fn typed_read_leniently_parses_numeric_and_bool_strings() {
        #[derive(Deserialize)]
        struct Server {
            port: u16,
            tls: bool,
        }
        #[derive(Deserialize)]
        struct Config {
            server: Server,
        }
        let text = "[server]\nport = 8080\ntls = true\n";
        let config: Config = parse(text, 64).expect("typed parse");
        assert_eq!(config.server.port, 8080);
        assert!(config.server.tls);
    }

    #[test]
    fn rejects_duplicate_keys_within_a_section() {
        let text = "[server]\nport = 1\nport = 2\n";
        let error = parse_value(text, 64).expect_err("duplicate key should fail");
        assert!(matches!(
            error,
            ConfigError::DuplicateKey { key } if key == "port"
        ));
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_value_leak() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("[server\npassword = {secret}\n");
        let error = parse_value(&text, 64).expect_err("invalid ini should fail");
        assert!(matches!(
            error,
            ConfigError::Parse {
                format: "ini",
                line: Some(_),
                column: Some(_)
            }
        ));
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn empty_document_parses_to_empty_table() {
        let value = parse_value("", 64).expect("parse empty document");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn comment_only_document_parses_to_empty_table() {
        let text = "; just a comment\n# another comment\n";
        let value = parse_value(text, 64).expect("parse comment-only document");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn enforces_depth_limit_for_section_tables() {
        let text = "[server]\nport = 8080\n";
        let error = parse_value(text, 1).expect_err("section table should exceed depth 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(parse_value(text, 2).is_ok());
    }
}
