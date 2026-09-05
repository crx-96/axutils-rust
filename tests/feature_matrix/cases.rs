use super::{
    dependencies,
    support::{self, run_fixture_cases, FixtureCase},
};

const OK: &[&str] = &[];

#[test]
#[ignore = "slow semantic feature/API matrix"]
fn semantic_phone_and_template_matrix() {
    run_fixture_cases(
        "phone-template",
        &[
            FixtureCase {
                feature: "",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "phone-validation",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "template-strfmt",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "template-minijinja",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-phone-provider",
                expected_success: false,
                diagnostic_tokens: &["is_phone"],
            },
            FixtureCase {
                feature: "negative-template-engine",
                expected_success: false,
                diagnostic_tokens: &["minijinja"],
            },
        ],
    );
    dependencies::phone_and_template();
}

#[test]
#[ignore = "slow Tokio and scheduler semantic feature/API matrix"]
fn semantic_tokio_scheduler_matrix() {
    run_fixture_cases(
        "tokio-scheduler",
        &[
            FixtureCase {
                feature: "tokio",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "task-group",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "scheduler",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-task-group",
                expected_success: false,
                diagnostic_tokens: &["tokiotaskgroup"],
            },
            FixtureCase {
                feature: "negative-tokio-isolation",
                expected_success: false,
                diagnostic_tokens: &[
                    "try_exists_async",
                    "load_value_async",
                    "send_async",
                    "execute_async",
                    "ping_async",
                    "sqlx",
                ],
            },
            FixtureCase {
                feature: "negative-scheduler",
                expected_success: false,
                diagnostic_tokens: &["scheduler"],
            },
        ],
    );
    dependencies::tokio_and_scheduler();
}

#[test]
#[ignore = "slow Axum semantic feature/API matrix"]
fn semantic_axum_matrix() {
    run_fixture_cases(
        "axum",
        &[
            FixtureCase {
                feature: "axum",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "axum-tower",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "axum-tower-http",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "axum-governor",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-axum-facade",
                expected_success: false,
                diagnostic_tokens: &["create_app"],
            },
            FixtureCase {
                feature: "negative-axum-tower",
                expected_success: false,
                diagnostic_tokens: &["with_concurrency_limit"],
            },
            FixtureCase {
                feature: "negative-axum-tower-http",
                expected_success: false,
                diagnostic_tokens: &["with_body_limit"],
            },
            FixtureCase {
                feature: "negative-axum-governor",
                expected_success: false,
                diagnostic_tokens: &["with_governor_peer"],
            },
        ],
    );
    dependencies::axum_layers();
}

#[test]
#[ignore = "slow Fs semantic feature/API matrix"]
fn semantic_fs_matrix() {
    run_fixture_cases(
        "fs",
        &[
            FixtureCase {
                feature: "fs-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "fs-temp",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "fs-temp-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-fs-async",
                expected_success: false,
                diagnostic_tokens: &["try_exists_async"],
            },
            FixtureCase {
                feature: "negative-fs-temp",
                expected_success: false,
                diagnostic_tokens: &["create_temp_file"],
            },
            FixtureCase {
                feature: "negative-fs-temp-async",
                expected_success: false,
                diagnostic_tokens: &["try_exists_async"],
            },
        ],
    );
    dependencies::fs_layers();
}

#[test]
#[ignore = "slow config semantic feature/API matrix"]
fn semantic_config_matrix() {
    run_fixture_cases(
        "config",
        &[
            FixtureCase {
                feature: "config",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "config-yaml",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "config-toml",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "config-ini",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "config-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-config-yaml",
                expected_success: false,
                diagnostic_tokens: &["yaml"],
            },
            FixtureCase {
                feature: "negative-config-toml",
                expected_success: false,
                diagnostic_tokens: &["toml"],
            },
            FixtureCase {
                feature: "negative-config-ini",
                expected_success: false,
                diagnostic_tokens: &["ini"],
            },
            FixtureCase {
                feature: "negative-config-async",
                expected_success: false,
                diagnostic_tokens: &["load_value_async"],
            },
        ],
    );
    dependencies::config_layers();
}

#[test]
#[ignore = "slow email semantic feature/API matrix"]
fn semantic_email_matrix() {
    run_fixture_cases(
        "email",
        &[
            FixtureCase {
                feature: "email",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "email-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-email-async",
                expected_success: false,
                diagnostic_tokens: &["send_async"],
            },
        ],
    );
    dependencies::email_layers();
}

#[test]
#[ignore = "slow HTTP semantic feature/API matrix"]
fn semantic_http_matrix() {
    run_fixture_cases(
        "http",
        &[
            FixtureCase {
                feature: "http",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "http-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "http-json",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "http-async-json",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-http-async",
                expected_success: false,
                diagnostic_tokens: &["execute_async"],
            },
            FixtureCase {
                feature: "negative-http-json",
                expected_success: false,
                diagnostic_tokens: &["function or associated item named `get`"],
            },
        ],
    );
    dependencies::http_layers();
}

#[test]
#[ignore = "slow Redis semantic feature/API matrix"]
fn semantic_redis_matrix() {
    run_fixture_cases(
        "redis",
        &[
            FixtureCase {
                feature: "redis",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "redis-cluster",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "redis-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "redis-cluster-async",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-redis-cluster",
                expected_success: false,
                diagnostic_tokens: &["cluster"],
            },
            FixtureCase {
                feature: "negative-redis-async",
                expected_success: false,
                diagnostic_tokens: &["ping_async"],
            },
        ],
    );
    dependencies::redis_layers();
}

