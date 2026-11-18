//! AES-CBC + PKCS#7 加解密原语。

use ::aes::{Aes128, Aes192, Aes256};
use ::cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{AesKey, AesKeyBits};

/// 在主体前 `plaintext_len` 字节上应用 PKCS#7，并将密文写回同一主体。
pub(super) fn encrypt_in_place(
    body: &mut [u8],
    plaintext_len: usize,
    key: &AesKey,
    iv: &[u8],
) -> Result<(), CryptoError> {
    // 同一套填充和错误映射用于所有密钥长度，出错时由持有主体的容器层擦除。
    macro_rules! encrypt_with {
        ($cipher_ty:ty) => {{
            let enc = <$cipher_ty>::new_from_slices(key.key_bytes(), iv)
                .map_err(|_| CryptoError::Encrypt)?;
            enc.encrypt_padded::<Pkcs7>(body, plaintext_len)
                .map(|_| ())
                .map_err(|_| CryptoError::Encrypt)
        }};
    }
    match key.bits() {
        AesKeyBits::Aes128 => encrypt_with!(::cbc::Encryptor<Aes128>),
        AesKeyBits::Aes192 => encrypt_with!(::cbc::Encryptor<Aes192>),
        AesKeyBits::Aes256 => encrypt_with!(::cbc::Encryptor<Aes256>),
    }
}

/// 检查块边界，解密并移除 PKCS#7 填充，失败时擦除部分明文。
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
