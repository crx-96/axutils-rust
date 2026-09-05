use std::sync::atomic::{AtomicUsize, Ordering};

use jsonwebtoken::{
    Algorithm as BackendAlgorithm, EncodingKey as BackendEncodingKey, Header as BackendHeader,
};
use serde::{ser::SerializeMap, Deserialize, Serialize, Serializer};
use serde_json::{Map as JsonMap, Value as JsonValue};

use super::super::claims::{preflight_claims, MAX_CLAIMS_BYTES};
use super::super::{JwtAlgorithm, JwtError};
use super::{JwtCodec, MAX_TOKEN_BYTES};
use crate::jwt::{JwtConfig, JwtSigningKey, JwtValidation, JwtVerificationKey};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Claims {
    exp: u64,
    sub: String,
}

fn codec(validation: JwtValidation) -> JwtCodec {
    JwtCodec::new(
        JwtConfig::new(
            JwtAlgorithm::Hs256,
            Some(JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap()),
            Some(JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap()),
            validation,
        )
        .unwrap(),
    )
}

#[test]
fn hmac_round_trip_uses_fixed_algorithm_and_generic_claims() {
    let codec = codec(JwtValidation::new());
    let claims = Claims {
        exp: 2_000_000_000,
        sub: "user-1".to_owned(),
    };
    let token = codec.encode(&claims).unwrap();
    assert_eq!(
        codec.decode_at::<Claims>(&token, 1_900_000_000).unwrap(),
        claims
    );
    assert!(token.starts_with("eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9."));
}

#[test]
fn invalid_rsa_der_is_a_key_error_at_decode_operation() {
    let verification = JwtVerificationKey::from_rsa_der([0x01, 0x02, 0x03]).unwrap();
    let codec = JwtCodec::new(
        JwtConfig::new(
            JwtAlgorithm::Rs256,
            None,
            Some(verification),
            JwtValidation::new().with_require_exp(false),
        )
        .unwrap(),
    );
    let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.e30.AA";
    assert!(matches!(
        codec.decode::<serde_json::Value>(token),
        Err(JwtError::UnsupportedKeyFormat {
            kind: "verification_key"
        })
    ));
}

#[test]
fn missing_keys_are_reported_after_structural_preflight() {
    let signing_only = JwtCodec::new(
        JwtConfig::new(
            JwtAlgorithm::Hs256,
            Some(JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap()),
            None,
            JwtValidation::new(),
        )
        .unwrap(),
    );
    assert!(matches!(
        signing_only.decode::<serde_json::Value>("not-a-token"),
        Err(JwtError::InvalidHeader { field: "segments" })
    ));
    let token = signing_only
        .encode(&Claims {
            exp: 2_000_000_000,
            sub: "missing-verification".to_owned(),
        })
        .unwrap();
    assert!(matches!(
        signing_only.decode::<Claims>(&token),
        Err(JwtError::MissingVerificationKey)
    ));

    let verification_only = JwtCodec::new(
        JwtConfig::new(
            JwtAlgorithm::Hs256,
            None,
            Some(JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap()),
            JwtValidation::new(),
        )
        .unwrap(),
    );
    assert!(matches!(
        verification_only.encode(&Claims {
            exp: 2_000_000_000,
            sub: "missing-signing".to_owned(),
        }),
        Err(JwtError::MissingSigningKey)
    ));
}