#[test]
#[ignore = "slow SQLx driver semantic feature/API matrix"]
fn semantic_sqlx_matrix() {
    run_fixture_cases(
        "sqlx",
        &[
            FixtureCase {
                feature: "sqlx-postgres",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "sqlx-mysql",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "sqlx-sqlite",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "sqlx",
                expected_success: true,
                diagnostic_tokens: OK,
            },
            FixtureCase {
                feature: "negative-sqlx-root",
                expected_success: false,
                diagnostic_tokens: &["sqlxclient"],
            },
            FixtureCase {
                feature: "negative-sqlx-old-init",
                expected_success: false,
                diagnostic_tokens: &["init"],
            },
        ],
    );
    dependencies::sqlx_drivers();
}

#[test]
#[ignore = "slow legacy public-path removal fixture"]
fn semantic_legacy_path_negative_matrix() {
    run_fixture_cases(
        "legacy-paths",
        &[FixtureCase {
            feature: "negative-legacy-paths",
            expected_success: false,
            diagnostic_tokens: &[
                "redisclient",
                "httpclient",
                "configutils",
                "cryptoutils",
                "jwtutils",
                "sqlxutils",
                "logutils",
            ],
        }],
    );
    dependencies::baseline();
}

#[test]
#[ignore = "slow retained independent feature/API matrix"]
fn semantic_retained_feature_matrix() {
    let cases = [
        "itoa",
        "ryu",
        "zmij",
        "uuid",
        "convert-float-both",
        "convert-all",
        "rand",
        "regex",
        "chrono",
        "time",
        "jiff",
        "chrono-time",
        "chrono-jiff",
        "time-jiff",
        "time-all",
        "base64",
        "md5",
        "aes",
        "encoding_rs",
        "aes-base64",
        "crypto-all",
        "jwt",
        "tracing",
        "logging",
    ]
    .map(|feature| FixtureCase {
        feature,
        expected_success: true,
        diagnostic_tokens: OK,
    });
    run_fixture_cases("retained-features", &cases);
    dependencies::retained_features();
}

#[test]
#[ignore = "slow retained feature negative API matrix"]
fn semantic_retained_feature_negative_matrix() {
    run_fixture_cases(
        "retained-feature-negatives",
        &[
            FixtureCase {
                feature: "negative-convert-integer",
                expected_success: false,
                diagnostic_tokens: &["integerbuffer"],
            },
            FixtureCase {
                feature: "negative-convert-float",
                expected_success: false,
                diagnostic_tokens: &["floatbuffer"],
            },
            FixtureCase {
                feature: "negative-convert-uuid",
                expected_success: false,
                diagnostic_tokens: &["uuidbuffer"],
            },
            FixtureCase {
                feature: "negative-convert-ryu",
                expected_success: false,
                diagnostic_tokens: &["ryu"],
            },
            FixtureCase {
                feature: "negative-convert-zmij",
                expected_success: false,
                diagnostic_tokens: &["zmij"],
            },
            FixtureCase {
                feature: "negative-convert-sealed",
                expected_success: false,
                diagnostic_tokens: &["integervalue"],
            },
            FixtureCase {
                feature: "negative-rand",
                expected_success: false,
                diagnostic_tokens: &["randomutils"],
            },
            FixtureCase {
                feature: "negative-redis-random",
                expected_success: false,
                diagnostic_tokens: &["randomutils"],
            },
            FixtureCase {
                feature: "negative-base64",
                expected_success: false,
                diagnostic_tokens: &["base64encode"],
            },
            FixtureCase {
                feature: "negative-md5",
                expected_success: false,
                diagnostic_tokens: &["md5"],
            },
            FixtureCase {
                feature: "negative-aes",
                expected_success: false,
                diagnostic_tokens: &["aescipher"],
            },
            FixtureCase {
                feature: "negative-encoding-rs",
                expected_success: false,
                diagnostic_tokens: &["gbk"],
            },
            FixtureCase {
                feature: "negative-aes-base64",
                expected_success: false,
                diagnostic_tokens: &["encryptbase64"],
            },
            FixtureCase {
                feature: "negative-jwt",
                expected_success: false,
                diagnostic_tokens: &["jwtcodec"],
            },
            FixtureCase {
                feature: "negative-jwt-config",
                expected_success: false,
                diagnostic_tokens: &["configloader"],
            },
            FixtureCase {
                feature: "negative-logging",
                expected_success: false,
                diagnostic_tokens: &["logconfig"],
            },
            FixtureCase {
                feature: "negative-time-unsuffixed-chrono",
                expected_success: false,
                diagnostic_tokens: &[
                    "formatdate",
                    "formatoptiondate",
                    "formatdatetime",
                    "formatoptiondatetime",
                    "formatdatetimewithoffset",
                    "formatoptiondatetimewithoffset",
                ],
            },
            FixtureCase {
                feature: "negative-time-unsuffixed-all",
                expected_success: false,
                diagnostic_tokens: &[
                    "formatdate",
                    "formatoptiondate",
                    "formatdatetime",
                    "formatoptiondatetime",
                    "formatdatetimewithoffset",
                    "formatoptiondatetimewithoffset",
                ],
            },
        ],
    );
}

#[test]
#[ignore = "slow removed provider feature-resolution contract"]
fn semantic_removed_provider_feature_matrix() {
    support::assert_removed_provider_features(&[
        "chrono_tz",
        "croner",
        "lettre",
        "libphonenumber",
        "minijinja",
        "serde",
        "serde-saphyr",
        "strfmt",
        "tempfile",
        "tempfile-async",
        "tokio-util",
        "toml",
        "tower",
        "tower-http",
        "tower_governor",
        "rust-ini",
        "mimalloc",
        "rpmalloc",
    ]);
}

#[test]
fn tree_cache_key_normalization_is_stable() {
    dependencies::tree_cache_key_normalization_smoke();
}
