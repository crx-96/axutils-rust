#![cfg(feature = "http")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use axutils::{
    DeduplicationPolicy, HttpClient, HttpConfig, HttpError, HttpHeaders, HttpMethod, HttpRequest,
    RetryPolicy,
};

struct TestResponse {
    status: u16,
    body: &'static [u8],
    delay: Duration,
}

fn spawn_server(responses: Vec<TestResponse>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept test request");
            read_request(&mut stream);
            if !response.delay.is_zero() {
                thread::sleep(response.delay);
            }
            let header = format!(
                "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                response.body.len()
            );
            stream
                .write_all(header.as_bytes())
                .expect("write response headers");
            stream
                .write_all(response.body)
                .expect("write response body");
            stream.flush().expect("flush response");
        }
    });
    (address, handle)
}

fn read_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set server read timeout");
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while request.len() < 64 * 1024 {
        let Ok(read) = stream.read(&mut byte) else {
            break;
        };
        if read == 0 {
            break;
        }
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header_text = String::from_utf8_lossy(&request);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
        })
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = request.len();
    let mut remaining = content_length.saturating_sub(request.len().saturating_sub(body_start));
    while remaining > 0 {
        let Ok(read) = stream.read(&mut byte) else {
            break;
        };
        if read == 0 {
            break;
        }
        remaining -= read;
    }
}

fn client(base_url: &str) -> HttpClient {
    let config = HttpConfig::builder()
        .base_url(base_url)
        .expect("valid base URL")
        .request_timeout(Duration::from_secs(2))
        .expect("valid timeout")
        .connect_timeout(Duration::from_millis(500))
        .expect("valid connect timeout")
        .build()
        .expect("valid HTTP config");
    HttpClient::new(config).expect("build HTTP client")
}

#[test]
fn sync_returns_http_errors_as_responses_and_retries_transient_statuses() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 503,
            body: b"temporary",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 503,
            body: b"temporary",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 200,
            body: b"ok",
            delay: Duration::ZERO,
        },
    ]);
    let retry = RetryPolicy::new()
        .with_max_retries(3)
        .expect("retry count")
        .with_backoff(Duration::from_millis(1), Duration::from_millis(2))
        .expect("backoff");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .request_timeout(Duration::from_secs(1))
        .expect("timeout")
        .retry_policy(retry)
        .build()
        .expect("config");
    let response = HttpClient::new(config)
        .expect("client")
        .execute(HttpRequest::new(HttpMethod::Get, "/status").expect("request"))
        .expect("response");

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().expect("UTF-8 response"), "ok");
    assert_eq!(response.attempts(), 3);
    server.join().expect("server thread");
}

#[test]
fn default_config_requires_absolute_urls_without_a_base_url() {
    let config = HttpConfig::builder().build().expect("default config");
    assert_eq!(config.base_url(), None);
    assert_eq!(config.request_timeout(), Duration::from_secs(30));
    assert_eq!(config.connect_timeout(), Duration::from_secs(10));
    assert_eq!(config.retry_policy().max_retries(), 3);
    assert_eq!(
        RetryPolicy::new().with_max_retries(0),
        Err(HttpError::InvalidConfig {
            field: "max_retries"
        })
    );

    let error = HttpClient::new(config)
        .expect("client")
        .execute(HttpRequest::new(HttpMethod::Get, "/relative").expect("request"))
        .expect_err("relative URL without base URL must fail");
    assert_eq!(error, HttpError::InvalidUrl);
}

#[test]
fn one_total_attempt_disables_automatic_retries() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 503,
        body: b"one attempt",
        delay: Duration::ZERO,
    }]);
    let retry = RetryPolicy::new()
        .with_max_retries(1)
        .expect("one total attempt");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .retry_policy(retry)
        .build()
        .expect("config");
    let response = HttpClient::new(config)
        .expect("client")
        .execute(HttpRequest::new(HttpMethod::Get, "/one-attempt").expect("request"))
        .expect("final HTTP response");

    assert_eq!(response.status(), 503);
    assert_eq!(response.attempts(), 1);
    server.join().expect("server thread");
}

