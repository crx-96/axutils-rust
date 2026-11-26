use super::{
    dependencies,
    support::{
        self, run_fixture_cases,
        ExpectedDiagnostic::{self, AtPath, Import, Member},
        FixtureCase,
    },
};

const OK: &[ExpectedDiagnostic] = &[];

#[test]
#[ignore = "common capabilities feature/API and dependency isolation matrix"]
fn common_capability_matrix() {
    run_fixture_cases(
        "common-capabilities",
        &[
            FixtureCase {
                feature: "",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "secure-random",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "tokio",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "sqlx-postgres",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-secure-random",
                expected_success: false,
                diagnostics: &[
                    Member("CryptoUtils", "secure_random_bytes"),
                    Member("CryptoUtils", "secure_random_digits"),
                    Member("CryptoUtils", "secure_random_hex"),
                ],
            },
            FixtureCase {
                feature: "negative-secure-random-rand",
                expected_success: false,
                diagnostics: &[
                    Member("CryptoUtils", "secure_random_bytes"),
                    Member("CryptoUtils", "secure_random_digits"),
                    Member("CryptoUtils", "secure_random_hex"),
                ],
            },
            FixtureCase {
                feature: "negative-task-guard",
                expected_success: false,
                diagnostics: &[AtPath("E0433", "tokio", "axutils::tokio::TokioTaskGuard")],
            },
            FixtureCase {
                feature: "negative-postgres-classifier-mysql",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0425",
                    "is_postgres_transaction_conflict",
                    "axutils::sqlx::is_postgres_transaction_conflict",
                )],
            },
            FixtureCase {
                feature: "negative-postgres-classifier-sqlite",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0425",
                    "is_postgres_transaction_conflict",
                    "axutils::sqlx::is_postgres_transaction_conflict",
                )],
            },
        ],
    );
    dependencies::common_capabilities();
}

#[test]
#[ignore = "下游 HTTP 压缩 feature 合并与 loopback 响应契约"]
fn http_downstream_compression_contract() {
    support::run_downstream_contract("http_compression", "test", &["", "compression"]);
}

#[test]
#[ignore = "下游 serde_json 任意精度 feature 的配置值和 JWT 契约"]
fn config_and_jwt_downstream_arbitrary_precision_contract() {
    support::run_downstream_contract(
        "config/arbitrary_precision",
        "test",
        &["", "jwt", "jwt-dual-provider"],
    );
}

#[test]
#[ignore = "独立 panic=abort 进程中的调度器计时契约"]
fn scheduler_abort_runtime_contract() {
    support::run_downstream_contract("scheduler_abort", "run", &[""]);
}

