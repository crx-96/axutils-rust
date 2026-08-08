#[cfg(any(
    feature = "http",
    feature = "http-tokio",
    feature = "http-serde",
    feature = "http-tokio-serde"
))]
fn compile_sync_api() {
    use std::time::Duration;

    use axutils::{
        DeduplicationPolicy, HttpClient, HttpConfig, HttpHeaders, HttpMethod, HttpRequest,
        HttpResponse, RetryPolicy,
    };

    let config = HttpConfig::builder()
        .base_url("https://example.com/api/")
        .expect("fixture base URL")
        .with_default_header("x-fixture", "ok")
        .expect("fixture header")
        .request_timeout(Duration::from_secs(5))
        .expect("fixture timeout")
        .deduplication_policy(DeduplicationPolicy::disabled())
        .build()
        .expect("fixture config");
    let client = HttpClient::new(config).expect("fixture client");
    let request = HttpRequest::builder()
        .method(HttpMethod::Get)
        .url("/health")
        .header("accept", "application/json")
        .expect("fixture header")
        .retry_policy(RetryPolicy::new())
        .build()
        .expect("fixture request");
    let _execute: fn(&HttpClient, HttpRequest) -> Result<HttpResponse, axutils::HttpError> =
        HttpClient::execute;
    let _headers: fn() -> HttpHeaders = HttpHeaders::new;
    let _ = (client, request);
}

#[cfg(any(feature = "http-serde", feature = "http-tokio-serde"))]
fn compile_serde_api() {
    use axutils::{HttpClient, HttpRequestOptions};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    struct Query {
        page: u32,
    }

    #[derive(Serialize)]
    struct Body {
        value: &'static str,
    }

    #[derive(Deserialize)]
    struct Reply {
        ok: bool,
    }

    let client = HttpClient::new(axutils::HttpConfig::default()).expect("fixture client");
    let _domain_options = axutils::http::HttpRequestOptions::new();
    let _nested_utils = axutils::utils::http_utils::HttpUtils::is_initialized();
    let _json: fn(&axutils::HttpResponse) -> Result<u8, axutils::HttpError> =
        axutils::HttpResponse::json::<u8>;
    let _body: fn(axutils::HttpResponse) -> Vec<u8> = axutils::HttpResponse::into_body;
    let _ = client.get::<Reply, _>("https://example.com", Some(Query { page: 1 }), None);
    let _ = client.post::<Reply, _>(
        "https://example.com",
        Some(Body { value: "ok" }),
        Some(HttpRequestOptions::new()),
    );
    let _ = client.delete::<Reply, _>("https://example.com", None::<()>, None);
    let _ = client.patch::<Reply, _>("https://example.com", None::<Body>, None);
    let _ = client.put::<Reply, _>("https://example.com", None::<Body>, None);
    let _ = client.options::<Reply, _>("https://example.com", None::<()>, None);
    let _ = client.get_bytes("https://example.com", None::<()>, None);
    let _ = client.post_bytes("https://example.com", None::<Body>, None);
}

#[cfg(all(feature = "http", not(feature = "http-tokio")))]
fn main() {
    compile_sync_api();
}

#[cfg(feature = "http-serde")]
fn main() {
    compile_sync_api();
    let _ = compile_serde_api;
}

#[cfg(feature = "http-tokio")]
async fn compile_async_api(client: &axutils::HttpClient, request: axutils::HttpRequest) {
    let _ = client.execute_async(request).await;
    let _execute_async = axutils::HttpUtils::execute_async;
    let _ = _execute_async;
}

#[cfg(feature = "http-tokio")]
fn main() {
    compile_sync_api();
    let _ = compile_async_api;
}

#[cfg(feature = "http-tokio-serde")]
async fn compile_serde_async_api(client: &axutils::HttpClient) {
    let _ = client
        .get_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .post_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .delete_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .patch_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .put_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .options_async::<u8, _>("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .get_bytes_async("https://example.com", None::<()>, None)
        .await;
    let _ = client
        .post_bytes_async("https://example.com", None::<()>, None)
        .await;
    let _ = axutils::HttpUtils::get_async::<u8, _>("https://example.com", None::<()>, None);
}

#[cfg(feature = "http-tokio-serde")]
fn main() {
    compile_sync_api();
    let _ = compile_serde_api;
    let _ = compile_serde_async_api;
}

#[cfg(feature = "tokio-only")]
fn main() {}

#[cfg(feature = "negative-http-module")]
fn main() {
    let _ = axutils::http::HttpClient::new;
}

#[cfg(feature = "negative-http-client")]
fn main() {
    let _ = axutils::HttpClient::new;
}

#[cfg(feature = "negative-http-utils")]
fn main() {
    let _ = axutils::HttpUtils::init;
}

#[cfg(feature = "negative-http-tokio-module")]
fn main() {
    let _ = axutils::http::HttpClient::new;
}

#[cfg(feature = "negative-http-tokio-client")]
fn main() {
    let _ = axutils::HttpClient::new;
}

#[cfg(feature = "negative-http-tokio-utils")]
fn main() {
    let _ = axutils::HttpUtils::init;
}

#[cfg(feature = "negative-http-async")]
fn main() {
    let _ = axutils::HttpClient::execute_async;
}

#[cfg(feature = "negative-http-serde")]
fn main() {
    let _ = axutils::HttpClient::get::<u8, ()>;
}

#[cfg(feature = "negative-http-tokio-serde")]
fn main() {
    let _ = axutils::HttpClient::get_async::<u8, ()>;
}

#[cfg(not(any(
    feature = "http",
    feature = "http-tokio",
    feature = "tokio-only",
    feature = "negative-http-module",
    feature = "negative-http-client",
    feature = "negative-http-utils",
    feature = "negative-http-tokio-module",
    feature = "negative-http-tokio-client",
    feature = "negative-http-tokio-utils",
    feature = "negative-http-async",
    feature = "negative-http-serde",
    feature = "negative-http-tokio-serde",
    feature = "http-serde",
    feature = "http-tokio-serde"
)))]
fn main() {}
