use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct TemporaryTarget(PathBuf);

impl Drop for TemporaryTarget {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn verifies_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("email_feature_matrix")
        .join("Cargo.toml");
    let target_dir = TemporaryTarget(env::temp_dir().join(format!(
        "axutils-email-feature-matrix-{}",
        std::process::id()
    )));

    for (feature, expected_success, diagnostic_token) in [
        ("", true, ""),
        ("tokio-only", true, ""),
        ("sync", true, ""),
        ("all", true, ""),
        ("negative-email-module", false, "email"),
        ("negative-email-client", false, "emailclient"),
        ("negative-email-utils", false, "emailutils"),
        ("negative-tokio-email-module", false, "email"),
        ("negative-tokio-email-client", false, "emailclient"),
        ("negative-tokio-email-utils", false, "emailutils"),
        ("negative-async", false, "send_async"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "fixture feature `{feature}` should compile successfully"
            );
        } else {
            assert!(
                !output.status.success(),
                "fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_dependency_boundaries();
}

#[test]
fn verifies_http_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("http_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-http-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("", true, ""),
        ("tokio-only", true, ""),
        ("http", true, ""),
        ("http-tokio", true, ""),
        ("serde-only", true, ""),
        ("http-serde", true, ""),
        ("http-tokio-serde", true, ""),
        ("negative-http-module", false, "http"),
        ("negative-http-client", false, "httpclient"),
        ("negative-http-utils", false, "httputils"),
        ("negative-http-tokio-module", false, "http"),
        ("negative-http-tokio-client", false, "httpclient"),
        ("negative-http-tokio-utils", false, "httputils"),
        ("negative-http-async", false, "execute_async"),
        ("negative-http-serde", false, "get"),
        ("negative-http-tokio-serde", false, "get_async"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "HTTP fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "HTTP fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_http_dependency_boundaries();
}

#[test]
fn verifies_format_template_feature_api_matrix() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("format_feature_matrix")
        .join("Cargo.toml");
    let target_dir = TemporaryTarget(env::temp_dir().join(format!(
        "axutils-format-feature-matrix-{}",
        std::process::id()
    )));

    for (feature, expected_success, diagnostic_token) in [
        ("format-serde-strfmt", true, ""),
        ("format-serde-minijinja", true, ""),
        ("format-serde-all", true, ""),
        ("negative-format-no-features", false, "templateengine"),
        ("negative-format-serde-only", false, "templateengine"),
        ("negative-format-strfmt-only", false, "templateengine"),
        ("negative-format-minijinja-only", false, "templateengine"),
        (
            "negative-format-serde-strfmt-missing-minijinja",
            false,
            "minijinja",
        ),
        (
            "negative-format-serde-minijinja-missing-strfmt",
            false,
            "strfmt",
        ),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "format fixture feature `{feature}` should compile successfully"
            );
        } else {
            assert!(
                !output.status.success(),
                "format fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }
}

#[test]
fn verifies_config_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("config_feature_matrix")
        .join("Cargo.toml");
    let target_dir = TemporaryTarget(env::temp_dir().join(format!(
        "axutils-config-feature-matrix-{}",
        std::process::id()
    )));

    for (feature, expected_success, diagnostic_token) in [
        ("", true, ""),
        ("tokio-only", true, ""),
        ("serde-only", true, ""),
        ("serde-toml", true, ""),
        ("serde-tokio", true, ""),
        ("all", true, ""),
        ("serde-tokio-all", true, ""),
        ("negative-config-module-no-serde", false, "config"),
        ("negative-config-utils-no-serde", false, "configutils"),
        ("negative-tokio-config-no-serde", false, "configutils"),
        ("negative-toml-only-no-serde", false, "configutils"),
        ("negative-config-async-no-tokio", false, "load_value_async"),
        ("negative-yaml-under-serde-only", false, "configformat"),
        ("negative-toml-under-serde-only", false, "configformat"),
        ("negative-ini-under-serde-only", false, "configformat"),
        ("negative-yaml-under-serde-tokio", false, "configformat"),
        ("negative-toml-under-serde-tokio", false, "configformat"),
        ("negative-ini-under-serde-tokio", false, "configformat"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "config fixture feature `{feature}` should compile successfully"
            );
        } else {
            assert!(
                !output.status.success(),
                "config fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_config_dependency_boundaries();
}

#[test]
fn verifies_crypto_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("crypto_feature_matrix")
        .join("Cargo.toml");
    let target_dir = TemporaryTarget(env::temp_dir().join(format!(
        "axutils-crypto-feature-matrix-{}",
        std::process::id()
    )));

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("encoding-rs-only", true, ""),
        ("base64-only", true, ""),
        ("md5-only", true, ""),
        ("aes-only", true, ""),
        ("base64-md5", true, ""),
        ("base64-aes", true, ""),
        ("base64-encoding-rs", true, ""),
        ("md5-aes", true, ""),
        ("md5-encoding-rs", true, ""),
        ("aes-encoding-rs", true, ""),
        ("base64-md5-aes", true, ""),
        ("base64-md5-encoding-rs", true, ""),
        ("base64-aes-encoding-rs", true, ""),
        ("md5-aes-encoding-rs", true, ""),
        ("all", true, ""),
        ("negative-none-base64", false, "base64_encode"),
        ("negative-none-md5", false, "md5"),
        ("negative-none-aes", false, "aeskey"),
        ("negative-none-aescipher", false, "aescipher"),
        ("negative-none-legacy-encoding", false, "gbk"),
        ("negative-encoding-rs-only-base64", false, "base64_encode"),
        ("negative-encoding-rs-only-md5", false, "md5"),
        ("negative-encoding-rs-only-aes", false, "aeskey"),
        ("negative-encoding-rs-only-aescipher", false, "aescipher"),
        ("negative-base64-only-md5", false, "md5"),
        ("negative-base64-only-aes", false, "aeskey"),
        ("negative-base64-only-aescipher", false, "aescipher"),
        ("negative-base64-only-legacy-encoding", false, "gbk"),
        ("negative-md5-only-base64", false, "base64_encode"),
        ("negative-md5-only-aes", false, "aeskey"),
        ("negative-md5-only-aescipher", false, "aescipher"),
        ("negative-md5-only-legacy-encoding", false, "gbk"),
        ("negative-aes-only-base64", false, "base64_encode"),
        ("negative-aes-only-md5", false, "md5"),
        ("negative-aes-only-legacy-encoding", false, "gbk"),
        (
            "negative-aes-only-aes-base64-combo",
            false,
            "aes_encrypt_base64",
        ),
        ("negative-aes-base64-md5", false, "md5"),
        ("negative-aes-base64-legacy-encoding", false, "gbk"),
        ("negative-base64-encoding-rs-md5", false, "md5"),
        ("negative-base64-encoding-rs-aes", false, "aeskey"),
        ("negative-base64-encoding-rs-aescipher", false, "aescipher"),
        ("negative-base64-md5-aes", false, "aeskey"),
        ("negative-base64-md5-aescipher", false, "aescipher"),
        ("negative-base64-md5-legacy-encoding", false, "gbk"),
        ("negative-base64-md5-encoding-rs", false, "aeskey"),
        (
            "negative-base64-md5-encoding-rs-aescipher",
            false,
            "aescipher",
        ),
        ("negative-md5-aes-base64-combo", false, "aes_encrypt_base64"),
        ("negative-md5-aes-legacy-encoding", false, "gbk"),
        ("negative-md5-aes-encoding-rs", false, "base64_encode"),
        (
            "negative-md5-aes-encoding-rs-base64-combo",
            false,
            "aes_encrypt_base64",
        ),
        ("negative-md5-encoding-rs-base64", false, "base64_encode"),
        ("negative-md5-encoding-rs-aes", false, "aeskey"),
        ("negative-md5-encoding-rs-aescipher", false, "aescipher"),
        (
            "negative-aes-encoding-rs-base64-combo",
            false,
            "aes_encrypt_base64",
        ),
        ("negative-aes-encoding-rs-md5", false, "md5"),
        ("negative-base64-md5-aes-encoding-rs", false, "gbk"),
        ("negative-base64-aes-encoding-rs-md5", false, "md5"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "crypto fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "crypto fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    let output = run_fixture(&fixture_manifest, &target_dir.0, "negative-none-aes-errors");
    assert!(
        !output.status.success(),
        "fixture feature `negative-none-aes-errors` should fail to compile"
    );
    assert_expected_diagnostics(
        &output,
        &["notinitialized", "alreadyinitialized"],
        "negative-none-aes-errors",
    );

    assert_crypto_dependency_boundaries();
}

#[test]
fn verifies_jwt_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("jwt_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-jwt-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("jwt-only", true, ""),
        ("serde-only", true, ""),
        ("jwt-serde", true, ""),
        ("jwt-lettre", true, ""),
        ("jwt-aes", true, ""),
        ("jwt-tokio", true, ""),
        ("jwt-regex", true, ""),
        ("all", true, ""),
        ("negative-none-jwt-module", false, "jwt"),
        ("negative-none-jwt-algorithm", false, "jwtalgorithm"),
        ("negative-none-jwt-signing-key", false, "jwtsigningkey"),
        (
            "negative-none-jwt-verification-key",
            false,
            "jwtverificationkey",
        ),
        ("negative-none-jwt-config", false, "jwtconfig"),
        ("negative-none-jwt-validation", false, "jwtvalidation"),
        ("negative-none-jwt-error", false, "jwterror"),
        ("negative-none-jwt-utils", false, "jwtutils"),
        ("negative-none-utils-jwt-utils", false, "jwtutils"),
        ("negative-none-direct-jwt-utils", false, "jwtutils"),
        ("negative-serde-only-jwt-module", false, "jwt"),
        ("negative-jwt-only-config", false, "config"),
        ("negative-jwt-only-config-loader", false, "configloader"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "JWT fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "JWT fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_jwt_dependency_boundaries();
}

fn assert_crypto_dependency_boundaries() {
    let base64_tree = cargo_tree("base64");
    assert!(has_package(&base64_tree, "base64"));
    assert!(!has_package(&base64_tree, "md-5"));
    assert!(!has_package(&base64_tree, "aes-gcm"));
    assert!(!has_package(&base64_tree, "cbc"));
    assert!(!has_package(&base64_tree, "zeroize"));
    assert!(!has_package(&base64_tree, "encoding_rs"));

    let md5_tree = cargo_tree("md5");
    assert!(has_package(&md5_tree, "md-5"));
    assert!(has_package(&md5_tree, "digest"));
    assert!(!has_package(&md5_tree, "base64"));
    assert!(!has_package(&md5_tree, "aes"));
    assert!(!has_package(&md5_tree, "aes-gcm"));
    assert!(!has_package(&md5_tree, "encoding_rs"));

    let aes_tree = cargo_tree("aes");
    assert!(has_package(&aes_tree, "aes"));
    assert!(has_package(&aes_tree, "aes-gcm"));
    assert!(has_package(&aes_tree, "cbc"));
    assert!(has_package(&aes_tree, "zeroize"));
    assert!(!has_package(&aes_tree, "base64"));
    assert!(!has_package(&aes_tree, "md-5"));
    assert!(!has_package(&aes_tree, "encoding_rs"));

    let encoding_rs_tree = cargo_tree("encoding_rs");
    assert!(has_package(&encoding_rs_tree, "encoding_rs"));
    assert!(!has_package(&encoding_rs_tree, "base64"));
    assert!(!has_package(&encoding_rs_tree, "md-5"));
    assert!(!has_package(&encoding_rs_tree, "aes"));

    for features in [
        "base64",
        "md5",
        "aes",
        "encoding_rs",
        "base64,md5,aes,encoding_rs",
    ] {
        assert_forbidden_tls_packages(&cargo_tree(features));
    }

    let base64_feature_tree = cargo_tree_with_edges("base64", "normal,build,features");
    assert!(!base64_feature_tree.contains("base64 feature \"simd-unsafe\""));
}

fn assert_config_dependency_boundaries() {
    let toml_only_tree = cargo_tree("toml");
    assert!(has_package(&toml_only_tree, "toml"));
    assert!(!has_package(&toml_only_tree, "serde-saphyr"));
    assert!(!has_package(&toml_only_tree, "rust-ini"));
    assert!(!has_package(&toml_only_tree, "serde_json"));

    let saphyr_only_tree = cargo_tree("serde-saphyr");
    assert!(has_package(&saphyr_only_tree, "serde-saphyr"));
    assert!(!has_package(&saphyr_only_tree, "toml"));
    assert!(!has_package(&saphyr_only_tree, "rust-ini"));

    let ini_only_tree = cargo_tree("rust-ini");
    assert!(has_package(&ini_only_tree, "rust-ini"));
    assert!(!has_package(&ini_only_tree, "toml"));
    assert!(!has_package(&ini_only_tree, "serde-saphyr"));

    let serde_only_tree = cargo_tree("serde");
    assert!(has_package(&serde_only_tree, "serde_json"));
    assert!(!has_package(&serde_only_tree, "toml"));
    assert!(!has_package(&serde_only_tree, "serde-saphyr"));
    assert!(!has_package(&serde_only_tree, "rust-ini"));

    let tokio_only_tree = cargo_tree("tokio");
    assert!(has_package(&tokio_only_tree, "tokio"));
    assert!(!has_package(&tokio_only_tree, "serde"));
    assert!(!has_package(&tokio_only_tree, "serde_json"));
    assert!(!has_package(&tokio_only_tree, "lettre"));

    let serde_tokio_tree = cargo_tree("serde,tokio");
    assert!(has_package(&serde_tokio_tree, "serde"));
    assert!(has_package(&serde_tokio_tree, "serde_json"));
    assert!(has_package(&serde_tokio_tree, "tokio"));
    assert!(!has_package(&serde_tokio_tree, "toml"));
    assert!(!has_package(&serde_tokio_tree, "serde-saphyr"));
    assert!(!has_package(&serde_tokio_tree, "rust-ini"));
    assert!(!has_package(&serde_tokio_tree, "lettre"));

    let tokio_feature_tree = cargo_tree_with_edges("tokio", "normal,build,features");
    assert!(tokio_feature_tree.contains("tokio feature \"fs\""));
    assert!(tokio_feature_tree.contains("tokio feature \"io-util\""));
}

fn run_fixture(manifest: &Path, target_dir: &Path, feature: &str) -> Output {
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--no-default-features")
        .arg("--offline")
        .env("CARGO_TERM_COLOR", "never");
    if !feature.is_empty() {
        command.arg("--features").arg(feature);
    }

    command
        .output()
        .unwrap_or_else(|_| panic!("failed to run cargo for fixture feature `{feature}`"))
}

fn unique_target(prefix: &str) -> TemporaryTarget {
    static TARGET_COUNTER: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..100 {
        let counter = TARGET_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("{prefix}-{}-{counter}", std::process::id()));
        if fs::create_dir(&path).is_ok() {
            return TemporaryTarget(path);
        }
    }
    panic!("failed to acquire an exclusive temporary target directory for `{prefix}`");
}

fn assert_expected_diagnostic(output: &Output, token: &str, feature: &str) {
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    assert!(
        diagnostics.contains("error") && diagnostics.contains(token),
        "fixture feature `{feature}` did not fail because the expected API was unavailable"
    );
}

fn assert_expected_diagnostics(output: &Output, tokens: &[&str], feature: &str) {
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    assert!(
        diagnostics.contains("error"),
        "fixture feature `{feature}` did not fail to compile"
    );
    for token in tokens {
        assert!(
            diagnostics.contains(token),
            "fixture feature `{feature}` diagnostic did not contain `{token}`"
        );
    }
}

fn assert_dependency_boundaries() {
    let tokio_tree = cargo_tree("tokio");
    assert!(has_package(&tokio_tree, "tokio"));
    assert!(!has_package(&tokio_tree, "lettre"));

    let lettre_tree = cargo_tree("lettre");
    assert!(has_package(&lettre_tree, "lettre"));
    assert!(!has_package(&lettre_tree, "tokio"));
    assert!(!has_package(&lettre_tree, "tokio-rustls"));
    assert!(has_package(&lettre_tree, "ring"));
    assert!(has_package(&lettre_tree, "webpki-roots"));
    assert_forbidden_tls_packages(&lettre_tree);

    let combined_tree = cargo_tree("lettre,tokio");
    assert!(has_package(&combined_tree, "lettre"));
    assert!(has_package(&combined_tree, "tokio"));
    assert!(has_package(&combined_tree, "tokio-rustls"));
    assert!(has_package(&combined_tree, "ring"));
    assert!(has_package(&combined_tree, "webpki-roots"));
    assert_forbidden_tls_packages(&combined_tree);
}

fn assert_http_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    assert!(!has_package(&no_feature_tree, "ureq"));
    assert!(!has_package(&no_feature_tree, "reqwest"));
    assert!(!has_package(&no_feature_tree, "url"));

    let tokio_tree = cargo_tree("tokio");
    assert!(has_package(&tokio_tree, "tokio"));
    assert!(!has_package(&tokio_tree, "ureq"));
    assert!(!has_package(&tokio_tree, "reqwest"));

    let http_tree = cargo_tree("http");
    assert!(has_package(&http_tree, "ureq"));
    assert!(has_package(&http_tree, "reqwest"));
    assert!(has_package(&http_tree, "url"));
    assert!(!has_package(&http_tree, "lettre"));
    assert_forbidden_tls_packages(&http_tree);

    let http_tokio_tree = cargo_tree("http,tokio");
    assert!(has_package(&http_tokio_tree, "ureq"));
    assert!(has_package(&http_tokio_tree, "reqwest"));
    assert!(has_package(&http_tokio_tree, "tokio"));
    assert!(!has_package(&http_tokio_tree, "lettre"));
    assert_forbidden_tls_packages(&http_tokio_tree);

    let http_serde_tree = cargo_tree("http,serde");
    assert!(has_package(&http_serde_tree, "ureq"));
    assert!(has_package(&http_serde_tree, "reqwest"));
    assert!(has_package(&http_serde_tree, "serde"));
    assert!(has_package(&http_serde_tree, "serde_json"));
    assert!(has_package(&http_serde_tree, "serde_urlencoded"));

    let http_feature_tree = cargo_tree_with_edges("http", "normal,build,features");
    assert!(http_feature_tree.contains("ureq feature \"rustls\""));
    assert!(http_feature_tree.contains("reqwest feature \"rustls-tls-webpki-roots\""));
    assert!(!http_feature_tree.contains("ureq feature \"gzip\""));
    assert!(!http_feature_tree.contains("reqwest feature \"native-tls\""));
}

fn assert_jwt_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    for package in [
        "jsonwebtoken",
        "rsa",
        "p256",
        "p384",
        "ed25519-dalek",
        "hmac",
    ] {
        assert!(!has_package(&no_feature_tree, package));
    }

    let jwt_tree = cargo_tree("jwt");
    assert!(has_package(&jwt_tree, "jsonwebtoken"));
    assert!(has_package(&jwt_tree, "rsa"));
    assert!(has_package(&jwt_tree, "p256"));
    assert!(has_package(&jwt_tree, "p384"));
    assert!(has_package(&jwt_tree, "ed25519-dalek"));
    assert!(has_package(&jwt_tree, "hmac"));
    for package in [
        "lettre",
        "aes",
        "aes-gcm",
        "cbc",
        "regex",
        "toml",
        "serde-saphyr",
        "rust-ini",
        "chrono",
        "jiff",
        "strfmt",
        "minijinja",
        "phonenumber",
        "encoding_rs",
        "md-5",
    ] {
        assert!(!has_package(&jwt_tree, package));
    }
    assert_forbidden_tls_packages(&jwt_tree);

    let jwt_feature_tree = cargo_tree_with_edges("jwt", "normal,build,features");
    assert!(jwt_feature_tree.contains("jsonwebtoken feature \"rust_crypto\""));
    assert!(jwt_feature_tree.contains("jsonwebtoken feature \"use_pem\""));
    assert!(!jwt_feature_tree.contains("jsonwebtoken feature \"aws_lc_rs\""));
}

fn cargo_tree(features: &str) -> String {
    cargo_tree_with_edges(features, "normal,build")
}

fn cargo_tree_with_edges(features: &str, edges: &str) -> String {
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .arg("--no-default-features")
        .arg("--features")
        .arg(features)
        .arg("--offline")
        .arg("--edges")
        .arg(edges)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|_| panic!("failed to run cargo tree for feature `{features}`"));
    assert!(
        output.status.success(),
        "cargo tree failed for feature `{features}`"
    );
    String::from_utf8(output.stdout).expect("cargo tree output should be UTF-8")
}

fn has_package(tree: &str, package: &str) -> bool {
    tree.lines()
        .any(|line| line.contains(&format!("{package} v")))
}

fn assert_forbidden_tls_packages(tree: &str) {
    for package in [
        "native-tls",
        "openssl",
        "openssl-sys",
        "aws-lc-rs",
        "aws-lc-sys",
        "rustls-native-certs",
        "rustls-platform-verifier",
        "hostname",
    ] {
        assert!(
            !has_package(tree, package),
            "forbidden TLS package `{package}` is present"
        );
    }
}
