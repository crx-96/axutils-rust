#![cfg(feature = "http")]

use std::io::ErrorKind;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
    listener
        .set_nonblocking(true)
        .expect("set test listener nonblocking");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        let expected = responses.len();
        for (received, response) in responses.into_iter().enumerate() {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "test server received {received} of {expected} expected requests"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept test request: {error}"),
                }
            };
            // Windows may inherit the listener's nonblocking mode; read timeouts do not
            // turn an accepted nonblocking stream into a blocking one.
            stream.set_nonblocking(false).unwrap_or_else(|error| {
                panic!("set test request #{received} stream blocking: {error}")
            });
            let peer = stream
                .peer_addr()
                .unwrap_or_else(|error| panic!("inspect test request #{received} peer: {error}"));
            read_request(&mut stream).unwrap_or_else(|error| {
                panic!("read test request #{received} from {peer}: {error}")
            });
            if !response.delay.is_zero() {
                thread::sleep(response.delay);
            }
            let header = format!(
                "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                response.body.len()
            );
            if let Err(error) = stream.write_all(header.as_bytes()) {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} headers not delivered: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("write test response #{received} to {peer} headers: {error}");
            }
            if let Err(error) = stream.write_all(response.body) {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} body not delivered: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("write test response #{received} to {peer} body: {error}");
            }
            if let Err(error) = stream.flush() {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} flush incomplete: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("flush test response #{received} to {peer}: {error}");
            }
        }
    });
    (address, handle)
}

fn is_client_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::NotConnected
    ) || matches!(error.raw_os_error(), Some(10053 | 10054))
}

fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while request.len() < 64 * 1024 {
        let read = stream.read(&mut byte)?;
        if read == 0 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "request ended before headers",
            ));
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
        let read = stream.read(&mut byte)?;
        if read == 0 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                format!("request body ended with {remaining} bytes remaining"),
            ));
        }
        remaining -= read;
    }
    Ok(request)
}

fn spawn_observing_server(
    response: TestResponse,
) -> (String, Arc<Mutex<Vec<u8>>>, thread::JoinHandle<()>) {
    // Bind the wildcard loopback-compatible IPv4 socket so the test can use two different
    // loopback host strings without depending on platform-specific `localhost` resolution.
    let listener = TcpListener::bind(("0.0.0.0", 0)).expect("bind observing server");
    listener
        .set_nonblocking(true)
        .expect("set observing listener nonblocking");
    let port = listener.local_addr().expect("server address").port();
    let address = format!("http://127.0.0.1:{port}");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_thread = Arc::clone(&observed);
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "observing server timed out");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept observing request: {error}"),
            }
        };
        // Keep the observing fixture's accepted stream compatible with its read timeout too.
        stream
            .set_nonblocking(false)
            .expect("set observing request stream blocking");
        let peer = stream.peer_addr().expect("inspect observing request peer");
        let request = read_request(&mut stream)
            .unwrap_or_else(|error| panic!("read observing request: {error}"));
        *observed_for_thread.lock().expect("observed request lock") = request;
        let header = format!(
            "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.status,
            response.body.len()
        );
        stream
            .write_all(header.as_bytes())
            .unwrap_or_else(|error| panic!("write observing response to {peer} headers: {error}"));
        stream
            .write_all(response.body)
            .unwrap_or_else(|error| panic!("write observing response to {peer} body: {error}"));
        stream
            .flush()
            .unwrap_or_else(|error| panic!("flush observing response to {peer}: {error}"));
    });
    (address, observed, handle)
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
fn transport_error_reports_retry_budget_separately_from_method_retryability() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve closed port");
    let address = listener.local_addr().expect("closed port address");
    drop(listener);

    let retry = RetryPolicy::new()
        .with_max_retries(3)
        .expect("retry count")
        .with_backoff(Duration::from_millis(1), Duration::from_millis(2))
        .expect("backoff");
    let config = HttpConfig::builder()
        .base_url(format!("http://{address}/"))
        .expect("base URL")
        .request_timeout(Duration::from_secs(1))
        .expect("timeout")
        .retry_policy(retry)
        .build()
        .expect("config");
    let error = HttpClient::new(config)
        .expect("client")
        .execute(
            HttpRequest::new(HttpMethod::Post, "/closed")
                .expect("request")
                .with_body(b"payload".to_vec())
                .expect("body"),
        )
        .expect_err("closed port should produce transport error");

    assert!(matches!(
        error,
        HttpError::Transport {
            attempts: 1,
            exhausted: false,
            ..
        }
    ));
}

