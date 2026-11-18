//! `serde_json` 后端：JSON 文本到 [`ConfigValue`] 或调用方类型的转换。

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::de::{
    self, Deserialize, DeserializeOwned, DeserializeSeed, Deserializer, IgnoredAny, MapAccess,
    SeqAccess, Visitor,
};
use serde_json::{Deserializer as JsonDeserializer, Error as JsonError};

use super::{
    error::ConfigError,
    value::{self as config_value, ErrorMarker},
    ConfigValue,
};

/// 读取一个完整的 JSON 值，只对实际出现的数组和对象应用加载器深度预算。
pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    let mut cursor = JsonCursor {
        text,
        rest: text,
        max_depth,
    };
    let value = cursor.value(0, "")?;
    // 根节点之后仅允许 JSON 空白。
    if cursor.peek().is_some() {
        return Err(cursor.syntax_error());
    }
    Ok(value)
}

/// 顺序消费容器分隔符，用 serde_json 解码单个标量和键，以原始 token 区分数字与对象。
struct JsonCursor<'a> {
    /// 完整输入，仅在诊断时用于还原行列。
    text: &'a str,
    /// 尚未消费的输入后缀，始终位于 UTF-8 字符边界。
    rest: &'a str,
    /// 包括根容器在内的最大嵌套层数。
    max_depth: usize,
}

impl<'a> JsonCursor<'a> {
    /// 跳过 JSON 允许的四种空白，返回下一个 token 的起始字节。
    fn peek(&mut self) -> Option<u8> {
        self.rest = self.rest.trim_start_matches([' ', '\t', '\r', '\n']);
        self.rest.as_bytes().first().copied()
    }

    /// 在下一 token 匹配指定 ASCII 分隔符时消费它。
    fn take(&mut self, delimiter: u8) -> bool {
        if self.peek() == Some(delimiter) {
            self.rest = &self.rest[1..];
            return true;
        }
        false
    }

    /// 将当前 token 内的相对位置换算为原始文档的字节列与行号，仅公开位置。
    fn error_at(&self, line: usize, column: usize) -> ConfigError {
        let prefix = &self.text[..self.text.len() - self.rest.len()];
        let column = if line == 1 {
            column + prefix.rsplit('\n').next().unwrap_or_default().len()
        } else {
            column
        };
        ConfigError::Parse {
            format: "json",
            line: Some(prefix.bytes().filter(|&b| b == b'\n').count() + line),
            column: (column != 0).then_some(column),
        }
    }

    /// 标记不符合预期的分隔符或提前结束的输入。
    fn syntax_error(&self) -> ConfigError {
        self.error_at(1, usize::from(!self.rest.is_empty()))
    }

