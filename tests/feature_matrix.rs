use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
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
        ("serde-only", true, ""),
        ("serde-toml", true, ""),
        ("all", true, ""),
        ("negative-config-module-no-serde", false, "config"),
        ("negative-config-utils-no-serde", false, "configutils"),
        ("negative-toml-only-no-serde", false, "configutils"),
        ("negative-yaml-under-serde-only", false, "configformat"),
        ("negative-toml-under-serde-only", false, "configformat"),
        ("negative-ini-under-serde-only", false, "configformat"),
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

fn cargo_tree(features: &str) -> String {
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .arg("--no-default-features")
        .arg("--features")
        .arg(features)
        .arg("--offline")
        .arg("--edges")
        .arg("normal,build")
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
