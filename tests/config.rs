#![cfg(feature = "serde")]

use std::path::{Path, PathBuf};

#[cfg(all(feature = "serde", feature = "tokio"))]
use std::fs;

#[cfg(all(feature = "serde", feature = "tokio"))]
use axutils::ConfigLoader;
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
    assert_eq!(config.server.host, "localhost");
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
    assert_eq!(config.server.host, "localhost");
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio", feature = "serde-saphyr"))]
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

#[cfg(all(feature = "serde", feature = "tokio", feature = "serde-saphyr"))]
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

#[cfg(all(feature = "serde", feature = "tokio", feature = "toml"))]
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

#[cfg(all(feature = "serde", feature = "tokio", feature = "rust-ini"))]
#[tokio::test]
async fn reads_ini_fixture_asynchronously() {
    let value = ConfigUtils::load_value_async(fixture("valid.ini"))
        .await
        .expect("async ini fixture should load");
    assert_eq!(value.get("top").and_then(|v| v.as_str()), Some("1"));
}

#[cfg(all(feature = "serde", feature = "tokio"))]
struct TempConfigFile {
    path: PathBuf,
}

#[cfg(all(feature = "serde", feature = "tokio"))]
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

#[cfg(all(feature = "serde", feature = "tokio"))]
impl Drop for TempConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
