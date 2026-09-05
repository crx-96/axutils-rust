//! AES 容器格式、显式 IV/nonce 与共享校验。

use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{cbc, gcm, random, AesKey, AesMode};

pub(crate) fn encrypt(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    encrypt_with_random(plaintext, key, mode, random::random_bytes)
}

pub(crate) fn decrypt(input: &[u8], key: &AesKey, mode: AesMode) -> Result<Vec<u8>, CryptoError> {
    let iv_len = mode.iv_length();
    let minimum = iv_len
        .checked_add(mode.min_body_length())
        .expect("iv_length + min_body_length never overflows usize");
    if input.len() < minimum {
        return Err(CryptoError::CiphertextTooShort {
            minimum,
            length: input.len(),
        });
    }
    let (iv, body) = input.split_at(iv_len);
    decrypt_with_iv(body, key, iv, mode)
}

pub(crate) fn encrypt_explicit_iv(
    plaintext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    encrypt_with_iv(plaintext, key, iv, mode)
}

pub(crate) fn decrypt_explicit_iv(
    ciphertext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    decrypt_with_iv(ciphertext, key, iv, mode)
}

pub(super) fn encrypt_with_random<F>(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
    mut source: F,
) -> Result<Vec<u8>, CryptoError>
where
    F: FnMut(usize) -> Result<Vec<u8>, CryptoError>,
{
    let iv = source(mode.iv_length())?;
    let mut body = encrypt_with_iv(plaintext, key, &iv, mode)?;
    let total = match iv.len().checked_add(body.len()) {
        Some(total) => total,
        None => {
            body.as_mut_slice().zeroize();
            return Err(CryptoError::OutputTooLarge {
                operation: "aes_encrypt",
            });
        }
    };
    let mut container = Vec::new();
    if container.try_reserve_exact(total).is_err() {
        body.as_mut_slice().zeroize();
        return Err(CryptoError::OutputTooLarge {
            operation: "aes_encrypt",
        });
    }
    container.extend_from_slice(&iv);
    container.extend_from_slice(&body);
    body.as_mut_slice().zeroize();
    Ok(container)
}

fn encrypt_with_iv(
    plaintext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    validate_iv(iv, mode)?;
    match mode {
        AesMode::Gcm => gcm::encrypt(plaintext, key, iv),
        AesMode::CbcPkcs7 => cbc::encrypt(plaintext, key, iv),
    }
}

fn decrypt_with_iv(
    ciphertext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    validate_iv(iv, mode)?;
    let minimum = mode.min_body_length();
    if ciphertext.len() < minimum {
        return Err(CryptoError::CiphertextTooShort {
            minimum,
            length: ciphertext.len(),
        });
    }
    match mode {
        AesMode::Gcm => gcm::decrypt(ciphertext, key, iv),
        AesMode::CbcPkcs7 => cbc::decrypt(ciphertext, key, iv),
    }
}

fn validate_iv(iv: &[u8], mode: AesMode) -> Result<(), CryptoError> {
    if iv.len() != mode.iv_length() {
        return Err(CryptoError::InvalidIvLength {
            expected: mode.iv_length(),
            length: iv.len(),
        });
    }
    Ok(())
}
