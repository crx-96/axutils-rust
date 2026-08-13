#![cfg(feature = "http")]

use std::thread;

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

    let mut workers = Vec::new();
    for _ in 0..8 {
        workers.push(thread::spawn(|| HttpUtils::init(HttpConfig::default())));
    }
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("initialization worker"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(HttpError::AlreadyInitialized)))
            .count(),
        7
    );
    assert!(HttpUtils::is_initialized());
    assert_eq!(
        HttpUtils::init(HttpConfig::default()),
        Err(HttpError::AlreadyInitialized)
    );
}
