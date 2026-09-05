#![cfg(feature = "jwt")]

use std::sync::{Arc, Barrier};
use std::thread;

use axutils::jwt::{
    JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};
use axutils::utils::JwtUtils;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Claims {
    exp: u64,
    sub: String,
}

fn config(secret: &[u8]) -> JwtConfig {
    JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(JwtSigningKey::from_hmac_secret(secret).unwrap()),
        Some(JwtVerificationKey::from_hmac_secret(secret).unwrap()),
        JwtValidation::new(),
    )
    .unwrap()
}

#[test]
fn global_jwt_initializes_once_and_is_deprecated_by_no_reset_semantics() {
    assert!(!JwtUtils::is_initialized());
    assert!(matches!(
        JwtUtils::codec().and_then(|codec| codec.encode(&Claims {
            exp: 2_000_000_000,
            sub: "before-init".to_owned(),
        })),
        Err(JwtError::NotInitialized)
    ));
    assert!(matches!(
        JwtUtils::codec().and_then(|codec| codec.decode::<Claims>("not-a-token")),
        Err(JwtError::NotInitialized)
    ));

    assert!(matches!(
        JwtConfig::new(JwtAlgorithm::Hs256, None, None, JwtValidation::new()),
        Err(JwtError::InvalidConfig { field: "keys" })
    ));
    assert!(!JwtUtils::is_initialized());

    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        JwtUtils::init(config(&[0x11; 32]))
    });
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        JwtUtils::init(config(&[0x22; 32]))
    });
    barrier.wait();
    let results = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(JwtError::AlreadyInitialized)))
            .count(),
        1
    );
    assert!(JwtUtils::is_initialized());

    let duplicate = config(&[0x33; 32]);
    assert!(matches!(
        JwtUtils::init(duplicate),
        Err(JwtError::AlreadyInitialized)
    ));

    let claims = Claims {
        exp: 2_000_000_000,
        sub: "global-round-trip".to_owned(),
    };
    let codec = JwtUtils::codec().unwrap();
    let token = codec.encode(&claims).unwrap();
    assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);

    let sentinel_token = "SENTINEL_JWT_TOKEN_VALUE";
    let error = codec.decode::<Claims>(sentinel_token).unwrap_err();
    assert!(!error.to_string().contains(sentinel_token));
    assert!(!format!("{error:?}").contains(sentinel_token));
    assert!(std::error::Error::source(&error).is_none());

    let sentinel_config = config(b"SENTINEL_JWT_SECRET_VALUE_123456");
    assert!(!format!("{sentinel_config:?}").contains("SENTINEL_JWT_SECRET_VALUE_123456"));
    assert!(matches!(
        JwtUtils::init(sentinel_config),
        Err(JwtError::AlreadyInitialized)
    ));
}
