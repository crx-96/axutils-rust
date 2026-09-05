use std::{env, fs, process};

use super::ConfigLoader;
use crate::config::{ConfigError, ConfigFormat};

#[test]
fn max_bytes_rejects_out_of_range_values() {
    assert!(matches!(
        ConfigLoader::new().with_max_bytes(0),
        Err(ConfigError::InvalidLimit)
    ));
    assert!(matches!(
        ConfigLoader::new().with_max_bytes(16 * 1024 * 1024 + 1),
        Err(ConfigError::InvalidLimit)
    ));
    assert!(ConfigLoader::new().with_max_bytes(1024).is_ok());
    assert!(ConfigLoader::new().with_max_bytes(16 * 1024 * 1024).is_ok());
}

#[test]
fn max_depth_rejects_out_of_range_values() {
    assert!(matches!(
        ConfigLoader::new().with_max_depth(0),
        Err(ConfigError::InvalidLimit)
    ));
    assert!(matches!(
        ConfigLoader::new().with_max_depth(257),
        Err(ConfigError::InvalidLimit)
    ));
    assert!(ConfigLoader::new().with_max_depth(1).is_ok());
    assert!(ConfigLoader::new().with_max_depth(256).is_ok());
}

#[test]
fn parse_value_dispatches_json_and_env() {
    let loader = ConfigLoader::new();
    let json = loader
        .parse_value(r#"{"a": 1}"#, ConfigFormat::Json)
        .expect("json should parse");
    assert_eq!(json.get("a").and_then(|value| value.as_i64()), Some(1));

    let env = loader
        .parse_value("A=1\n", ConfigFormat::Env)
        .expect("env should parse");
    assert_eq!(env.get("A").and_then(|value| value.as_str()), Some("1"));
}

#[test]
fn parse_dispatches_typed_json() {
    #[derive(serde::Deserialize)]
    struct Config {
        a: i64,
    }
    let config: Config = ConfigLoader::new()
        .parse(r#"{"a": 5}"#, ConfigFormat::Json)
        .expect("typed json should parse");
    assert_eq!(config.a, 5);
}

#[test]
fn with_format_overrides_extension_inference_for_load() {
    let path = env::temp_dir().join(format!(
        "axutils-config-loader-test-{}-override.txt",
        process::id()
    ));
    fs::write(&path, r#"{"a": 42}"#).expect("write temp file");

    let value = ConfigLoader::new()
        .with_format(ConfigFormat::Json)
        .load_value(&path)
        .expect("override should force json parsing");
    assert_eq!(value.get("a").and_then(|item| item.as_i64()), Some(42));

    let _ = fs::remove_file(&path);
}

#[test]
fn load_value_reports_unknown_extension_without_override() {
    let path = env::temp_dir().join(format!(
        "axutils-config-loader-test-{}-noext",
        process::id()
    ));
    fs::write(&path, "{}").expect("write temp file");

    let result = ConfigLoader::new().load_value(&path);
    assert!(matches!(result, Err(ConfigError::UnknownExtension)));

    let _ = fs::remove_file(&path);
}

#[test]
fn load_value_enforces_configured_max_bytes() {
    let path = env::temp_dir().join(format!(
        "axutils-config-loader-test-{}-too-large.json",
        process::id()
    ));
    fs::write(&path, format!(r#"{{"a": "{}"}}"#, "x".repeat(2048))).expect("write temp file");

    let result = ConfigLoader::new()
        .with_max_bytes(1024)
        .expect("valid limit")
        .load_value(&path);
    assert!(matches!(
        result,
        Err(ConfigError::FileTooLarge { limit: 1024, .. })
    ));

    let _ = fs::remove_file(&path);
}
