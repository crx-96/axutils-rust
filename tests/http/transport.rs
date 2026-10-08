use super::*;

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
            kind: HttpTransportErrorKind::Timeout,
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
    assert_send_sync::<HttpUtils>();
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
fn url_entry_points_reject_empty_userinfo_before_normalization() {
    for url in [
        "http://@example.com/",
        "http://:@example.com/",
        "https://@example.com/",
        "https://:@example.com/",
        " http://@example.com/ ",
        "http:@example.com/",
        "http:/@example.com/",
        r"http:\\@example.com/",
        "hTtPs:////:@example.com/",
        " //example.com/",
        " //@example.com/",
    ] {
        assert!(
            matches!(
                HttpRequest::new(HttpMethod::Get, url),
                Err(HttpError::InvalidUrl)
            ),
            "request accepted {url:?}"
        );
        assert!(
            matches!(
                HttpRequest::builder()
                    .method(HttpMethod::Get)
                    .url(url)
                    .build(),
                Err(HttpError::InvalidUrl)
            ),
            "request builder accepted {url:?}"
        );
        assert!(
            matches!(
                HttpConfig::builder().base_url(url),
                Err(HttpError::InvalidUrl)
            ),
            "config accepted {url:?}"
        );
    }
    for url in [
        "https://example.com/users/@me",
        "https://example.com/?email=user@example.com",
    ] {
        assert!(HttpRequest::new(HttpMethod::Get, url).is_ok());
        assert!(HttpRequest::builder()
            .method(HttpMethod::Get)
            .url(url)
            .build()
            .is_ok());
        assert!(HttpConfig::builder().base_url(url).is_ok());
    }
    assert!(HttpRequest::new(HttpMethod::Get, "@me").is_ok());
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

#[cfg(feature = "http-async")]
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

#[cfg(feature = "http-async")]
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
