#![allow(dead_code)]

#[cfg(feature = "phone-validation")]
fn phone_validation() {
    use axutils::utils::RegUtils;

    let _ = RegUtils::is_phone;
}

#[cfg(feature = "template-strfmt")]
fn template_strfmt() {
    use axutils::utils::{FormatUtils, TemplateEngine};

    let _ = FormatUtils::template::<()>;
    let _ = TemplateEngine::Strfmt;
}

#[cfg(feature = "template-minijinja")]
fn template_minijinja() {
    use axutils::utils::{FormatUtils, TemplateEngine};

    let _ = FormatUtils::template::<()>;
    let _ = TemplateEngine::MiniJinja;
}

#[cfg(feature = "tokio")]
fn tokio() {
    use axutils::{tokio::TokioConfig, utils::TokioUtils};

    let _ = TokioConfig::new;
    let _ = TokioUtils::try_current_handle;
}

#[cfg(feature = "task-group")]
fn task_group() {
    use axutils::tokio::TokioTaskGroup;

    let _ = TokioTaskGroup::new;
}

#[cfg(feature = "scheduler")]
fn scheduler() {
    use axutils::{
        scheduler::{Scheduler, SchedulerConfig, TaskSchedule},
        utils::SchedulerUtils,
    };

    let _ = Scheduler::new;
    let _ = SchedulerConfig::new;
    let _ = TaskSchedule::once;
    let _ = SchedulerUtils::is_initialized;
}

#[cfg(feature = "axum")]
fn axum() {
    use axutils::{
        axum::{AxumApp, AxumConfig},
        utils::AxumUtils,
    };

    let _ = AxumApp::<()>::create_router();
    let _ = AxumApp::new()
        .into_server_builder()
        .config(AxumConfig::new());
    let _ = AxumUtils::is_initialized;
}

#[cfg(feature = "axum-tower")]
fn axum_tower() {
    use axutils::axum::AxumServerBuilder;

    let _ = AxumServerBuilder::with_concurrency_limit;
}

#[cfg(feature = "axum-tower-http")]
fn axum_tower_http() {
    use axutils::axum::AxumServerBuilder;

    let _ = AxumServerBuilder::with_body_limit;
}

#[cfg(feature = "axum-governor")]
fn axum_governor() {
    use axutils::axum::AxumServerBuilder;

    let _ = AxumServerBuilder::with_governor_peer;
}

#[cfg(feature = "fs-async")]
fn fs_async() {
    use axutils::utils::FsUtils;

    let _ = FsUtils::try_exists_async::<&str>;
}

#[cfg(feature = "fs-temp")]
fn fs_temp() {
    use axutils::{
        fs::{FsTempConfig, FsTempFile},
        utils::FsUtils,
    };

    let _ = FsTempConfig::new;
    let _ = FsTempFile::path;
    let _ = FsUtils::create_temp_file;
}

#[cfg(feature = "fs-temp-async")]
fn fs_temp_async() {
    use axutils::{
        fs::{FsAsyncTempFile, FsTempConfig},
        utils::FsUtils,
    };

    let _ = FsTempConfig::new;
    let _ = FsAsyncTempFile::path;
    let _ = FsUtils::create_temp_file_async;
}

#[cfg(feature = "config")]
fn config() {
    use axutils::{
        config::{ConfigFormat, ConfigLoader, ConfigValue},
        utils::ConfigUtils,
    };

    let _ = ConfigFormat::Json;
    let _ = ConfigLoader::new;
    let _ = ConfigValue::Null;
    let _ = ConfigUtils::loader;
}

#[cfg(feature = "config-yaml")]
fn config_yaml() {
    use axutils::config::ConfigFormat;

    let _ = ConfigFormat::Yaml;
}

#[cfg(feature = "config-toml")]
fn config_toml() {
    use axutils::config::ConfigFormat;

    let _ = ConfigFormat::Toml;
}

#[cfg(feature = "config-ini")]
fn config_ini() {
    use axutils::config::ConfigFormat;

    let _ = ConfigFormat::Ini;
}

#[cfg(feature = "config-async")]
fn config_async() {
    use axutils::{config::ConfigLoader, utils::ConfigUtils};

    let _ = ConfigLoader::new().load_value_async("config.json");
    let _ = ConfigUtils::load_value_async("config.json");
}

