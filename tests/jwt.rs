#![cfg(feature = "jwt")]

use axutils::{
    JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};

#[test]
fn exposes_only_the_frozen_algorithm_set() {
    let algorithms = [
        JwtAlgorithm::Hs256,
        JwtAlgorithm::Hs384,
        JwtAlgorithm::Hs512,
        JwtAlgorithm::Rs256,
        JwtAlgorithm::Rs384,
        JwtAlgorithm::Rs512,
        JwtAlgorithm::Ps256,
        JwtAlgorithm::Ps384,
        JwtAlgorithm::Ps512,
        JwtAlgorithm::Es256,
        JwtAlgorithm::Es384,
        JwtAlgorithm::Ed25519,
    ];
    assert_eq!(algorithms.len(), 12);
}

#[test]
fn key_constructors_enforce_resource_and_ed25519_length_limits() {
    assert!(matches!(
        JwtSigningKey::from_hmac_secret([]),
        Err(JwtError::InvalidKey {
            kind: "hmac_secret"
        })
    ));
    assert!(matches!(
        JwtVerificationKey::from_hmac_secret(vec![0u8; 4097]),
        Err(JwtError::InvalidKey {
            kind: "hmac_secret"
        })
    ));

    for length in [0, 31, 33] {
        assert!(matches!(
            JwtVerificationKey::from_ed_der(vec![0u8; length]),
            Err(JwtError::UnsupportedKeyFormat {
                kind: "ed_public_der"
            })
        ));
    }
    assert!(JwtVerificationKey::from_ed_der([0u8; 32]).is_ok());

    assert!(matches!(
        JwtSigningKey::from_rsa_der(vec![0u8; 128 * 1024 + 1]),
        Err(JwtError::InvalidConfig { field: "key_size" })
    ));
}

#[test]
fn pem_labels_are_checked_before_backend_parsing() {
    let public_rsa = b"-----BEGIN RSA PUBLIC KEY-----\nnot-a-key\n-----END RSA PUBLIC KEY-----";
    let ec_private = b"-----BEGIN EC PRIVATE KEY-----\nnot-a-key\n-----END EC PRIVATE KEY-----";
    let ed_public = b"-----BEGIN PUBLIC KEY-----\nnot-a-key\n-----END PUBLIC KEY-----";

    assert!(matches!(
        JwtSigningKey::from_rsa_pem(public_rsa),
        Err(JwtError::InvalidKey { kind: "rsa_pem" })
    ));
    assert!(matches!(
        JwtVerificationKey::from_rsa_pem(
            b"-----BEGIN RSA PRIVATE KEY-----\nnot-a-key\n-----END RSA PRIVATE KEY-----"
        ),
        Err(JwtError::InvalidKey { kind: "rsa_pem" })
    ));
    assert!(matches!(
        JwtSigningKey::from_ec_pem(ec_private),
        Err(JwtError::InvalidKey { kind: "ec_pem" })
    ));
    assert!(matches!(
        JwtSigningKey::from_ed_pem(ed_public),
        Err(JwtError::InvalidKey { kind: "ed_pem" })
    ));
}

#[test]
fn config_rejects_missing_keys_mismatches_and_weak_hmac_secrets() {
    assert!(matches!(
        JwtConfig::new(JwtAlgorithm::Hs256, None, None, JwtValidation::new()),
        Err(JwtError::InvalidConfig { field: "keys" })
    ));

    let signing = JwtSigningKey::from_hmac_secret([0u8; 32]).unwrap();
    assert!(matches!(
        JwtConfig::new(
            JwtAlgorithm::Rs256,
            Some(signing),
            None,
            JwtValidation::new()
        ),
        Err(JwtError::InvalidKey {
            kind: "signing_algorithm_key"
        })
    ));

    let weak = JwtSigningKey::from_hmac_secret([0u8; 63]).unwrap();
    assert!(matches!(
        JwtConfig::new(JwtAlgorithm::Hs512, Some(weak), None, JwtValidation::new()),
        Err(JwtError::InvalidKey {
            kind: "hmac_secret_length"
        })
    ));
}

#[test]
fn config_rejects_a_parseable_rsa_modulus_below_2048_bits() {
    fn der_length(length: usize) -> Vec<u8> {
        if length < 128 {
            vec![length as u8]
        } else {
            let bytes = length.to_be_bytes();
            let first = bytes.iter().position(|byte| *byte != 0).unwrap();
            let significant = &bytes[first..];
            let mut encoded = vec![0x80 | significant.len() as u8];
            encoded.extend_from_slice(significant);
            encoded
        }
    }

    fn der_integer(mut value: Vec<u8>) -> Vec<u8> {
        if value.first().is_some_and(|byte| byte & 0x80 != 0) {
            value.insert(0, 0);
        }
        let mut result = vec![0x02];
        result.extend(der_length(value.len()));
        result.extend(value);
        result
    }

    let mut body = der_integer(vec![0]);
    body.extend(der_integer(vec![0x80; 128]));
    body.extend(der_integer(vec![1, 0, 1]));
    let mut private_der = vec![0x30];
    private_der.extend(der_length(body.len()));
    private_der.extend(body);

    let signing = JwtSigningKey::from_rsa_der(private_der).unwrap();
    assert!(matches!(
        JwtConfig::new(
            JwtAlgorithm::Rs256,
            Some(signing),
            None,
            JwtValidation::new()
        ),
        Err(JwtError::InvalidKey {
            kind: "rsa_modulus_bits"
        })
    ));
}

#[test]
fn validation_builders_reject_invalid_allowlists_and_leeway() {
    assert!(matches!(
        JwtValidation::new().with_audience(""),
        Err(JwtError::InvalidConfig { field: "audience" })
    ));
    assert!(matches!(
        JwtValidation::new().with_issuers(["issuer", "issuer"]),
        Err(JwtError::InvalidConfig { field: "issuers" })
    ));
    assert!(matches!(
        JwtValidation::new().with_subject("bad\nsubject"),
        Err(JwtError::InvalidConfig { field: "subject" })
    ));
    assert!(matches!(
        JwtValidation::new().with_leeway(86_401),
        Err(JwtError::InvalidConfig { field: "leeway" })
    ));

    let values = (0..33).map(|index| format!("aud-{index}"));
    assert!(matches!(
        JwtValidation::new().with_audiences(values),
        Err(JwtError::InvalidConfig { field: "audience" })
    ));
}

#[test]
fn public_debug_display_and_source_are_redacted() {
    let sentinel = "SENTINEL_JWT_SECRET_VALUE_1234567890";
    let config = JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(JwtSigningKey::from_hmac_secret(sentinel.as_bytes()).unwrap()),
        None,
        JwtValidation::new(),
    )
    .unwrap();
    assert!(!format!("{config:?}").contains(sentinel));

    let error = JwtError::InvalidToken { segment: "claims" };
    assert!(!error.to_string().contains(sentinel));
    assert!(!format!("{error:?}").contains(sentinel));
    assert!(std::error::Error::source(&error).is_none());
}
