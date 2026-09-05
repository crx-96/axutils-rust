#![cfg(feature = "jwt")]

use axutils::jwt::{
    JwtAlgorithm, JwtCodec, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/jwt_codec_tests.rs"
));
