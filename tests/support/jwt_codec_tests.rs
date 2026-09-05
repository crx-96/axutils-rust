use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Claims {
    exp: u64,
    sub: String,
}

#[allow(unreachable_patterns)]
fn algorithm_codec(algorithm: JwtAlgorithm) -> JwtCodec {
    if matches!(
        algorithm,
        JwtAlgorithm::Hs256 | JwtAlgorithm::Hs384 | JwtAlgorithm::Hs512
    ) {
        return JwtCodec::new(
            JwtConfig::new(
                algorithm,
                Some(JwtSigningKey::from_hmac_secret([0x11; 64]).unwrap()),
                Some(JwtVerificationKey::from_hmac_secret([0x11; 64]).unwrap()),
                JwtValidation::new(),
            )
            .unwrap(),
        );
    }
    let (signing, verification) = match algorithm {
        JwtAlgorithm::Rs256
        | JwtAlgorithm::Rs384
        | JwtAlgorithm::Rs512
        | JwtAlgorithm::Ps256
        | JwtAlgorithm::Ps384
        | JwtAlgorithm::Ps512 => (
            JwtSigningKey::from_rsa_pem(fixture("rsa_private.pem").as_bytes()).unwrap(),
            JwtVerificationKey::from_rsa_pem(fixture("rsa_public.pem").as_bytes()).unwrap(),
        ),
        JwtAlgorithm::Es256 => (
            JwtSigningKey::from_ec_pem(fixture("ec256_private.pem").as_bytes()).unwrap(),
            JwtVerificationKey::from_ec_pem(fixture("ec256_public.pem").as_bytes()).unwrap(),
        ),
        JwtAlgorithm::Es384 => (
            JwtSigningKey::from_ec_pem(fixture("ec384_private.pem").as_bytes()).unwrap(),
            JwtVerificationKey::from_ec_pem(fixture("ec384_public.pem").as_bytes()).unwrap(),
        ),
        JwtAlgorithm::Ed25519 => (
            JwtSigningKey::from_ed_pem(fixture("ed25519_private.pem").as_bytes()).unwrap(),
            JwtVerificationKey::from_ed_pem(fixture("ed25519_public.pem").as_bytes()).unwrap(),
        ),
        _ => unreachable!(),
    };
    JwtCodec::new(
        JwtConfig::new(
            algorithm,
            Some(signing),
            Some(verification),
            JwtValidation::new(),
        )
        .unwrap(),
    )
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("jwt_keys")
            .join(name),
    )
    .unwrap_or_else(|_| panic!("missing or unreadable JWT test fixture `{name}`"))
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
fn asymmetric_fixture_material_is_present_for_round_trip_tests() {
    for name in [
        "rsa_private.pem",
        "rsa_public.pem",
        "ec256_private.pem",
        "ec256_public.pem",
        "ec384_private.pem",
        "ec384_public.pem",
        "ed25519_private.pem",
        "ed25519_public.pem",
    ] {
        let _ = fixture(name);
    }
}

#[test]
fn every_supported_algorithm_round_trips_with_public_key_verification() {
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
    for algorithm in algorithms {
        let codec = algorithm_codec(algorithm);
        let claims = Claims {
            exp: 2_000_000_000,
            sub: "algorithm-test".to_owned(),
        };
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);
        let tampered_algorithm = if algorithm == JwtAlgorithm::Hs256 {
            "HS512"
        } else {
            "HS256"
        };
        let tampered_header = replace_header(
            &token,
            &format!(r#"{{"typ":"JWT","alg":"{tampered_algorithm}"}}"#),
        );
        assert!(matches!(
            codec.decode::<Claims>(&tampered_header),
            Err(JwtError::InvalidHeader { field: "alg" })
        ));
    }
}