#[test]
fn malformed_root_and_duplicate_claims_are_rejected_before_decode() {
    assert!(preflight_claims(br"[]").is_err());
    assert!(preflight_claims(br#"{"a":1,"a":2}"#).is_err());
}

fn base64url_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    let mut index = 0;
    while index < input.len() {
        let first = input[index];
        result.push(ALPHABET[(first >> 2) as usize] as char);
        if index + 1 >= input.len() {
            result.push(ALPHABET[((first & 0x03) << 4) as usize] as char);
            break;
        }
        let second = input[index + 1];
        result.push(ALPHABET[((first & 0x03) << 4 | second >> 4) as usize] as char);
        if index + 2 >= input.len() {
            result.push(ALPHABET[((second & 0x0f) << 2) as usize] as char);
            break;
        }
        let third = input[index + 2];
        result.push(ALPHABET[((second & 0x0f) << 2 | third >> 6) as usize] as char);
        result.push(ALPHABET[(third & 0x3f) as usize] as char);
        index += 3;
    }
    result
}

fn replace_header(token: &str, header_json: &str) -> String {
    let mut parts = token.split('.');
    let _old_header = parts.next().unwrap();
    let payload = parts.next().unwrap();
    let signature = parts.next().unwrap();
    format!(
        "{}.{}.{}",
        base64url_encode(header_json.as_bytes()),
        payload,
        signature
    )
}

#[test]
fn header_is_fixed_and_rejects_unknown_or_duplicate_fields() {
    let codec = codec(JwtValidation::new());
    let token = codec
        .encode(&Claims {
            exp: 2_000_000_000,
            sub: "header-test".to_owned(),
        })
        .unwrap();
    let changed_algorithm = replace_header(&token, r#"{"typ":"JWT","alg":"HS512"}"#);
    assert!(matches!(
        codec.decode_at::<Claims>(&changed_algorithm, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "alg" })
    ));
    let unknown_field = replace_header(
        &token,
        r#"{"typ":"JWT","alg":"HS256","kid":"not-accepted"}"#,
    );
    assert!(matches!(
        codec.decode_at::<Claims>(&unknown_field, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "json" })
    ));
    let duplicate_field = replace_header(&token, r#"{"typ":"JWT","alg":"HS256","alg":"HS256"}"#);
    assert!(matches!(
        codec.decode_at::<Claims>(&duplicate_field, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "json" })
    ));
    let mut parts = token.split('.');
    let valid_header = parts.next().unwrap();
    let valid_payload = parts.next().unwrap();
    let valid_signature = parts.next().unwrap();
    let invalid_header = format!("!.{valid_payload}.{valid_signature}");
    assert!(matches!(
        codec.decode_at::<Claims>(&invalid_header, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "base64" })
    ));
    let invalid_payload = format!("{valid_header}.!.{valid_signature}");
    assert!(matches!(
        codec.decode_at::<Claims>(&invalid_payload, 1_900_000_000),
        Err(JwtError::InvalidToken {
            segment: "payload_base64"
        })
    ));
    let truncated = format!("{valid_header}.{valid_payload}");
    assert!(matches!(
        codec.decode_at::<Claims>(&truncated, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "segments" })
    ));
    let empty_payload = format!("{valid_header}..{valid_signature}");
    assert!(matches!(
        codec.decode_at::<Claims>(&empty_payload, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "segments" })
    ));
    let empty_signature = format!("{valid_header}.{valid_payload}.");
    assert!(matches!(
        codec.decode_at::<Claims>(&empty_signature, 1_900_000_000),
        Err(JwtError::InvalidHeader { field: "segments" })
    ));
    let exact_signature_length = MAX_TOKEN_BYTES
        .checked_sub(valid_header.len())
        .and_then(|length| length.checked_sub(valid_payload.len()))
        .and_then(|length| length.checked_sub(2))
        .unwrap();
    let exact_token = format!(
        "{valid_header}.{valid_payload}.{}",
        "A".repeat(exact_signature_length)
    );
    assert_eq!(exact_token.len(), MAX_TOKEN_BYTES);
    assert!(matches!(
        codec.decode_at::<Claims>(&exact_token, 1_900_000_000),
        Err(JwtError::InvalidToken { .. })
    ));
}

#[test]
fn standard_claims_use_strict_types_and_checked_time_boundaries() {
    let exp_codec = codec(JwtValidation::new());
    let at_boundary = exp_codec
        .encode(&serde_json::json!({ "exp": 1_900_000_000 - 60 }))
        .unwrap();
    assert!(exp_codec
        .decode_at::<serde_json::Value>(&at_boundary, 1_900_000_000)
        .is_ok());
    let past_boundary = exp_codec
        .encode(&serde_json::json!({ "exp": 1_900_000_000 - 61 }))
        .unwrap();
    assert!(matches!(
        exp_codec.decode_at::<serde_json::Value>(&past_boundary, 1_900_000_000),
        Err(JwtError::InvalidClaim { claim: "exp" })
    ));

    let malformed_without_validation = codec(JwtValidation::new().with_validate_exp(false));
    let malformed = malformed_without_validation
        .encode(&serde_json::json!({ "exp": 1.0 }))
        .unwrap();
    assert!(matches!(
        malformed_without_validation.decode_at::<serde_json::Value>(&malformed, 1),
        Err(JwtError::InvalidClaim { claim: "exp" })
    ));

    let nbf_validation = JwtValidation::new()
        .with_require_exp(false)
        .with_validate_nbf(true);
    let nbf_codec = codec(nbf_validation);
    let nbf_at_boundary = nbf_codec
        .encode(&serde_json::json!({ "nbf": 1_900_000_000 + 60 }))
        .unwrap();
    assert!(nbf_codec
        .decode_at::<serde_json::Value>(&nbf_at_boundary, 1_900_000_000)
        .is_ok());
    let nbf_outside = nbf_codec
        .encode(&serde_json::json!({ "nbf": 1_900_000_000 + 61 }))
        .unwrap();
    assert!(matches!(
        nbf_codec.decode_at::<serde_json::Value>(&nbf_outside, 1_900_000_000),
        Err(JwtError::InvalidClaim { claim: "nbf" })
    ));
}

