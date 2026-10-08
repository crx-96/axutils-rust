use super::*;

#[test]
fn factories_create_empty_router_and_app() {
    let router: Router = AxumApp::create_router();
    assert!(!router.has_routes());

    let router: Router<String> = AxumApp::<String>::create_router();
    assert!(!router.has_routes());
    assert!(AxumApp::from_router(router)
        .with_state("app-state".to_owned())
        .build()
        .is_ok());

    let app: AxumApp = AxumApp::new();
    assert!(app.into_server_builder().build().is_ok());
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
    tokio_time::sleep(Duration::from_millis(20)).await;
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
    tokio_time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        handle.shutdown(AxumShutdownReason::Programmatic).unwrap(),
        AxumShutdownReason::Programmatic
    );
    let outcome = tokio_time::timeout(Duration::from_secs(1), running)
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
    tokio_time::sleep(Duration::from_millis(20)).await;
    let other = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind other");
    assert!(matches!(
        server.serve(other).await,
        Err(AxumError::AlreadyRunning)
    ));
    task.abort();
    let _ = task.await;
    tokio_task::yield_now().await;
    assert!(matches!(
        server
            .serve_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await,
        Err(AxumError::Abandoned)
    ));
}

#[tokio::test]
async fn panicking_shutdown_future_returns_background_error_and_abandons_server() {
    let server = server();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let result = tokio_time::timeout(
        Duration::from_secs(1),
        server.serve_with_shutdown(listener, async {
            panic!("test shutdown future panic");
        }),
    )
    .await
    .expect("shutdown future panic must stop serving");

    assert!(matches!(result, Err(AxumError::BackgroundTask)));
    assert!(matches!(
        server
            .shutdown_handle()
            .shutdown(AxumShutdownReason::Programmatic),
        Err(AxumError::Abandoned)
    ));
}

#[test]
fn config_rejects_unbounded_values() {
    assert!(matches!(
        AxumConfig::new().with_max_body_bytes(0),
        Err(AxumError::InvalidConfig {
            field: "max_body_bytes"
        })
    ));
    assert!(matches!(
        AxumConfig::new().with_max_concurrency(0),
        Err(AxumError::InvalidConfig {
            field: "max_concurrency"
        })
    ));
}

#[test]
fn global_axum_utils_initializes_only_once() {
    let first = AxumUtils::init(server());
    assert!(first.is_ok(), "fresh test process must initialize once");
    assert!(AxumUtils::is_initialized());
    assert!(matches!(
        AxumUtils::init(server()),
        Err(AxumError::AlreadyInitialized)
    ));
}
