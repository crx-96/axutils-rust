//! `serde_json` 后端：JSON 文本到 [`ConfigValue`] 或调用方类型的转换。

use serde::de::{DeserializeOwned, DeserializeSeed};

use super::{
    error::ConfigError,
    value::{classify_marker, ConfigValueSeed, ErrorMarker},
    ConfigValue,
};

pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = ConfigValueSeed::root(max_depth)
        .deserialize(&mut deserializer)
        .map_err(|error| map_value_error(&error, max_depth))?;
    deserializer
        .end()
        .map_err(|error| map_parse_error(&error))?;
    Ok(value)
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ConfigError> {
    serde_json::from_str(text).map_err(|error| map_parse_error(&error))
}

fn map_value_error(error: &serde_json::Error, max_depth: usize) -> ConfigError {
    match classify_marker(&error.to_string()) {
        ErrorMarker::DepthLimitExceeded => ConfigError::DepthLimitExceeded { limit: max_depth },
        ErrorMarker::ValueOutOfRange(key) => ConfigError::ValueOutOfRange {
            key: key.to_owned(),
        },
        ErrorMarker::None => map_parse_error(error),
    }
}

fn map_parse_error(error: &serde_json::Error) -> ConfigError {
    ConfigError::Parse {
        format: "json",
        line: (error.line() != 0).then(|| error.line()),
        column: (error.column() != 0).then(|| error.column()),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_value};
    use crate::ConfigError;
    use serde::Deserialize;

    #[test]
    fn parses_typed_and_untyped_json() {
        let text = r#"{"server": {"port": 8080, "tls": true}}"#;
        let value = parse_value(text, 64).expect("parse untyped");
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
        let typed: Config = parse(text).expect("parse typed");
        assert_eq!(typed.server.port, 8080);
        assert!(typed.server.tls);
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_snippet() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("{{\"password\": \"{secret}\", invalid}}");
        let error = parse_value(&text, 64).expect_err("invalid json should fail");
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
        let value = parse_value("{}", 64).expect("parse empty object");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn depth_exactly_at_limit_succeeds_and_one_more_level_fails() {
        // depth 1: {"a": null} — one table level.
        let shallow = parse_value(r#"{"a": null}"#, 1).expect("depth 1 should fit budget 1");
        assert!(shallow.as_table().is_some());

        let nested = r#"{"a": {"b": null}}"#;
        let error = parse_value(nested, 1).expect_err("depth 2 should exceed budget 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(parse_value(nested, 2).is_ok());
    }

    #[test]
    fn integer_overflowing_i64_but_within_u64_is_rejected() {
        // u64::MAX fits serde_json's u64 fast path (no float fallback) and exceeds i64::MAX,
        // exercising this crate's own overflow check in `ConfigValueVisitor::visit_u64`.
        let text = format!("{{\"count\": {}}}", u64::MAX);
        let error = parse_value(&text, 64).expect_err("overflow should fail");
        assert!(matches!(
            error,
            ConfigError::ValueOutOfRange { key } if key == "count"
        ));
    }

    #[test]
    fn rejects_trailing_non_whitespace_after_the_root_value() {
        let error = parse_value(r#"{"a": 1} trailing"#, 64)
            .expect_err("trailing content should not be ignored");
        assert!(matches!(error, ConfigError::Parse { format: "json", .. }));
    }
}