#[test]
fn encode_serializes_custom_claims_once_and_rejects_duplicate_object_keys() {
    struct CountingClaims<'a>(&'a AtomicUsize);

    impl Serialize for CountingClaims<'_> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.0.fetch_add(1, Ordering::Relaxed);
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("exp", &2_000_000_000u64)?;
            map.end()
        }
    }

    struct DuplicateClaims;

    impl Serialize for DuplicateClaims {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("exp", &2_000_000_000u64)?;
            map.serialize_entry("exp", &2_000_000_000u64)?;
            map.end()
        }
    }

    let counter = AtomicUsize::new(0);
    let codec = codec(JwtValidation::new());
    assert!(codec.encode(&CountingClaims(&counter)).is_ok());
    assert_eq!(counter.load(Ordering::Relaxed), 1);
    assert!(matches!(
        codec.encode(&DuplicateClaims),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));
}

#[test]
fn audience_issuer_and_subject_rules_are_independent() {
    let validation = JwtValidation::new()
        .with_require_exp(false)
        .with_require_aud(true)
        .with_audiences(["service-a", "service-b"])
        .unwrap()
        .with_require_iss(true)
        .with_issuer("issuer-a")
        .unwrap()
        .with_require_sub(true)
        .with_subject("user-a")
        .unwrap();
    let configured_codec = codec(validation);
    let valid = configured_codec
        .encode(&serde_json::json!({
            "aud": ["other", "service-b"],
            "iss": "issuer-a",
            "sub": "user-a"
        }))
        .unwrap();
    assert!(configured_codec
        .decode_at::<serde_json::Value>(&valid, 1)
        .is_ok());

    let wrong_audience = configured_codec
        .encode(&serde_json::json!({
            "aud": ["other"],
            "iss": "issuer-a",
            "sub": "user-a"
        }))
        .unwrap();
    assert!(matches!(
        configured_codec.decode_at::<serde_json::Value>(&wrong_audience, 1),
        Err(JwtError::InvalidClaim { claim: "aud" })
    ));
    let issuer_array = configured_codec
        .encode(&serde_json::json!({
            "aud": "service-a",
            "iss": ["issuer-a"],
            "sub": "user-a"
        }))
        .unwrap();
    assert!(matches!(
        configured_codec.decode_at::<serde_json::Value>(&issuer_array, 1),
        Err(JwtError::InvalidClaim { claim: "iss" })
    ));
    let missing_subject = configured_codec
        .encode(&serde_json::json!({
            "aud": "service-a",
            "iss": "issuer-a"
        }))
        .unwrap();
    assert!(matches!(
        configured_codec.decode_at::<serde_json::Value>(&missing_subject, 1),
        Err(JwtError::MissingRequiredClaim { claim: "sub" })
    ));

    let wrong_issuer = configured_codec
        .encode(&serde_json::json!({
            "aud": "service-a",
            "iss": "Issuer-A",
            "sub": "user-a"
        }))
        .unwrap();
    assert!(matches!(
        configured_codec.decode_at::<serde_json::Value>(&wrong_issuer, 1),
        Err(JwtError::InvalidClaim { claim: "iss" })
    ));
    let wrong_subject = configured_codec
        .encode(&serde_json::json!({
            "aud": "service-a",
            "iss": "issuer-a",
            "sub": "User-A"
        }))
        .unwrap();
    assert!(matches!(
        configured_codec.decode_at::<serde_json::Value>(&wrong_subject, 1),
        Err(JwtError::InvalidClaim { claim: "sub" })
    ));

    let require_nbf = codec(
        JwtValidation::new()
            .with_require_exp(false)
            .with_require_nbf(true),
    );
    let missing_nbf = require_nbf.encode(&serde_json::json!({})).unwrap();
    assert!(matches!(
        require_nbf.decode_at::<serde_json::Value>(&missing_nbf, 1),
        Err(JwtError::MissingRequiredClaim { claim: "nbf" })
    ));

    let malformed = codec(JwtValidation::new().with_require_exp(false));
    for (claim, value) in [
        ("nbf", serde_json::json!("not-a-number")),
        ("aud", serde_json::json!([])),
        ("iss", serde_json::json!(["issuer-a"])),
        ("sub", serde_json::json!(42)),
    ] {
        let mut object = JsonMap::new();
        object.insert(claim.to_owned(), value);
        let token = malformed.encode(&JsonValue::Object(object)).unwrap();
        assert!(matches!(
            malformed.decode_at::<serde_json::Value>(&token, 1),
            Err(JwtError::InvalidClaim { claim: actual }) if actual == claim
        ));
    }
}

