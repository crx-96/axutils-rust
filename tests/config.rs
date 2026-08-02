#![cfg(feature = "serde")]

use std::path::{Path, PathBuf};

use axutils::{ConfigError, ConfigFormat, ConfigUtils};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Server {
    host: String,
    port: u16,
    tls: bool,
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("config")
        .join(name)
}

#[test]
fn reads_json_fixture_untyped_and_typed() {
    let path = fixture("valid.json");
    let value = ConfigUtils::load_value(&path).expect("json fixture should load");
    assert_eq!(
        value.get("server.host").and_then(|v| v.as_str()),
        Some("localhost")
    );

    #[derive(Deserialize)]
    struct Config {
        server: Server,
    }
    let config: Config = ConfigUtils::load(&path).expect("json fixture should load typed");
    assert_eq!(config.server.port, 8080);
    assert!(config.server.tls);
}

#[cfg(feature = "serde-saphyr")]
#[test]
fn reads_yaml_fixture_untyped_and_typed() {
    let path = fixture("valid.yaml");
    let value = ConfigUtils::load_value(&path).expect("yaml fixture should load");
    assert_eq!(
        value.get("server.port").and_then(|v| v.as_i64()),
        Some(8080)
    );

    #[derive(Deserialize)]
    struct Config {
        server: Server,
    }
    let config: Config = ConfigUtils::load(&path).expect("yaml fixture should load typed");
    assert_eq!(config.server.host, "localhost");
}

#[cfg(feature = "toml")]
#[test]
fn reads_toml_fixture_untyped_and_typed() {
    let path = fixture("valid.toml");
    let value = ConfigUtils::load_value(&path).expect("toml fixture should load");
    assert_eq!(
        value.get("server.tls").and_then(|v| v.as_bool()),
        Some(true)
    );

    #[derive(Deserialize)]
    struct Config {
        server: Server,
    }
    let config: Config = ConfigUtils::load(&path).expect("toml fixture should load typed");
    assert_eq!(config.server.port, 8080);
}

#[cfg(feature = "rust-ini")]
#[test]
fn reads_ini_fixture_untyped_and_typed() {
    let path = fixture("valid.ini");
    let value = ConfigUtils::load_value(&path).expect("ini fixture should load");
    assert_eq!(value.get("top").and_then(|v| v.as_str()), Some("1"));

    #[derive(Deserialize)]
    struct Config {
        server: Server,
    }
    let config: Config = ConfigUtils::load(&path).expect("ini fixture should load typed");
    assert_eq!(config.server.host, "localhost");
    assert!(config.server.tls);
}

#[test]
fn reads_dotenv_fixture_by_filename_and_by_extension() {
    let dotfile = fixture(".env");
    let value = ConfigUtils::load_value(&dotfile).expect(".env fixture should load");
    assert_eq!(
        value.get("MODE").and_then(|v| v.as_str()),
        Some("production")
    );

    let named = fixture("sample.env");
    let value = ConfigUtils::load_value(&named).expect("sample.env fixture should load");
    assert_eq!(
        value.get("HOST").and_then(|v| v.as_str()),
        Some("localhost")
    );
    assert_eq!(
        value.get("GREETING").and_then(|v| v.as_str()),
        Some("hello, localhost")
    );
}

#[test]
fn explicit_format_override_reads_a_json_fixture_with_any_extension() {
    let path = fixture("valid.json");
    let value = ConfigUtils::load_value_as(&path, ConfigFormat::Json)
        .expect("explicit json format should load regardless of inference");
    assert_eq!(
        value.get("server.port").and_then(|v| v.as_i64()),
        Some(8080)
    );
}

#[test]
fn parse_error_never_leaks_the_sentinel_password_from_the_fixture() {
    let path = fixture("invalid_with_sentinel.json");
    let error = ConfigUtils::load_value(&path).expect_err("malformed fixture should fail");
    assert!(matches!(error, ConfigError::Parse { format: "json", .. }));

    let display = error.to_string();
    let debug = format!("{error:?}");
    let secret = "sentinel-password-must-not-leak-a1b2c3";
    assert!(!display.contains(secret));
    assert!(!debug.contains(secret));
}

#[test]
fn missing_file_reports_io_error() {
    let path = fixture("does-not-exist.json");
    let error = ConfigUtils::load_value(&path).expect_err("missing file should fail");
    assert!(matches!(
        error,
        ConfigError::Io {
            kind: std::io::ErrorKind::NotFound,
            ..
        }
    ));
}
