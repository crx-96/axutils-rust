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

#[cfg(feature = "all")]
fn main() {
    use axutils::{ConfigFormat, ConfigUtils};

    let yaml = ConfigUtils::parse_value("a: 1\n", ConfigFormat::Yaml).expect("yaml should parse");
    let toml = ConfigUtils::parse_value("a = 1\n", ConfigFormat::Toml).expect("toml should parse");
    let ini =
        ConfigUtils::parse_value("[a]\nb = 1\n", ConfigFormat::Ini).expect("ini should parse");
    let _ = (yaml, toml, ini);
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

#[cfg(not(any(
    feature = "serde-only",
    feature = "serde-toml",
    feature = "all",
    feature = "negative-config-module-no-serde",
    feature = "negative-config-utils-no-serde",
    feature = "negative-toml-only-no-serde",
    feature = "negative-yaml-under-serde-only",
    feature = "negative-toml-under-serde-only",
    feature = "negative-ini-under-serde-only",
)))]
fn main() {}
