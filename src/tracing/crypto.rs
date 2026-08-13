use std::time::Instant;

use crate::crypto::CryptoError;

pub(crate) fn record_init(
    operation: &'static str,
    result: &Result<(), CryptoError>,
    started: Instant,
) {
    let duration_ms = super::duration_ms(started);
    match result {
        Ok(()) => ::tracing::debug!(
            target: "axutils::crypto",
            operation,
            outcome = "success",
            duration_ms,
        ),
        Err(error) => ::tracing::warn!(
            target: "axutils::crypto",
            operation,
            outcome = "error",
            error_kind = error_kind(error),
            duration_ms,
        ),
    }
}

fn error_kind(error: &CryptoError) -> &'static str {
    match error {
        CryptoError::OddHexLength { .. } => "odd_hex_length",
        CryptoError::InvalidHex { .. } => "invalid_hex",
        CryptoError::TextDecodeInvalid { .. } => "text_decode_invalid",
        CryptoError::OutputTooLarge { .. } => "output_too_large",
        #[cfg(feature = "encoding_rs")]
        CryptoError::TextEncodeUnmappable { .. } => "text_encode_unmappable",
        #[cfg(feature = "base64")]
        CryptoError::Base64Decode { .. } => "base64_decode",
        CryptoError::InvalidKeyLength { .. } => "invalid_key_length",
        CryptoError::InvalidIvLength { .. } => "invalid_iv_length",
        CryptoError::NotInitialized => "not_initialized",
        CryptoError::AlreadyInitialized => "already_initialized",
        CryptoError::CiphertextTooShort { .. } => "ciphertext_too_short",
        CryptoError::Decrypt => "decrypt",
        CryptoError::Encrypt => "encrypt",
        CryptoError::RandomSource => "random_source",
    }
}
