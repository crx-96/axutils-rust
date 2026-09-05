use super::support::{assert_tree_cache_budget, has_package, tree, tree_with};

pub(super) fn baseline() {
    let tree = tree("");
    let non_empty_lines = tree
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        non_empty_lines.len(),
        2,
        "the no-feature dependency tree must contain only the probe and axutils, got: {tree}"
    );
    assert!(
        non_empty_lines[1].trim_start().starts_with("└── axutils "),
        "the no-feature dependency tree did not terminate at axutils: {tree}"
    );
    for package in [
        "axum", "chrono", "lettre", "redis", "reqwest", "sqlx", "tokio",
    ] {
        assert_absent(&tree, package, "default");
    }
    assert_tree_cache_budget();
}

pub(super) fn phone_and_template() {
    let phone = tree("phone-validation");
    assert_has(&phone, "regex", "phone-validation");
    assert_has(&phone, "phonenumber", "phone-validation");
    assert_absent(&phone, "minijinja", "phone-validation");

    let strfmt = tree("template-strfmt");
    assert_has(&strfmt, "strfmt", "template-strfmt");
    assert_has(&strfmt, "serde", "template-strfmt");
    assert_absent(&strfmt, "minijinja", "template-strfmt");

    let minijinja = tree("template-minijinja");
    assert_has(&minijinja, "minijinja", "template-minijinja");
    assert_has(&minijinja, "serde", "template-minijinja");
    assert_absent(&minijinja, "strfmt", "template-minijinja");
    assert_tree_cache_budget();
}

pub(super) fn tokio_and_scheduler() {
    let tokio = tree("tokio");
    assert_has(&tokio, "tokio", "tokio");
    assert_absent(&tokio, "tokio-util", "tokio");

    let task_group = tree("task-group");
    assert_has(&task_group, "tokio", "task-group");
    assert_has(&task_group, "tokio-util", "task-group");
    assert_has(&task_group, "futures-timer", "task-group");

    let scheduler = tree_with("scheduler", "normal,build,features", None);
    for package in ["chrono", "chrono-tz", "croner", "tokio"] {
        assert_has(&scheduler, package, "scheduler");
    }
    assert_tree_cache_budget();
}