    /// 由后端校验并解码一个标量，按实际消费的字节数推进，保留后续兄弟节点。
    fn scalar<T: Deserialize<'a>>(&mut self) -> Result<T, ConfigError> {
        let mut tokens = JsonDeserializer::from_str(self.rest).into_iter::<T>();
        let value = tokens
            .next()
            .ok_or_else(|| self.syntax_error())?
            .map_err(|error| self.error_at(error.line(), error.column()))?;
        self.rest = &self.rest[tokens.byte_offset()..];
        Ok(value)
    }

    /// 在创建容器前检查预算，数组元素沿用最近的字段名以诊断整数越界。
    fn value(&mut self, depth: usize, key: &str) -> Result<ConfigValue, ConfigError> {
        match self.peek() {
            Some(b'[' | b'{') if depth == self.max_depth => Err(ConfigError::DepthLimitExceeded {
                limit: self.max_depth,
            }),
            Some(b'[') => {
                self.rest = &self.rest[1..];
                let mut items = Vec::new();
                if !self.take(b']') {
                    loop {
                        items.push(self.value(depth + 1, key)?);
                        if self.take(b']') {
                            break;
                        }
                        if !self.take(b',') {
                            return Err(self.syntax_error());
                        }
                    }
                }
                Ok(ConfigValue::Array(items))
            }
            Some(b'{') => {
                self.rest = &self.rest[1..];
                let mut table = BTreeMap::new();
                if !self.take(b'}') {
                    loop {
                        // 解码转义后比较键，在读取重复字段的值之前返回诊断。
                        if self.peek() != Some(b'"') {
                            return Err(self.syntax_error());
                        }
                        let field: String = self.scalar()?;
                        if table.contains_key(&field) {
                            return Err(ConfigError::DuplicateKey { key: field });
                        }
                        if !self.take(b':') {
                            return Err(self.syntax_error());
                        }
                        let value = self.value(depth + 1, &field)?;
                        table.insert(field, value);
                        if self.take(b'}') {
                            break;
                        }
                        if !self.take(b',') {
                            return Err(self.syntax_error());
                        }
                    }
                }
                Ok(ConfigValue::Table(table))
            }
            Some(b'"') => self.scalar().map(ConfigValue::String),
            Some(b't' | b'f') => self.scalar().map(ConfigValue::Bool),
            Some(b'n') => self.scalar::<()>().map(|()| ConfigValue::Null),
            Some(b'-' | b'0'..=b'9') => self.number(key),
            _ => Err(self.syntax_error()),
        }
    }

    /// 数字先由后端校验语法，再按字面量区分整数和浮点，保留 JSON 负零的符号。
    fn number(&mut self, key: &str) -> Result<ConfigValue, ConfigError> {
        // 起始字节已经确认为数字。让后端校验并跳过该 token，直接借用其消费的字节。
        let start = self.rest;
        self.scalar::<IgnoredAny>()?;
        let token = &start[..start.len() - self.rest.len()];
        if token == "-0" || token.contains(['.', 'e', 'E']) {
            // 从数字起点读取 f64，使指数溢出的诊断保留后端报告的字节列。
            self.rest = start;
            self.scalar().map(ConfigValue::Float)
        } else {
            token.parse::<i64>().map(ConfigValue::Integer).map_err(|_| {
                ConfigError::ValueOutOfRange {
                    key: key.to_owned(),
                }
            })
        }
    }
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ConfigError> {
    // Typed JSON intentionally uses two bounded passes: the first preserves the crate's
    // duplicate-key error contract, and the second lets serde_json deserialize into `T`.
    // File callers are bounded by the loader's max-bytes check; in-memory callers still own
    // the size of `text`.
    reject_duplicate_keys(text)?;
    serde_json::from_str(text).map_err(|error| map_parse_error(&error))
}

fn map_value_error(error: &JsonError, max_depth: usize) -> ConfigError {
    match config_value::classify_marker(&error.to_string()) {
        ErrorMarker::DepthLimitExceeded => ConfigError::DepthLimitExceeded { limit: max_depth },
        ErrorMarker::DuplicateKey(key) => ConfigError::DuplicateKey {
            key: key.to_owned(),
        },
        ErrorMarker::ValueOutOfRange(key) => ConfigError::ValueOutOfRange {
            key: key.to_owned(),
        },
        ErrorMarker::None => map_parse_error(error),
    }
}

fn reject_duplicate_keys(text: &str) -> Result<(), ConfigError> {
    let mut deserializer = JsonDeserializer::from_str(text);
    DuplicateKeySeed
        .deserialize(&mut deserializer)
        .map_err(|error| map_value_error(&error, 0))?;
    deserializer.end().map_err(|error| map_parse_error(&error))
}

struct DuplicateKeySeed;

impl<'de> DeserializeSeed<'de> for DuplicateKeySeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateKeyVisitor)
    }
}

struct DuplicateKeyVisitor;

impl<'de> Visitor<'de> for DuplicateKeyVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateKeySeed.deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E: de::Error>(self, _value: String) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence
            .next_element_seed::<DuplicateKeySeed>(DuplicateKeySeed)?
            .is_some()
        {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(config_value::duplicate_key_error_for_deserializer(&key));
            }
            map.next_value_seed(DuplicateKeySeed)?;
        }
        Ok(())
    }
}

