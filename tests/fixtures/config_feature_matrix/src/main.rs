#[cfg(feature = "serde-only")]
fn main() {
    use axutils::{ConfigFormat, ConfigUtils};

    let json = ConfigUtils::parse_value(r#"{"a": 1}"#, ConfigFormat::Json)
        .expect("json should parse under serde-only");
    let env = ConfigUtils::parse_value("A=1\n", ConfigFormat::Env)
        .expect("env should parse under serde-only");
    let _ = (json, env);
}

#[cfg(feature = "serde-toml")]
fn main() {
    use axutils::{ConfigFormat, ConfigUtils};

    let _ = ConfigUtils::parse_value("a = 1\n", ConfigFormat::Toml)
        .expect("toml should parse under serde+toml");
}

#[cfg(feature = "serde-tokio")]
async fn compile_async_api() {
    let _ = axutils::ConfigUtils::load_value_async("config.json").await;
    let _ = axutils::ConfigUtils::load_async::<axutils::ConfigValue>("config.json").await;
    let _ = axutils::ConfigUtils::load_value_as_async("config.txt", axutils::ConfigFormat::Json)
        .await;
    let _ = axutils::ConfigUtils::load_as_async::<axutils::ConfigValue>(
        "config.txt",
        axutils::ConfigFormat::Json,
    )
    .await;

    let loader = axutils::ConfigLoader::new();
    let _ = loader.load_value_async("config.json").await;
    let _ = loader.load_async::<axutils::ConfigValue>("config.json").await;
}

#[cfg(feature = "serde-tokio")]
fn main() {
    let _ = compile_async_api;
}

#[cfg(feature = "all")]
fn main() {
    use axutils::{ConfigFormat, ConfigUtils};

    let yaml = ConfigUtils::parse_value("a: 1\n", ConfigFormat::Yaml).expect("yaml should parse");
    let toml = ConfigUtils::parse_value("a = 1\n", ConfigFormat::Toml).expect("toml should parse");
    let ini =
        ConfigUtils::parse_value("[a]\nb = 1\n", ConfigFormat::Ini).expect("ini should parse");
    let _ = (yaml, toml, ini);
}

#[cfg(feature = "serde-tokio-all")]
async fn compile_async_all_api() {
    let _ = axutils::ConfigFormat::Yaml;
    let _ = axutils::ConfigFormat::Toml;
    let _ = axutils::ConfigFormat::Ini;
    let _ = axutils::ConfigUtils::load_value_async("config.json").await;
    let _ = axutils::ConfigUtils::load_as_async::<axutils::ConfigValue>(
        "config.txt",
        axutils::ConfigFormat::Json,
    )
    .await;
    let loader = axutils::ConfigLoader::new();
    let _ = loader.load_async::<axutils::ConfigValue>("config.json").await;
}

#[cfg(feature = "serde-tokio-all")]
fn main() {
    let _ = compile_async_all_api;
}

#[cfg(feature = "tokio-only")]
fn main() {}

#[cfg(feature = "negative-tokio-config-no-serde")]
fn main() {
    // `tokio` alone (without `serde`) must not export any config API either.
    let _ = axutils::ConfigUtils::loader;
    let _ = axutils::config::ConfigLoader::new;
}

#[cfg(feature = "negative-config-module-no-serde")]
fn main() {
    let _ = axutils::config::ConfigLoader::new;
}

#[cfg(feature = "negative-config-utils-no-serde")]
fn main() {
    let _ = axutils::ConfigUtils::loader;
}

#[cfg(feature = "negative-toml-only-no-serde")]
fn main() {
    // `toml` alone (without `serde`) must not export any config API either.
    let _ = axutils::ConfigUtils::loader;
}

#[cfg(feature = "negative-config-async-no-tokio")]
fn main() {
    let _ = axutils::ConfigUtils::load_value_async;
}

#[cfg(feature = "negative-yaml-under-serde-only")]
fn main() {
    let _ = axutils::ConfigFormat::Yaml;
}

#[cfg(feature = "negative-toml-under-serde-only")]
fn main() {
    let _ = axutils::ConfigFormat::Toml;
}

#[cfg(feature = "negative-ini-under-serde-only")]
fn main() {
    let _ = axutils::ConfigFormat::Ini;
}

#[cfg(feature = "negative-yaml-under-serde-tokio")]
fn main() {
    // `serde,tokio` without backend features must not expose YAML/TOML/INI variants.
    let _ = axutils::ConfigFormat::Yaml;
}

#[cfg(feature = "negative-toml-under-serde-tokio")]
fn main() {
    let _ = axutils::ConfigFormat::Toml;
}

#[cfg(feature = "negative-ini-under-serde-tokio")]
fn main() {
    let _ = axutils::ConfigFormat::Ini;
}

#[cfg(not(any(
    feature = "serde-only",
    feature = "serde-toml",
    feature = "serde-tokio",
    feature = "all",
    feature = "serde-tokio-all",
    feature = "tokio-only",
    feature = "negative-tokio-config-no-serde",
    feature = "negative-config-module-no-serde",
    feature = "negative-config-utils-no-serde",
    feature = "negative-toml-only-no-serde",
    feature = "negative-config-async-no-tokio",
    feature = "negative-yaml-under-serde-only",
    feature = "negative-toml-under-serde-only",
    feature = "negative-ini-under-serde-only",
    feature = "negative-yaml-under-serde-tokio",
    feature = "negative-toml-under-serde-tokio",
    feature = "negative-ini-under-serde-tokio",
)))]
fn main() {}