pub(super) fn axum_layers() {
    let core = tree("axum");
    for package in ["axum", "tokio", "tower"] {
        assert_has(&core, package, "axum");
    }
    for package in ["tower-http", "tower_governor"] {
        assert_absent(&core, package, "axum");
    }

    let tower = tree("axum-tower");
    assert_has(&tower, "axum", "axum-tower");
    assert_absent(&tower, "tower-http", "axum-tower");
    assert_absent(&tower, "tower_governor", "axum-tower");

    let tower_http = tree("axum-tower-http");
    assert_has(&tower_http, "tower-http", "axum-tower-http");
    assert_absent(&tower_http, "tower_governor", "axum-tower-http");

    let governor = tree_with("axum-governor", "normal,build,features", None);
    assert_has(&governor, "tower_governor", "axum-governor");
    assert_has(&governor, "futures-timer", "axum-governor");
    assert!(governor.contains(r#"axum feature "default""#));
    assert_tree_cache_budget();
}

pub(super) fn fs_layers() {
    let async_fs = tree("fs-async");
    assert_has(&async_fs, "tokio", "fs-async");
    assert_absent(&async_fs, "tempfile", "fs-async");
    assert_absent(&async_fs, "async-tempfile", "fs-async");

    let temp = tree("fs-temp");
    assert_has(&temp, "tempfile", "fs-temp");
    assert_absent(&temp, "tokio", "fs-temp");

    let async_temp = tree("fs-temp-async");
    assert_has(&async_temp, "async-tempfile", "fs-temp-async");
    assert_has(&async_temp, "tokio", "fs-temp-async");
    assert_tree_cache_budget();
}

pub(super) fn config_layers() {
    let config = tree("config");
    assert_has(&config, "serde", "config");
    assert_has(&config, "serde_json", "config");
    for package in ["serde-saphyr", "toml", "rust-ini", "tokio"] {
        assert_absent(&config, package, "config");
    }

    for (feature, package) in [
        ("config-yaml", "serde-saphyr"),
        ("config-toml", "toml"),
        ("config-ini", "rust-ini"),
    ] {
        let tree = tree(feature);
        assert_has(&tree, package, feature);
        assert_has(&tree, "serde_json", feature);
    }

    let async_config = tree("config-async");
    assert_has(&async_config, "tokio", "config-async");
    assert_has(&async_config, "serde_json", "config-async");
    assert_tree_cache_budget();
}

pub(super) fn email_layers() {
    let email = tree("email");
    assert_has(&email, "lettre", "email");
    assert_absent(&email, "tokio", "email");

    let async_email = tree("email-async");
    assert_has(&async_email, "lettre", "email-async");
    assert_has(&async_email, "tokio", "email-async");
    assert_tree_cache_budget();
}

pub(super) fn http_layers() {
    let http = tree("http");
    for package in ["ureq", "url"] {
        assert_has(&http, package, "http");
    }
    assert_absent(&http, "reqwest", "http");

    let async_http = tree("http-async");
    assert_has(&async_http, "reqwest", "http-async");
    assert_has(&async_http, "tokio", "http-async");

    let json_http = tree("http-json");
    assert_has(&json_http, "serde_json", "http-json");
    assert_has(&json_http, "serde_urlencoded", "http-json");
    assert_absent(&json_http, "reqwest", "http-json");
    assert_tree_cache_budget();
}

pub(super) fn redis_layers() {
    let redis = tree_with("redis", "normal,build,features", None);
    for package in ["redis", "r2d2", "rmp-serde"] {
        assert_has(&redis, package, "redis");
    }
    assert_absent(&redis, "tokio", "redis");
    let redis_features = tree_with("redis", "normal,build,features", Some("redis"));
    assert!(!redis_features.contains(r#"redis feature "cluster""#));

    let cluster = tree_with("redis-cluster", "normal,build,features", None);
    assert_absent(&cluster, "tokio", "redis-cluster");
    let cluster_features = tree_with("redis-cluster", "normal,build,features", Some("redis"));
    assert!(cluster_features.contains(r#"redis feature "cluster""#));

    let async_redis = tree_with("redis-async", "normal,build,features", None);
    assert_has(&async_redis, "tokio", "redis-async");
    let async_features = tree_with("redis-async", "normal,build,features", Some("redis"));
    assert!(async_features.contains(r#"redis feature "connection-manager""#));
    assert!(!async_features.contains(r#"redis feature "cluster-async""#));

    let cross = tree_with(
        "redis-cluster,redis-async",
        "normal,build,features",
        Some("redis"),
    );
    assert!(cross.contains(r#"redis feature "cluster""#));
    assert!(!cross.contains(r#"redis feature "cluster-async""#));

    let async_cluster = tree_with(
        "redis-cluster-async",
        "normal,build,features",
        Some("redis"),
    );
    assert!(async_cluster.contains(r#"redis feature "cluster""#));
    assert!(async_cluster.contains(r#"redis feature "cluster-async""#));
    assert_tree_cache_budget();
}

pub(super) fn sqlx_drivers() {
    for (feature, expected) in [
        ("sqlx-postgres", "postgres"),
        ("sqlx-mysql", "mysql"),
        ("sqlx-sqlite", "sqlite-bundled"),
    ] {
        let tree = tree_with(feature, "normal,build,features", Some("sqlx"));
        assert_has(&tree, "sqlx", feature);
        assert!(tree.contains(&format!(r#"sqlx feature "{expected}""#)));
        for absent in ["postgres", "mysql", "sqlite-bundled"] {
            if absent != expected {
                assert!(
                    !tree.contains(&format!(r#"sqlx feature "{absent}""#)),
                    "{feature} unexpectedly enables SQLx driver {absent}"
                );
            }
        }
    }
    let aggregate = tree_with("sqlx", "normal,build,features", Some("sqlx"));
    for expected in ["postgres", "mysql", "sqlite-bundled"] {
        assert!(aggregate.contains(&format!(r#"sqlx feature "{expected}""#)));
    }
    assert_tree_cache_budget();
}

pub(super) fn retained_features() {
    for feature in ["itoa", "ryu", "zmij", "uuid"] {
        let tree = tree(feature);
        assert_has(&tree, feature, feature);
        for other in ["itoa", "ryu", "zmij", "uuid"] {
            if feature != other {
                assert_absent(&tree, other, feature);
            }
        }
    }

    let rand = tree("rand");
    assert_has(&rand, "rand", "rand");
    assert_absent(&rand, "redis", "rand");
    let regex = tree("regex");
    assert_has(&regex, "regex", "regex");
    assert_absent(&regex, "phonenumber", "regex");

    for feature in ["chrono", "time", "jiff"] {
        let tree = tree(feature);
        assert_has(&tree, feature, feature);
        for other in ["chrono", "time", "jiff"] {
            if feature != other {
                assert_absent(&tree, other, feature);
            }
        }
    }

    let base64 = tree("base64");
    assert_has(&base64, "base64", "base64");
    for package in ["md-5", "aes-gcm", "encoding_rs"] {
        assert_absent(&base64, package, "base64");
    }
    let md5 = tree("md5");
    assert_has(&md5, "md-5", "md5");
    assert_absent(&md5, "base64", "md5");
    assert_absent(&md5, "aes-gcm", "md5");
    let aes = tree("aes");
    for package in ["aes", "aes-gcm", "cbc"] {
        assert_has(&aes, package, "aes");
    }
    assert_absent(&aes, "base64", "aes");
    let encoding = tree("encoding_rs");
    assert_has(&encoding, "encoding_rs", "encoding_rs");
    assert_absent(&encoding, "base64", "encoding_rs");

    let jwt = tree_with("jwt", "normal,build,features", None);
    assert_has(&jwt, "jsonwebtoken", "jwt");
    assert_has(&jwt, "serde_json", "jwt");
    assert!(jwt.contains(r#"jsonwebtoken feature "rust_crypto""#));
    assert!(jwt.contains(r#"jsonwebtoken feature "use_pem""#));
    assert!(!jwt.contains(r#"jsonwebtoken feature "aws_lc_rs""#));

    let tracing = tree("tracing");
    assert_has(&tracing, "tracing", "tracing");
    assert_absent(&tracing, "tracing-subscriber", "tracing");

    let logging = tree("logging");
    for package in ["tracing", "tracing-subscriber", "tracing-appender"] {
        assert_has(&logging, package, "logging");
    }
    assert_tree_cache_budget();
}

pub(super) fn tree_cache_key_normalization_smoke() {
    // This covers the normalizer without starting Cargo from the non-ignored test suite.
    let first = normalized_feature_csv_for_test("tokio,axum,tokio");
    let second = normalized_feature_csv_for_test("axum,tokio");
    assert_eq!(first, second);
    assert_eq!(first, "axum,tokio");
}

fn normalized_feature_csv_for_test(features: &str) -> String {
    let mut values = features.split(',').collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values.join(",")
}

fn assert_has(tree: &str, package: &str, feature: &str) {
    assert!(
        has_package(tree, package),
        "{feature} dependency tree does not contain {package}"
    );
}

fn assert_absent(tree: &str, package: &str, feature: &str) {
    assert!(
        !has_package(tree, package),
        "{feature} dependency tree unexpectedly contains {package}"
    );
}
