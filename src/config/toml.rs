//! `toml` 后端：TOML 文本到 [`ConfigValue`] 或调用方类型的转换。

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use toml::de::{DeTable, DeValue, Error as TomlError};

use super::{error as config_error, ConfigError, ConfigValue};

/// 先完成 TOML 语法校验，再按真实节点类型转换无类型值并施加容器深度预算。
///
/// `DeTable` 保留日期与普通表的区别，避免 serde 的日期伪表标记与用户键名冲突。
pub(super) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    // 沿用后端的完整文档解析，使语法错误、重复键及后端递归保护先于值转换生效。
    let table = DeTable::parse(text).map_err(|error| map_parse_error(text, &error))?;
    convert_table(table.into_inner(), text, max_depth, max_depth)
}

/// 转换一层实际 TOML 表；键名仅用于子值的脱敏错误分类，输出使用稳定的有序表。
fn convert_table(
    table: DeTable<'_>,
    text: &str,
    remaining_depth: usize,
    limit: usize,
) -> Result<ConfigValue, ConfigError> {
    // 根表和嵌套表各消耗一层预算；日期标量不经过本函数。
    let child_depth = remaining_depth
        .checked_sub(1)
        .ok_or(ConfigError::DepthLimitExceeded { limit })?;
    let mut output = BTreeMap::new();
    for (key, value) in table {
        let key = key.into_inner().into_owned();
        let offset = value.span().start;
        let value = convert_value(value.into_inner(), text, offset, &key, child_depth, limit)?;
        output.insert(key, value);
    }
    Ok(ConfigValue::Table(output))
}

