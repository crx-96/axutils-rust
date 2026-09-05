use std::time::Instant;

use crate::jwt::JwtError;

pub(crate) fn record_client_init(result: &Result<(), JwtError>, started: Instant) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::jwt",
            operation = "codec_init",
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::jwt",
            operation = "codec_init",
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

fn error_kind(error: &JwtError) -> &'static str {
    match error {
        JwtError::InvalidConfig { .. } => "invalid_config",
        JwtError::InvalidKey { .. } => "invalid_key",
        JwtError::UnsupportedKeyFormat { .. } => "unsupported_key_format",
        JwtError::MissingSigningKey => "missing_signing_key",
        JwtError::MissingVerificationKey => "missing_verification_key",
        JwtError::NotInitialized => "not_initialized",
        JwtError::AlreadyInitialized => "already_initialized",
        JwtError::TokenTooLarge { .. } => "token_too_large",
        JwtError::ClaimsTooLarge { .. } => "claims_too_large",
        JwtError::InvalidHeader { .. } => "invalid_header",
        JwtError::InvalidClaim { .. } => "invalid_claim",
        JwtError::MissingRequiredClaim { .. } => "missing_required_claim",
        JwtError::InvalidToken { .. } => "invalid_token",
    }
}
