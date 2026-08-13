//! `serde_json` 后端：JSON 文本到 [`ConfigValue`] 或调用方类型的转换。

use std::{collections::BTreeSet, fmt};

use serde::de::{
    self, DeserializeOwned, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor,
};

use super::{
    error::ConfigError,
    value::{classify_marker, duplicate_key_error_for_deserializer, ConfigValueSeed, ErrorMarker},
    ConfigValue,
};

pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    // `ConfigValueSeed` applies the crate's explicit 1..=256 depth budget and maps its
    // marker to `ConfigError::DepthLimitExceeded`; do not let serde_json's smaller default
    // recursion limit preempt that stable error contract.
    deserializer.disable_recursion_limit();
    let value = ConfigValueSeed::root(max_depth)
        .deserialize(&mut deserializer)
        .map_err(|error| map_value_error(&error, max_depth))?;
    deserializer
        .end()
        .map_err(|error| map_parse_error(&error))?;
    Ok(value)
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ConfigError> {
    // Typed JSON intentionally uses two bounded passes: the first preserves the crate's
    // duplicate-key error contract, and the second lets serde_json deserialize into `T`.
    // File callers are bounded by the loader's max-bytes check; in-memory callers still own
    // the size of `text`.
    reject_duplicate_keys(text)?;
    serde_json::from_str(text).map_err(|error| map_parse_error(&error))
}

fn map_value_error(error: &serde_json::Error, max_depth: usize) -> ConfigError {
    match classify_marker(&error.to_string()) {
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
    let mut deserializer = serde_json::Deserializer::from_str(text);
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
                return Err(duplicate_key_error_for_deserializer(&key));
            }
            map.next_value_seed(DuplicateKeySeed)?;
        }
        Ok(())
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
    fn untyped_json_uses_loader_depth_above_serde_default_limit() {
        let mut text = String::from("null");
        for _ in 0..129 {
            text = format!(r#"{{"nested":{text}}}"#);
        }

        assert!(parse_value(&text, 256).is_ok());
        assert!(matches!(
            parse_value(&text, 128),
            Err(ConfigError::DepthLimitExceeded { limit: 128 })
        ));
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

    #[test]
    fn rejects_duplicate_object_keys_for_untyped_and_typed_json() {
        let text = r#"{"server":{"port":8080,"port":9090}}"#;
        let untyped = parse_value(text, 64).expect_err("untyped duplicate should fail");
        assert!(matches!(
            untyped,
            ConfigError::DuplicateKey { key } if key == "port"
        ));

        let typed = parse::<
            std::collections::BTreeMap<String, std::collections::BTreeMap<String, i64>>,
        >(text)
        .expect_err("typed duplicate should fail");
        assert!(matches!(
            typed,
            ConfigError::DuplicateKey { key } if key == "port"
        ));
    }
}
