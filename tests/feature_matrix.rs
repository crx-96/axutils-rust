use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct TemporaryTarget(PathBuf);

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
        ("all", true, ""),
        ("serde-tokio-all", true, ""),
        ("negative-config-module-no-serde", false, "configloader"),
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
