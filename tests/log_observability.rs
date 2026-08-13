#![cfg(feature = "tracing")]

#[cfg(feature = "http")]
use std::io::Read;
use std::io::Write;
#[cfg(feature = "http")]
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "serde")]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
#[cfg(feature = "http")]
use std::thread;
#[cfg(any(feature = "http", all(feature = "sqlx", feature = "tokio")))]
use std::time::Duration;

#[cfg(all(feature = "sqlx", feature = "tokio"))]
use axutils::SqlxError;
#[cfg(feature = "serde")]
use axutils::{ConfigFormat, ConfigLoader};
#[cfg(feature = "http")]
use axutils::{
    HttpClient, HttpConfig, HttpError, HttpMethod, HttpRequest, HttpTransportErrorKind, RetryPolicy,
};
use tracing_subscriber::fmt::writer::MakeWriter;

#[cfg(feature = "http")]
const HTTP_SENTINEL: &str = "AXUTILS_HTTP_URL_SECRET";
#[cfg(feature = "http")]
const HTTP_HEADER_SENTINEL: &str = "AXUTILS_HTTP_HEADER_SECRET";
#[cfg(feature = "http")]
const HTTP_BODY_SENTINEL: &str = "AXUTILS_HTTP_BODY_SECRET";
#[cfg(all(feature = "sqlx", feature = "tokio"))]
const SQL_SENTINEL: &str = "AXUTILS_SQL_BIND_SECRET";
#[cfg(all(feature = "sqlx", feature = "tokio"))]
const SQL_TEXT_SENTINEL: &str = "AXUTILS_SQL_TEXT_SECRET";
#[cfg(feature = "serde")]
const CONFIG_SENTINEL: &str = "AXUTILS_CONFIG_PATH_SECRET";

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

