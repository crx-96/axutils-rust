use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct TemporaryTarget(PathBuf);

struct FsMatrixTarget(Option<PathBuf>);

impl FsMatrixTarget {
    fn path(&self) -> &Path {
        self.0.as_deref().expect("Fs matrix target is active")
    }

    fn cleanup(mut self) {
        let path = self.0.take().expect("Fs matrix target is active");
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!(
                "Fs feature matrix cleanup failed for {}: {error}",
                path.display()
            ),
        }
    }
}

impl Drop for FsMatrixTarget {
    fn drop(&mut self) {
        let Some(path) = self.0.take() else {
            return;
        };
        if let Err(error) = fs::remove_dir_all(&path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!(
                    "Fs feature matrix cleanup failed for {}: {error}",
                    path.display()
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
enum FixtureAction {
    Check,
    Build,
    Run,
}

impl Drop for TemporaryTarget {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "慢速 scheduler feature/API 与依赖契约矩阵"]
fn verifies_scheduler_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/scheduler_feature_matrix/Cargo.toml");
    let target_dir = unique_target("axutils-scheduler-feature-matrix");

    for feature in [
        "none",
        "chrono",
        "chrono-tz",
        "tokio",
        "croner",
        "chrono-chrono-tz",
        "chrono-tokio",
        "chrono-croner",
        "chrono-tz-tokio",
        "chrono-tz-croner",
        "tokio-croner",
        "chrono-chrono-tz-tokio",
        "chrono-chrono-tz-croner",
        "chrono-tokio-croner",
        "chrono-tz-tokio-croner",
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        assert!(
            !output.status.success(),
            "incomplete scheduler fixture `{feature}` unexpectedly compiled"
        );
        assert_expected_diagnostic(&output, "Scheduler", feature);
    }

    let output = run_fixture(&fixture_manifest, &target_dir.0, "all");
    assert!(
        output.status.success(),
        "complete scheduler fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_fixture(
        &fixture_manifest,
        &target_dir.0,
        "negative-root-module-alias",
    );
    assert!(
        !output.status.success(),
        "scheduler_utils root alias exists"
    );
    assert_expected_diagnostic(&output, "scheduler_utils", "negative-root-module-alias");

    assert_scheduler_dependency_boundaries();
}

#[test]
#[ignore = "慢速 Axum/Tokio feature/API 契约矩阵"]
fn verifies_axum_tokio_feature_api_matrix() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/axum_tokio_feature_matrix/Cargo.toml");
    let target_dir = unique_target("axutils-axum-tokio-feature-matrix");
    for (feature, expected, token) in [
        ("none", true, ""),
        ("tokio-only", true, ""),
        ("axum-only", true, ""),
        ("core", true, ""),
        ("task-group", true, ""),
        ("tower", true, ""),
        ("tower-http", true, ""),
        ("governor", true, ""),
        ("providers-only", true, ""),
        ("provider-tower-only", true, ""),
        ("provider-tower-http-only", true, ""),
        ("provider-governor-only", true, ""),
        ("provider-tokio-util-only", true, ""),
        ("tracing-positive", true, ""),
        ("negative-no-tokio-axum-server", false, "axumserver"),
        ("negative-no-axum-server", false, "axumserver"),
        ("negative-no-task-group", false, "tokiotaskgroup"),
        ("negative-no-tower-method", false, "with_concurrency_limit"),
        ("negative-no-tower-http-method", false, "with_body_limit"),
        ("negative-no-tracing-method", false, "with_http_trace"),
        ("negative-no-governor-method", false, "with_governor_peer"),
        ("negative-no-tokio-root", false, "tokioconfig"),
        ("negative-no-tokio-module", false, "tokio"),
        ("negative-no-tokio-utils", false, "tokioutils"),
        ("negative-no-tokio-utils-module", false, "tokio_utils"),
        ("negative-no-tokio-axum-module", false, "axum"),
        ("negative-no-tokio-axum-utils", false, "axumutils"),
        ("negative-no-tokio-axum-utils-module", false, "axum_utils"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        assert_eq!(
            output.status.success(),
            expected,
            "fixture {feature} unexpected status: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if !expected {
            assert_expected_diagnostic(&output, token, feature);
        }
    }

    let none = cargo_tree_strict("");
    for package in [
        "axum",
        "tokio",
        "tokio-util",
        "futures-timer",
        "tower-http",
        "tower_governor",
    ] {
        assert!(
            !has_package(&none, package),
            "default tree contains {package}"
        );
    }
    let tokio = cargo_tree_strict("tokio");
    assert!(has_package(&tokio, "tokio"));
    for package in ["axum", "tower-http", "tower_governor", "futures-timer"] {
        assert!(
            !has_package(&tokio, package),
            "tokio-only tree contains {package}"
        );
    }
    let providers = cargo_tree_strict("tower,tower-http,tower_governor,tokio-util");
    for package in [
        "tower",
        "tower-http",
        "tower_governor",
        "tokio-util",
        "futures-timer",
    ] {
        assert!(
            has_package(&providers, package),
            "provider tree misses {package}"
        );
    }
    assert!(
        !has_package(&providers, "axum"),
        "providers-only must not pull Axum"
    );

    let core = cargo_tree_with_edges_strict("axum,tokio", "normal,build,features");
    for package in ["axum", "tokio", "tower"] {
        assert!(has_package(&core, package));
    }
    assert!(!has_package(&core, "tower-http"));
    assert!(!has_package(&core, "tower_governor"));
    assert!(!core.contains(r#"axutils feature "tower""#));

    let governor =
        cargo_tree_with_edges_strict("axum,tokio,tower_governor", "normal,build,features");
    for expanded in [
        r#"axum feature "default""#,
        r#"axum feature "form""#,
        r#"axum feature "json""#,
        r#"axum feature "query""#,
        r#"tokio feature "macros""#,
    ] {
        assert!(
            governor.contains(expanded),
            "governor/axum expansion missing {expanded}"
        );
    }
    for forbidden in ["native-tls", "openssl", "openssl-sys"] {
        assert!(
            !has_package(&governor, forbidden),
            "governor tree contains {forbidden}"
        );
    }
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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
        ("negative-email-module", false, "emailclient"),
        ("negative-email-client", false, "emailclient"),
        ("negative-email-utils", false, "emailutils"),
        ("negative-tokio-email-module", false, "emailclient"),
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
#[ignore = "慢速 FsUtils feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_fs_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fs_feature_matrix")
        .join("Cargo.toml");
    let target_dir = strict_fs_target("axutils-fs-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("", true, ""),
        ("serde-only", true, ""),
        ("tokio-only", true, ""),
        ("serde-tokio", true, ""),
        ("tempfile-only", true, ""),
        ("tokio-tempfile", true, ""),
        ("tempfile-async", true, ""),
        ("tempfile-both", true, ""),
        ("all", true, ""),
        ("negative-tokio-no-tempfile", false, "create_temp_file"),
        ("negative-no-tempfile-sync", false, "create_temp_file"),
        (
            "negative-no-tempfile-async",
            false,
            "create_temp_file_async",
        ),
        (
            "negative-tempfile-only-async",
            false,
            "create_temp_file_async",
        ),
        ("negative-tempfile-async-sync", false, "create_temp_file"),
        ("negative-no-domain-fs-utils", false, "FsUtils"),
        ("negative-no-domain-fs-operation", false, "read_bytes"),
        ("negative-no-domain-fs-transfer", false, "copy_file_with"),
        ("negative-no-utils-fs-transfer", false, "FsTransferOptions"),
        ("negative-no-utils-fs-temp", false, "FsTempConfig"),
        ("negative-no-root-fs-utils-module", false, "fs_utils"),
        ("negative-no-utils-fs-error", false, "FsError"),
        ("negative-no-nested-fs-error", false, "FsError"),
    ] {
        let output = run_fs_fixture(&fixture_manifest, target_dir.path(), feature);
        if expected_success {
            assert!(
                output.status.success(),
                "FsUtils fixture feature `{feature}` should compile successfully. stdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "FsUtils fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    let no_tokio = run_fs_fixture(
        &fixture_manifest,
        target_dir.path(),
        "negative-no-tokio-async",
    );
    assert!(
        !no_tokio.status.success(),
        "FsUtils async methods should not compile without tokio"
    );
    assert_expected_diagnostics(
        &no_tokio,
        &[
            "try_exists_async",
            "is_file_async",
            "is_dir_async",
            "metadata_async",
            "symlink_metadata_async",
            "create_file_async",
            "create_dir_async",
            "create_dir_all_async",
            "list_dir_async",
            "remove_file_async",
            "remove_dir_async",
            "remove_dir_all_async",
            "move_path_async",
            "copy_file_async",
            "copy_file_with_async",
            "read_bytes_async",
            "read_to_string_async",
            "write_async",
            "append_async",
        ],
        "negative-no-tokio-async",
    );

    assert_fs_dependency_boundaries();
    target_dir.cleanup();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_tracing_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("tracing_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-tracing-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("tracing", true, ""),
        ("logging", true, ""),
        ("direct-tracing", true, ""),
        ("negative-none-root", false, "logutils"),
        ("negative-none-config", false, "logconfig"),
        ("negative-tracing-root", false, "logutils"),
        ("negative-tracing-config", false, "logconfig"),
        ("negative-tracing-utils", false, "logutils"),
        ("negative-tracing-module", false, "logutils"),
        ("negative-no-root-module", false, "log_utils"),
    ] {
        let action = if expected_success {
            FixtureAction::Run
        } else {
            FixtureAction::Check
        };
        let output = run_fixture_action(&fixture_manifest, &target_dir.0, feature, action);
        if expected_success {
            assert!(
                output.status.success(),
                "tracing fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "tracing fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_tracing_dependency_boundaries();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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
        ("negative-http-module", false, "httpclient"),
        ("negative-http-client", false, "httpclient"),
        ("negative-http-utils", false, "httputils"),
        ("negative-http-tokio-module", false, "httpclient"),
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
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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

    assert_format_dependency_boundaries();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_time_backend_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("time_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-time-feature-matrix");

    for feature in [
        "none",
        "chrono-only",
        "time-only",
        "jiff-only",
        "chrono-time",
        "chrono-jiff",
        "time-jiff",
        "all",
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        assert!(
            output.status.success(),
            "time fixture feature `{feature}` should compile successfully: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for feature in [
        "negative-chrono-time-alias",
        "negative-chrono-jiff-alias",
        "negative-time-jiff-alias",
        "negative-all-alias",
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        assert!(
            !output.status.success(),
            "time fixture feature `{feature}` should fail to compile"
        );
        assert_expected_diagnostics(
            &output,
            &[
                "format_date",
                "format_option_date",
                "format_datetime",
                "format_option_datetime",
                "format_datetime_with_offset",
                "format_option_datetime_with_offset",
            ],
            feature,
        );
    }

    assert_time_dependency_boundaries();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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
        ("serde-yaml", true, ""),
        ("serde-tokio-yaml", true, ""),
        ("all", true, ""),
        ("serde-tokio-all", true, ""),
        ("negative-config-module-no-serde", false, "configloader"),
        ("negative-config-utils-no-serde", false, "configutils"),
        ("negative-tokio-config-no-serde", false, "configutils"),
        ("negative-toml-only-no-serde", false, "configutils"),
        ("negative-yaml-only-no-serde", false, "configformat"),
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
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
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
        ("negative-none-jwt-module", false, "jwtalgorithm"),
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
        ("negative-serde-only-jwt-module", false, "jwtalgorithm"),
        ("negative-jwt-only-config", false, "configloader"),
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

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_redis_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("redis_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-redis-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("tokio-only", true, ""),
        ("redis", true, ""),
        ("redis-tokio", true, ""),
        ("redis-serde", true, ""),
        ("redis-tokio-serde", true, ""),
        ("all", true, ""),
        ("negative-no-redis-module", false, "redisclient"),
        ("negative-no-redis-root", false, "redisclient"),
        ("negative-no-redis-utils", false, "redisutils"),
        ("negative-tokio-redis-module", false, "redisclient"),
        ("negative-tokio-redis-root", false, "redisclient"),
        ("negative-tokio-redis-utils", false, "redisutils"),
        ("negative-redis-async", false, "get_async"),
        ("negative-redis-async-lock", false, "redisasynclockguard"),
        ("negative-redis-utils-async", false, "get_async"),
        ("negative-redis-config", false, "configloader"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "Redis fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "Redis fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_redis_dependency_boundaries();
}

#[test]
#[ignore = "慢速 rand/Redis feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_rand_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rand_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-rand-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("rand", true, ""),
        ("rand-redis", true, ""),
        ("redis", true, ""),
        ("redis-tokio", true, ""),
        ("negative-no-rand-root", false, "randomutils"),
        ("negative-no-rand-module", false, "randomutils"),
        ("negative-no-rand-utils", false, "randomutils"),
        ("negative-redis-random-utils", false, "randomutils"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "rand fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "rand fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_rand_dependency_boundaries();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_sqlx_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sqlx_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-sqlx-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("tokio-only", true, ""),
        ("sqlx-only", true, ""),
        ("sqlx-tokio", true, ""),
        ("negative-no-sqlx-module", false, "sqlxclient"),
        ("negative-no-sqlx-root", false, "sqlxclient"),
        ("negative-no-sqlx-utils", false, "sqlxutils"),
        ("negative-sqlx-only-module", false, "sqlxclient"),
        ("negative-sqlx-only-root", false, "sqlxclient"),
        ("negative-sqlx-only-utils", false, "sqlxutils"),
        ("negative-sqlx-only-async", false, "sqlxclient"),
        ("negative-tokio-module", false, "sqlxclient"),
        ("negative-tokio-root", false, "sqlxclient"),
        ("negative-tokio-utils", false, "sqlxutils"),
        ("negative-tokio-async", false, "sqlxutils"),
        ("negative-dynamic-sql", false, "sqlsafestr"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "SQLx fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "SQLx fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_sqlx_dependency_boundaries();
}

#[test]
#[ignore = "慢速 feature/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_convert_feature_api_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("convert_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-convert-feature-matrix");

    for (feature, expected_success, diagnostic_token) in [
        ("none", true, ""),
        ("itoa-only", true, ""),
        ("ryu-only", true, ""),
        ("zmij-only", true, ""),
        ("uuid-only", true, ""),
        ("itoa-ryu", true, ""),
        ("itoa-zmij", true, ""),
        ("itoa-uuid", true, ""),
        ("ryu-zmij", true, ""),
        ("ryu-uuid", true, ""),
        ("zmij-uuid", true, ""),
        ("itoa-ryu-zmij", true, ""),
        ("itoa-ryu-uuid", true, ""),
        ("itoa-zmij-uuid", true, ""),
        ("ryu-zmij-uuid", true, ""),
        ("all", true, ""),
        ("negative-no-itoa-integer", false, "integer_to_string"),
        ("negative-no-float", false, "float_to_string"),
        ("negative-no-uuid", false, "string_to_uuid"),
        ("negative-no-ryu-variant", false, "ryu"),
        ("negative-no-zmij-variant", false, "zmij"),
        ("negative-float-default", false, "default"),
        ("negative-float-suffix", false, "float_to_string_ryu"),
        ("negative-integer-custom", false, "custom"),
        ("negative-float-custom", false, "custom"),
        ("negative-integer-sealed", false, "integer"),
        ("negative-float-sealed", false, "float"),
        ("negative-utils-domain-types", false, "IntegerBuffer"),
    ] {
        let output = run_fixture(&fixture_manifest, &target_dir.0, feature);
        if expected_success {
            assert!(
                output.status.success(),
                "ConvertUtils fixture feature `{feature}` should compile successfully: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            assert!(
                !output.status.success(),
                "ConvertUtils fixture feature `{feature}` should fail to compile"
            );
            assert_expected_diagnostic(&output, diagnostic_token, feature);
        }
    }

    assert_convert_dependency_boundaries();
}

#[test]
#[ignore = "慢速 allocator/依赖契约矩阵；使用 cargo test --no-default-features --test feature_matrix -- --ignored 执行"]
fn verifies_allocator_feature_matrix_and_dependency_boundaries() {
    let fixture_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("allocator_feature_matrix")
        .join("Cargo.toml");
    let target_dir = unique_target("axutils-allocator-feature-matrix");

    for feature in [
        "none",
        "mimalloc",
        "rpmalloc",
        "mimalloc-serde",
        "rpmalloc-serde",
    ] {
        let output = run_fixture_action(
            &fixture_manifest,
            &target_dir.0,
            feature,
            FixtureAction::Run,
        );
        assert!(
            output.status.success(),
            "allocator fixture feature `{feature}` should link and run successfully: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("axutils_allocator_fixture_ok"),
            "allocator fixture feature `{feature}` did not print its success sentinel"
        );
    }

    let conflict = run_fixture_action(
        &fixture_manifest,
        &target_dir.0,
        "mimalloc-rpmalloc",
        FixtureAction::Build,
    );
    assert!(
        !conflict.status.success(),
        "mimalloc + rpmalloc should fail during fixture build"
    );
    assert_output_contains(
        &conflict,
        &["axutils_allocator_conflict", "mimalloc", "rpmalloc"],
        "mimalloc-rpmalloc",
    );

    for feature in ["mimalloc-downstream-system", "rpmalloc-downstream-system"] {
        let output = run_fixture_action(
            &fixture_manifest,
            &target_dir.0,
            feature,
            FixtureAction::Build,
        );
        assert!(
            !output.status.success(),
            "fixture feature `{feature}` should reject a second downstream global allocator"
        );
    }

    assert_allocator_dependency_boundaries();
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

    let serde_saphyr_tree = cargo_tree("serde,serde-saphyr");
    assert!(has_package(&serde_saphyr_tree, "serde"));
    assert!(has_package(&serde_saphyr_tree, "serde_json"));
    assert!(has_package(&serde_saphyr_tree, "serde-saphyr"));
    assert!(!has_package(&serde_saphyr_tree, "toml"));
    assert!(!has_package(&serde_saphyr_tree, "rust-ini"));

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

    let serde_tokio_saphyr_tree = cargo_tree("serde,serde-saphyr,tokio");
    assert!(has_package(&serde_tokio_saphyr_tree, "serde-saphyr"));
    assert!(has_package(&serde_tokio_saphyr_tree, "tokio"));
    assert!(!has_package(&serde_tokio_saphyr_tree, "toml"));
    assert!(!has_package(&serde_tokio_saphyr_tree, "rust-ini"));

    let tokio_feature_tree = cargo_feature_tree_inverted("tokio", "tokio");
    assert!(tokio_feature_tree.contains("tokio feature \"fs\""));
    assert!(tokio_feature_tree.contains("tokio feature \"io-util\""));
}

fn assert_format_dependency_boundaries() {
    let minijinja_tree = cargo_tree("serde,minijinja");
    assert!(has_package(&minijinja_tree, "serde"));
    assert!(has_package(&minijinja_tree, "serde_json"));
    assert!(has_package(&minijinja_tree, "minijinja"));
    assert!(!has_package(&minijinja_tree, "strfmt"));

    let strfmt_tree = cargo_tree("serde,strfmt");
    assert!(has_package(&strfmt_tree, "serde"));
    assert!(has_package(&strfmt_tree, "serde_json"));
    assert!(has_package(&strfmt_tree, "strfmt"));
    assert!(!has_package(&strfmt_tree, "minijinja"));
}

fn run_fixture(manifest: &Path, target_dir: &Path, feature: &str) -> Output {
    run_fixture_action(manifest, target_dir, feature, FixtureAction::Check)
}

fn run_fixture_action(
    manifest: &Path,
    target_dir: &Path,
    feature: &str,
    action: FixtureAction,
) -> Output {
    let (_fixture, temporary_manifest) = copy_fixture_to_temporary_directory(manifest);
    let mut command = Command::new("cargo");
    let cargo_action = match action {
        FixtureAction::Check => "check",
        FixtureAction::Build => "build",
        FixtureAction::Run => "run",
    };
    command
        .arg(cargo_action)
        .arg("--manifest-path")
        .arg(&temporary_manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--no-default-features")
        .arg("--offline")
        .env("CARGO_TERM_COLOR", "never");
    match action {
        FixtureAction::Check | FixtureAction::Build => {
            command.arg("--message-format=json");
        }
        FixtureAction::Run => {
            command.arg("--quiet");
        }
    }
    if !feature.is_empty() {
        command.arg("--features").arg(feature);
    }

    command
        .output()
        .unwrap_or_else(|_| panic!("failed to run cargo for fixture feature `{feature}`"))
}

fn run_fs_fixture(manifest: &Path, target_dir: &Path, feature: &str) -> Output {
    let (fixture, temporary_manifest) = copy_fs_fixture_to_temporary_directory(manifest);
    let mut command = Command::new("cargo");
    command
        .arg("check")
        .arg("--manifest-path")
        .arg(&temporary_manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--no-default-features")
        .arg("--offline")
        .arg("--message-format=json")
        .env("CARGO_TERM_COLOR", "never");
    if !feature.is_empty() {
        command.arg("--features").arg(feature);
    }
    let output = command
        .output()
        .unwrap_or_else(|_| panic!("failed to run FsUtils fixture feature `{feature}`"));
    fixture.cleanup();
    output
}

fn copy_fs_fixture_to_temporary_directory(manifest: &Path) -> (FsMatrixTarget, PathBuf) {
    let source = manifest
        .parent()
        .unwrap_or_else(|| panic!("fixture manifest has no parent: {}", manifest.display()));
    let destination = strict_fs_target("axutils-fs-feature-fixture");
    copy_fixture_directory(source, destination.path());

    let copied_manifest = destination.path().join("Cargo.toml");
    let manifest_text = fs::read_to_string(&copied_manifest).unwrap_or_else(|_| {
        panic!(
            "failed to read copied FsUtils fixture: {}",
            copied_manifest.display()
        )
    });
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let manifest_text =
        manifest_text.replace("path = \"../../..\"", &format!("path = \"{repository}\""));
    fs::write(&copied_manifest, manifest_text).unwrap_or_else(|_| {
        panic!(
            "failed to write temporary FsUtils fixture: {}",
            copied_manifest.display()
        )
    });

    (destination, copied_manifest)
}

fn copy_fixture_to_temporary_directory(manifest: &Path) -> (TemporaryTarget, PathBuf) {
    let source = manifest
        .parent()
        .unwrap_or_else(|| panic!("fixture manifest has no parent: {}", manifest.display()));
    let destination = unique_target("axutils-feature-fixture");
    copy_fixture_directory(source, &destination.0);

    let copied_manifest = destination.0.join("Cargo.toml");
    let manifest_text = fs::read_to_string(&copied_manifest).unwrap_or_else(|_| {
        panic!(
            "failed to read copied fixture: {}",
            copied_manifest.display()
        )
    });
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let manifest_text =
        manifest_text.replace("path = \"../../..\"", &format!("path = \"{repository}\""));
    fs::write(&copied_manifest, manifest_text).unwrap_or_else(|_| {
        panic!(
            "failed to write temporary fixture: {}",
            copied_manifest.display()
        )
    });

    (destination, copied_manifest)
}

fn copy_fixture_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap_or_else(|_| {
        panic!(
            "failed to create temporary fixture directory: {}",
            destination.display()
        )
    });
    for entry in fs::read_dir(source)
        .unwrap_or_else(|_| panic!("failed to read fixture directory: {}", source.display()))
    {
        let entry = entry.expect("failed to read fixture entry");
        let name = entry.file_name();
        if name == "Cargo.lock" || name == "target" {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        if entry.file_type().expect("fixture entry type").is_dir() {
            copy_fixture_directory(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap_or_else(|_| {
                panic!(
                    "failed to copy fixture file {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            });
        }
    }
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

fn strict_fs_target(prefix: &str) -> FsMatrixTarget {
    static FS_TARGET_COUNTER: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..100 {
        let counter = FS_TARGET_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("{prefix}-{}-{counter}", std::process::id()));
        if fs::create_dir(&path).is_ok() {
            return FsMatrixTarget(Some(path));
        }
    }
    panic!("failed to acquire an exclusive FsUtils feature matrix target");
}

fn assert_expected_diagnostic(output: &Output, token: &str, feature: &str) {
    let diagnostics = rust_error_diagnostics(output);
    let token = normalize_diagnostic(token);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| normalize_diagnostic(diagnostic).contains(&token)),
        "fixture feature `{feature}` did not produce a target Rust API diagnostic for `{token}`"
    );
}

fn assert_expected_diagnostics(output: &Output, tokens: &[&str], feature: &str) {
    let diagnostics = rust_error_diagnostics(output);
    assert!(
        !diagnostics.is_empty(),
        "fixture feature `{feature}` did not emit a supported Rust compiler error"
    );
    for token in tokens {
        let normalized_token = normalize_diagnostic(token);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| { normalize_diagnostic(diagnostic).contains(&normalized_token) }),
            "fixture feature `{feature}` diagnostic did not contain `{token}`"
        );
    }
}

fn assert_output_contains(output: &Output, tokens: &[&str], feature: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostics = format!("{stdout}\n{stderr}");
    for token in tokens {
        assert!(
            diagnostics.contains(token),
            "fixture feature `{feature}` diagnostics did not contain `{token}`\n{diagnostics}"
        );
    }
}

fn rust_error_diagnostics(output: &Output) -> Vec<&str> {
    const EXPECTED_API_ERROR_CODES: [&str; 7] = [
        "E0277", "E0412", "E0425", "E0432", "E0433", "E0599", "E0603",
    ];
    let stdout = std::str::from_utf8(&output.stdout).expect("cargo JSON output should be UTF-8");
    stdout
        .lines()
        .filter_map(|line| {
            let is_target_api_error = line.contains(r#""reason":"compiler-message""#)
                && line.contains(r#""level":"error""#)
                && EXPECTED_API_ERROR_CODES
                    .iter()
                    .any(|code| line.contains(&format!(r#""code":"{code}""#)));
            is_target_api_error
                .then(|| line.find(r#""message":{"#))
                .flatten()
                .map(|message_start| &line[message_start..])
        })
        .collect()
}

fn normalize_diagnostic(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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

fn assert_fs_dependency_boundaries() {
    let no_feature_tree = cargo_tree_strict("");
    assert!(!has_package(&no_feature_tree, "tokio"));
    assert!(!has_package(&no_feature_tree, "serde"));
    assert!(!has_package(&no_feature_tree, "serde_json"));
    assert_fs_no_unrelated_packages(&no_feature_tree, &[], "no-feature");

    let tokio_tree = cargo_tree_strict("tokio");
    assert!(has_package(&tokio_tree, "tokio"));
    assert!(!has_package(&tokio_tree, "serde"));
    assert!(!has_package(&tokio_tree, "serde_json"));
    assert_fs_no_unrelated_packages(&tokio_tree, &["tokio"], "tokio-only");

    let tempfile_tree = cargo_tree_strict("tempfile");
    assert!(has_package(&tempfile_tree, "tempfile"));
    assert!(!has_package(&tempfile_tree, "async-tempfile"));
    assert!(!has_package(&tempfile_tree, "tokio"));
    assert_fs_no_unrelated_packages(&tempfile_tree, &["tempfile"], "tempfile-only");
    let tempfile_features = cargo_tree_with_edges_strict("tempfile", "normal,build,features");
    assert!(!tempfile_features.contains("tempfile feature \"getrandom\""));

    let tokio_tempfile_tree = cargo_tree_strict("tokio,tempfile");
    assert!(has_package(&tokio_tempfile_tree, "tokio"));
    assert!(has_package(&tokio_tempfile_tree, "tempfile"));
    assert!(!has_package(&tokio_tempfile_tree, "async-tempfile"));
    assert_fs_no_unrelated_packages(
        &tokio_tempfile_tree,
        &["tokio", "tempfile"],
        "tokio-tempfile",
    );

    let async_tempfile_tree = cargo_tree_strict("tempfile-async");
    assert!(has_package(&async_tempfile_tree, "async-tempfile"));
    assert!(has_package(&async_tempfile_tree, "tokio"));
    assert!(!has_package(&async_tempfile_tree, "tempfile"));
    assert_fs_no_unrelated_packages(
        &async_tempfile_tree,
        &["async-tempfile", "tokio"],
        "tempfile-async",
    );
    let async_tempfile_features =
        cargo_tree_with_edges_strict("tempfile-async", "normal,build,features");
    assert!(!async_tempfile_features.contains("async-tempfile feature \"uuid\""));

    let serde_tree = cargo_tree_strict("serde");
    assert!(has_package(&serde_tree, "serde"));
    assert!(has_package(&serde_tree, "serde_json"));
    assert!(has_package(&serde_tree, "serde_urlencoded"));
    assert!(!has_package(&serde_tree, "tokio"));
    assert_fs_no_unrelated_packages(
        &serde_tree,
        &[
            "serde",
            "serde_json",
            "serde_urlencoded",
            "itoa",
            "ryu",
            "zmij",
        ],
        "serde-only",
    );

    let serde_tokio_tree = cargo_tree_strict("serde,tokio");
    for package in ["serde", "serde_json", "serde_urlencoded", "tokio"] {
        assert!(
            has_package(&serde_tokio_tree, package),
            "FsUtils serde-tokio tree is missing `{package}`"
        );
    }
    assert_fs_no_unrelated_packages(
        &serde_tokio_tree,
        &[
            "serde",
            "serde_json",
            "serde_urlencoded",
            "itoa",
            "ryu",
            "zmij",
            "tokio",
        ],
        "serde-tokio",
    );

    let tokio_feature_tree = cargo_feature_tree_inverted("tokio", "tokio");
    let actual_tokio_features = tokio_feature_tree
        .lines()
        .filter_map(|line| {
            let marker = "tokio feature \"";
            let start = line.find(marker)? + marker.len();
            let end = line[start..].find('\"')? + start;
            Some(line[start..end].to_owned())
        })
        .collect::<BTreeSet<_>>();
    let mut expected_tokio_features = [
        "bytes",
        "fs",
        "io-util",
        "libc",
        "mio",
        "net",
        "rt",
        "rt-multi-thread",
        "signal",
        "signal-hook-registry",
        "socket2",
        "sync",
        "time",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    #[cfg(windows)]
    expected_tokio_features.insert("windows-sys".to_owned());
    assert_eq!(
        actual_tokio_features, expected_tokio_features,
        "FsUtils Tokio production feature set changed"
    );
}

fn assert_fs_no_unrelated_packages(tree: &str, allowed: &[&str], feature_set: &str) {
    for package in [
        "itoa",
        "ryu",
        "zmij",
        "uuid",
        "jsonwebtoken",
        "rand",
        "regex",
        "phonenumber",
        "serde",
        "serde_json",
        "serde_urlencoded",
        "strfmt",
        "minijinja",
        "chrono",
        "time",
        "jiff",
        "lettre",
        "ureq",
        "reqwest",
        "url",
        "redis",
        "sqlx",
        "futures-util",
        "r2d2",
        "rmp-serde",
        "tokio",
        "toml",
        "serde-saphyr",
        "rust-ini",
        "base64",
        "md-5",
        "aes",
        "aes-gcm",
        "cbc",
        "zeroize",
        "encoding_rs",
        "mimalloc",
        "rpmalloc",
        "tracing",
        "tracing-subscriber",
        "tracing-appender",
        "tempfile",
        "async-tempfile",
    ] {
        if allowed.contains(&package) {
            continue;
        }
        assert!(
            !has_package(tree, package),
            "FsUtils {feature_set} tree unexpectedly contains unrelated package `{package}`"
        );
    }
}

fn assert_scheduler_dependency_boundaries() {
    let none = cargo_tree_strict("");
    for package in ["chrono", "chrono-tz", "croner", "tokio"] {
        assert!(
            !has_package(&none, package),
            "default tree unexpectedly contains scheduler dependency `{package}`"
        );
    }

    let croner_only = cargo_tree_with_edges_strict("croner", "normal,build,features");
    for package in ["chrono", "croner", "derive_builder", "strum"] {
        assert!(
            has_package(&croner_only, package),
            "croner-only tree is missing `{package}`"
        );
    }
    for package in ["chrono-tz", "tokio"] {
        assert!(
            !has_package(&croner_only, package),
            "croner-only tree unexpectedly contains `{package}`"
        );
    }
    assert!(
        !croner_only.contains(r#"axutils feature "chrono""#),
        "croner must not enable axutils's public chrono feature"
    );
    assert!(
        croner_only.contains(r#"chrono feature "clock""#),
        "Croner must retain its reviewed chrono/clock dependency"
    );

    let full =
        cargo_tree_with_edges_strict("chrono,chrono_tz,tokio,croner", "normal,build,features");
    for package in [
        "chrono",
        "chrono-tz",
        "croner",
        "derive_builder",
        "strum",
        "tokio",
    ] {
        assert!(
            has_package(&full, package),
            "complete scheduler tree is missing `{package}`"
        );
    }
    for package in [
        "tokio-cron-scheduler",
        "native-tls",
        "openssl",
        "openssl-sys",
        "sqlx",
        "postgres",
        "nats",
        "async-nats",
    ] {
        assert!(
            !has_package(&full, package),
            "complete scheduler tree unexpectedly contains `{package}`"
        );
    }

    let tokio_feature_tree = cargo_feature_tree_inverted("chrono,chrono_tz,tokio,croner", "tokio");
    let actual_tokio_features = tokio_feature_tree
        .lines()
        .filter_map(|line| {
            let marker = "tokio feature \"";
            let start = line.find(marker)? + marker.len();
            let end = line[start..].find('"')? + start;
            Some(line[start..end].to_owned())
        })
        .collect::<BTreeSet<_>>();
    for feature in [
        "fs",
        "io-util",
        "net",
        "rt",
        "rt-multi-thread",
        "signal",
        "sync",
        "time",
    ] {
        assert!(
            actual_tokio_features.contains(feature),
            "scheduler production tree is missing Tokio feature `{feature}`"
        );
    }
    for feature in ["macros", "test-util"] {
        assert!(
            !actual_tokio_features.contains(feature),
            "scheduler production tree contains dev-only Tokio feature `{feature}`"
        );
    }
}

fn cargo_tree_strict(features: &str) -> String {
    cargo_tree_with_edges_strict(features, "normal,build")
}

fn cargo_tree_with_edges_strict(features: &str, edges: &str) -> String {
    let (manifest_target, manifest) = dependency_tree_manifest_strict(features);
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg("selected")
        .arg("--offline")
        .arg("--edges")
        .arg(edges)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|_| panic!("failed to run strict cargo tree for feature `{features}`"));
    assert!(
        output.status.success(),
        "strict cargo tree failed for feature `{features}`\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    manifest_target.cleanup();
    String::from_utf8(output.stdout).expect("cargo tree output should be UTF-8")
}

fn dependency_tree_manifest_strict(features: &str) -> (FsMatrixTarget, PathBuf) {
    let target = strict_fs_target("axutils-fs-dependency-tree");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let forwarded_features = features
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(|feature| format!("\"axutils/{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = target.path().join("Cargo.toml");
    let manifest_text = format!(
        "[package]\nname = \"axutils-fs-dependency-tree\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\nselected = [{forwarded_features}]\n\n[dependencies]\naxutils = {{ path = \"{repository}\", default-features = false }}\n"
    );
    fs::write(&manifest, manifest_text).expect("failed to write strict dependency tree manifest");
    let source = target.path().join("src");
    fs::create_dir_all(&source).expect("failed to create strict dependency tree source");
    fs::write(source.join("lib.rs"), "").expect("failed to write strict dependency tree source");
    (target, manifest)
}

fn assert_tracing_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    for package in [
        "tracing",
        "tracing-subscriber",
        "tracing-appender",
        "crossbeam-channel",
        "matchers",
        "once_cell",
        "regex-automata",
        "regex-syntax",
        "thread_local",
    ] {
        assert!(
            !has_package(&no_feature_tree, package),
            "tracing dependency `{package}` is present without the feature"
        );
    }

    let tracing_tree = cargo_tree("tracing");
    assert!(has_package(&tracing_tree, "tracing"));
    for package in [
        "tracing-subscriber",
        "tracing-appender",
        "crossbeam-channel",
        "matchers",
        "regex-automata",
        "regex-syntax",
        "thread_local",
    ] {
        assert!(
            !has_package(&tracing_tree, package),
            "tracing feature unexpectedly pulls `{package}`"
        );
    }
    for package in ["tokio", "ureq", "reqwest", "lettre", "sqlx", "redis"] {
        assert!(
            !has_package(&tracing_tree, package),
            "tracing feature unexpectedly pulls `{package}`"
        );
    }

    let logging_tree = cargo_tree("logging");
    for package in [
        "tracing",
        "tracing-subscriber",
        "tracing-appender",
        "crossbeam-channel",
        "matchers",
        "once_cell",
        "regex-automata",
        "regex-syntax",
        "thread_local",
    ] {
        assert!(
            has_package(&logging_tree, package),
            "logging dependency `{package}` is missing"
        );
    }

    let feature_tree = cargo_tree_with_edges("logging", "normal,build,features");
    assert!(feature_tree.contains("tracing-subscriber feature \"fmt\""));
    assert!(feature_tree.contains("tracing-subscriber feature \"registry\""));
    assert!(feature_tree.contains("tracing-subscriber feature \"std\""));
    assert!(feature_tree.contains("tracing-subscriber feature \"env-filter\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"json\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"ansi\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"tracing-log\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"smallvec\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"time\""));
    assert!(!feature_tree.contains("tracing-subscriber feature \"local-time\""));
    for package in [
        "serde_json",
        "nu-ansi-term",
        "tracing-log",
        "tokio",
        "native-tls",
        "openssl",
        "openssl-sys",
        "rustls",
        "rustls-webpki",
        "rustls-pki-types",
        "tokio-rustls",
        "hyper-rustls",
        "rustls-native-certs",
        "rustls-platform-verifier",
        "webpki-roots",
    ] {
        assert!(
            !has_package(&logging_tree, package),
            "logging feature unexpectedly pulls `{package}`"
        );
    }
}

fn assert_allocator_dependency_boundaries() {
    let no_allocator_tree = cargo_tree("");
    assert!(!has_package(&no_allocator_tree, "mimalloc"));
    assert!(!has_package(&no_allocator_tree, "rpmalloc"));

    let mimalloc_tree = cargo_tree("mimalloc");
    assert!(has_package(&mimalloc_tree, "mimalloc"));
    assert!(has_package(&mimalloc_tree, "libmimalloc-sys"));
    assert!(!has_package(&mimalloc_tree, "rpmalloc"));
    assert!(!has_package(&mimalloc_tree, "rpmalloc-sys"));

    let rpmalloc_tree = cargo_tree("rpmalloc");
    assert!(has_package(&rpmalloc_tree, "rpmalloc"));
    assert!(has_package(&rpmalloc_tree, "rpmalloc-sys"));
    assert!(!has_package(&rpmalloc_tree, "mimalloc"));
    assert!(!has_package(&rpmalloc_tree, "libmimalloc-sys"));

    let mimalloc_feature_tree = cargo_tree_with_edges("mimalloc", "normal,build,features");
    for feature in [
        "secure",
        "v2",
        "debug",
        "debug_in_debug",
        "extended",
        "override",
        "local_dynamic_tls",
        "win_direct_tls",
        "no_thp",
    ] {
        assert!(
            !mimalloc_feature_tree.contains(&format!("mimalloc feature \"{feature}\"")),
            "mimalloc unexpectedly enables upstream feature `{feature}`"
        );
    }

    let rpmalloc_feature_tree = cargo_tree_with_edges("rpmalloc", "normal,build,features");
    for feature in [
        "statistics",
        "validate_args",
        "asserts",
        "guards",
        "adaptive_thread_cache",
        "global_cache",
        "thread_cache",
        "unlimited_cache",
        "unlimited_global_cache",
        "unlimited_thread_cache",
    ] {
        assert!(
            !rpmalloc_feature_tree.contains(&format!("rpmalloc feature \"{feature}\"")),
            "rpmalloc unexpectedly enables upstream feature `{feature}`"
        );
    }
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
    assert!(has_package_version_prefix(&http_tree, "reqwest", "0.13"));
    assert!(has_package(&http_tree, "url"));
    assert!(!has_package(&http_tree, "lettre"));
    assert_http_tls_packages(&http_tree);

    let http_tokio_tree = cargo_tree("http,tokio");
    assert!(has_package(&http_tokio_tree, "ureq"));
    assert!(has_package(&http_tokio_tree, "reqwest"));
    assert!(has_package_version_prefix(
        &http_tokio_tree,
        "reqwest",
        "0.13"
    ));
    assert!(has_package(&http_tokio_tree, "tokio"));
    assert!(!has_package(&http_tokio_tree, "lettre"));
    assert_http_tls_packages(&http_tokio_tree);

    let http_serde_tree = cargo_tree("http,serde");
    assert!(has_package(&http_serde_tree, "ureq"));
    assert!(has_package(&http_serde_tree, "reqwest"));
    assert!(has_package(&http_serde_tree, "serde"));
    assert!(has_package(&http_serde_tree, "serde_json"));
    assert!(has_package(&http_serde_tree, "serde_urlencoded"));
    assert_http_tls_packages(&http_serde_tree);

    let http_lettre_tree = cargo_tree("http,lettre");
    assert!(has_package(&http_lettre_tree, "reqwest"));
    assert!(has_package(&http_lettre_tree, "lettre"));
    assert_http_tls_packages(&http_lettre_tree);

    let http_lettre_tokio_tree = cargo_tree("http,lettre,tokio");
    assert!(has_package(&http_lettre_tokio_tree, "reqwest"));
    assert!(has_package(&http_lettre_tokio_tree, "lettre"));
    assert!(has_package(&http_lettre_tokio_tree, "tokio"));
    assert_http_tls_packages(&http_lettre_tokio_tree);

    let http_feature_tree = cargo_tree_with_edges("http", "normal,build,features");
    assert!(http_feature_tree.contains("ureq feature \"rustls\""));
    assert!(http_feature_tree.contains("reqwest feature \"rustls\""));
    assert!(http_feature_tree.contains("reqwest feature \"__rustls-aws-lc-rs\""));
    assert!(http_feature_tree.contains("rustls-platform-verifier"));
    assert!(!http_feature_tree.contains("ureq feature \"gzip\""));
    assert!(!http_feature_tree.contains("reqwest feature \"rustls-tls-webpki-roots\""));
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

fn assert_time_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    for package in ["chrono", "time", "jiff"] {
        assert!(!has_package(&no_feature_tree, package));
    }

    for (feature, expected, forbidden) in [
        ("chrono", "chrono", ["time", "jiff"]),
        ("time", "time", ["chrono", "jiff"]),
        ("jiff", "jiff", ["chrono", "time"]),
    ] {
        let tree = cargo_tree(feature);
        assert!(has_package(&tree, expected));
        for package in forbidden {
            assert!(!has_package(&tree, package));
        }
    }

    let all_tree = cargo_tree("chrono,time,jiff");
    for package in ["chrono", "time", "jiff"] {
        assert!(has_package(&all_tree, package));
    }
}

fn assert_redis_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    assert!(!has_package(&no_feature_tree, "redis"));
    assert!(!has_package(&no_feature_tree, "rand"));
    assert!(!has_package(&no_feature_tree, "r2d2"));
    assert!(!has_package(&no_feature_tree, "rmp-serde"));

    let tokio_tree = cargo_tree("tokio");
    assert!(has_package(&tokio_tree, "tokio"));
    assert!(!has_package(&tokio_tree, "redis"));
    assert!(!has_package(&tokio_tree, "rand"));
    assert!(!has_package(&tokio_tree, "r2d2"));
    assert!(!has_package(&tokio_tree, "rmp-serde"));

    let redis_tree = cargo_tree("redis");
    assert!(has_package(&redis_tree, "redis"));
    assert!(has_package(&redis_tree, "r2d2"));
    assert!(has_package(&redis_tree, "rmp-serde"));
    assert!(has_package(&redis_tree, "rand"));
    assert!(!has_package(&redis_tree, "tokio"));
    assert!(!has_package(&redis_tree, "serde_json"));

    let redis_feature_tree = cargo_feature_tree_inverted("redis", "redis");
    for feature in ["tokio-comp", "cluster-async", "connection-manager"] {
        assert!(
            !redis_feature_tree.contains(&format!("redis feature \"{feature}\"")),
            "redis alone unexpectedly enables async redis feature `{feature}`"
        );
    }

    let serde_tree = cargo_tree("serde");
    assert!(has_package(&serde_tree, "serde_json"));
    assert!(!has_package(&serde_tree, "redis"));
    assert!(!has_package(&serde_tree, "r2d2"));
    assert!(!has_package(&serde_tree, "rmp-serde"));

    let redis_serde_tree = cargo_tree("redis,serde");
    assert!(has_package(&redis_serde_tree, "redis"));
    assert!(has_package(&redis_serde_tree, "r2d2"));
    assert!(has_package(&redis_serde_tree, "rmp-serde"));
    assert!(has_package(&redis_serde_tree, "serde_json"));

    let redis_tokio_tree = cargo_tree("redis,tokio");
    assert!(has_package(&redis_tokio_tree, "redis"));
    assert!(has_package(&redis_tokio_tree, "r2d2"));
    assert!(has_package(&redis_tokio_tree, "rmp-serde"));
    assert!(has_package(&redis_tokio_tree, "rand"));
    assert!(has_package(&redis_tokio_tree, "tokio"));

    let redis_tokio_feature_tree = cargo_feature_tree_inverted("redis,tokio", "redis");
    for feature in ["tokio-comp", "cluster-async", "connection-manager"] {
        assert!(
            redis_tokio_feature_tree.contains(&format!("redis feature \"{feature}\"")),
            "redis+tokio does not enable required async redis feature `{feature}`"
        );
    }
}

fn assert_rand_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    assert!(!has_package(&no_feature_tree, "rand"));

    for features in ["rand", "redis", "redis,tokio"] {
        let tree = cargo_tree(features);
        assert!(
            has_package(&tree, "rand"),
            "feature combination `{features}` should include rand"
        );
        let inverted = cargo_tree_inverted_with_edges(features, "rand", "all");
        assert!(
            has_package_version_prefix(&inverted, "rand", "0.10"),
            "rand 0.10.x should be reachable for `{features}`: {inverted}"
        );
    }
}

fn assert_sqlx_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    for package in [
        "sqlx",
        "sqlx-core",
        "sqlx-postgres",
        "sqlx-mysql",
        "sqlx-sqlite",
        "futures-util",
    ] {
        assert!(!has_package(&no_feature_tree, package));
    }

    let tokio_tree = cargo_tree("tokio");
    assert!(has_package(&tokio_tree, "tokio"));
    for package in [
        "sqlx",
        "sqlx-core",
        "sqlx-postgres",
        "sqlx-mysql",
        "sqlx-sqlite",
        "futures-util",
    ] {
        assert!(!has_package(&tokio_tree, package));
    }

    let sqlx_tree = cargo_tree("sqlx");
    for package in [
        "sqlx",
        "sqlx-core",
        "sqlx-postgres",
        "sqlx-mysql",
        "sqlx-sqlite",
        "futures-util",
    ] {
        assert!(has_package(&sqlx_tree, package));
    }
    assert!(has_package_version_prefix(&sqlx_tree, "sqlx", "0.9"));
    assert!(has_package_version_prefix(&sqlx_tree, "sqlx-core", "0.9"));
    assert!(has_package_version_prefix(&sqlx_tree, "sqlx-sqlite", "0.9"));
    assert!(!has_package(&sqlx_tree, "tokio"));
    assert!(!has_package(&sqlx_tree, "tokio-stream"));
    assert!(!has_package(&sqlx_tree, "sqlx-macros"));
    assert_forbidden_tls_packages(&sqlx_tree);

    let sqlx_feature_tree = cargo_tree_with_edges("sqlx", "normal,build,features");
    for feature in ["runtime-tokio", "macros", "migrate", "json"] {
        assert!(
            !sqlx_feature_tree.contains(&format!("sqlx feature \"{feature}\"")),
            "SQLx facade unexpectedly enables feature `{feature}`"
        );
    }
    assert!(sqlx_feature_tree.contains("sqlx feature \"sqlite-bundled\""));
    assert!(sqlx_feature_tree.contains("sqlx-sqlite feature \"bundled\""));
    assert!(sqlx_feature_tree.contains("libsqlite3-sys feature \"bundled\""));
    for feature in [
        "sqlite",
        "sqlite-deserialize",
        "sqlite-load-extension",
        "sqlite-unlock-notify",
    ] {
        assert!(
            !sqlx_feature_tree.contains(&format!("sqlx feature \"{feature}\"")),
            "SQLx facade unexpectedly enables feature `{feature}`"
        );
    }
    assert!(sqlx_feature_tree.contains("sqlx-core feature \"migrate\""));
    assert!(!sqlx_feature_tree.contains("sqlx-core feature \"json\""));

    let sqlx_tokio_tree = cargo_tree("sqlx,tokio");
    for package in [
        "sqlx",
        "sqlx-core",
        "sqlx-postgres",
        "sqlx-mysql",
        "sqlx-sqlite",
        "futures-util",
        "tokio",
        "tokio-stream",
    ] {
        assert!(has_package(&sqlx_tokio_tree, package));
    }
    assert_forbidden_tls_packages(&sqlx_tokio_tree);

    let sqlx_tokio_feature_tree = cargo_tree_with_edges("sqlx,tokio", "normal,build,features");
    assert!(sqlx_tokio_feature_tree.contains("tokio feature \"rt\""));
    for feature in ["macros", "migrate", "json"] {
        assert!(
            !sqlx_tokio_feature_tree.contains(&format!("sqlx feature \"{feature}\"")),
            "SQLx facade unexpectedly enables feature `{feature}` with Tokio"
        );
    }
}

fn assert_convert_dependency_boundaries() {
    let no_feature_tree = cargo_tree("");
    for package in ["itoa", "ryu", "zmij", "uuid"] {
        assert!(!has_package(&no_feature_tree, package));
    }

    let itoa_tree = cargo_tree("itoa");
    assert!(has_package(&itoa_tree, "itoa"));
    for package in ["ryu", "zmij", "uuid"] {
        assert!(!has_package(&itoa_tree, package));
    }

    let ryu_tree = cargo_tree("ryu");
    assert!(has_package(&ryu_tree, "ryu"));
    for package in ["itoa", "zmij", "uuid"] {
        assert!(!has_package(&ryu_tree, package));
    }

    let zmij_tree = cargo_tree("zmij");
    assert!(has_package(&zmij_tree, "zmij"));
    for package in ["itoa", "ryu", "uuid"] {
        assert!(!has_package(&zmij_tree, package));
    }

    let uuid_tree = cargo_tree("uuid");
    assert!(has_package(&uuid_tree, "uuid"));
    for package in ["itoa", "ryu", "zmij", "getrandom", "rand", "serde"] {
        assert!(!has_package(&uuid_tree, package));
    }
    let uuid_feature_tree = cargo_tree_with_edges("uuid", "normal,build,features");
    assert!(uuid_feature_tree.contains("uuid feature \"std\""));
    for feature in ["v1", "v3", "v4", "v5", "v6", "v7", "fast-rng", "serde"] {
        assert!(
            !uuid_feature_tree.contains(&format!("uuid feature \"{feature}\"")),
            "uuid unexpectedly enables upstream feature `{feature}`"
        );
    }

    let float_tree = cargo_tree("ryu,zmij");
    assert!(has_package(&float_tree, "ryu"));
    assert!(has_package(&float_tree, "zmij"));
    assert!(!has_package(&float_tree, "itoa"));
    assert!(!has_package(&float_tree, "uuid"));

    let all_tree = cargo_tree("itoa,ryu,zmij,uuid");
    for package in ["itoa", "ryu", "zmij", "uuid"] {
        assert!(has_package(&all_tree, package));
    }
}

fn cargo_tree(features: &str) -> String {
    cargo_tree_with_edges(features, "normal,build")
}

fn cargo_tree_with_edges(features: &str, edges: &str) -> String {
    let (manifest_target, manifest) = dependency_tree_manifest(features);
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg("selected")
        .arg("--offline")
        .arg("--edges")
        .arg(edges)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|_| panic!("failed to run cargo tree for feature `{features}`"));
    assert!(
        output.status.success(),
        "cargo tree failed for feature `{features}`\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop(manifest_target);
    String::from_utf8(output.stdout).expect("cargo tree output should be UTF-8")
}

fn dependency_tree_manifest(features: &str) -> (TemporaryTarget, PathBuf) {
    let target = unique_target("axutils-dependency-tree");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    let forwarded_features = features
        .split(',')
        .filter(|feature| !feature.is_empty())
        .map(|feature| format!("\"axutils/{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = target.0.join("Cargo.toml");
    let manifest_text = format!(
        "[package]\nname = \"axutils-dependency-tree\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\nselected = [{forwarded_features}]\n\n[dependencies]\naxutils = {{ path = \"{repository}\", default-features = false }}\n"
    );
    fs::write(&manifest, manifest_text).expect("failed to write dependency tree manifest");
    let source = target.0.join("src");
    fs::create_dir_all(&source).expect("failed to create dependency tree source directory");
    fs::write(source.join("lib.rs"), "").expect("failed to write dependency tree source");
    (target, manifest)
}

fn cargo_feature_tree_inverted(features: &str, package: &str) -> String {
    let (manifest_target, manifest) = dependency_tree_manifest(features);
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg("selected")
        .arg("--offline")
        .arg("--edges")
        .arg("features")
        .arg("--invert")
        .arg(package)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|_| panic!("failed to run inverted cargo tree for `{package}`"));
    assert!(
        output.status.success(),
        "inverted cargo tree failed for feature `{features}` and package `{package}`\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop(manifest_target);
    String::from_utf8(output.stdout).expect("cargo tree output should be UTF-8")
}

fn cargo_tree_inverted_with_edges(features: &str, package: &str, edges: &str) -> String {
    let (manifest_target, manifest) = dependency_tree_manifest(features);
    let output = Command::new("cargo")
        .arg("tree")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--no-default-features")
        .arg("--features")
        .arg("selected")
        .arg("--offline")
        .arg("--edges")
        .arg(edges)
        .arg("--invert")
        .arg(package)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .unwrap_or_else(|_| panic!("failed to run inverted cargo tree for `{package}`"));
    assert!(
        output.status.success(),
        "inverted cargo tree failed for feature `{features}` and package `{package}`\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop(manifest_target);
    String::from_utf8(output.stdout).expect("cargo tree output should be UTF-8")
}

fn has_package(tree: &str, package: &str) -> bool {
    tree.lines()
        .flat_map(str::split_whitespace)
        .any(|token| token == package)
}

fn has_package_version_prefix(tree: &str, package: &str, version: &str) -> bool {
    tree.lines()
        .any(|line| line.contains(&format!("{package} v{version}.")))
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

fn assert_http_tls_packages(tree: &str) {
    for package in ["native-tls", "openssl", "openssl-sys"] {
        assert!(
            !has_package(tree, package),
            "HTTP TLS tree unexpectedly contains `{package}`"
        );
    }
    assert!(has_package(tree, "aws-lc-rs"));
    assert!(has_package(tree, "aws-lc-sys"));
    assert!(has_package(tree, "rustls-platform-verifier"));
}
