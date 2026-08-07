#[cfg(any(
    feature = "jwt-only",
    feature = "jwt-serde",
    feature = "jwt-lettre",
    feature = "jwt-aes",
    feature = "jwt-tokio",
    feature = "jwt-regex",
    feature = "all",
))]
fn assert_jwt_api() {
    use axutils::{
        JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtUtils, JwtValidation,
        JwtVerificationKey,
    };

    #[derive(serde::Deserialize, serde::Serialize)]
    struct Claims {
        exp: u64,
    }

    let _: axutils::JwtAlgorithm = JwtAlgorithm::Hs256;
    let _: axutils::jwt::JwtAlgorithm = JwtAlgorithm::Hs256;
    let _: axutils::JwtSigningKey = JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap();
    let _: axutils::jwt::JwtSigningKey = JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap();
    let _: axutils::JwtVerificationKey =
        JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap();
    let _: axutils::jwt::JwtVerificationKey =
        JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap();
    let signing = JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap();
    let verification = JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap();
    let _: axutils::JwtValidation = JwtValidation::new();
    let _: axutils::jwt::JwtValidation = JwtValidation::new();
    let _: axutils::JwtConfig = JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(signing),
        Some(verification),
        JwtValidation::new(),
    )
    .unwrap();
    let _: axutils::jwt::JwtConfig = JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(JwtSigningKey::from_hmac_secret([0x11; 32]).unwrap()),
        Some(JwtVerificationKey::from_hmac_secret([0x11; 32]).unwrap()),
        JwtValidation::new(),
    )
    .unwrap();
    let _: axutils::jwt::JwtError = JwtError::NotInitialized;
    let _: axutils::JwtError = JwtError::NotInitialized;

    let _: axutils::JwtUtils = axutils::JwtUtils;
    let _: axutils::utils::JwtUtils = axutils::utils::JwtUtils;
    let _: axutils::utils::jwt_utils::JwtUtils = axutils::utils::jwt_utils::JwtUtils;
    let _: fn(&Claims) -> Result<String, JwtError> = JwtUtils::encode::<Claims>;
    let _: fn(&str) -> Result<Claims, JwtError> = JwtUtils::decode::<Claims>;
}

#[cfg(any(
    feature = "jwt-only",
    feature = "jwt-serde",
    feature = "jwt-lettre",
    feature = "jwt-aes",
    feature = "jwt-tokio",
    feature = "jwt-regex",
    feature = "all",
))]
fn main() {
    assert_jwt_api();
}

#[cfg(any(feature = "none", feature = "serde-only"))]
fn main() {}

#[cfg(feature = "negative-none-jwt-module")]
fn main() {
    use axutils::jwt::JwtAlgorithm;
    let _ = JwtAlgorithm::Hs256;
}

#[cfg(feature = "negative-none-jwt-algorithm")]
fn main() {
    let _ = axutils::JwtAlgorithm::Hs256;
}

#[cfg(feature = "negative-none-jwt-signing-key")]
fn main() {
    let _ = axutils::JwtSigningKey::from_hmac_secret;
}

#[cfg(feature = "negative-none-jwt-verification-key")]
fn main() {
    let _ = axutils::JwtVerificationKey::from_hmac_secret;
}

#[cfg(feature = "negative-none-jwt-config")]
fn main() {
    let _ = axutils::JwtConfig::new;
}

#[cfg(feature = "negative-none-jwt-validation")]
fn main() {
    let _ = axutils::JwtValidation::new;
}

#[cfg(feature = "negative-none-jwt-error")]
fn main() {
    let _ = axutils::JwtError::NotInitialized;
}

#[cfg(feature = "negative-none-jwt-utils")]
fn main() {
    let _ = axutils::JwtUtils::is_initialized;
}

#[cfg(feature = "negative-none-utils-jwt-utils")]
fn main() {
    let _ = axutils::utils::JwtUtils::is_initialized;
}

#[cfg(feature = "negative-none-direct-jwt-utils")]
fn main() {
    let _ = axutils::utils::jwt_utils::JwtUtils::is_initialized;
}

#[cfg(feature = "negative-serde-only-jwt-module")]
fn main() {
    use axutils::jwt::JwtAlgorithm;
    let _ = JwtAlgorithm::Hs256;
}

#[cfg(feature = "negative-jwt-only-config")]
fn main() {
    use axutils::config::ConfigLoader;
    let _ = ConfigLoader::new;
}

#[cfg(feature = "negative-jwt-only-config-loader")]
fn main() {
    let _ = axutils::ConfigLoader::new;
}

#[cfg(not(any(
    feature = "none",
    feature = "jwt-only",
    feature = "serde-only",
    feature = "jwt-serde",
    feature = "jwt-lettre",
    feature = "jwt-aes",
    feature = "jwt-tokio",
    feature = "jwt-regex",
    feature = "all",
    feature = "negative-none-jwt-module",
    feature = "negative-none-jwt-algorithm",
    feature = "negative-none-jwt-signing-key",
    feature = "negative-none-jwt-verification-key",
    feature = "negative-none-jwt-config",
    feature = "negative-none-jwt-validation",
    feature = "negative-none-jwt-error",
    feature = "negative-none-jwt-utils",
    feature = "negative-none-utils-jwt-utils",
    feature = "negative-none-direct-jwt-utils",
    feature = "negative-serde-only-jwt-module",
    feature = "negative-jwt-only-config",
    feature = "negative-jwt-only-config-loader",
)))]
fn main() {}
