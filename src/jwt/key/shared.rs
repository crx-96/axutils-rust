use jsonwebtoken::errors::{Error, ErrorKind};

use super::super::{EcCurve, JwtError};

pub(super) const MAX_KEY_BYTES: usize = 128 * 1024;
pub(super) const MAX_HMAC_SECRET_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RsaDerKind {
    Public,
    Private,
    Unknown,
}

pub(super) fn validate_key_size(bytes: &[u8], kind: &'static str) -> Result<(), JwtError> {
    if bytes.is_empty() {
        return Err(JwtError::InvalidKey { kind });
    }
    if bytes.len() > MAX_KEY_BYTES {
        return Err(JwtError::InvalidConfig { field: "key_size" });
    }
    Ok(())
}

pub(super) fn validate_pem_label(
    bytes: &[u8],
    allowed: &[&str],
    kind: &'static str,
    signing: bool,
) -> Result<(), JwtError> {
    let Some(label) = pem_label(bytes) else {
        return Err(JwtError::UnsupportedKeyFormat { kind });
    };
    if allowed.contains(&label) {
        return Ok(());
    }
    if signing || label.contains("PRIVATE") || label.contains("PUBLIC") {
        Err(JwtError::InvalidKey { kind })
    } else {
        Err(JwtError::UnsupportedKeyFormat { kind })
    }
}

pub(super) fn map_backend_key_error(error: &Error, kind: &'static str) -> JwtError {
    match error.kind() {
        ErrorKind::InvalidEcdsaKey
        | ErrorKind::InvalidEddsaKey
        | ErrorKind::InvalidKeyFormat
        | ErrorKind::InvalidRsaKey(_) => JwtError::UnsupportedKeyFormat { kind },
        _ => JwtError::InvalidKey { kind },
    }
}

pub(super) fn rsa_modulus_bits(bytes: &[u8]) -> Option<usize> {
    let (sequence, remainder) = read_der_value_and_rest(bytes, 0x30)?;
    if !remainder.is_empty() {
        return None;
    }
    let (first, rest) = read_der_value_and_rest(sequence, 0x02)?;
    let modulus = if first.len() == 1 && (first[0] == 0 || first[0] == 1) {
        read_der_value_and_rest(rest, 0x02)?.0
    } else {
        first
    };
    let first_nonzero = modulus.iter().position(|byte| *byte != 0)?;
    let significant = &modulus[first_nonzero..];
    Some((significant.len() - 1) * 8 + (8 - significant[0].leading_zeros() as usize))
}

pub(super) fn rsa_der_kind(bytes: &[u8]) -> RsaDerKind {
    let Some((sequence, remainder)) = read_der_value_and_rest(bytes, 0x30) else {
        return RsaDerKind::Unknown;
    };
    if !remainder.is_empty() {
        return RsaDerKind::Unknown;
    }
    let Some((first, rest)) = read_der_value_and_rest(sequence, 0x02) else {
        return RsaDerKind::Unknown;
    };
    let Some((second, remainder)) = read_der_value_and_rest(rest, 0x02) else {
        return RsaDerKind::Unknown;
    };
    if first.len() == 1 && (first[0] == 0 || first[0] == 1) {
        return RsaDerKind::Private;
    }
    let Some(modulus_bits) = rsa_modulus_bits(bytes) else {
        return RsaDerKind::Unknown;
    };
    let exponent = second
        .iter()
        .position(|byte| *byte != 0)
        .map(|index| &second[index..]);
    let exponent_is_usable = exponent.is_some_and(|exponent| {
        exponent.last().is_some_and(|byte| byte & 1 == 1) && (exponent.len() > 1 || exponent[0] > 2)
    }) && remainder.is_empty();
    if modulus_bits >= 2048 && exponent_is_usable {
        RsaDerKind::Public
    } else {
        RsaDerKind::Unknown
    }
}

pub(super) fn ec_curve_from_private_der(bytes: &[u8]) -> Option<EcCurve> {
    const P256_OID: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
    const P384_OID: &[u8] = &[0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x22];
    if bytes
        .windows(P256_OID.len())
        .any(|window| window == P256_OID)
    {
        Some(EcCurve::P256)
    } else if bytes
        .windows(P384_OID.len())
        .any(|window| window == P384_OID)
    {
        Some(EcCurve::P384)
    } else {
        None
    }
}

pub(super) fn ec_curve_from_public_point(bytes: &[u8]) -> Option<EcCurve> {
    match (bytes.first(), bytes.len()) {
        (Some(0x04), 65) => Some(EcCurve::P256),
        (Some(0x04), 97) => Some(EcCurve::P384),
        _ => None,
    }
}

fn pem_label(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let start_marker = "-----BEGIN ";
    let start = text.find(start_marker)? + start_marker.len();
    let end = text[start..].find("-----")?;
    Some(&text[start..start + end])
}

fn read_der_value_and_rest(input: &[u8], expected_tag: u8) -> Option<(&[u8], &[u8])> {
    if input.len() < 2 || input[0] != expected_tag {
        return None;
    }
    let (length, header_size) = der_length(&input[1..])?;
    let end = header_size.checked_add(length)?.checked_add(1)?;
    if end > input.len() {
        return None;
    }
    Some((&input[1 + header_size..end], &input[end..]))
}

fn der_length(input: &[u8]) -> Option<(usize, usize)> {
    let first = *input.first()?;
    if first & 0x80 == 0 {
        return Some((first as usize, 1));
    }
    let count = (first & 0x7f) as usize;
    if count == 0 || count > std::mem::size_of::<usize>() || input.len() < count + 1 {
        return None;
    }
    let mut length = 0usize;
    for byte in &input[1..=count] {
        length = length.checked_shl(8)?.checked_add(*byte as usize)?;
    }
    Some((length, count + 1))
}
