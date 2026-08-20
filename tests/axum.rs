#![cfg(all(feature = "axum", feature = "tokio"))]

use axum::{routing::get, Router};
use axutils::{AxumApp, AxumError, AxumShutdownReason};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

fn server() -> axutils::AxumServer {
    AxumApp::from_router(Router::new().route("/health", get(|| async { "ok" })))
        .into_server_builder()
        .build()
        .expect("build server")
}

#[tokio::test]
async fn loopback_serves_and_preserves_custom_shutdown_reason() {
    let server = server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let (tx, rx) = oneshot::channel();
    let running = {
        let server = server.clone();
        tokio::spawn(async move {
            server
                .serve_with_shutdown(listener, async move {
                    let _ = rx.await;
                    AxumShutdownReason::Custom("test".into())
                })
                .await
        })
    };
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
        .await
        .expect("write");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.expect("read");
    let text = String::from_utf8(response).expect("utf8 response");
    assert!(text.starts_with("HTTP/1.1 200"));
    assert!(text.ends_with("ok"));
    tx.send(()).expect("shutdown");
    let outcome = running.await.expect("join").expect("serve");
    assert_eq!(outcome.local_addr(), addr);
    assert_eq!(outcome.reason(), &AxumShutdownReason::Custom("test".into()));
    assert!(matches!(
        server.serve_addr(addr).await,
        Err(AxumError::AlreadyStopped)
    ));
}

#[tokio::test]
async fn bind_failure_rolls_back_to_ready() {
    let server = server();
    assert!(matches!(
        server
            .shutdown_handle()
            .shutdown(AxumShutdownReason::Programmatic),
        Err(AxumError::NotRunning)
    ));
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = occupied.local_addr().unwrap();
    assert!(matches!(
        server.serve_addr(addr).await,
        Err(AxumError::Io(_))
    ));
    drop(occupied);
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let outcome = server
        .serve_with_shutdown(listener, async { AxumShutdownReason::Programmatic })
        .await
        .unwrap();
    assert_eq!(outcome.reason(), &AxumShutdownReason::Programmatic);
}

#[tokio::test]
async fn shutdown_is_idempotent_and_first_reason_wins() {
    let server = server();
    let handle = server.shutdown_handle();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let running = {
        let server = server.clone();
        tokio::spawn(async move { server.serve(listener).await })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        handle
            .shutdown(AxumShutdownReason::Programmatic)
            .expect("first"),
        AxumShutdownReason::Programmatic
    );
    assert_eq!(
        handle
            .shutdown(AxumShutdownReason::Custom("late".into()))
            .expect("repeat"),
        AxumShutdownReason::Programmatic
    );
    assert_eq!(
        running.await.expect("join").expect("serve").reason(),
        &AxumShutdownReason::Programmatic
    );
}

#[tokio::test]
async fn programmatic_shutdown_stops_custom_serve_and_preserves_first_reason() {
    let server = server();
    let handle = server.shutdown_handle();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let running = {
        let server = server.clone();
        tokio::spawn(async move {
            server
                .serve_with_shutdown(listener, std::future::pending())
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        handle.shutdown(AxumShutdownReason::Programmatic).unwrap(),
        AxumShutdownReason::Programmatic
    );
    let outcome = tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(outcome.reason(), &AxumShutdownReason::Programmatic);
}

#[tokio::test]
async fn concurrent_serve_is_rejected_and_aborted_future_is_abandoned() {
    let server = server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let task = {
        let server = server.clone();
        tokio::spawn(async move {
            server
                .serve_with_shutdown(listener, std::future::pending())
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(20)).await;
    let other = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind other");
    assert!(matches!(
        server.serve(other).await,
        Err(AxumError::AlreadyRunning)
    ));
    task.abort();
    let _ = task.await;
    tokio::task::yield_now().await;
    assert!(matches!(
        server
            .serve_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await,
        Err(AxumError::Abandoned)
    ));
}

#[test]
fn config_rejects_unbounded_values() {
    assert!(matches!(
        axutils::AxumConfig::new().with_max_body_bytes(0),
        Err(AxumError::InvalidConfig {
            field: "max_body_bytes"
        })
    ));
    assert!(matches!(
        axutils::AxumConfig::new().with_max_concurrency(0),
        Err(AxumError::InvalidConfig {
            field: "max_concurrency"
        })
    ));
}

#[test]
fn global_axum_utils_initializes_only_once() {
    let first = axutils::AxumUtils::init(server());
    assert!(first.is_ok(), "fresh test process must initialize once");
    assert!(axutils::AxumUtils::is_initialized());
    assert!(matches!(
        axutils::AxumUtils::init(server()),
        Err(AxumError::AlreadyInitialized)
    ));
}

#[cfg(feature = "tower-http")]
#[tokio::test]
async fn request_id_removes_spoofed_input_and_overwrites_handler_response() {
    use axum::{
        http::{HeaderMap, HeaderValue},
        routing::get,
    };
    async fn handler(headers: HeaderMap) -> (HeaderMap, String) {
        let seen = headers
            .get("x-request-id")
            .expect("internal id")
            .to_str()
            .expect("text")
            .to_owned();
        let mut response = HeaderMap::new();
        response.insert("x-request-id", HeaderValue::from_static("handler-conflict"));
        (response, seen)
    }
    let server = AxumApp::from_router(Router::new().route("/id", get(handler)))
        .into_server_builder()
        .with_request_id()
        .build()
        .expect("build");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Custom("done".into())
            })
            .await
    });
    let response = http_request(
        addr,
        "/id",
        "x-request-id: spoofed\r\nx-request-id: second\r\n",
    )
    .await;
    let split = response.split("\r\n\r\n").collect::<Vec<_>>();
    let header_id = response
        .lines()
        .find_map(|line| line.strip_prefix("x-request-id: "))
        .expect("response id");
    assert_ne!(header_id, "spoofed");
    assert_ne!(header_id, "handler-conflict");
    assert_eq!(split[1], header_id);
    tx.send(()).expect("stop");
    task.await.expect("join").expect("serve");
}