#[test]
fn claims_resource_limits_accept_boundary_and_reject_overflow() {
    let codec = codec(JwtValidation::new().with_require_exp(false));
    let exact_value = serde_json::json!({
        "data": "x".repeat(MAX_CLAIMS_BYTES - 11)
    });
    assert_eq!(
        serde_json::to_vec(&exact_value).unwrap().len(),
        MAX_CLAIMS_BYTES
    );
    let exact_token = codec.encode(&exact_value).unwrap();
    assert!(codec.decode::<serde_json::Value>(&exact_token).is_ok());
    let oversized_value = serde_json::json!({
        "data": "x".repeat(MAX_CLAIMS_BYTES - 10)
    });
    assert!(matches!(
        codec.encode(&oversized_value),
        Err(JwtError::ClaimsTooLarge { .. })
    ));

    let nested = |depth: usize| {
        let mut value = JsonValue::Null;
        for _ in 0..depth {
            let mut object = JsonMap::new();
            object.insert("nested".to_owned(), value);
            value = JsonValue::Object(object);
        }
        value
    };
    assert!(codec.encode(&nested(32)).is_ok());
    assert!(matches!(
        codec.encode(&nested(33)),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));

    let backend_signed = |value: &JsonValue| {
        jsonwebtoken::encode(
            &BackendHeader::new(BackendAlgorithm::HS256),
            value,
            &BackendEncodingKey::from_secret(&[0x11; 64]),
        )
        .unwrap()
    };
    let deep_token = backend_signed(&nested(33));
    assert!(matches!(
        codec.decode::<serde_json::Value>(&deep_token),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));

    let members = |count: usize| {
        let mut object = JsonMap::new();
        for index in 0..count {
            object.insert(format!("field-{index}"), JsonValue::Null);
        }
        JsonValue::Object(object)
    };
    assert!(codec.encode(&members(256)).is_ok());
    assert!(matches!(
        codec.encode(&members(257)),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));
    let oversized_members_token = backend_signed(&members(257));
    assert!(matches!(
        codec.decode::<serde_json::Value>(&oversized_members_token),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));

    let array = |count: usize| {
        serde_json::json!({
            "values": (0..count).map(|_| JsonValue::Null).collect::<Vec<_>>()
        })
    };
    assert!(codec.encode(&array(256)).is_ok());
    assert!(matches!(
        codec.encode(&array(257)),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));
    let oversized_array_token = backend_signed(&array(257));
    assert!(matches!(
        codec.decode::<serde_json::Value>(&oversized_array_token),
        Err(JwtError::InvalidToken { segment: "claims" })
    ));

    let oversized_token = "x".repeat(MAX_TOKEN_BYTES + 1);
    assert!(matches!(
        codec.decode::<serde_json::Value>(&oversized_token),
        Err(JwtError::TokenTooLarge { .. })
    ));
}

#[test]
fn checked_clock_arithmetic_rejects_numeric_date_overflow() {
    let exp_codec = codec(JwtValidation::new());
    let exp_token = exp_codec
        .encode(&serde_json::json!({ "exp": u64::MAX }))
        .unwrap();
    assert!(matches!(
        exp_codec.decode_at::<serde_json::Value>(&exp_token, 1),
        Err(JwtError::InvalidClaim { claim: "exp" })
    ));

    let nbf_codec = codec(
        JwtValidation::new()
            .with_require_exp(false)
            .with_validate_nbf(true),
    );
    let nbf_token = nbf_codec.encode(&serde_json::json!({ "nbf": 0 })).unwrap();
    assert!(matches!(
        nbf_codec.decode_at::<serde_json::Value>(&nbf_token, u64::MAX),
        Err(JwtError::InvalidClaim { claim: "nbf" })
    ));
}

#[test]
fn send_sync_and_sensitive_debug_boundaries_hold() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<JwtCodec>();
    assert_send_sync::<JwtSigningKey>();
    assert_send_sync::<JwtVerificationKey>();
    assert_send_sync::<JwtConfig>();
    let key = JwtSigningKey::from_hmac_secret(b"SENTINEL_SIGNING_KEY_MATERIAL_123456").unwrap();
    let key_display = format!("{key}");
    let config =
        JwtConfig::new(JwtAlgorithm::Hs256, Some(key), None, JwtValidation::new()).unwrap();
    let debug = format!("{config:?}");
    assert!(!debug.contains("SENTINEL_SIGNING_KEY_MATERIAL_123456"));
    assert!(!format!("{config}").contains("SENTINEL_SIGNING_KEY_MATERIAL_123456"));
    assert!(!key_display.contains("SENTINEL_SIGNING_KEY_MATERIAL_123456"));
}