#[test]
fn absolute_request_url_takes_precedence_over_configured_base_url() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"absolute",
        delay: Duration::ZERO,
    }]);
    let config = HttpConfig::builder()
        .base_url("http://127.0.0.1:1/")
        .expect("base URL")
        .build()
        .expect("config");
    let response = HttpClient::new(config)
        .expect("client")
        .execute(
            HttpRequest::new(HttpMethod::Get, format!("{address}/absolute"))
                .expect("absolute request"),
        )
        .expect("absolute URL should be used");
    assert_eq!(response.body(), b"absolute");
    server.join().expect("server thread");
}

#[test]
fn client_and_global_entry_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<HttpClient>();
    assert_send_sync::<axutils::HttpUtils>();
}

#[test]
fn non_idempotent_methods_do_not_retry_by_default_and_redirects_are_returned() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 503,
        body: b"no retry",
        delay: Duration::ZERO,
    }]);
    let response = client(&address)
        .execute(
            HttpRequest::new(HttpMethod::Post, "/post")
                .expect("request")
                .with_body(b"payload".to_vec())
                .expect("body"),
        )
        .expect("response");
    assert_eq!(response.status(), 503);
    assert_eq!(response.attempts(), 1);
    server.join().expect("server thread");

    let (address, server) = spawn_server(vec![TestResponse {
        status: 302,
        body: b"redirect",
        delay: Duration::ZERO,
    }]);
    let response = client(&address)
        .execute(HttpRequest::new(HttpMethod::Get, "/redirect").expect("request"))
        .expect("response");
    assert_eq!(response.status(), 302);
    server.join().expect("server thread");
}

#[test]
fn response_limits_and_header_validation_are_enforced() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"012345",
        delay: Duration::ZERO,
    }]);
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .max_response_body_bytes(4)
        .expect("response limit")
        .build()
        .expect("config");
    let error = HttpClient::new(config)
        .expect("client")
        .execute(HttpRequest::new(HttpMethod::Get, "/large").expect("request"))
        .expect_err("large body must fail");
    assert_eq!(error, HttpError::ResponseTooLarge { limit: 4 });
    server.join().expect("server thread");

    assert!(HttpRequest::new(HttpMethod::Get, "http://user:password@example.com/").is_err());
    assert!(HttpRequest::new(HttpMethod::Get, "http://example.com/#fragment").is_err());
    assert!(HttpRequest::new(HttpMethod::Get, "//example.com/private").is_err());
    let mut headers = HttpHeaders::new();
    assert!(headers.set("X-Test", "ok\r\nInjected: yes").is_err());
    headers
        .append("authorization", "Bearer token")
        .expect("first auth");
    assert_eq!(
        headers.append("Authorization", "second"),
        Err(HttpError::DuplicateSensitiveHeader)
    );
}

#[test]
fn sync_single_flight_merges_concurrent_safe_requests() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"shared",
        delay: Duration::from_millis(100),
    }]);
    let client = Arc::new(client(&address));
    let mut workers = Vec::new();
    for _ in 0..4 {
        let client = Arc::clone(&client);
        workers.push(thread::spawn(move || {
            client
                .execute(HttpRequest::new(HttpMethod::Get, "/same").expect("request"))
                .expect("response")
        }));
    }
    for worker in workers {
        let response = worker.join().expect("worker");
        assert_eq!(response.body(), b"shared");
    }
    server.join().expect("server thread");
}