#[test]
fn retry_wait_timeout_does_not_claim_retry_budget_is_exhausted() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 503,
        body: b"temporary",
        delay: Duration::ZERO,
    }]);
    let retry = RetryPolicy::new()
        .with_max_retries(3)
        .expect("retry count")
        .with_backoff(Duration::from_millis(200), Duration::from_millis(200))
        .expect("backoff");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .request_timeout(Duration::from_millis(50))
        .expect("timeout")
        .retry_policy(retry)
        .build()
        .expect("config");
    let error = HttpClient::new(config)
        .expect("client")
        .execute(HttpRequest::new(HttpMethod::Get, "/deadline").expect("request"))
        .expect_err("retry delay should exceed deadline");
    assert!(matches!(
        error,
        HttpError::Transport {
            kind: axutils::HttpTransportErrorKind::Timeout,
            attempts: 1,
            exhausted: false,
        }
    ));
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
fn cross_origin_requests_filter_sensitive_defaults_at_the_network_boundary() {
    let (address, observed, server) = spawn_observing_server(TestResponse {
        status: 200,
        body: b"ok",
        delay: Duration::ZERO,
    });
    let mut headers = HttpHeaders::new();
    headers
        .set("Authorization", "Bearer should-not-cross-origin")
        .expect("authorization header");
    headers
        .set("Cookie", "session=should-not-cross-origin")
        .expect("cookie header");
    headers.set("X-Visible", "kept").expect("ordinary header");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .default_headers(headers)
        .build()
        .expect("config");
    let client = HttpClient::new(config).expect("client");
    let port = address.rsplit(':').next().expect("port");
    let response = client
        .execute(
            HttpRequest::new(
                HttpMethod::Get,
                format!("http://127.0.0.2:{port}/cross-origin"),
            )
            .expect("request"),
        )
        .expect("cross-origin response");
    assert_eq!(response.body(), b"ok");
    server.join().expect("observing server");

    let observed = observed.lock().expect("observed request lock").clone();
    let request = String::from_utf8_lossy(&observed);
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
    assert!(!request.to_ascii_lowercase().contains("cookie:"));
    assert!(request.to_ascii_lowercase().contains("x-visible: kept"));
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
fn in_flight_capacity_bypasses_new_keys_without_blocking_existing_request() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 200,
            body: b"first",
            delay: Duration::from_millis(100),
        },
        TestResponse {
            status: 200,
            body: b"second",
            delay: Duration::ZERO,
        },
    ]);
    let policy = DeduplicationPolicy::in_flight(1).expect("in-flight policy");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .deduplication_policy(policy)
        .build()
        .expect("config");
    let client = Arc::new(HttpClient::new(config).expect("client"));
    let first_client = Arc::clone(&client);
    let first = thread::spawn(move || {
        first_client
            .execute(HttpRequest::new(HttpMethod::Get, "/first").expect("request"))
            .expect("first response")
    });
    thread::sleep(Duration::from_millis(20));
    let second = client
        .execute(HttpRequest::new(HttpMethod::Get, "/second").expect("request"))
        .expect("capacity-bypassed response");
    let first = first.join().expect("first worker");
    assert!(matches!(first.body(), b"first" | b"second"));
    assert!(matches!(second.body(), b"first" | b"second"));
    assert_ne!(first.body(), second.body());
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

#[test]
fn completed_cache_evicts_by_entry_and_body_budgets() {
    let (address, server) = spawn_server(vec![
        TestResponse {
            status: 200,
            body: b"one",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 200,
            body: b"two",
            delay: Duration::ZERO,
        },
        TestResponse {
            status: 200,
            body: b"three",
            delay: Duration::ZERO,
        },
    ]);
    let policy = DeduplicationPolicy::with_completed_ttl(Duration::from_secs(30), 8, 2, 4)
        .expect("cache policy");
    let config = HttpConfig::builder()
        .base_url(&address)
        .expect("base URL")
        .deduplication_policy(policy)
        .build()
        .expect("config");
    let client = HttpClient::new(config).expect("client");
    let first = client
        .execute(HttpRequest::new(HttpMethod::Get, "/first").expect("request"))
        .expect("first response");
    let second = client
        .execute(HttpRequest::new(HttpMethod::Get, "/second").expect("request"))
        .expect("second response");
    let first_again = client
        .execute(HttpRequest::new(HttpMethod::Get, "/first").expect("request"))
        .expect("evicted first response");
    assert_eq!(first.body(), b"one");
    assert_eq!(second.body(), b"two");
    assert_eq!(first_again.body(), b"three");
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

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follower_timeout_does_not_cancel_a_longer_leader() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"shared-after-follower-timeout",
        delay: Duration::from_millis(150),
    }]);
    let client = Arc::new(client(&address));
    let leader_client = Arc::clone(&client);
    let leader = tokio::spawn(async move {
        leader_client
            .execute_async(
                HttpRequest::new(HttpMethod::Get, "/follower-timeout")
                    .expect("request")
                    .with_timeout(Duration::from_millis(500))
                    .expect("leader timeout"),
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let follower = client
        .execute_async(
            HttpRequest::new(HttpMethod::Get, "/follower-timeout")
                .expect("request")
                .with_timeout(Duration::from_millis(30))
                .expect("follower timeout"),
        )
        .await
        .expect_err("follower should time out independently");
    assert_eq!(follower, HttpError::CoalescedWaitTimeout);
    let leader_response = leader.await.expect("leader task").expect("leader response");
    assert_eq!(leader_response.body(), b"shared-after-follower-timeout");
    server.join().expect("server thread");
}

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_leader_publishes_coalesced_cancellation_to_follower() {
    let (address, server) = spawn_server(vec![TestResponse {
        status: 200,
        body: b"leader-was-cancelled",
        delay: Duration::from_millis(200),
    }]);
    let client = Arc::new(client(&address));
    let leader_client = Arc::clone(&client);
    let leader = tokio::spawn(async move {
        leader_client
            .execute_async(HttpRequest::new(HttpMethod::Get, "/leader-cancel").expect("request"))
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    let follower_client = Arc::clone(&client);
    let follower = tokio::spawn(async move {
        follower_client
            .execute_async(HttpRequest::new(HttpMethod::Get, "/leader-cancel").expect("request"))
            .await
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    leader.abort();
    assert!(leader
        .await
        .expect_err("leader should be cancelled")
        .is_cancelled());
    assert_eq!(
        follower
            .await
            .expect("follower task")
            .expect_err("follower should receive cancellation"),
        HttpError::CoalescedRequestCancelled
    );
    server.join().expect("server thread");
}