#[cfg(feature = "email")]
fn email() {
    use axutils::{
        email::{EmailClient, EmailConfig, EmailMessage, EmailSecurity},
        utils::EmailUtils,
    };

    let _ = EmailClient::new;
    let _ = EmailConfig::new(
        "smtp.example.com",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "application-password",
        "sender@example.com",
    );
    let _ = EmailMessage::text(vec!["recipient@example.com".to_owned()], "subject", "body");
    let _ = EmailUtils::client;
}

#[cfg(feature = "email-async")]
fn email_async() {
    use axutils::email::EmailClient;

    let _ = EmailClient::send_async;
}

#[cfg(feature = "http")]
fn http() {
    use axutils::{
        http::{HttpClient, HttpConfig, HttpRequest},
        utils::HttpUtils,
    };

    let _ = HttpClient::new;
    let _ = HttpConfig::builder;
    let _ = HttpRequest::builder;
    let _ = HttpUtils::client;
}

#[cfg(feature = "http-async")]
fn http_async() {
    use axutils::http::HttpClient;

    let _ = HttpClient::execute_async;
}

#[cfg(feature = "http-json")]
fn http_json() {
    use axutils::http::{HttpClient, HttpConfig};

    let client = HttpClient::new(HttpConfig::default()).unwrap();
    let _ = client.get::<String, ()>("https://example.invalid", None, None);
}

#[cfg(feature = "http-async-json")]
fn http_async_json() {
    use axutils::http::{HttpClient, HttpConfig};

    let client = HttpClient::new(HttpConfig::default()).unwrap();
    let _ = client.get_async::<String, ()>("https://example.invalid", None, None);
}

#[cfg(feature = "redis")]
fn redis() {
    use axutils::{
        redis::{RedisClient, RedisConfig, RedisTransaction},
        utils::RedisUtils,
    };

    let _ = RedisClient::new;
    let _ = RedisConfig::single("redis://127.0.0.1:6379/0");
    let _ = RedisTransaction::set::<&str, u8>;
    let _ = RedisUtils::client;
}

#[cfg(feature = "redis-cluster")]
fn redis_cluster() {
    use axutils::redis::RedisConfig;

    let _ = RedisConfig::cluster::<Vec<String>, String>;
}

#[cfg(feature = "redis-async")]
fn redis_async() {
    use axutils::{
        redis::{RedisAsyncLockGuard, RedisClient},
        utils::RedisUtils,
    };

    let _ = RedisClient::ping_async;
    let _ = RedisAsyncLockGuard::release;
    let _ = RedisUtils::init_async;
}

#[cfg(feature = "redis-cluster-async")]
fn redis_cluster_async() {
    use axutils::redis::{RedisClient, RedisConfig};

    let _ = RedisClient::ping_async;
    let _ = RedisConfig::cluster(["redis://127.0.0.1:7000/0"]);
}

#[cfg(any(
    feature = "sqlx-postgres",
    feature = "sqlx-mysql",
    feature = "sqlx-sqlite",
    feature = "sqlx"
))]
fn sqlx() {
    use axutils::{
        sqlx::{SqlxClient, SqlxConfig},
        utils::SqlxUtils,
    };

    let _ = SqlxClient::connect;
    #[cfg(any(feature = "sqlx-postgres", feature = "sqlx"))]
    let _ = SqlxConfig::new("postgres://localhost/example");
    #[cfg(any(feature = "sqlx-mysql", feature = "sqlx"))]
    let _ = SqlxConfig::new("mysql://localhost/example");
    #[cfg(any(feature = "sqlx-sqlite", feature = "sqlx"))]
    let _ = SqlxConfig::new("sqlite::memory:");
    let _ = SqlxUtils::init_async;
}

#[cfg(feature = "itoa")]
fn itoa() {
    use axutils::{convert::IntegerBuffer, utils::ConvertUtils};

    let mut buffer = IntegerBuffer::new();
    let _ = ConvertUtils::integer_to_str(42_i32, &mut buffer);
    let _ = ConvertUtils::integer_to_string(i128::MIN);
    let _ = ConvertUtils::string_to_integer::<i64>("-64");
}