#[test]
fn coalescing_key_includes_headers_and_get_bodies_are_not_default_repeatable() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 200,
            body: b"one",
            delay: Duration::from_millis(50),
        },
        TestResponse {
            status: 200,
            body: b"two",
            delay: Duration::from_millis(50),
        },
    ]);
    let shared_client = Arc::new(client(&address));
    let first_client = Arc::clone(&shared_client);
    let first = thread::spawn(move || {
        first_client
            .execute(
                HttpRequest::new(HttpMethod::Get, "/headers")
                    .expect("request")
                    .with_header("x-variant", "one")
                    .expect("header"),
            )
            .expect("response")
    });
    let second_client = Arc::clone(&shared_client);
    let second = thread::spawn(move || {
        second_client
            .execute(
                HttpRequest::new(HttpMethod::Get, "/headers")
                    .expect("request")
                    .with_header("x-variant", "two")
                    .expect("header"),
            )
            .expect("response")
    });
    let bodies = [first.join().expect("first"), second.join().expect("second")]
        .map(|response| response.body().to_vec());
    assert!(bodies.iter().any(|body| body == b"one"));
    assert!(bodies.iter().any(|body| body == b"two"));
    server.join().expect("server thread");

    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 200,
            body: b"body-one",
            delay: Duration::from_millis(50),
        },
        TestResponse {
            status: 200,
            body: b"body-two",
            delay: Duration::from_millis(50),
        },
    ]);
    let client = Arc::new(client(&address));
    let mut workers = Vec::new();
    for body in [b"one".to_vec(), b"two".to_vec()] {
        let client = Arc::clone(&client);
        workers.push(thread::spawn(move || {
            client
                .execute(
                    HttpRequest::new(HttpMethod::Get, "/get-body")
                        .expect("request")
                        .with_body(body)
                        .expect("body"),
                )
                .expect("response")
        }));
    }
    for worker in workers {
        assert!(worker.join().expect("worker").is_success());
    }
    server.join().expect("server thread");
}

#[test]
fn completed_cache_requires_explicit_ttl_and_expires() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 200,
            body: b"first",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 200,
            body: b"second",
            delay: Duration::ZERO,
        },
    ]);
    let policy = DeduplicationPolicy::with_completed_ttl(Duration::from_millis(30), 8, 4, 1024)
        .expect("cache policy");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .deduplication_policy(policy)
        .build()
        .expect("config");
    let client = HttpClient::new(config).expect("client");
    let request = || HttpRequest::new(HttpMethod::Get, "/cached").expect("request");
    assert_eq!(client.execute(request()).expect("first").body(), b"first");
    assert_eq!(client.execute(request()).expect("cached").body(), b"first");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(
        client.execute(request()).expect("expired").body(),
        b"second"
    );
    server.join().expect("server thread");
}

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_entry_rejects_tokio_runtime_and_async_entry_works() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"async",
        delay: Duration::ZERO,
    }]);
    let client = client(&address);
    let request = HttpRequest::new(HttpMethod::Get, "/async").expect("request");
    assert!(matches!(
        client.execute(request.clone()),
        Err(HttpError::BlockingInAsyncRuntime)
    ));
    let response = client.execute_async(request).await.expect("async response");
    assert_eq!(response.body(), b"async");
    server.join().expect("server thread");
}

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_retry_attempt_count_includes_initial_request() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 503,
            body: b"temporary",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 503,
            body: b"temporary",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 200,
            body: b"async-ok",
            delay: Duration::ZERO,
        },
    ]);
    let client = client(&address);
    let response = client
        .execute_async(HttpRequest::new(HttpMethod::Get, "/async-retry").expect("request"))
        .await
        .expect("async response");

    assert_eq!(response.status(), 200);
    assert_eq!(response.body(), b"async-ok");
    assert_eq!(response.attempts(), 3);
    server.join().expect("server thread");
}

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_single_flight_merges_safe_requests() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"async-shared",
        delay: Duration::from_millis(80),
    }]);
    let client = Arc::new(client(&address));
    let mut tasks = Vec::new();
    for _ in 0..3 {
        let client = Arc::clone(&client);
        tasks.push(tokio::spawn(async move {
            client
                .execute_async(HttpRequest::new(HttpMethod::Get, "/async-same").expect("request"))
                .await
                .expect("response")
        }));
    }
    for task in tasks {
        assert_eq!(task.await.expect("task").body(), b"async-shared");
    }
    server.join().expect("server thread");
}