#[cfg(feature = "tower-http")]
#[tokio::test]
async fn timeout_returns_configured_408() {
    let server = AxumApp::from_router(Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            "late"
        }),
    ))
    .into_server_builder()
    .with_timeout(
        Duration::from_millis(10),
        axutils::AxumTimeoutStatus::RequestTimeout,
    )
    .expect("timeout")
    .build()
    .expect("build");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Custom("done".into())
            })
            .await
    });
    let response = http_request(addr, "/slow", "").await;
    assert!(response.starts_with("HTTP/1.1 408"));
    tx.send(()).expect("stop");
    task.await.expect("join").expect("serve");
}

async fn http_request(addr: SocketAddr, path: &str, headers: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request =
        format!("GET {path} HTTP/1.1\r\nhost: localhost\r\n{headers}connection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.expect("read");
    String::from_utf8(response).expect("utf8")
}

#[cfg(feature = "tower")]
#[tokio::test]
async fn concurrency_limit_fails_fast_with_503() {
    use axum::extract::State;
    use std::sync::Arc;
    struct Gate {
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }
    async fn held(State(gate): State<Arc<Gate>>) -> &'static str {
        gate.entered.notify_one();
        gate.release.notified().await;
        "ok"
    }
    let gate = Arc::new(Gate {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let router = Router::new()
        .route("/held", get(held))
        .with_state(gate.clone());
    let server = AxumApp::from_router(router)
        .into_server_builder()
        .with_concurrency_limit(1)
        .expect("limit")
        .build()
        .expect("build");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel();
    let serving = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Custom("done".into())
            })
            .await
    });
    let first = tokio::spawn(http_request(addr, "/held", ""));
    gate.entered.notified().await;
    let second = tokio::time::timeout(Duration::from_millis(200), http_request(addr, "/held", ""))
        .await
        .expect("fail fast");
    assert!(second.starts_with("HTTP/1.1 503"));
    gate.release.notify_one();
    assert!(first.await.expect("first").starts_with("HTTP/1.1 200"));
    tx.send(()).expect("stop");
    serving.await.expect("join").expect("serve");
}

#[tokio::test]
async fn deferred_layers_keep_declaration_order_and_matched_scope() {
    use std::sync::{Arc, Mutex};
    let events = Arc::new(Mutex::new(Vec::new()));
    let layer = |name: &'static str, events: Arc<Mutex<Vec<&'static str>>>| {
        axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| {
                let events = events.clone();
                async move {
                    events.lock().expect("events").push(name);
                    let response = next.run(request).await;
                    events.lock().expect("events").push(match name {
                        "a" => "A",
                        "b" => "B",
                        _ => "M",
                    });
                    response
                }
            },
        )
    };
    let server = AxumApp::new()
        .route("/ok", get(|| async { "ok" }))
        .with_layer(layer("a", events.clone()))
        .with_layer(layer("b", events.clone()))
        .with_matched_route_layer(layer("m", events.clone()))
        .into_server_builder()
        .build()
        .expect("build");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel();
    let serving = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    assert!(http_request(addr, "/ok", "")
        .await
        .starts_with("HTTP/1.1 200"));
    assert_eq!(
        &*events.lock().expect("events"),
        &["a", "b", "m", "M", "B", "A"]
    );
    events.lock().expect("events").clear();
    assert!(http_request(addr, "/missing", "")
        .await
        .starts_with("HTTP/1.1 404"));
    assert_eq!(&*events.lock().expect("events"), &["a", "b", "B", "A"]);
    tx.send(()).expect("stop");
    serving.await.expect("join").expect("serve");
}

