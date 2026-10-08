#![cfg(feature = "config")]

use std::path::{Path, PathBuf};

#[cfg(feature = "config-async")]
use std::fs;

#[cfg(any(feature = "config-async", feature = "config-toml"))]
use axutils::config::ConfigLoader;
use axutils::{
    config::{ConfigError, ConfigFormat},
    utils::ConfigUtils,
};
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
    assert_eq!(config.server.host, "localhost");
    assert_eq!(config.server.port, 8080);
    assert!(config.server.tls);
}

#[cfg(feature = "config-yaml")]
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

#[cfg(feature = "config-toml")]
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
    assert_eq!(config.server.host, "localhost");
    assert_eq!(config.server.port, 8080);
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_preserves_datetime_marker_keys_as_ordinary_user_data() {
    let loader = ConfigLoader::new();
    let marker = "$__toml_private_datetime";
    for text in [
        "\"$__toml_private_datetime\" = \"2024-02-29T01:02:03Z\"\n",
        "\"$__toml_private_\\u0064atetime\" = \"2024-02-29T01:02:03Z\"\n",
    ] {
        let value = loader.parse_value(text, ConfigFormat::Toml).unwrap();
        assert_eq!(
            value.as_table().unwrap().get(marker).unwrap().as_str(),
            Some("2024-02-29T01:02:03Z")
        );
    }
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_escaped_datetime_marker_keeps_non_string_values() {
    let loader = ConfigLoader::new();
    for text in [
        "\"$__toml_private_datetime\" = 123\n",
        "\"$__toml_private_\\u0064atetime\" = 123\n",
    ] {
        let value = loader.parse_value(text, ConfigFormat::Toml).unwrap();
        assert_eq!(
            value
                .as_table()
                .unwrap()
                .get("$__toml_private_datetime")
                .unwrap()
                .as_i64(),
            Some(123)
        );
    }
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_marker_fields_do_not_hide_sibling_fields_or_real_datetimes() {
    let text = "[metadata]\n\"$__toml_private_datetime\" = \"2024-02-29T01:02:03Z\"\nother = true\ncreated = 2024-02-29T01:02:03Z\n";
    let value = ConfigLoader::new()
        .parse_value(text, ConfigFormat::Toml)
        .unwrap();
    let table = value.get("metadata").unwrap().as_table().unwrap();
    assert_eq!(table.len(), 3);
    assert_eq!(table.get("other").unwrap().as_bool(), Some(true));
    for key in ["$__toml_private_datetime", "created"] {
        assert_eq!(
            table.get(key).unwrap().as_str(),
            Some("2024-02-29T01:02:03Z")
        );
    }
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_datetimes_use_scalar_depth_and_containers_keep_the_depth_budget() {
    let loader = ConfigLoader::new().with_max_depth(1).unwrap();
    let value = loader
        .parse_value("created = 2024-02-29T01:02:03Z\n", ConfigFormat::Toml)
        .unwrap();
    assert_eq!(
        value.get("created").unwrap().as_str(),
        Some("2024-02-29T01:02:03Z")
    );

    for text in [
        "[event]\ncreated = 2024-02-29T01:02:03Z\n",
        "created = [2024-02-29T01:02:03Z]\n",
    ] {
        assert!(matches!(
            loader.parse_value(text, ConfigFormat::Toml),
            Err(ConfigError::DepthLimitExceeded { limit: 1 })
        ));
        assert!(ConfigLoader::new()
            .with_max_depth(2)
            .unwrap()
            .parse_value(text, ConfigFormat::Toml)
            .is_ok());
    }
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_numeric_conversion_retains_radices_bounds_and_error_categories() {
    let loader = ConfigLoader::new();
    let text = "negative = -9223372036854775808\npositive = +9223372036854775807\nhex = 0x7fff_ffff_ffff_ffff\noctal = 0o17\nbinary = 0b1010\nratio = -1.25e+2\n";
    let value = loader.parse_value(text, ConfigFormat::Toml).unwrap();
    for (key, expected) in [
        ("negative", i64::MIN),
        ("positive", i64::MAX),
        ("hex", i64::MAX),
        ("octal", 15),
        ("binary", 10),
    ] {
        assert_eq!(value.get(key).unwrap().as_i64(), Some(expected));
    }
    assert_eq!(value.get("ratio").unwrap().as_f64(), Some(-125.0));

    for number in [
        "9223372036854775808",
        "-9223372036854775809",
        "0xffffffffffffffff",
        "340282366920938463463374607431768211455",
    ] {
        assert!(
            matches!(loader.parse_value(&format!("count = {number}\n"), ConfigFormat::Toml), Err(ConfigError::ValueOutOfRange { key }) if key == "count")
        );
    }
    for number in [
        "340282366920938463463374607431768211456",
        "-170141183460469231731687303715884105729",
        "1e999",
    ] {
        assert!(matches!(
            loader.parse_value(&format!("count = {number}\n"), ConfigFormat::Toml),
            Err(ConfigError::Parse {
                format: "toml",
                line: Some(1),
                column: Some(9)
            })
        ));
    }
    for number in ["inf", "+inf", "-inf", "nan"] {
        assert!(!loader
            .parse_value(&format!("ratio = {number}\n"), ConfigFormat::Toml)
            .unwrap()
            .get("ratio")
            .unwrap()
            .as_f64()
            .unwrap()
            .is_finite());
    }
}

#[cfg(feature = "config-toml")]
#[test]
fn toml_syntax_errors_precede_conversion_and_depth_errors() {
    let loader = ConfigLoader::new().with_max_depth(1).unwrap();
    for text in [
        "[event]\ncreated = 1\ninvalid ===\n",
        "count = 9223372036854775808\ninvalid ===\n",
        "count = 1\ncount = 2\n",
    ] {
        assert!(matches!(
            loader.parse_value(text, ConfigFormat::Toml),
            Err(ConfigError::Parse { format: "toml", .. })
        ));
    }
}

#[cfg(feature = "config-ini")]
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

#[cfg(feature = "config-async")]
#[tokio::test]
async fn reads_json_fixture_async_untyped_and_typed() {
    let path = fixture("valid.json");
    let value = ConfigUtils::load_value_async(&path)
        .await
        .expect("async json fixture should load");
    assert_eq!(
        value.get("server.host").and_then(|v| v.as_str()),
        Some("localhost")
    );

    #[derive(Deserialize)]
    struct Config {
        server: Server,
    }
    let config: Config = ConfigUtils::load_async(&path)
        .await
        .expect("async json fixture should load typed");
    assert_eq!(config.server.port, 8080);
    assert!(config.server.tls);

    let direct: Config = ConfigLoader::new()
        .load_async(&path)
        .await
        .expect("ConfigLoader async method should load typed json");
    assert_eq!(direct.server.host, "localhost");
}

#[cfg(feature = "config-async")]
#[tokio::test]
async fn async_explicit_format_overrides_extension_for_untyped_and_typed_loads() {
    let file = TempConfigFile::new("explicit-format-async.txt", br#"{"port": 8080}"#);

    let value = ConfigUtils::load_value_as_async(file.path(), ConfigFormat::Json)
        .await
        .expect("explicit async json format should load");
    assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));

    #[derive(Deserialize)]
    struct Config {
        port: u16,
    }
    let config: Config = ConfigUtils::load_as_async(file.path(), ConfigFormat::Json)
        .await
        .expect("explicit async json format should load typed");
    assert_eq!(config.port, 8080);
}

#[cfg(feature = "config-async")]
#[tokio::test]
async fn async_loader_preserves_format_limits_and_env_setting() {
    let explicit = TempConfigFile::new("loader-format-async.txt", br#"{"port": 8080}"#);
    let value = ConfigLoader::new()
        .with_format(ConfigFormat::Json)
        .with_max_bytes(1024)
        .expect("minimum byte limit should be valid")
        .with_max_depth(8)
        .expect("depth limit should be valid")
        .with_env_substitution(false)
        .load_value_async(explicit.path())
        .await
        .expect("custom loader settings should be used asynchronously");
    assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));

    let deep = TempConfigFile::new("loader-depth-async.json", br#"{"a":{"b":{"c":1}}}"#);
    let error = ConfigLoader::new()
        .with_max_depth(2)
        .expect("depth limit should be valid")
        .load_value_async(deep.path())
        .await
        .expect_err("async loader should preserve depth limits");
    assert!(matches!(
        error,
        ConfigError::DepthLimitExceeded { limit: 2 }
    ));

    let too_large = TempConfigFile::new("loader-size-async.json", vec![b'a'; 1025]);
    let error = ConfigLoader::new()
        .with_max_bytes(1024)
        .expect("minimum byte limit should be valid")
        .load_value_async(too_large.path())
        .await
        .expect_err("async loader should preserve byte limits");
    assert!(matches!(
        error,
        ConfigError::FileTooLarge { limit: 1024, .. }
    ));

    let env = TempConfigFile::new(
        "loader-env-async.env",
        b"VALUE=\"${ASYNC_UNDEFINED_VALUE}\"\n",
    );
    let error = ConfigLoader::new()
        .with_env_substitution(false)
        .load_value_async(env.path())
        .await
        .expect_err("disabled env fallback should reject undefined variables");
    assert!(matches!(error, ConfigError::UndefinedVariable { .. }));
}

#[cfg(feature = "config-async")]
#[tokio::test]
async fn async_loader_can_be_reused_by_multiple_reads_without_state_leaks() {
    let json = fixture("valid.json");
    let env = fixture("sample.env");
    let loader = ConfigUtils::loader();
    let (json_result, env_result) = tokio::join!(
        loader.load_value_async(&json),
        loader.load_value_async(&env)
    );

    let json_value = json_result.expect("concurrent async json read should succeed");
    assert_eq!(
        json_value.get("server.port").and_then(|v| v.as_i64()),
        Some(8080)
    );
    let env_value = env_result.expect("concurrent async env read should succeed");
    assert_eq!(
        env_value.get("GREETING").and_then(|v| v.as_str()),
        Some("hello, localhost")
    );
}

#[cfg(feature = "config-async")]
#[tokio::test]
async fn async_errors_keep_existing_categories_and_redaction() {
    let invalid = fixture("invalid_with_sentinel.json");
    let error = ConfigUtils::load_value_async(&invalid)
        .await
        .expect_err("invalid async fixture should fail");
    assert!(matches!(error, ConfigError::Parse { format: "json", .. }));
    let display = error.to_string();
    let debug = format!("{error:?}");
    let secret = "sentinel-password-must-not-leak-a1b2c3";
    assert!(!display.contains(secret));
    assert!(!debug.contains(secret));

    let missing = fixture("does-not-exist.json");
    let error = ConfigUtils::load_value_async(&missing)
        .await
        .expect_err("missing async fixture should fail");
    assert!(matches!(
        error,
        ConfigError::Io {
            kind: std::io::ErrorKind::NotFound,
            ..
        }
    ));

    let unknown = std::env::temp_dir().join(format!(
        "axutils-config-integration-test-{}-async.unknown",
        std::process::id()
    ));
    let error = ConfigUtils::load_value_async(&unknown)
        .await
        .expect_err("unknown async extension should fail before opening the file");
    assert!(matches!(error, ConfigError::UnknownExtension));

    #[allow(dead_code)]
    #[derive(Debug, Deserialize)]
    struct WrongServer {
        host: String,
        port: String,
        tls: bool,
    }
    #[allow(dead_code)]
    #[derive(Debug, Deserialize)]
    struct WrongConfig {
        server: WrongServer,
    }
    let path = fixture("valid.json");
    let error = ConfigUtils::load_async::<WrongConfig>(&path)
        .await
        .expect_err("async type mismatch should fail");
    let sync_error =
        ConfigUtils::load::<WrongConfig>(&path).expect_err("sync type mismatch should fail");
    assert_eq!(error, sync_error);
}

#[cfg(feature = "config-async")]
#[tokio::test]
async fn reads_dotenv_fixture_asynchronously() {
    let value = ConfigUtils::load_value_async(fixture("sample.env"))
        .await
        .expect("async dotenv fixture should load");
    assert_eq!(
        value.get("GREETING").and_then(|v| v.as_str()),
        Some("hello, localhost")
    );
}

#[cfg(all(feature = "config-async", feature = "config-yaml"))]
#[tokio::test]
async fn reads_yaml_fixture_asynchronously() {
    let value = ConfigUtils::load_value_async(fixture("valid.yaml"))
        .await
        .expect("async yaml fixture should load");
    assert_eq!(
        value.get("server.port").and_then(|v| v.as_i64()),
        Some(8080)
    );
}

#[cfg(all(feature = "config-async", feature = "config-yaml"))]
#[tokio::test]
async fn reads_typed_yaml_fixture_asynchronously() {
    #[derive(Debug, Deserialize)]
    struct Config {
        server: Server,
    }

    let config: Config = ConfigUtils::load_async(fixture("valid.yaml"))
        .await
        .expect("async typed yaml fixture should load");
    assert_eq!(config.server.host, "localhost");
    assert_eq!(config.server.port, 8080);
    assert!(config.server.tls);
}

#[cfg(all(feature = "config-async", feature = "config-toml"))]
#[tokio::test]
async fn reads_toml_fixture_asynchronously() {
    let value = ConfigUtils::load_value_async(fixture("valid.toml"))
        .await
        .expect("async toml fixture should load");
    assert_eq!(
        value.get("server.tls").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[cfg(all(feature = "config-async", feature = "config-ini"))]
#[tokio::test]
async fn reads_ini_fixture_asynchronously() {
    let value = ConfigUtils::load_value_async(fixture("valid.ini"))
        .await
        .expect("async ini fixture should load");
    assert_eq!(value.get("top").and_then(|v| v.as_str()), Some("1"));
}

#[cfg(feature = "config-async")]
struct TempConfigFile {
    path: PathBuf,
}

#[cfg(feature = "config-async")]
impl TempConfigFile {
    fn new(name: &str, contents: impl AsRef<[u8]>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "axutils-config-integration-test-{}-{name}",
            std::process::id()
        ));
        fs::write(&path, contents).expect("write temporary config file");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(feature = "config-async")]
impl Drop for TempConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