fn map_parse_error(error: &JsonError) -> ConfigError {
    ConfigError::Parse {
        format: "json",
        line: (error.line() != 0).then(|| error.line()),
        column: (error.column() != 0).then(|| error.column()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super as json_config;
    use crate::config::ConfigError;
    use serde::Deserialize;

    #[test]
    fn parses_typed_and_untyped_json() {
        let text = r#"{"server": {"port": 8080, "tls": true}}"#;
        let value = json_config::parse_value(text, 64).expect("parse untyped");
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
        let typed: Config = json_config::parse(text).expect("parse typed");
        assert_eq!(typed.server.port, 8080);
        assert!(typed.server.tls);
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_snippet() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("{{\"password\": \"{secret}\", invalid}}");
        let error = json_config::parse_value(&text, 64).expect_err("invalid json should fail");
        assert!(matches!(
            error,
            ConfigError::Parse {
                format: "json",
                line: Some(_),
                column: Some(_)
            }
        ));
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn empty_object_parses_to_empty_table() {
        let value = json_config::parse_value("{}", 64).expect("parse empty object");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn depth_exactly_at_limit_succeeds_and_one_more_level_fails() {
        // depth 1: {"a": null} — one table level.
        let shallow =
            json_config::parse_value(r#"{"a": null}"#, 1).expect("depth 1 should fit budget 1");
        assert!(shallow.as_table().is_some());

        let nested = r#"{"a": {"b": null}}"#;
        let error =
            json_config::parse_value(nested, 1).expect_err("depth 2 should exceed budget 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(json_config::parse_value(nested, 2).is_ok());
    }

    #[test]
    fn untyped_json_uses_loader_depth_above_serde_default_limit() {
        let mut text = String::from("null");
        for _ in 0..256 {
            text = format!(r#"{{"nested":{text}}}"#);
        }

        assert!(json_config::parse_value(&text, 256).is_ok());
        assert!(matches!(
            json_config::parse_value(&text, 128),
            Err(ConfigError::DepthLimitExceeded { limit: 128 })
        ));
        assert_eq!(
            json_config::parse_value(&format!("[{text}]"), 256),
            Err(ConfigError::DepthLimitExceeded { limit: 256 })
        );
    }

    #[test]
    fn integer_overflowing_i64_but_within_u64_is_rejected() {
        // 超出值树的有符号范围时，诊断保留触发字段名。
        let text = format!("{{\"count\": {}}}", u64::MAX);
        let error = json_config::parse_value(&text, 64).expect_err("overflow should fail");
        assert!(matches!(
            error,
            ConfigError::ValueOutOfRange { key } if key == "count"
        ));
    }

    #[test]
    fn json_tokens_preserve_numeric_ranges_and_signed_zero() {
        for text in ["18446744073709551616", "-9223372036854775809"] {
            assert_eq!(
                json_config::parse_value(text, 1),
                Err(ConfigError::ValueOutOfRange { key: String::new() })
            );
        }
        assert_eq!(
            json_config::parse_value("[\n  1e99999999999]", 1),
            Err(ConfigError::Parse {
                format: "json",
                line: Some(2),
                column: Some(14),
            })
        );
        let zero = json_config::parse_value("-0", 1).unwrap().as_f64().unwrap();
        assert_eq!(zero.to_bits(), (-0.0_f64).to_bits());
        assert_eq!(
            json_config::parse_value("{\n  \"n\": 1e999\n}", 1),
            Err(ConfigError::Parse {
                format: "json",
                line: Some(2),
                column: Some(12)
            })
        );
    }

    #[test]
    fn rejects_malformed_containers_and_scalars_with_original_location() {
        for text in [
            "[1,]",
            r#"{"a":1,}"#,
            "[0 1]",
            r#"{"a" 10}"#,
            "[01]",
            "1.",
            "-",
            "[truex]",
            r#"{"\uD800":1}"#,
            "null\u{000b}",
        ] {
            assert!(
                matches!(
                    json_config::parse_value(text, 1),
                    Err(ConfigError::Parse { format: "json", .. })
                ),
                "{text}"
            );
        }
        for (text, column) in [("[\n  \"\\x\"]", 5), ("[\n  true false]", 8)] {
            assert_eq!(
                json_config::parse_value(text, 1),
                Err(ConfigError::Parse {
                    format: "json",
                    line: Some(2),
                    column: Some(column)
                })
            );
        }
    }

    #[test]
    fn rejects_trailing_non_whitespace_after_the_root_value() {
        let error = json_config::parse_value(r#"{"a": 1} trailing"#, 64)
            .expect_err("trailing content should not be ignored");
        assert!(matches!(error, ConfigError::Parse { format: "json", .. }));
    }

    #[test]
    fn rejects_duplicate_object_keys_for_untyped_and_typed_json() {
        let text = r#"{"server":{"port":8080,"port":9090}}"#;
        let untyped =
            json_config::parse_value(text, 64).expect_err("untyped duplicate should fail");
        assert!(matches!(
            untyped,
            ConfigError::DuplicateKey { key } if key == "port"
        ));

        let typed = json_config::parse::<BTreeMap<String, BTreeMap<String, i64>>>(text)
            .expect_err("typed duplicate should fail");
        assert!(matches!(
            typed,
            ConfigError::DuplicateKey { key } if key == "port"
        ));
    }
}
