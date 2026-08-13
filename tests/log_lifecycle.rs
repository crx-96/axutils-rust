#![cfg(feature = "tracing")]

use std::io::Write;
use std::sync::{Arc, Mutex};

#[cfg(feature = "aes")]
use axutils::{AesMode, CryptoUtils};
#[cfg(feature = "lettre")]
use axutils::{EmailConfig, EmailSecurity, EmailUtils};
#[cfg(feature = "http")]
use axutils::{HttpConfig, HttpUtils};
#[cfg(feature = "jwt")]
use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtUtils, JwtValidation};
#[cfg(feature = "redis")]
use axutils::{RedisConfig, RedisUtils};
#[cfg(all(feature = "sqlx", feature = "tokio"))]
use axutils::{SqlxConfig, SqlxUtils};
use tracing_subscriber::fmt::writer::MakeWriter;

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("capture lock")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Capture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn capture_for(action: impl FnOnce()) -> String {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();
    tracing::subscriber::with_default(subscriber, action);
    let bytes = capture.lock().expect("capture lock").clone();
    String::from_utf8(bytes).expect("UTF-8 logs")
}

fn assert_single_init_event(output: &str, target: &str, operation: &str) {
    let count = output
        .lines()
        .filter(|line| line.contains(target) && line.contains(operation))
        .count();
    assert_eq!(
        count, 1,
        "expected one {target}/{operation} event:\n{output}"
    );
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(feature = "http")]
fn captures_http_init_without_base_url() {
    const URL_SENTINEL: &str = "AXUTILS_HTTP_CONFIG_SECRET";
    let output = capture_for(|| {
        let config = HttpConfig::builder()
            .base_url(format!("https://{URL_SENTINEL}.example.com"))
            .expect("HTTP config URL")
            .build()
            .expect("HTTP config");
        HttpUtils::init(config).expect("HTTP utility init");
    });
    assert_single_init_event(&output, "axutils::http", "client_init");
    assert!(!output.contains(URL_SENTINEL));
}

#[test]
#[cfg(feature = "redis")]
fn captures_redis_init_without_credentials_or_url() {
    const PASSWORD_SENTINEL: &str = "AXUTILS_REDIS_PASSWORD_SECRET";
    let url = format!("redis://:{PASSWORD_SENTINEL}@127.0.0.1:6379/0");
    let output = capture_for(|| {
        RedisUtils::init(RedisConfig::single(&url).expect("Redis config"))
            .expect("Redis utility init");
    });
    assert_single_init_event(&output, "axutils::redis", "client_init");
    assert!(!output.contains(PASSWORD_SENTINEL));
    assert!(!output.contains(&url));
}

#[test]
#[cfg(feature = "lettre")]
fn captures_email_init_without_account_data() {
    const HOST_SENTINEL: &str = "smtp-secret.example.com";
    const PASSWORD_SENTINEL: &str = "AXUTILS_EMAIL_PASSWORD_SECRET";
    let output = capture_for(|| {
        EmailUtils::init(
            EmailConfig::new(
                HOST_SENTINEL,
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                PASSWORD_SENTINEL,
                "sender@example.com",
            )
            .expect("email config"),
        )
        .expect("email utility init");
    });
    assert_single_init_event(&output, "axutils::email", "client_init");
    assert!(!output.contains(HOST_SENTINEL));
    assert!(!output.contains(PASSWORD_SENTINEL));
}

#[test]
#[cfg(feature = "jwt")]
fn captures_jwt_init_without_secret() {
    const SECRET_SENTINEL: &[u8; 32] = b"AXUTILS_JWT_SECRET_KEY_32_BYTES!";
    let output = capture_for(|| {
        JwtUtils::init(
            JwtConfig::new(
                JwtAlgorithm::Hs256,
                Some(JwtSigningKey::from_hmac_secret(SECRET_SENTINEL).expect("JWT key")),
                None,
                JwtValidation::new(),
            )
            .expect("JWT config"),
        )
        .expect("JWT utility init");
    });
    assert_single_init_event(&output, "axutils::jwt", "codec_init");
    assert!(!output.contains("AXUTILS_JWT_SECRET"));
    assert!(!output.contains(&format!("{SECRET_SENTINEL:?}")));
}

#[test]
#[cfg(feature = "aes")]
fn captures_aes_init_without_key_material() {
    const KEY_SENTINEL: &[u8; 32] = b"AXUTILS_AES_SECRET_KEY_32_BYTES!";
    let output = capture_for(|| {
        CryptoUtils::aes_init_from_bytes(KEY_SENTINEL, AesMode::Gcm).expect("AES utility init");
    });
    assert_single_init_event(&output, "axutils::crypto", "aes_init_from_bytes");
    assert!(!output.contains("AXUTILS_AES_SECRET"));
    assert!(!output.contains(&format!("{KEY_SENTINEL:?}")));
}

#[test]
#[cfg(all(feature = "sqlx", feature = "tokio"))]
fn captures_sqlx_global_init_once() {
    let output = capture_for(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        runtime.block_on(async {
            SqlxUtils::init(SqlxConfig::new("sqlite::memory:").expect("SQLite config"))
                .await
                .expect("SQLx utility init");
        });
    });
    assert_single_init_event(&output, "axutils::sqlx", "client_init");
    assert_eq!(
        output
            .lines()
            .filter(|line| line.contains("axutils::sqlx") && line.contains("connect"))
            .count(),
        1,
        "expected one SQLx connect event:\n{output}"
    );
}