#[test]
fn matched_layer_on_empty_router_returns_error_instead_of_panicking() {
    let result = AxumApp::new()
        .with_matched_route_layer(axum::middleware::from_fn(
            |request: axum::extract::Request, next: axum::middleware::Next| async move {
                next.run(request).await
            },
        ))
        .into_server_builder()
        .build();
    assert!(matches!(
        result,
        Err(AxumError::InvalidConfig {
            field: "matched_route_layer"
        })
    ));
}

#[cfg(feature = "tower-http")]
#[test]
fn tower_http_config_rejects_unbounded_or_unsafe_values() {
    use axum::http::HeaderValue;
    use axutils::{AxumCorsConfig, AxumCorsOrigin, AxumTimeoutStatus};
    assert!(AxumApp::new()
        .into_server_builder()
        .with_timeout(Duration::ZERO, AxumTimeoutStatus::GatewayTimeout)
        .is_err());
    assert!(AxumApp::new()
        .into_server_builder()
        .with_body_limit(0)
        .is_err());
    assert!(AxumApp::new()
        .into_server_builder()
        .with_body_limit(64 * 1024 * 1024 + 1)
        .is_err());
    let unsafe_cors = AxumCorsConfig {
        origins: AxumCorsOrigin::Any,
        allow_credentials: true,
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(unsafe_cors)
        .is_err());
    let wildcard_list = AxumCorsConfig {
        origins: AxumCorsOrigin::List(vec![HeaderValue::from_static("*")]),
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(wildcard_list)
        .is_err());
    let wildcard_method = AxumCorsConfig {
        methods: vec![axum::http::Method::from_bytes(b"*").unwrap()],
        allow_credentials: true,
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(wildcard_method)
        .is_err());
    let wildcard_header = AxumCorsConfig {
        headers: vec![axum::http::HeaderName::from_bytes(b"*").unwrap()],
        allow_credentials: true,
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(wildcard_header)
        .is_err());
    let wildcard_expose = AxumCorsConfig {
        expose_headers: vec![axum::http::HeaderName::from_bytes(b"*").unwrap()],
        allow_credentials: true,
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(wildcard_expose)
        .is_err());
    let too_many = AxumCorsConfig {
        origins: AxumCorsOrigin::List(vec![HeaderValue::from_static("https://example.com"); 65]),
        ..Default::default()
    };
    assert!(AxumApp::new()
        .into_server_builder()
        .with_cors(too_many)
        .is_err());
}

#[cfg(feature = "tower-http")]
async fn sensitive_panic_handler() -> &'static str {
    panic!("sensitive-panic-payload")
}

#[cfg(feature = "tower-http")]
#[tokio::test]
async fn catch_panic_returns_sanitized_500() {
    let server = AxumApp::new()
        .route("/panic", get(sensitive_panic_handler))
        .into_server_builder()
        .with_catch_panic()
        .build()
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let running = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let response = http_request(addr, "/panic", "").await;
    assert!(response.starts_with("HTTP/1.1 500"));
    assert!(!response.contains("sensitive-panic-payload"));
    tx.send(()).unwrap();
    running.await.unwrap().unwrap();
}

#[cfg(feature = "tower_governor")]
#[test]
fn governor_rejects_excessive_burst() {
    assert!(AxumApp::new()
        .into_server_builder()
        .with_governor_peer(
            Duration::from_secs(1),
            std::num::NonZeroU32::new(65_537).unwrap()
        )
        .is_err());
}