#[test]
fn der_key_constructors_round_trip_each_asymmetric_family() {
    let rsa_private_pem = fixture("rsa_private.pem");
    let rsa_public_pem = fixture("rsa_public.pem");
    let rsa_private = EncodingKey::from_rsa_pem(rsa_private_pem.as_bytes())
        .unwrap()
        .as_bytes()
        .to_vec();
    let rsa_public = DecodingKey::from_rsa_pem(rsa_public_pem.as_bytes())
        .unwrap()
        .try_get_as_bytes()
        .unwrap()
        .to_vec();
    for algorithm in [
        JwtAlgorithm::Rs256,
        JwtAlgorithm::Rs384,
        JwtAlgorithm::Rs512,
        JwtAlgorithm::Ps256,
        JwtAlgorithm::Ps384,
        JwtAlgorithm::Ps512,
    ] {
        let codec = JwtCodec::new(
            JwtConfig::new(
                algorithm,
                Some(JwtSigningKey::from_rsa_der(&rsa_private).unwrap()),
                Some(JwtVerificationKey::from_rsa_der(&rsa_public).unwrap()),
                JwtValidation::new(),
            )
            .unwrap(),
        );
        let claims = Claims {
            exp: 2_000_000_000,
            sub: "rsa-der".to_owned(),
        };
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);
    }

    let ec_families = [
        (JwtAlgorithm::Es256, "ec256_private.pem", "ec256_public.pem"),
        (JwtAlgorithm::Es384, "ec384_private.pem", "ec384_public.pem"),
    ];
    for (algorithm, private_name, public_name) in ec_families {
        let private_pem = fixture(private_name);
        let public_pem = fixture(public_name);
        let private = EncodingKey::from_ec_pem(private_pem.as_bytes())
            .unwrap()
            .as_bytes()
            .to_vec();
        let public = DecodingKey::from_ec_pem(public_pem.as_bytes())
            .unwrap()
            .try_get_as_bytes()
            .unwrap()
            .to_vec();
        let codec = JwtCodec::new(
            JwtConfig::new(
                algorithm,
                Some(JwtSigningKey::from_ec_der(&private).unwrap()),
                Some(JwtVerificationKey::from_ec_der(&public).unwrap()),
                JwtValidation::new(),
            )
            .unwrap(),
        );
        let claims = Claims {
            exp: 2_000_000_000,
            sub: "ec-der".to_owned(),
        };
        let token = codec.encode(&claims).unwrap();
        assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);
    }

    let ed_private_pem = fixture("ed25519_private.pem");
    let ed_public_pem = fixture("ed25519_public.pem");
    let ed_private = EncodingKey::from_ed_pem(ed_private_pem.as_bytes())
        .unwrap()
        .as_bytes()
        .to_vec();
    let ed_public = DecodingKey::from_ed_pem(ed_public_pem.as_bytes())
        .unwrap()
        .try_get_as_bytes()
        .unwrap()
        .to_vec();
    let codec = JwtCodec::new(
        JwtConfig::new(
            JwtAlgorithm::Ed25519,
            Some(JwtSigningKey::from_ed_der(&ed_private).unwrap()),
            Some(JwtVerificationKey::from_ed_der(&ed_public).unwrap()),
            JwtValidation::new(),
        )
        .unwrap(),
    );
    let claims = Claims {
        exp: 2_000_000_000,
        sub: "ed-der".to_owned(),
    };
    let token = codec.encode(&claims).unwrap();
    assert_eq!(codec.decode::<Claims>(&token).unwrap(), claims);
}

#[test]
fn config_rejects_cross_curve_ecdsa_keys_before_initialization() {
    let ec256_private = fixture("ec256_private.pem");
    let p256 = JwtSigningKey::from_ec_pem(ec256_private.as_bytes()).unwrap();
    assert!(matches!(
        JwtConfig::new(JwtAlgorithm::Es384, Some(p256), None, JwtValidation::new()),
        Err(JwtError::InvalidKey { kind: "ec_curve" })
    ));
    let ec384_private = fixture("ec384_private.pem");
    let p384 = JwtSigningKey::from_ec_pem(ec384_private.as_bytes()).unwrap();
    assert!(matches!(
        JwtConfig::new(JwtAlgorithm::Es256, Some(p384), None, JwtValidation::new()),
        Err(JwtError::InvalidKey { kind: "ec_curve" })
    ));
}

#[test]
fn config_rejects_rsa_public_der_as_a_signing_key() {
    let public_pem = fixture("rsa_public.pem");
    let backend = DecodingKey::from_rsa_pem(public_pem.as_bytes()).unwrap();
    let public_der = backend.try_get_as_bytes().unwrap().to_vec();
    let signing = JwtSigningKey::from_rsa_der(public_der).unwrap();
    assert!(matches!(
        JwtConfig::new(
            JwtAlgorithm::Rs256,
            Some(signing),
            None,
            JwtValidation::new()
        ),
        Err(JwtError::InvalidKey {
            kind: "signing_key_role"
        })
    ));
}