/// 转换真实 TOML 节点；`key` 为最近的表字段名，`offset` 用于保留后端数值错误的位置。
fn convert_value(
    value: DeValue<'_>,
    text: &str,
    offset: usize,
    key: &str,
    remaining_depth: usize,
    limit: usize,
) -> Result<ConfigValue, ConfigError> {
    match value {
        // 文本、布尔和日期均为标量；普通用户表即使使用日期内部标记键，也只进入 Table 分支。
        DeValue::String(value) => Ok(ConfigValue::String(value.into_owned())),
        DeValue::Boolean(value) => Ok(ConfigValue::Bool(value)),
        DeValue::Datetime(value) => Ok(ConfigValue::String(value.to_string())),
        DeValue::Integer(value) => {
            // 后端已去除进制前缀与分隔下划线；as_str 保留十进制正负号，radix 指明原进制。
            let raw = value.as_str();
            let radix = value.radix();
            if let Ok(value) = i64::from_str_radix(raw, radix) {
                return Ok(ConfigValue::Integer(value));
            }
            // 保留旧 serde visitor 的分类：后端能表示但 ConfigValue 无法表示时属于范围错误。
            if i128::from_str_radix(raw, radix).is_ok() || u128::from_str_radix(raw, radix).is_ok()
            {
                return Err(ConfigError::ValueOutOfRange {
                    key: key.to_owned(),
                });
            }
            Err(parse_error_at(text, offset))
        }
        DeValue::Float(value) => {
            // 仅 TOML 的显式 inf 字面量可以成为无穷大；指数溢出仍按后端语义返回解析错误。
            let raw = value.as_str();
            match raw.parse::<f64>() {
                Ok(value) if !value.is_infinite() || raw.contains("inf") => {
                    Ok(ConfigValue::Float(value))
                }
                _ => Err(parse_error_at(text, offset)),
            }
        }
        DeValue::Array(values) => {
            // 数组自身消耗一层预算，元素沿用最近的表字段名，避免把配置值加入错误上下文。
            let child_depth = remaining_depth
                .checked_sub(1)
                .ok_or(ConfigError::DepthLimitExceeded { limit })?;
            values
                .into_iter()
                .map(|value| {
                    let offset = value.span().start;
                    convert_value(value.into_inner(), text, offset, key, child_depth, limit)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(ConfigValue::Array)
        }
        DeValue::Table(table) => convert_table(table, text, remaining_depth, limit),
    }
}

/// 使用 TOML 原生反序列化保留调用方类型和后端自身的递归保护。
pub(super) fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ConfigError> {
    toml::from_str(text).map_err(|error| map_parse_error(text, &error))
}

/// 将原始解析错误收敛为格式和位置，不保留可能含配置值的上游消息。
fn map_parse_error(text: &str, error: &TomlError) -> ConfigError {
    let (line, column) = match error.span() {
        Some(span) => {
            let (line, column) = config_error::line_column_at(text, span.start);
            (Some(line), Some(column))
        }
        None => (None, None),
    };
    ConfigError::Parse {
        format: "toml",
        line,
        column,
    }
}

/// 用节点的起始字节位置构造数值转换错误，与后端的 span 定位保持一致。
fn parse_error_at(text: &str, byte_offset: usize) -> ConfigError {
    let (line, column) = config_error::line_column_at(text, byte_offset);
    ConfigError::Parse {
        format: "toml",
        line: Some(line),
        column: Some(column),
    }
}

#[cfg(test)]
mod tests {
    use super as toml_config;
    use crate::config::ConfigError;
    use serde::Deserialize;

    #[test]
    fn parses_typed_and_untyped_toml() {
        let text = "[server]\nport = 8080\ntls = true\n";
        let value = toml_config::parse_value(text, 64).expect("parse untyped");
        assert_eq!(
            value.get("server.port").and_then(|v| v.as_i64()),
            Some(8080)
        );

        #[derive(Deserialize)]
        struct Server {
            port: u16,
            tls: bool,
        }
        #[derive(Deserialize)]
        struct Config {
            server: Server,
        }
        let typed: Config = toml_config::parse(text).expect("parse typed");
        assert_eq!(typed.server.port, 8080);
        assert!(typed.server.tls);
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_snippet() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("password = \"{secret}\"\ninvalid ===\n");
        let error = toml_config::parse_value(&text, 64).expect_err("invalid toml should fail");
        assert!(matches!(error, ConfigError::Parse { format: "toml", .. }));
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn empty_document_parses_to_empty_table() {
        let value = toml_config::parse_value("", 64).expect("parse empty document");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn comment_only_document_parses_to_empty_table() {
        let text = "# just a comment\n# another comment\n";
        let value = toml_config::parse_value(text, 64).expect("parse comment-only document");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn depth_exactly_at_limit_succeeds_and_one_more_level_fails() {
        let shallow = "a = 1\n";
        assert!(toml_config::parse_value(shallow, 1).is_ok());

        let nested = "[a]\nb = 1\n";
        let error =
            toml_config::parse_value(nested, 1).expect_err("depth 2 should exceed budget 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(toml_config::parse_value(nested, 2).is_ok());
    }

    #[test]
    fn integer_exceeding_i64_but_within_i128_is_rejected() {
        let text = "count = 99999999999999999999\n";
        let error = toml_config::parse_value(text, 64).expect_err("overflow should fail");
        assert!(matches!(
            error,
            ConfigError::ValueOutOfRange { key } if key == "count"
        ));
    }

    #[test]
    fn date_time_values_are_preserved_as_strings() {
        let text = "created = 2024-02-29T01:02:03Z\n";
        let value = toml_config::parse_value(text, 64).expect("parse date-time");
        assert_eq!(
            value.get("created").and_then(|v| v.as_str()),
            Some("2024-02-29T01:02:03Z")
        );
    }

    #[test]
    fn duplicate_keys_are_rejected_by_the_toml_syntax_itself() {
        let text = "a = 1\na = 2\n";
        let error = toml_config::parse_value(text, 64).expect_err("duplicate key should fail");
        assert!(matches!(error, ConfigError::Parse { format: "toml", .. }));
    }

    #[test]
    fn datetime_marker_does_not_discard_other_fields_in_the_same_table() {
        let text =
            "[metadata]\n\"!first\" = \"value\"\n\"$__toml_private_datetime\" = \"literal\"\n";
        let value = toml_config::parse_value(text, 64).expect("table should retain all fields");
        let metadata = value
            .get("metadata")
            .and_then(|value| value.as_table())
            .expect("metadata should remain a table");
        assert_eq!(
            metadata.get("!first").and_then(|value| value.as_str()),
            Some("value")
        );
        assert_eq!(
            metadata
                .get("$__toml_private_datetime")
                .and_then(|value| value.as_str()),
            Some("literal")
        );
    }

    #[test]
    fn datetime_marker_only_user_field_remains_a_table() {
        let text = "\"$__toml_private_datetime\" = \"literal\"\n";
        let value = toml_config::parse_value(text, 64).expect("table should remain a table");
        let table = value.as_table().expect("root should remain a table");
        assert_eq!(
            table
                .get("$__toml_private_datetime")
                .and_then(|value| value.as_str()),
            Some("literal")
        );
    }
}