#[cfg(feature = "tower_governor")]
#[tokio::test]
async fn governor_peer_limits_loopback_and_sanitizes_429() {
    use std::num::NonZeroU32;
    let server = AxumApp::from_router(Router::new().route("/", get(|| async { "ok" })))
        .into_server_builder()
        .with_governor_peer(
            Duration::from_secs(60),
            NonZeroU32::new(1).expect("nonzero"),
        )
        .expect("governor")
        .build()
        .expect("build");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel();
    let serving = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Custom("done".into())
            })
            .await
    });
    assert!(http_request(addr, "/", "")
        .await
        .starts_with("HTTP/1.1 200"));
    let limited = http_request(addr, "/", "").await;
    assert!(limited.starts_with("HTTP/1.1 429"));
    assert!(limited.ends_with("rate limit exceeded"));
    tx.send(()).expect("stop");
    serving.await.expect("join").expect("serve");
}
#[cfg(feature = "tower-http")]
#[tokio::test]
async fn timeout_and_catch_panic_responses_keep_internal_request_id() {
    use axum::routing::get;
    let timeout_server =
        AxumApp::from_router(Router::new().route("/slow", get(std::future::pending::<()>)))
            .into_server_builder()
            .with_request_id()
            .with_timeout(
                Duration::from_millis(5),
                axutils::AxumTimeoutStatus::RequestTimeout,
            )
            .unwrap()
            .build()
            .unwrap();
    let timeout_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let timeout_addr = timeout_listener.local_addr().unwrap();
    let timeout_task = tokio::spawn(async move {
        timeout_server
            .serve_with_shutdown(timeout_listener, async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let response = http_request(timeout_addr, "/slow", "").await;
    assert!(response.starts_with("HTTP/1.1 408"));
    assert!(response.to_ascii_lowercase().contains("x-request-id:"));
    timeout_task.await.unwrap().unwrap();

    async fn panic_handler() -> &'static str {
        panic!("non-secret-test-panic")
    }
    let panic_server = AxumApp::from_router(Router::new().route("/panic", get(panic_handler)))
        .into_server_builder()
        .with_request_id()
        .with_catch_panic()
        .build()
        .unwrap();
    let panic_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let panic_addr = panic_listener.local_addr().unwrap();
    let panic_task = tokio::spawn(async move {
        panic_server
            .serve_with_shutdown(panic_listener, async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let response = http_request(panic_addr, "/panic", "").await;
    assert!(response.starts_with("HTTP/1.1 500"));
    assert!(response.to_ascii_lowercase().contains("x-request-id:"));
    panic_task.await.unwrap().unwrap();
}

#[cfg(feature = "tower-http")]
async fn raw_http(addr: SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(request).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}

#[cfg(feature = "tower-http")]
#[tokio::test]
async fn body_limit_rejects_content_length_and_streaming_overflow() {
    use axum::routing::post;
    let server = AxumApp::from_router(
        Router::new().route("/body", post(|body: String| async move { body })),
    )
    .into_server_builder()
    .with_body_limit(4)
    .unwrap()
    .build()
    .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let known = raw_http(addr, b"POST /body HTTP/1.1\r\nhost: localhost\r\ncontent-length: 5\r\nconnection: close\r\n\r\n12345").await;
    assert!(known.starts_with("HTTP/1.1 413"));
    let chunked = raw_http(addr, b"POST /body HTTP/1.1\r\nhost: localhost\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n3\r\n123\r\n2\r\n45\r\n0\r\n\r\n").await;
    assert!(chunked.starts_with("HTTP/1.1 413"));
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[cfg(feature = "tower-http")]
#[tokio::test]
async fn cors_handles_simple_and_preflight_requests() {
    use axum::http::{HeaderValue, Method};
    use axutils::{AxumCorsConfig, AxumCorsOrigin};
    let cors = AxumCorsConfig {
        origins: AxumCorsOrigin::List(vec![HeaderValue::from_static("https://example.test")]),
        methods: vec![Method::GET],
        ..Default::default()
    };
    let server = AxumApp::from_router(Router::new().route("/cors", get(|| async { "ok" })))
        .into_server_builder()
        .with_cors(cors)
        .unwrap()
        .build()
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let simple = http_request(addr, "/cors", "origin: https://example.test\r\n")
        .await
        .to_ascii_lowercase();
    assert!(simple.starts_with("http/1.1 200"));
    assert!(simple.contains("access-control-allow-origin: https://example.test"));
    let preflight = raw_http(addr, b"OPTIONS /cors HTTP/1.1\r\nhost: localhost\r\norigin: https://example.test\r\naccess-control-request-method: GET\r\nconnection: close\r\n\r\n").await.to_ascii_lowercase();
    assert!(preflight.starts_with("http/1.1 200"));
    assert!(preflight.contains("access-control-allow-origin: https://example.test"));
    assert!(preflight.contains("access-control-allow-methods: get"));
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[cfg(feature = "tower_governor")]
#[tokio::test]
async fn unchecked_forwarded_governor_uses_forwarded_client_key() {
    use std::num::NonZeroU32;
    let server = AxumApp::from_router(Router::new().route("/g", get(|| async { "ok" })))
        .into_server_builder()
        .with_governor_forwarded_headers_unchecked(
            Duration::from_secs(3600),
            NonZeroU32::new(1).unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    assert!(http_request(addr, "/g", "x-forwarded-for: 192.0.2.1\r\n")
        .await
        .starts_with("HTTP/1.1 200"));
    assert!(http_request(addr, "/g", "x-forwarded-for: 192.0.2.2\r\n")
        .await
        .starts_with("HTTP/1.1 200"));
    assert!(http_request(addr, "/g", "x-forwarded-for: 192.0.2.1\r\n")
        .await
        .starts_with("HTTP/1.1 429"));
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[cfg(all(feature = "tower-http", feature = "tracing"))]
#[tokio::test(flavor = "current_thread")]
async fn trace_uses_matched_route_and_redacts_raw_request_data() {
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);
    struct BufferWriter(Arc<Mutex<Vec<u8>>>);
    impl Write for BufferWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
        type Writer = BufferWriter;
        fn make_writer(&'a self) -> Self::Writer {
            BufferWriter(self.0.clone())
        }
    }
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(Buffer(bytes.clone()))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let server = AxumApp::from_router(Router::new().route("/items/{id}", get(|| async { "ok" })))
        .into_server_builder()
        .with_http_trace()
        .build()
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    assert!(http_request(
        addr,
        "/items/42?query-secret=hidden",
        "x-secret: header-secret\r\n"
    )
    .await
    .starts_with("HTTP/1.1 200"));
    assert!(http_request(addr, "/missing?query-secret=hidden", "")
        .await
        .starts_with("HTTP/1.1 404"));
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
    let output = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
    assert!(output.contains("/items/{id}"));
    assert!(output.contains("<unmatched>"));
    assert!(!output.contains("request_id=<missing>"));
    assert!(!output.contains("query-secret"));
    assert!(!output.contains("header-secret"));
}

#[cfg(feature = "tower")]
#[tokio::test]
async fn concurrency_permit_is_released_when_request_is_cancelled() {
    use axum::extract::State;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    type TestState = (Arc<tokio::sync::Notify>, Arc<AtomicBool>);
    async fn slow(State((started, dropped)): State<TestState>) -> &'static str {
        let _guard = DropFlag(dropped);
        started.notify_one();
        std::future::pending::<()>().await;
        "never"
    }
    let started = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let router = Router::new()
        .route("/slow", get(slow))
        .route("/ok", get(|| async { "ok" }))
        .with_state((started.clone(), dropped.clone()));
    let server = AxumApp::from_router(router)
        .into_server_builder()
        .with_concurrency_limit(1)
        .unwrap()
        .build()
        .unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_with_shutdown(listener, async move {
                let _ = rx.await;
                AxumShutdownReason::Programmatic
            })
            .await
    });
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /slow HTTP/1.1\r\nhost: localhost\r\n\r\n")
        .await
        .unwrap();
    started.notified().await;
    drop(stream);
    tokio::time::timeout(Duration::from_secs(1), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(http_request(addr, "/ok", "")
        .await
        .starts_with("HTTP/1.1 200"));
    tx.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[cfg(feature = "tower_governor")]
#[test]
fn governor_cleanup_runs_on_runtime_without_tokio_time_driver() {
    use std::{num::NonZeroU32, panic::catch_unwind};
    let result = catch_unwind(|| {
        axutils::TokioUtils::run(
            &axutils::TokioConfig::new().with_time_enabled(false),
            async {
                let server = AxumApp::from_router(Router::new().route("/", get(|| async { "ok" })))
                    .into_server_builder()
                    .with_governor_peer(Duration::from_secs(1), NonZeroU32::new(1).unwrap())
                    .unwrap()
                    .build()
                    .unwrap();
                let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
                server
                    .serve_with_shutdown(listener, async {
                        futures_timer::Delay::new(Duration::from_millis(5)).await;
                        AxumShutdownReason::Programmatic
                    })
                    .await
                    .unwrap();
            },
        )
    });
    assert!(result.is_ok());
    result.unwrap().unwrap();
}
