//! AES-GCM 加解密原语。

use ::aes::{Aes128, Aes192, Aes256};
use ::aes_gcm::{
    aead::{array::typenum::U12, AeadInOut, KeyInit},
    AesGcm, Key as GcmKey, Nonce as GcmNonce, Tag as GcmTag,
};
use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{AesKey, AesKeyBits};

/// 在主体的明文区域就地加密，将 16 字节认证标签写到预留的尾部。
pub(super) fn encrypt_in_place(
    body: &mut [u8],
    plaintext_len: usize,
    key: &AesKey,
    nonce: &[u8],
) -> Result<(), CryptoError> {
    let nonce = <&GcmNonce<U12>>::try_from(nonce).map_err(|_| CryptoError::Encrypt)?;
    let (message, tag_out) = body.split_at_mut(plaintext_len);

    // 从借用的密钥构造后端，沿用空 AAD，容器前缀和标签区均不作为消息输入。
    macro_rules! encrypt_with {
        ($cipher_ty:ty) => {{
            let cipher =
                <$cipher_ty>::new_from_slice(key.key_bytes()).map_err(|_| CryptoError::Encrypt)?;
            cipher
                .encrypt_inout_detached(nonce, b"", message.into())
                .map_err(|_| CryptoError::Encrypt)?
        }};
    }
    let tag = match key.bits() {
        AesKeyBits::Aes128 => encrypt_with!(AesGcm<Aes128, U12>),
        AesKeyBits::Aes192 => encrypt_with!(AesGcm<Aes192, U12>),
        AesKeyBits::Aes256 => encrypt_with!(AesGcm<Aes256, U12>),
    };
    // 只有加密成功才写入标签，失败时由容器层清零主体。
    tag_out.copy_from_slice(&tag);
    Ok(())
}

/// 校验主体尾部的认证标签，成功后返回明文，失败时擦除工作缓冲区。
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