#[test]
#[cfg(feature = "http")]
fn captures_sync_http_events_without_sensitive_context() {
    let (url, server) = spawn_http_server();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let client = HttpClient::new(HttpConfig::default()).expect("HTTP client");
        let request = HttpRequest::new(HttpMethod::Get, format!("{url}/{HTTP_SENTINEL}"))
            .expect("HTTP request")
            .with_header("x-axutils-secret", HTTP_HEADER_SENTINEL)
            .expect("HTTP request header")
            .with_body(HTTP_BODY_SENTINEL.as_bytes())
            .expect("HTTP request body");
        let response = client.execute(request).expect("loopback HTTP response");
        assert_eq!(response.status(), 200);
    });

    server.join().expect("HTTP server");
    let output = captured(&capture);
    assert!(
        output.contains("axutils::http"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("request_complete"),
        "captured output:\n{output}"
    );
    assert_eq!(count_event(&output, "axutils::http", "request_complete"), 1);
    assert!(!output.contains(HTTP_SENTINEL));
    assert!(!output.contains(HTTP_HEADER_SENTINEL));
    assert!(!output.contains(HTTP_BODY_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(feature = "serde")]
fn captures_sync_config_events_without_sensitive_context() {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();
    let config_path = unique_config_path("sync");
    std::fs::write(&config_path, format!(r#"{{"value":"{CONFIG_SENTINEL}"}}"#))
        .expect("write config");

    tracing::subscriber::with_default(subscriber, || {
        let value = ConfigLoader::new()
            .load_value(&config_path)
            .expect("config value");
        assert_eq!(
            value.get("value").and_then(|value| value.as_str()),
            Some(CONFIG_SENTINEL)
        );
        assert!(ConfigLoader::new()
            .parse_value("{", ConfigFormat::Json)
            .is_err());
        assert!(ConfigLoader::new()
            .parse::<u16>(
                "PORT=\"${AXUTILS_LOG_OBSERVABILITY_MISSING_VARIABLE}\"",
                ConfigFormat::Env,
            )
            .is_err());
        let missing_path = unique_config_path("missing");
        let _ = std::fs::remove_file(&missing_path);
        assert!(ConfigLoader::new().load_value(missing_path).is_err());
    });

    let _ = std::fs::remove_file(&config_path);
    let output = captured(&capture);
    assert!(
        output.contains("axutils::config"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("operation=\"read\"") || output.contains("operation = \"read\""),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("operation=\"parse\"") || output.contains("operation = \"parse\""),
        "captured output:\n{output}"
    );
    assert!(output.contains("error_kind=\"parse\"") || output.contains("error_kind = \"parse\""));
    assert!(
        output.contains("error_kind=\"undefined_variable\"")
            || output.contains("error_kind = \"undefined_variable\""),
        "captured output:\n{output}"
    );
    assert!(output.contains("error_kind=\"io\"") || output.contains("error_kind = \"io\""));
    assert_eq!(count_event(&output, "axutils::config", "read"), 2);
    assert_eq!(count_event(&output, "axutils::config", "parse"), 3);
    assert!(!output.contains(CONFIG_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(feature = "serde")]
fn info_filter_hides_success_events_but_keeps_failures() {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::INFO)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        ConfigLoader::new()
            .parse_value("{}", ConfigFormat::Json)
            .expect("valid JSON");
        assert!(ConfigLoader::new()
            .parse_value("{", ConfigFormat::Json)
            .is_err());
    });

    let output = captured(&capture);
    assert_eq!(count_event(&output, "axutils::config", "parse"), 1);
    assert!(output.contains("outcome=\"error\"") || output.contains("outcome = \"error\""));
    assert!(!output.contains("outcome=\"success\""));
}

#[test]
#[cfg(all(feature = "http", feature = "tokio"))]
fn captures_async_http_events_without_sensitive_context() {
    let (url, server) = spawn_http_server();
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    tracing::subscriber::with_default(subscriber, || {
        runtime.block_on(async {
            let client = HttpClient::new(HttpConfig::default()).expect("HTTP client");
            let request = HttpRequest::new(HttpMethod::Get, format!("{url}/{HTTP_SENTINEL}"))
                .expect("HTTP request");
            let response = client
                .execute_async(request)
                .await
                .expect("async HTTP response");
            assert_eq!(response.status(), 200);
        })
    });

    server.join().expect("HTTP server");
    let output = captured(&capture);
    assert!(
        output.contains("axutils::http"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("request_complete"),
        "captured output:\n{output}"
    );
    assert_eq!(count_event(&output, "axutils::http", "request_complete"), 1);
    assert!(!output.contains(HTTP_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(all(feature = "sqlx", feature = "tokio"))]
fn captures_sqlx_events_and_exact_row_counts_without_sensitive_context() {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");

    tracing::subscriber::with_default(subscriber, || {
        runtime.block_on(async {
            let sqlx = axutils::SqlxClient::connect(
                axutils::SqlxConfig::new("sqlite::memory:")
                    .expect("SQLite config")
                    .with_max_rows(1)
                    .expect("row limit"),
            )
            .await
            .expect("SQLite client");
            let timeout_client = axutils::SqlxClient::connect(
                axutils::SqlxConfig::new("sqlite::memory:")
                    .expect("timeout SQLite config")
                    .with_acquire_timeout(Duration::from_millis(10))
                    .expect("timeout acquire limit"),
            )
            .await
            .expect("timeout SQLite client");
            let held = timeout_client.begin_async().await.expect("hold connection");
            assert!(matches!(
                timeout_client
                    .execute_async(timeout_client.query("SELECT 1"))
                    .await,
                Err(SqlxError::PoolAcquireTimeout)
            ));
            held.rollback().await.expect("release held connection");
            sqlx.execute_async(sqlx.query(&format!(
                "CREATE TABLE {SQL_TEXT_SENTINEL} (value TEXT NOT NULL)"
            )))
            .await
            .expect("create table");
            sqlx.execute_async(
                sqlx.query(&format!(
                    "INSERT INTO {SQL_TEXT_SENTINEL} (value) VALUES (?)"
                ))
                .bind(SQL_SENTINEL),
            )
            .await
            .expect("insert value");
            sqlx.execute_async(
                sqlx.query(&format!(
                    "INSERT INTO {SQL_TEXT_SENTINEL} (value) VALUES (?)"
                ))
                .bind(SQL_SENTINEL),
            )
            .await
            .expect("insert second value");
            assert!(matches!(
                sqlx.fetch_all_async(
                    sqlx.query(&format!("SELECT value FROM {SQL_TEXT_SENTINEL}")),
                )
                    .await,
                Err(SqlxError::RowLimitExceeded { limit: 1 })
            ));
            assert!(matches!(
                sqlx.fetch_one_async(sqlx.query(&format!(
                    "SELECT value FROM {SQL_TEXT_SENTINEL} WHERE 1 = 0"
                )))
                .await,
                Err(SqlxError::RowNotFound)
            ));
        })
    });

    let output = captured(&capture);
    assert!(
        output.contains("axutils::sqlx"),
        "captured output:\n{output}"
    );
    assert!(output.contains("connect"), "captured output:\n{output}");
    assert!(output.contains("fetch_all"), "captured output:\n{output}");
    assert!(
        output.contains("row_limit_exceeded"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("pool_acquire_timeout"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("row_not_found"),
        "captured output:\n{output}"
    );
    let row_limit_events = output
        .lines()
        .filter(|line| line.contains("fetch_all") && line.contains("row_limit_exceeded"))
        .collect::<Vec<_>>();
    assert_eq!(row_limit_events.len(), 1, "captured output:\n{output}");
    assert!(
        row_limit_events[0]
            .split_ascii_whitespace()
            .any(|field| field == "rows=1"),
        "row-limit event must report the retained row count:\n{output}"
    );
    assert_eq!(count_event(&output, "axutils::sqlx", "connect"), 2);
    assert_eq!(count_event(&output, "axutils::sqlx", "execute"), 4);
    assert_eq!(count_event(&output, "axutils::sqlx", "fetch_all"), 1);
    assert_eq!(count_event(&output, "axutils::sqlx", "fetch_one"), 1);
    assert!(!output.contains(SQL_SENTINEL));
    assert!(!output.contains(SQL_TEXT_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(all(feature = "serde", feature = "tokio"))]
fn captures_async_config_events_without_sensitive_context() {
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();
    let config_path = unique_config_path("async");
    std::fs::write(&config_path, format!(r#"{{"value":"{CONFIG_SENTINEL}"}}"#))
        .expect("write config");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");

    tracing::subscriber::with_default(subscriber, || {
        runtime.block_on(async {
            let value = ConfigLoader::new()
                .load_value_async(&config_path)
                .await
                .expect("async config value");
            assert_eq!(
                value.get("value").and_then(|value| value.as_str()),
                Some(CONFIG_SENTINEL)
            );
        })
    });

    let _ = std::fs::remove_file(&config_path);
    let output = captured(&capture);
    assert!(
        output.contains("axutils::config"),
        "captured output:\n{output}"
    );
    assert!(
        output.contains("mode=\"async\"") || output.contains("mode = \"async\""),
        "captured output:\n{output}"
    );
    assert_eq!(count_event(&output, "axutils::config", "read"), 1);
    assert_eq!(count_event(&output, "axutils::config", "parse"), 1);
    assert!(!output.contains(CONFIG_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

#[test]
#[cfg(feature = "http")]
fn captures_http_retry_and_timeout_failure_events() {
    let (retry_url, retry_server) = spawn_status_http_server(vec![503, 200]);
    let capture = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(Capture(Arc::clone(&capture)))
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        let retry_policy = RetryPolicy::new()
            .with_max_retries(2)
            .expect("two total attempts")
            .with_backoff(Duration::from_millis(1), Duration::from_millis(1))
            .expect("retry backoff");
        let retry_config = HttpConfig::builder()
            .base_url(&retry_url)
            .expect("retry base URL")
            .request_timeout(Duration::from_secs(1))
            .expect("retry timeout")
            .retry_policy(retry_policy)
            .build()
            .expect("retry config");
        let retry_client = HttpClient::new(retry_config).expect("retry client");
        let response = retry_client
            .execute(HttpRequest::new(HttpMethod::Get, "/retry").expect("retry request"))
            .expect("retry response");
        assert_eq!(response.status(), 200);

        let (timeout_url, timeout_server) = spawn_delayed_http_server(Duration::from_millis(100));
        let timeout_config = HttpConfig::builder()
            .base_url(&timeout_url)
            .expect("timeout base URL")
            .request_timeout(Duration::from_millis(20))
            .expect("timeout request budget")
            .connect_timeout(Duration::from_millis(20))
            .expect("timeout connect budget")
            .retry_policy(
                RetryPolicy::new()
                    .with_max_retries(1)
                    .expect("single timeout attempt"),
            )
            .build()
            .expect("timeout config");
        let timeout_result = HttpClient::new(timeout_config)
            .expect("timeout client")
            .execute(HttpRequest::new(HttpMethod::Get, "/timeout").expect("timeout request"));
        assert!(matches!(
            timeout_result,
            Err(HttpError::Transport {
                kind: HttpTransportErrorKind::Timeout,
                ..
            })
        ));
        timeout_server.join().expect("timeout server");
    });

    retry_server.join().expect("retry server");
    let output = captured(&capture);
    assert!(
        output.contains("request_retry"),
        "captured output:\n{output}"
    );
    assert_eq!(count_event(&output, "axutils::http", "request_complete"), 2);
    assert_eq!(count_event(&output, "axutils::http", "request_retry"), 1);
    assert!(output.contains("scheduled"), "captured output:\n{output}");
    assert!(
        output.contains("error_kind=\"timeout\"") || output.contains("error_kind = \"timeout\"")
    );
    assert!(!output.contains(HTTP_SENTINEL));
    assert!(!output.contains('\u{1b}'));
}

fn captured(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(buffer.lock().expect("capture lock").clone()).expect("UTF-8 logs")
}

fn count_event(output: &str, target: &str, operation: &str) -> usize {
    output
        .lines()
        .filter(|line| line.contains(target) && line.contains(operation))
        .count()
}

#[cfg(feature = "http")]
fn spawn_http_server() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback HTTP server");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept loopback request");
        read_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .expect("write loopback response");
    });
    (address, handle)
}

#[cfg(feature = "http")]
fn spawn_status_http_server(statuses: Vec<u16>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind status HTTP server");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        for status in statuses {
            let (mut stream, _) = listener.accept().expect("accept status HTTP request");
            read_request(&mut stream);
            let body = if status >= 500 {
                b"retry".as_slice()
            } else {
                b"ok".as_slice()
            };
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write status response headers");
            stream.write_all(body).expect("write status response body");
        }
    });
    (address, handle)
}

#[cfg(feature = "http")]
fn spawn_delayed_http_server(delay: Duration) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind delayed HTTP server");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept delayed HTTP request");
        read_request(&mut stream);
        thread::sleep(delay);
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    (address, handle)
}

#[cfg(feature = "http")]
fn read_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .expect("set read timeout");
    let mut buffer = [0_u8; 1024];
    let mut request = Vec::new();
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut buffer).expect("read request");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        assert!(request.len() < 64 * 1024, "request too large");
    }
}

#[cfg(feature = "serde")]
fn unique_config_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "axutils-log-observability-{suffix}-{}-{CONFIG_SENTINEL}.json",
        std::process::id()
    ))
}