#[cfg(feature = "ryu")]
fn ryu() {
    use axutils::{
        convert::{FloatBuffer, FloatFormat},
        utils::ConvertUtils,
    };

    let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    let _ = ConvertUtils::float_to_str(1.25_f64, &mut buffer);
    let _ = ConvertUtils::float_to_string(1.25_f64, FloatFormat::Ryu);
}

#[cfg(feature = "zmij")]
fn zmij() {
    use axutils::{
        convert::{FloatBuffer, FloatFormat},
        utils::ConvertUtils,
    };

    let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
    let _ = ConvertUtils::float_to_str(1.25_f64, &mut buffer);
    let _ = ConvertUtils::float_to_string(1.25_f64, FloatFormat::Zmij);
}

#[cfg(feature = "uuid")]
fn uuid() {
    use axutils::{convert::UuidBuffer, utils::ConvertUtils};

    let uuid = ConvertUtils::string_to_uuid("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let mut buffer = UuidBuffer::new();
    let _ = ConvertUtils::uuid_to_str(&uuid, &mut buffer);
    let _ = ConvertUtils::uuid_to_string(&uuid);
}

#[cfg(feature = "rand")]
fn rand() {
    use axutils::utils::{LetterCase, RandomUtils};

    let _ = RandomUtils::numeric_string(4);
    let _ = RandomUtils::alphabetic_string(4, LetterCase::Lower);
    let _ = RandomUtils::integer(1..=1);
}

#[cfg(feature = "regex")]
fn regex() {
    use axutils::utils::RegUtils;

    let _ = RegUtils::is_email;
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff"))]
fn time() {
    use axutils::utils::TimeUtils;

    let _ = TimeUtils::try_timestamp;
}

#[cfg(feature = "chrono")]
fn chrono_backend() {
    use axutils::utils::TimeUtils;
    use chrono::NaiveDate;

    let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let _ = TimeUtils::format_date_chrono(date, None);
}

#[cfg(feature = "time")]
fn time_backend() {
    use axutils::utils::TimeUtils;
    use time::{Date, Month};

    let date = Date::from_calendar_date(2024, Month::February, 29).unwrap();
    let _ = TimeUtils::format_date_time(date, None);
}

#[cfg(feature = "jiff")]
fn jiff_backend() {
    use axutils::utils::TimeUtils;
    use jiff::civil::Date;

    let date = Date::new(2024, 2, 29).unwrap();
    let _ = TimeUtils::format_date_jiff(date, None);
}

fn crypto_baseline() {
    use axutils::{crypto::TextEncoding, utils::CryptoUtils};

    let encoded = CryptoUtils::hex_encode([0x00, 0xff]).unwrap();
    assert_eq!(CryptoUtils::hex_decode(&encoded).unwrap(), [0x00, 0xff]);
    assert_eq!(TextEncoding::Utf8.decode(b"hi").unwrap(), "hi");
}

#[cfg(any(
    feature = "base64",
    feature = "md5",
    feature = "aes",
    feature = "encoding_rs"
))]
fn crypto() {
    use axutils::utils::CryptoUtils;

    let _ = CryptoUtils::hex_encode([0x00]);
}

#[cfg(feature = "base64")]
fn base64() {
    use axutils::{crypto::Base64Options, utils::CryptoUtils};

    let _ = CryptoUtils::base64_encode("payload", Base64Options::STANDARD);
}

#[cfg(feature = "md5")]
fn md5() {
    use axutils::utils::CryptoUtils;

    let _ = CryptoUtils::md5_hex("payload");
}

#[cfg(feature = "aes")]
fn aes() {
    use axutils::crypto::{AesCipher, AesMode};

    let _ = AesCipher::from_key_bytes([0_u8; 16], AesMode::Gcm);
}

#[cfg(feature = "encoding_rs")]
fn encoding_rs() {
    use axutils::crypto::TextEncoding;

    let _ = TextEncoding::Gbk;
}

#[cfg(feature = "jwt")]
fn jwt() {
    use axutils::{
        jwt::{
            JwtAlgorithm, JwtCodec, JwtConfig, JwtSigningKey, JwtValidation, JwtVerificationKey,
        },
        utils::JwtUtils,
    };

    let signing = JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap();
    let verification = JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap();
    let config = JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(signing),
        Some(verification),
        JwtValidation::new(),
    )
    .unwrap();
    let _ = JwtConfig::new;
    let _ = JwtCodec::new(config);
    let _ = JwtUtils::codec;
}

