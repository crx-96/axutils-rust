use super::super::{JwtError, KeyFamily};
use jsonwebtoken::errors::Error as JsonWebTokenError;

pub(super) fn map_encode_error(error: &JsonWebTokenError) -> JwtError {
    use jsonwebtoken::errors::ErrorKind;

    match error.kind() {
        ErrorKind::InvalidEcdsaKey
        | ErrorKind::InvalidEddsaKey
        | ErrorKind::InvalidKeyFormat
        | ErrorKind::InvalidRsaKey(_)
        | ErrorKind::RsaFailedSigning
        | ErrorKind::Signing(_)
        | ErrorKind::Provider(_) => JwtError::UnsupportedKeyFormat {
            kind: "signing_key",
        },
        ErrorKind::InvalidAlgorithm => JwtError::InvalidKey {
            kind: "signing_algorithm",
        },
        _ => JwtError::InvalidToken { segment: "encode" },
    }
}

pub(super) fn map_decode_error(error: &JsonWebTokenError, family: KeyFamily) -> JwtError {
    use jsonwebtoken::errors::ErrorKind;

    match error.kind() {
        ErrorKind::InvalidEcdsaKey
        | ErrorKind::InvalidEddsaKey
        | ErrorKind::InvalidKeyFormat
        | ErrorKind::InvalidRsaKey(_)
        | ErrorKind::Provider(_)
            if family != KeyFamily::Hmac =>
        {
            JwtError::UnsupportedKeyFormat {
                kind: "verification_key",
            }
        }
        ErrorKind::InvalidAlgorithm => JwtError::InvalidHeader { field: "alg" },
        _ => JwtError::InvalidToken { segment: "token" },
    }
}
