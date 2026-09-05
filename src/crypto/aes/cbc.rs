//! AES-CBC + PKCS#7 加解密原语。

use ::aes::{Aes128, Aes192, Aes256};
use ::cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{AesKey, AesKeyBits};

pub(super) fn encrypt(plaintext: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = key.key_bytes();
    let block = 16usize;
    let blocks = plaintext.len() / block;
    let padded_len = blocks
        .checked_add(1)
        .and_then(|blocks| blocks.checked_mul(block))
        .ok_or(CryptoError::OutputTooLarge {
            operation: "aes_cbc_encrypt",
        })?;
    let mut out = Vec::new();
    out.try_reserve_exact(padded_len)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "aes_cbc_encrypt",
        })?;
    out.resize(padded_len, 0);

    macro_rules! encrypt_with {
        ($cipher_ty:ty) => {{
            let enc = match <$cipher_ty>::new_from_slices(key_bytes, iv) {
                Ok(enc) => enc,
                Err(_) => {
                    out.as_mut_slice().zeroize();
                    return Err(CryptoError::Encrypt);
                }
            };
            match enc.encrypt_padded_b2b::<Pkcs7>(plaintext, &mut out) {
                Ok(written) => written.len(),
                Err(_) => {
                    out.as_mut_slice().zeroize();
                    return Err(CryptoError::Encrypt);
                }
            }
        }};
    }

    let written = match key.bits() {
        AesKeyBits::Aes128 => encrypt_with!(::cbc::Encryptor<Aes128>),
        AesKeyBits::Aes192 => encrypt_with!(::cbc::Encryptor<Aes192>),
        AesKeyBits::Aes256 => encrypt_with!(::cbc::Encryptor<Aes256>),
    };
    out.truncate(written);
    Ok(out)
}

pub(super) fn decrypt(ciphertext: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if !ciphertext.len().is_multiple_of(16) {
        return Err(CryptoError::Decrypt);
    }
    let key_bytes = key.key_bytes();
    let mut out = Vec::new();
    out.try_reserve_exact(ciphertext.len())
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "aes_cbc_decrypt",
        })?;
    out.resize(ciphertext.len(), 0);

    macro_rules! decrypt_with {
        ($cipher_ty:ty) => {{
            let dec =
                <$cipher_ty>::new_from_slices(key_bytes, iv).map_err(|_| CryptoError::Decrypt)?;
            dec.decrypt_padded_b2b::<Pkcs7>(ciphertext, &mut out)
                .map(|s| s.len())
                .map_err(|_| CryptoError::Decrypt)
        }};
    }

    let result = match key.bits() {
        AesKeyBits::Aes128 => decrypt_with!(::cbc::Decryptor<Aes128>),
        AesKeyBits::Aes192 => decrypt_with!(::cbc::Decryptor<Aes192>),
        AesKeyBits::Aes256 => decrypt_with!(::cbc::Decryptor<Aes256>),
    };
    match result {
        Ok(written) => {
            out[written..].zeroize();
            out.truncate(written);
            Ok(out)
        }
        Err(error) => {
            out.as_mut_slice().zeroize();
            Err(error)
        }
    }
}
