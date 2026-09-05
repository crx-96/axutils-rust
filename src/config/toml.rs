//! `toml` 后端：TOML 文本到 [`ConfigValue`] 或调用方类型的转换。

use serde::de::{DeserializeOwned, DeserializeSeed};
use toml::{
    self,
    de::{Deserializer as TomlDeserializer, Error as TomlError},
    Value as TomlValue,
};

use super::{error as config_error, value as config_value};
use super::{
    error::ConfigError,
    value::{ConfigValueSeed, ErrorMarker, TOML_DATETIME_FIELD},
    ConfigValue,
};

pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    // `toml_datetime` represents a datetime through a map whose sole key is a private
    // string. A user is nevertheless allowed to create a TOML table with that same key.
    // When the marker occurs in the source, parse the public TOML value tree first so the
    // actual node kind remains distinguishable from the serde pseudo-table representation.
    if text.contains(TOML_DATETIME_FIELD) {
        if let Ok(value) = toml::from_str::<TomlValue>(text) {
            return convert_toml_value(value, max_depth, max_depth);
        }
    }
    let deserializer =
        TomlDeserializer::parse(text).map_err(|error| map_parse_error(text, &error))?;
    ConfigValueSeed::root_for_toml(max_depth)
        .deserialize(deserializer)
        .map_err(|error| map_value_error(text, &error, max_depth))
}

fn convert_toml_value(
    value: TomlValue,
    remaining_depth: usize,
    limit: usize,
) -> Result<ConfigValue, ConfigError> {
    match value {
        TomlValue::String(value) => Ok(ConfigValue::String(value)),
        TomlValue::Integer(value) => Ok(ConfigValue::Integer(value)),
        TomlValue::Float(value) => Ok(ConfigValue::Float(value)),
        TomlValue::Boolean(value) => Ok(ConfigValue::Bool(value)),
        TomlValue::Datetime(value) => Ok(ConfigValue::String(value.to_string())),
        TomlValue::Array(values) => {
            let remaining_depth = remaining_depth
                .checked_sub(1)
                .ok_or(ConfigError::DepthLimitExceeded { limit })?;
            values
                .into_iter()
                .map(|value| convert_toml_value(value, remaining_depth, limit))
                .collect::<Result<Vec<_>, _>>()
                .map(ConfigValue::Array)
        }
        TomlValue::Table(values) => {
            let remaining_depth = remaining_depth
                .checked_sub(1)
                .ok_or(ConfigError::DepthLimitExceeded { limit })?;
            values
                .into_iter()
                .map(|(key, value)| {
                    convert_toml_value(value, remaining_depth, limit).map(|value| (key, value))
                })
                .collect::<Result<_, _>>()
                .map(ConfigValue::Table)
        }
    }
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ConfigError> {
    toml::from_str(text).map_err(|error| map_parse_error(text, &error))
}

fn map_value_error(text: &str, error: &TomlError, max_depth: usize) -> ConfigError {
    match config_value::classify_marker(error.message()) {
        ErrorMarker::DepthLimitExceeded => ConfigError::DepthLimitExceeded { limit: max_depth },
        ErrorMarker::DuplicateKey(key) => ConfigError::DuplicateKey {
            key: key.to_owned(),
        },
        ErrorMarker::ValueOutOfRange(key) => ConfigError::ValueOutOfRange {
            key: key.to_owned(),
        },
        ErrorMarker::None => map_parse_error(text, error),
    }
}

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
