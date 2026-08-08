#![cfg(feature = "http")]

use axutils::{HttpConfig, HttpError, HttpMethod, HttpRequest, HttpUtils};

#[test]
fn global_http_entry_initializes_once_without_network_side_effects() {
    assert!(!HttpUtils::is_initialized());
    assert!(matches!(
        HttpUtils::execute(
            HttpRequest::new(HttpMethod::Get, "https://example.com/").expect("request")
        ),
        Err(HttpError::NotInitialized)
    ));

    HttpUtils::init(HttpConfig::default()).expect("first initialization");
    assert!(HttpUtils::is_initialized());
    assert_eq!(
        HttpUtils::init(HttpConfig::default()),
        Err(HttpError::AlreadyInitialized)
    );
}