#[cfg(feature = "tracing")]
fn tracing() {}

#[cfg(feature = "logging")]
fn logging() {
    use axutils::{logging::LogConfig, utils::LogUtils};

    let _ = LogConfig::new;
    let _ = LogUtils::init;
    let _ = LogUtils::is_initialized;
}

#[cfg(feature = "negative-legacy-paths")]
fn negative_legacy_paths() {
    let _ = axutils::RedisClient::new;
    let _ = axutils::HttpClient::new;
    let _ = axutils::ConfigUtils::loader;
    let _ = axutils::CryptoUtils::hex_encode;
    let _ = axutils::JwtUtils::codec;
    let _ = axutils::SqlxUtils::init_async;
    let _ = axutils::LogUtils::init;
    let _ = axutils::utils::redis_utils::RedisUtils::client;
    let _ = axutils::utils::http_utils::HttpUtils::client;
    let _ = axutils::utils::config_utils::ConfigUtils::loader;
    let _ = axutils::utils::crypto_utils::CryptoUtils::hex_encode;
    let _ = axutils::utils::jwt_utils::JwtUtils::codec;
    let _ = axutils::utils::sqlx_utils::SqlxUtils::init_async;
    let _ = axutils::utils::log_utils::LogUtils::init;
}

#[cfg(feature = "negative-phone-provider")]
fn negative_phone_provider() {
    let _ = axutils::utils::RegUtils::is_phone;
}

#[cfg(feature = "negative-template-engine")]
fn negative_template_engine() {
    let _ = axutils::utils::TemplateEngine::MiniJinja;
}

#[cfg(feature = "negative-task-group")]
fn negative_task_group() {
    let _ = axutils::tokio::TokioTaskGroup::new;
}

#[cfg(feature = "negative-tokio-isolation")]
fn negative_tokio_isolation() {
    let _ = axutils::utils::FsUtils::try_exists_async::<&str>;
    let _ = axutils::config::ConfigLoader::load_value_async;
    let _ = axutils::email::EmailClient::send_async;
    let _ = axutils::http::HttpClient::execute_async;
    let _ = axutils::redis::RedisClient::ping_async;
    let _ = axutils::sqlx::SqlxClient::connect;
}

#[cfg(feature = "negative-scheduler")]
fn negative_scheduler() {
    let _ = axutils::scheduler::Scheduler::new;
}

#[cfg(feature = "negative-axum-facade")]
fn negative_axum_facade() {
    let _ = axutils::utils::AxumUtils::create_app;
}

#[cfg(feature = "negative-axum-tower")]
fn negative_axum_tower() {
    let _ = axutils::axum::AxumServerBuilder::with_concurrency_limit;
}

#[cfg(feature = "negative-axum-tower-http")]
fn negative_axum_tower_http() {
    let _ = axutils::axum::AxumServerBuilder::with_body_limit;
}

#[cfg(feature = "negative-axum-governor")]
fn negative_axum_governor() {
    let _ = axutils::axum::AxumServerBuilder::with_governor_peer;
}

#[cfg(feature = "negative-fs-async")]
fn negative_fs_async() {
    let _ = axutils::utils::FsUtils::try_exists_async;
}

#[cfg(feature = "negative-fs-temp")]
fn negative_fs_temp() {
    let _ = axutils::utils::FsUtils::create_temp_file;
}

#[cfg(feature = "negative-fs-temp-async")]
fn negative_fs_temp_async() {
    let _ = axutils::utils::FsUtils::try_exists_async::<&str>;
}

#[cfg(feature = "negative-config-yaml")]
fn negative_config_yaml() {
    let _ = axutils::config::ConfigFormat::Yaml;
}

#[cfg(feature = "negative-config-toml")]
fn negative_config_toml() {
    let _ = axutils::config::ConfigFormat::Toml;
}

#[cfg(feature = "negative-config-ini")]
fn negative_config_ini() {
    let _ = axutils::config::ConfigFormat::Ini;
}

#[cfg(feature = "negative-config-async")]
fn negative_config_async() {
    let _ = axutils::config::ConfigLoader::load_value_async;
}