#[test]
#[ignore = "slow semantic feature/API matrix"]
fn semantic_phone_and_template_matrix() {
    run_fixture_cases(
        "phone-template",
        &[
            FixtureCase {
                feature: "",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "phone-validation",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "template-strfmt",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "template-minijinja",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-phone-provider",
                expected_success: false,
                diagnostics: &[Member("RegUtils", "is_phone")],
            },
            FixtureCase {
                feature: "negative-template-engine",
                expected_success: false,
                diagnostics: &[Member("TemplateEngine", "MiniJinja")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "task-group",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "scheduler",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-task-group",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "TokioTaskGroup",
                    "axutils::tokio::TokioTaskGroup::new",
                )],
            },
            FixtureCase {
                feature: "negative-tokio-isolation",
                expected_success: false,
                diagnostics: &[
                    Member("FsUtils", "try_exists_async"),
                    AtPath(
                        "E0433",
                        "config",
                        "axutils::config::ConfigLoader::load_value_async",
                    ),
                    AtPath("E0433", "email", "axutils::email::EmailClient::send_async"),
                    AtPath("E0433", "http", "axutils::http::HttpClient::execute_async"),
                    AtPath("E0433", "redis", "axutils::redis::RedisClient::ping_async"),
                    AtPath("E0433", "sqlx", "axutils::sqlx::SqlxClient::connect"),
                ],
            },
            FixtureCase {
                feature: "negative-scheduler",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "scheduler",
                    "axutils::scheduler::Scheduler::new",
                )],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "axum-tower",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "axum-tower-http",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "axum-governor",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-axum-facade",
                expected_success: false,
                diagnostics: &[Member("AxumUtils", "create_app")],
            },
            FixtureCase {
                feature: "negative-axum-tower",
                expected_success: false,
                diagnostics: &[Member("AxumServerBuilder", "with_concurrency_limit")],
            },
            FixtureCase {
                feature: "negative-axum-tower-http",
                expected_success: false,
                diagnostics: &[Member("AxumServerBuilder", "with_body_limit")],
            },
            FixtureCase {
                feature: "negative-axum-governor",
                expected_success: false,
                diagnostics: &[Member("AxumServerBuilder", "with_governor_peer")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "fs-temp",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "fs-temp-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-fs-async",
                expected_success: false,
                diagnostics: &[Member("FsUtils", "try_exists_async")],
            },
            FixtureCase {
                feature: "negative-fs-temp",
                expected_success: false,
                diagnostics: &[Member("FsUtils", "create_temp_file")],
            },
            FixtureCase {
                feature: "negative-fs-temp-async",
                expected_success: false,
                diagnostics: &[Member("FsUtils", "try_exists_async")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "config-yaml",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "config-toml",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "config-ini",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "config-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-config-yaml",
                expected_success: false,
                diagnostics: &[Member("ConfigFormat", "Yaml")],
            },
            FixtureCase {
                feature: "negative-config-toml",
                expected_success: false,
                diagnostics: &[Member("ConfigFormat", "Toml")],
            },
            FixtureCase {
                feature: "negative-config-ini",
                expected_success: false,
                diagnostics: &[Member("ConfigFormat", "Ini")],
            },
            FixtureCase {
                feature: "negative-config-async",
                expected_success: false,
                diagnostics: &[Member("ConfigLoader", "load_value_async")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "email-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-email-async",
                expected_success: false,
                diagnostics: &[Member("EmailClient", "send_async")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "http-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "http-json",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "http-async-json",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-http-async",
                expected_success: false,
                diagnostics: &[Member("HttpClient", "execute_async")],
            },
            FixtureCase {
                feature: "negative-http-json",
                expected_success: false,
                diagnostics: &[Member("HttpClient", "get")],
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "redis-cluster",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "redis-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "redis-cluster-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-redis-cluster",
                expected_success: false,
                diagnostics: &[Member("RedisConfig", "cluster")],
            },
            FixtureCase {
                feature: "negative-redis-async",
                expected_success: false,
                diagnostics: &[Member("RedisClient", "ping_async")],
            },
        ],
    );
    dependencies::redis_layers();
}

#[test]
#[ignore = "Redis 缓存失效队列的独立 feature、公开 API 与依赖隔离"]
fn semantic_redis_invalidation_matrix() {
    run_fixture_cases(
        "redis-invalidation",
        &[
            FixtureCase {
                feature: "redis-invalidation",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "redis-invalidation,tracing",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "redis-invalidation,redis-cluster-async",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-redis-invalidation-async",
                expected_success: false,
                diagnostics: &[
                    Import("axutils::redis::RedisInvalidationQueue"),
                    Import("axutils::redis::RedisInvalidationConfig"),
                    Import("axutils::redis::RedisInvalidationEnqueue"),
                ],
            },
            FixtureCase {
                feature: "negative-redis-invalidation-tokio",
                expected_success: false,
                diagnostics: &[
                    Import("axutils::redis::RedisInvalidationQueue"),
                    Import("axutils::redis::RedisInvalidationConfig"),
                    Import("axutils::redis::RedisInvalidationEnqueue"),
                ],
            },
            FixtureCase {
                feature: "negative-redis-invalidation-combined",
                expected_success: false,
                diagnostics: &[
                    Import("axutils::redis::RedisInvalidationQueue"),
                    Import("axutils::redis::RedisInvalidationConfig"),
                    Import("axutils::redis::RedisInvalidationEnqueue"),
                ],
            },
        ],
    );
    dependencies::redis_invalidation();
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
                diagnostics: OK,
            },
            FixtureCase {
                feature: "sqlx-mysql",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "sqlx-sqlite",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "sqlx",
                expected_success: true,
                diagnostics: OK,
            },
            FixtureCase {
                feature: "negative-sqlx-root",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "SqlxClient",
                    "axutils::SqlxClient::connect",
                )],
            },
            FixtureCase {
                feature: "negative-sqlx-old-init",
                expected_success: false,
                diagnostics: &[Member("SqlxUtils", "init")],
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
            diagnostics: &[
                AtPath("E0433", "RedisClient", "axutils::RedisClient::new"),
                AtPath("E0433", "HttpClient", "axutils::HttpClient::new"),
                AtPath("E0433", "ConfigUtils", "axutils::ConfigUtils::loader"),
                AtPath("E0433", "CryptoUtils", "axutils::CryptoUtils::hex_encode"),
                AtPath("E0433", "JwtUtils", "axutils::JwtUtils::codec"),
                AtPath("E0433", "SqlxUtils", "axutils::SqlxUtils::init_async"),
                AtPath("E0433", "LogUtils", "axutils::LogUtils::init"),
                AtPath(
                    "E0603",
                    "redis_utils",
                    "axutils::utils::redis_utils::RedisUtils::client",
                ),
                AtPath(
                    "E0603",
                    "http_utils",
                    "axutils::utils::http_utils::HttpUtils::client",
                ),
                AtPath(
                    "E0603",
                    "config_utils",
                    "axutils::utils::config_utils::ConfigUtils::loader",
                ),
                AtPath(
                    "E0603",
                    "crypto_utils",
                    "axutils::utils::crypto_utils::CryptoUtils::hex_encode",
                ),
                AtPath(
                    "E0603",
                    "jwt_utils",
                    "axutils::utils::jwt_utils::JwtUtils::codec",
                ),
                AtPath(
                    "E0603",
                    "sqlx_utils",
                    "axutils::utils::sqlx_utils::SqlxUtils::init_async",
                ),
                AtPath(
                    "E0603",
                    "log_utils",
                    "axutils::utils::log_utils::LogUtils::init",
                ),
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
        diagnostics: OK,
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
                diagnostics: &[AtPath(
                    "E0433",
                    "IntegerBuffer",
                    "axutils::convert::IntegerBuffer::new",
                )],
            },
            FixtureCase {
                feature: "negative-convert-float",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "FloatBuffer",
                    "axutils::convert::FloatBuffer::new",
                )],
            },
            FixtureCase {
                feature: "negative-convert-uuid",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "UuidBuffer",
                    "axutils::convert::UuidBuffer::new",
                )],
            },
            FixtureCase {
                feature: "negative-convert-ryu",
                expected_success: false,
                diagnostics: &[Member("FloatFormat", "Ryu")],
            },
            FixtureCase {
                feature: "negative-convert-zmij",
                expected_success: false,
                diagnostics: &[Member("FloatFormat", "Zmij")],
            },
            FixtureCase {
                feature: "negative-convert-sealed",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0277",
                    "Custom: axutils::convert::integer::sealed::IntegerSealed",
                    "axutils::convert::IntegerValue",
                )],
            },
            FixtureCase {
                feature: "negative-rand",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "RandomUtils",
                    "axutils::utils::RandomUtils::numeric_string",
                )],
            },
            FixtureCase {
                feature: "negative-redis-random",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "RandomUtils",
                    "axutils::utils::RandomUtils::numeric_string",
                )],
            },
            FixtureCase {
                feature: "negative-base64",
                expected_success: false,
                diagnostics: &[Member("CryptoUtils", "base64_encode")],
            },
            FixtureCase {
                feature: "negative-md5",
                expected_success: false,
                diagnostics: &[Member("CryptoUtils", "md5")],
            },
            FixtureCase {
                feature: "negative-aes",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "AesCipher",
                    "axutils::crypto::AesCipher::from_key_bytes",
                )],
            },
            FixtureCase {
                feature: "negative-encoding-rs",
                expected_success: false,
                diagnostics: &[Member("TextEncoding", "Gbk")],
            },
            FixtureCase {
                feature: "negative-aes-base64",
                expected_success: false,
                diagnostics: &[Member("AesCipher", "encrypt_base64")],
            },
            FixtureCase {
                feature: "negative-jwt",
                expected_success: false,
                diagnostics: &[AtPath("E0433", "jwt", "axutils::jwt::JwtCodec::new")],
            },
            FixtureCase {
                feature: "negative-jwt-config",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "config",
                    "axutils::config::ConfigLoader::new",
                )],
            },
            FixtureCase {
                feature: "negative-logging",
                expected_success: false,
                diagnostics: &[AtPath(
                    "E0433",
                    "logging",
                    "axutils::logging::LogConfig::new",
                )],
            },
            FixtureCase {
                feature: "negative-time-unsuffixed-chrono",
                expected_success: false,
                diagnostics: &[
                    Member("TimeUtils", "format_date"),
                    Member("TimeUtils", "format_option_date"),
                    Member("TimeUtils", "format_datetime"),
                    Member("TimeUtils", "format_option_datetime"),
                    Member("TimeUtils", "format_datetime_with_offset"),
                    Member("TimeUtils", "format_option_datetime_with_offset"),
                ],
            },
            FixtureCase {
                feature: "negative-time-unsuffixed-all",
                expected_success: false,
                diagnostics: &[
                    Member("TimeUtils", "format_date"),
                    Member("TimeUtils", "format_option_date"),
                    Member("TimeUtils", "format_datetime"),
                    Member("TimeUtils", "format_option_datetime"),
                    Member("TimeUtils", "format_datetime_with_offset"),
                    Member("TimeUtils", "format_option_datetime_with_offset"),
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
    assert_eq!(
        support::normalize_feature_csv(" tokio,axum,,tokio "),
        "axum,tokio"
    );
    assert_eq!(support::normalize_feature_csv(" , "), "");
}
