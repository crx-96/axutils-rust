//! AES-GCM 加解密原语。

use ::aes::{Aes128, Aes192, Aes256};
use ::aes_gcm::{
    aead::{array::typenum::U12, AeadInOut, KeyInit},
    AesGcm, Key as GcmKey, Nonce as GcmNonce, Tag as GcmTag,
};
use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{AesKey, AesKeyBits};

pub(super) fn encrypt(
    plaintext: &[u8],
    key: &AesKey,
    nonce: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = key.key_bytes();
    let total = plaintext
        .len()
        .checked_add(16)
        .ok_or(CryptoError::OutputTooLarge {
            operation: "aes_gcm_encrypt",
        })?;
    let mut buf = Vec::new();
    buf.try_reserve_exact(total)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "aes_gcm_encrypt",
        })?;
    buf.extend_from_slice(plaintext);
    let nonce = match GcmNonce::<U12>::try_from(nonce) {
        Ok(nonce) => nonce,
        Err(_) => {
            buf.as_mut_slice().zeroize();
            return Err(CryptoError::Encrypt);
        }
    };

    macro_rules! encrypt_with {
        ($cipher_ty:ty) => {{
            let gkey = match GcmKey::<$cipher_ty>::try_from(key_bytes) {
                Ok(key) => key,
                Err(_) => {
                    buf.as_mut_slice().zeroize();
                    return Err(CryptoError::Encrypt);
                }
            };
            let cipher = <$cipher_ty>::new(&gkey);
            match cipher.encrypt_inout_detached(&nonce, b"", buf.as_mut_slice().into()) {
                Ok(tag) => tag,
                Err(_) => {
                    buf.as_mut_slice().zeroize();
                    return Err(CryptoError::Encrypt);
                }
            }
        }};
    }

    let tag = match key.bits() {
        AesKeyBits::Aes128 => encrypt_with!(AesGcm<Aes128, U12>),
        AesKeyBits::Aes192 => encrypt_with!(AesGcm<Aes192, U12>),
        AesKeyBits::Aes256 => encrypt_with!(AesGcm<Aes256, U12>),
    };
    buf.extend_from_slice(&tag);
    Ok(buf)
}

pub(super) fn decrypt(
    ciphertext: &[u8],
    key: &AesKey,
    nonce: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = key.key_bytes();
    let nonce = GcmNonce::<U12>::try_from(nonce).map_err(|_| CryptoError::Decrypt)?;
    let split = ciphertext.len() - 16;
    let (body, tag) = ciphertext.split_at(split);
    let mut buf = Vec::new();
    buf.try_reserve_exact(body.len())
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "aes_gcm_decrypt",
        })?;
    buf.extend_from_slice(body);
    let tag = match GcmTag::try_from(tag) {
        Ok(tag) => tag,
        Err(_) => {
            buf.as_mut_slice().zeroize();
            return Err(CryptoError::Decrypt);
        }
    };

    macro_rules! decrypt_with {
        ($cipher_ty:ty) => {{
            let gkey = match GcmKey::<$cipher_ty>::try_from(key_bytes) {
                Ok(key) => key,
                Err(_) => {
                    buf.as_mut_slice().zeroize();
                    return Err(CryptoError::Decrypt);
                }
            };
            let cipher = <$cipher_ty>::new(&gkey);
            cipher.decrypt_inout_detached(&nonce, b"", buf.as_mut_slice().into(), &tag)
        }};
    }

    let result = match key.bits() {
        AesKeyBits::Aes128 => decrypt_with!(AesGcm<Aes128, U12>),
        AesKeyBits::Aes192 => decrypt_with!(AesGcm<Aes192, U12>),
        AesKeyBits::Aes256 => decrypt_with!(AesGcm<Aes256, U12>),
    };
    if result.is_err() {
        buf.as_mut_slice().zeroize();
        return Err(CryptoError::Decrypt);
    }
    Ok(buf)
}