#[cfg(feature = "negative-email-async")]
fn negative_email_async() {
    let _ = axutils::email::EmailClient::send_async;
}

#[cfg(feature = "negative-http-async")]
fn negative_http_async() {
    let _ = axutils::http::HttpClient::execute_async;
}

#[cfg(feature = "negative-http-json")]
fn negative_http_json() {
    let _ = axutils::http::HttpClient::get::<(), ()>;
}

#[cfg(feature = "negative-redis-cluster")]
fn negative_redis_cluster() {
    let _ = axutils::redis::RedisConfig::cluster::<Vec<String>, String>;
}

#[cfg(feature = "negative-redis-async")]
fn negative_redis_async() {
    let _ = axutils::redis::RedisClient::ping_async;
}

#[cfg(feature = "negative-sqlx-root")]
fn negative_sqlx_root() {
    let _ = axutils::SqlxClient::connect;
}

#[cfg(feature = "negative-sqlx-old-init")]
fn negative_sqlx_old_init() {
    let _ = axutils::utils::SqlxUtils::init;
}

#[cfg(feature = "negative-convert-integer")]
fn negative_convert_integer() {
    let _ = axutils::convert::IntegerBuffer::new;
}

#[cfg(feature = "negative-convert-float")]
fn negative_convert_float() {
    let _ = axutils::convert::FloatBuffer::new;
}

#[cfg(feature = "negative-convert-uuid")]
fn negative_convert_uuid() {
    let _ = axutils::convert::UuidBuffer::new;
}

#[cfg(feature = "negative-convert-ryu")]
fn negative_convert_ryu() {
    let _ = axutils::convert::FloatFormat::Ryu;
}

#[cfg(feature = "negative-convert-zmij")]
fn negative_convert_zmij() {
    let _ = axutils::convert::FloatFormat::Zmij;
}

#[cfg(feature = "negative-convert-sealed")]
fn negative_convert_sealed() {
    struct Custom;
    impl axutils::convert::IntegerValue for Custom {
        fn format_into<'a>(_: Self, _: &'a mut axutils::convert::IntegerBuffer) -> &'a str {
            "custom"
        }
    }
}

#[cfg(feature = "negative-rand")]
fn negative_rand() {
    let _ = axutils::utils::RandomUtils::numeric_string;
}

#[cfg(feature = "negative-redis-random")]
fn negative_redis_random() {
    let _ = axutils::utils::RandomUtils::numeric_string;
}

#[cfg(feature = "negative-base64")]
fn negative_base64() {
    let _ = axutils::utils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-md5")]
fn negative_md5() {
    let _ = axutils::utils::CryptoUtils::md5;
}

#[cfg(feature = "negative-aes")]
fn negative_aes() {
    let _ = axutils::crypto::AesCipher::from_key_bytes::<&[u8]>;
}

#[cfg(feature = "negative-encoding-rs")]
fn negative_encoding_rs() {
    let _ = axutils::crypto::TextEncoding::Gbk;
}

#[cfg(feature = "negative-aes-base64")]
fn negative_aes_base64() {
    let _ = axutils::crypto::AesCipher::encrypt_base64;
}

#[cfg(feature = "negative-jwt")]
fn negative_jwt() {
    let _ = axutils::jwt::JwtCodec::new;
}

#[cfg(feature = "negative-jwt-config")]
fn negative_jwt_config() {
    let _ = axutils::config::ConfigLoader::new;
}

#[cfg(feature = "negative-logging")]
fn negative_logging() {
    let _ = axutils::logging::LogConfig::new;
}

#[cfg(any(
    feature = "negative-time-unsuffixed-chrono",
    feature = "negative-time-unsuffixed-all"
))]
fn negative_time_unsuffixed() {
    let _ = axutils::utils::TimeUtils::format_date;
    let _ = axutils::utils::TimeUtils::format_option_date;
    let _ = axutils::utils::TimeUtils::format_datetime;
    let _ = axutils::utils::TimeUtils::format_option_datetime;
    let _ = axutils::utils::TimeUtils::format_datetime_with_offset;
    let _ = axutils::utils::TimeUtils::format_option_datetime_with_offset;
}

fn main() {}
