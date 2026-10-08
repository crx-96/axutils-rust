use super::*;

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

#[cfg(feature = "http-async")]
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

#[cfg(feature = "http-async")]
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
    tokio_time::sleep(Duration::from_millis(20)).await;
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

#[cfg(feature = "http-async")]
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
    tokio_time::sleep(Duration::from_millis(20)).await;
    let follower_client = Arc::clone(&client);
    let follower = tokio::spawn(async move {
        follower_client
            .execute_async(HttpRequest::new(HttpMethod::Get, "/leader-cancel").expect("request"))
            .await
    });
    tokio_time::sleep(Duration::from_millis(20)).await;
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
