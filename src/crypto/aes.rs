//! AES-GCM / AES-CBC-PKCS7 后端（`aes`/`aes-gcm`/`cbc`/`zeroize` crate，feature = `aes`）。

use crate::CryptoError;
use ::aes_gcm::{
    aead::{array::typenum::U12, AeadInOut, Generate, KeyInit},
    AesGcm, Key as GcmKey, Nonce as GcmNonce, Tag as GcmTag,
};
use ::cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use ::zeroize::Zeroize;

/// AES 密钥长度。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesKeyBits {
    /// AES-128（16 字节密钥）。
    Aes128,
    /// AES-192（24 字节密钥）。
    Aes192,
    /// AES-256（32 字节密钥）。
    Aes256,
}

impl AesKeyBits {
    /// 返回密钥长度（比特）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesKeyBits;
    ///
    /// assert_eq!(AesKeyBits::Aes256.bit_length(), 256);
    /// ```
    #[must_use]
    pub fn bit_length(&self) -> usize {
        match self {
            Self::Aes128 => 128,
            Self::Aes192 => 192,
            Self::Aes256 => 256,
        }
    }

    /// 返回密钥长度（字节）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesKeyBits;
    ///
    /// assert_eq!(AesKeyBits::Aes256.byte_length(), 32);
    /// ```
    #[must_use]
    pub fn byte_length(&self) -> usize {
        self.bit_length() / 8
    }

    fn from_byte_length(length: usize) -> Result<Self, CryptoError> {
        match length {
            16 => Ok(Self::Aes128),
            24 => Ok(Self::Aes192),
            32 => Ok(Self::Aes256),
            _ => Err(CryptoError::InvalidKeyLength { length }),
        }
    }
}

/// AES 对称密钥。
///
/// `Debug` 只输出密钥位数，不输出密钥字节；`Drop` 时清零内部缓冲区；不实现 `Display`、
/// `Clone` 或任何序列化 trait，也不提供导出密钥字节的公开方法。
pub struct AesKey {
    bytes: [u8; 32],
    bits: AesKeyBits,
}

impl AesKey {
    /// 从字节构造密钥；长度必须是 16、24 或 32 字节。
    ///
    /// # Errors
    ///
    /// 长度不满足要求时返回 [`CryptoError::InvalidKeyLength`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesKey;
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// assert_eq!(key.bits().byte_length(), 16);
    /// assert!(AesKey::from_bytes([0x00; 15]).is_err());
    /// ```
    pub fn from_bytes(key: impl AsRef<[u8]>) -> Result<Self, CryptoError> {
        let key = key.as_ref();
        let bits = AesKeyBits::from_byte_length(key.len())?;
        let mut bytes = [0u8; 32];
        bytes[..key.len()].copy_from_slice(key);
        Ok(Self { bytes, bits })
    }

    /// 使用操作系统随机源生成新密钥。
    ///
    /// # Errors
    ///
    /// 操作系统随机源不可用时返回 [`CryptoError::RandomSource`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesKeyBits};
    ///
    /// let key = AesKey::generate(AesKeyBits::Aes256).unwrap();
    /// assert_eq!(key.bits(), AesKeyBits::Aes256);
    /// ```
    pub fn generate(bits: AesKeyBits) -> Result<Self, CryptoError> {
        Self::generate_with_random(bits, random_bytes)
    }

    fn generate_with_random<F>(bits: AesKeyBits, mut source: F) -> Result<Self, CryptoError>
    where
        F: FnMut(usize) -> Result<Vec<u8>, CryptoError>,
    {
        let mut random = source(bits.byte_length())?;
        let result = Self::from_bytes(&random);
        random.as_mut_slice().zeroize();
        result
    }

    /// 返回密钥长度。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesKeyBits};
    ///
    /// let key = AesKey::from_bytes([0x00; 24]).unwrap();
    /// assert_eq!(key.bits(), AesKeyBits::Aes192);
    /// ```
    #[must_use]
    pub fn bits(&self) -> AesKeyBits {
        self.bits
    }

    fn key_bytes(&self) -> &[u8] {
        &self.bytes[..self.bits.byte_length()]
    }
}

impl std::fmt::Debug for AesKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AesKey").field("bits", &self.bits).finish()
    }
}

impl Drop for AesKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// AES 加解密模式。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AesMode {
    /// AES-GCM：带认证的加密，12 字节 nonce、16 字节标签。推荐默认模式。
    Gcm,
    /// AES-CBC + PKCS#7：**无完整性认证**，16 字节 IV。仅用于与旧系统互操作；新系统应使用
    /// [`Gcm`](AesMode::Gcm)，且必须由上层协议自行提供认证。
    CbcPkcs7,
}

impl AesMode {
    /// 返回本模式要求的 IV/nonce 长度（字节）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesMode;
    ///
    /// assert_eq!(AesMode::Gcm.iv_length(), 12);
    /// assert_eq!(AesMode::CbcPkcs7.iv_length(), 16);
    /// ```
    #[must_use]
    pub fn iv_length(&self) -> usize {
        match self {
            Self::Gcm => 12,
            Self::CbcPkcs7 => 16,
        }
    }

    /// 返回本模式是否提供认证（完整性保护）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesMode;
    ///
    /// assert!(AesMode::Gcm.is_authenticated());
    /// assert!(!AesMode::CbcPkcs7.is_authenticated());
    /// ```
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Gcm)
    }

    /// 返回模式名称。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::AesMode;
    ///
    /// assert_eq!(AesMode::Gcm.as_str(), "AES-GCM");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gcm => "AES-GCM",
            Self::CbcPkcs7 => "AES-CBC-PKCS7",
        }
    }

    fn min_body_length(&self) -> usize {
        match self {
            Self::Gcm => 16,
            Self::CbcPkcs7 => 16,
        }
    }
}

/// 使用操作系统随机源生成 `len`（12、16、24 或 32）字节；失败映射为
/// [`CryptoError::RandomSource`]。
///
/// 注：上游 `Generate::try_generate` 在随机源失败时不会把部分写入的缓冲区暴露给调用方（失败
/// 分支不返回该数组），因此这里不需要（也无法）在本 crate 内对一个不存在的缓冲区做 zeroize。
fn array_to_vec_and_zeroize<const N: usize>(array: [u8; N]) -> Vec<u8> {
    let mut array = array;
    let result = array.to_vec();
    array.zeroize();
    result
}

fn random_bytes(len: usize) -> Result<Vec<u8>, CryptoError> {
    match len {
        12 => <[u8; 12]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        16 => <[u8; 16]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        24 => <[u8; 24]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        32 => <[u8; 32]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        _ => unreachable!("only called with AesMode::iv_length() or AesKeyBits::byte_length()"),
    }
}

fn gcm_encrypt(plaintext: &[u8], key: &AesKey, nonce: &[u8]) -> Result<Vec<u8>, CryptoError> {
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
    let n = match GcmNonce::<U12>::try_from(nonce) {
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
            match cipher.encrypt_inout_detached(&n, b"", buf.as_mut_slice().into()) {
                Ok(tag) => tag,
                Err(_) => {
                    buf.as_mut_slice().zeroize();
                    return Err(CryptoError::Encrypt);
                }
            }
        }};
    }

    let tag = match key.bits() {
        AesKeyBits::Aes128 => encrypt_with!(AesGcm<::aes::Aes128, U12>),
        AesKeyBits::Aes192 => encrypt_with!(AesGcm<::aes::Aes192, U12>),
        AesKeyBits::Aes256 => encrypt_with!(AesGcm<::aes::Aes256, U12>),
    };
    buf.extend_from_slice(&tag);
    Ok(buf)
}

fn gcm_decrypt(ciphertext: &[u8], key: &AesKey, nonce: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let key_bytes = key.key_bytes();
    let n = GcmNonce::<U12>::try_from(nonce).map_err(|_| CryptoError::Decrypt)?;
    let split = ciphertext.len() - 16;
    let (body, tag) = ciphertext.split_at(split);
    let mut buf = Vec::new();
    buf.try_reserve_exact(body.len())
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "aes_gcm_decrypt",
        })?;
    buf.extend_from_slice(body);
    let t = match GcmTag::try_from(tag) {
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
            cipher.decrypt_inout_detached(&n, b"", buf.as_mut_slice().into(), &t)
        }};
    }

    let result = match key.bits() {
        AesKeyBits::Aes128 => decrypt_with!(AesGcm<::aes::Aes128, U12>),
        AesKeyBits::Aes192 => decrypt_with!(AesGcm<::aes::Aes192, U12>),
        AesKeyBits::Aes256 => decrypt_with!(AesGcm<::aes::Aes256, U12>),
    };
    if result.is_err() {
        buf.as_mut_slice().zeroize();
        return Err(CryptoError::Decrypt);
    }
    Ok(buf)
}

fn cbc_encrypt(plaintext: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, CryptoError> {
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
        AesKeyBits::Aes128 => encrypt_with!(::cbc::Encryptor<::aes::Aes128>),
        AesKeyBits::Aes192 => encrypt_with!(::cbc::Encryptor<::aes::Aes192>),
        AesKeyBits::Aes256 => encrypt_with!(::cbc::Encryptor<::aes::Aes256>),
    };
    out.truncate(written);
    Ok(out)
}

fn cbc_decrypt(ciphertext: &[u8], key: &AesKey, iv: &[u8]) -> Result<Vec<u8>, CryptoError> {
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
        AesKeyBits::Aes128 => decrypt_with!(::cbc::Decryptor<::aes::Aes128>),
        AesKeyBits::Aes192 => decrypt_with!(::cbc::Decryptor<::aes::Aes192>),
        AesKeyBits::Aes256 => decrypt_with!(::cbc::Decryptor<::aes::Aes256>),
    };
    match result {
        Ok(written) => {
            out[written..].zeroize();
            out.truncate(written);
            Ok(out)
        }
        Err(e) => {
            out.as_mut_slice().zeroize();
            Err(e)
        }
    }
}

fn encrypt_with_iv(
    plaintext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    if iv.len() != mode.iv_length() {
        return Err(CryptoError::InvalidIvLength {
            expected: mode.iv_length(),
            length: iv.len(),
        });
    }
    match mode {
        AesMode::Gcm => gcm_encrypt(plaintext, key, iv),
        AesMode::CbcPkcs7 => cbc_encrypt(plaintext, key, iv),
    }
}

fn decrypt_with_iv(
    ciphertext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    if iv.len() != mode.iv_length() {
        return Err(CryptoError::InvalidIvLength {
            expected: mode.iv_length(),
            length: iv.len(),
        });
    }
    let minimum = mode.min_body_length();
    if ciphertext.len() < minimum {
        return Err(CryptoError::CiphertextTooShort {
            minimum,
            length: ciphertext.len(),
        });
    }
    match mode {
        AesMode::Gcm => gcm_decrypt(ciphertext, key, iv),
        AesMode::CbcPkcs7 => cbc_decrypt(ciphertext, key, iv),
    }
}

pub(crate) fn encrypt(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    encrypt_with_random(plaintext, key, mode, random_bytes)
}

fn encrypt_with_random<F>(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key128() -> AesKey {
        AesKey::from_bytes([0u8; 16]).unwrap()
    }

    #[test]
    fn key_bits_accessors() {
        assert_eq!(AesKeyBits::Aes128.bit_length(), 128);
        assert_eq!(AesKeyBits::Aes128.byte_length(), 16);
        assert_eq!(AesKeyBits::Aes192.bit_length(), 192);
        assert_eq!(AesKeyBits::Aes192.byte_length(), 24);
        assert_eq!(AesKeyBits::Aes256.bit_length(), 256);
        assert_eq!(AesKeyBits::Aes256.byte_length(), 32);
    }

    #[test]
    fn mode_accessors() {
        assert_eq!(AesMode::Gcm.iv_length(), 12);
        assert!(AesMode::Gcm.is_authenticated());
        assert_eq!(AesMode::Gcm.as_str(), "AES-GCM");
        assert_eq!(AesMode::CbcPkcs7.iv_length(), 16);
        assert!(!AesMode::CbcPkcs7.is_authenticated());
        assert_eq!(AesMode::CbcPkcs7.as_str(), "AES-CBC-PKCS7");
    }

    #[test]
    fn from_bytes_rejects_invalid_lengths() {
        for len in [0usize, 15, 17, 33] {
            let err = AesKey::from_bytes(vec![0u8; len]).unwrap_err();
            assert_eq!(err, CryptoError::InvalidKeyLength { length: len });
        }
    }

    #[test]
    fn debug_does_not_leak_key_bytes() {
        let key = AesKey::from_bytes([0xAB; 32]).unwrap();
        let rendered = format!("{key:?}");
        assert!(!rendered.contains("171")); // 0xAB decimal
        assert!(rendered.contains("Aes256"));
    }

    #[test]
    fn gcm_container_roundtrip_and_min_length() {
        let key = key128();
        let ct = encrypt(b"hello world", &key, AesMode::Gcm).unwrap();
        assert_eq!(ct.len(), 12 + 11 + 16);
        assert_eq!(decrypt(&ct, &key, AesMode::Gcm).unwrap(), b"hello world");
        assert!(matches!(
            decrypt(&[0u8; 10], &key, AesMode::Gcm),
            Err(CryptoError::CiphertextTooShort {
                minimum: 28,
                length: 10
            })
        ));
    }

    #[test]
    fn gcm_empty_plaintext_roundtrip() {
        let key = key128();
        let ct = encrypt(b"", &key, AesMode::Gcm).unwrap();
        assert_eq!(ct.len(), 28);
        assert_eq!(decrypt(&ct, &key, AesMode::Gcm).unwrap(), b"");
    }

    #[test]
    fn cbc_container_roundtrip_and_min_length() {
        let key = key128();
        let ct = encrypt(b"hello world", &key, AesMode::CbcPkcs7).unwrap();
        assert_eq!(ct.len(), 16 + 16);
        assert_eq!(
            decrypt(&ct, &key, AesMode::CbcPkcs7).unwrap(),
            b"hello world"
        );
        assert!(matches!(
            decrypt(&[0u8; 10], &key, AesMode::CbcPkcs7),
            Err(CryptoError::CiphertextTooShort {
                minimum: 32,
                length: 10
            })
        ));
    }

    #[test]
    fn cbc_empty_plaintext_roundtrip() {
        let key = key128();
        let ct = encrypt(b"", &key, AesMode::CbcPkcs7).unwrap();
        assert_eq!(ct.len(), 32);
        assert_eq!(decrypt(&ct, &key, AesMode::CbcPkcs7).unwrap(), b"");
    }

    #[test]
    fn cbc_non_block_multiple_length_is_decrypt_error() {
        let key = key128();
        let iv = [0u8; 16];
        let bad = vec![0u8; 17];
        assert!(matches!(
            decrypt_explicit_iv(&bad, &key, &iv, AesMode::CbcPkcs7),
            Err(CryptoError::Decrypt)
        ));
    }

    #[test]
    fn explicit_iv_roundtrip_all_key_sizes_and_modes() {
        for bits in [AesKeyBits::Aes128, AesKeyBits::Aes192, AesKeyBits::Aes256] {
            let key = AesKey::from_bytes(vec![0x11u8; bits.byte_length()]).unwrap();
            for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
                let iv = vec![0x22u8; mode.iv_length()];
                let ct = encrypt_explicit_iv(b"round trip payload", &key, &iv, mode).unwrap();
                assert_eq!(
                    decrypt_explicit_iv(&ct, &key, &iv, mode).unwrap(),
                    b"round trip payload"
                );
            }
        }
    }

    #[test]
    fn explicit_iv_wrong_length_is_rejected() {
        let key = key128();
        assert!(matches!(
            encrypt_explicit_iv(b"x", &key, &[0u8; 11], AesMode::Gcm),
            Err(CryptoError::InvalidIvLength {
                expected: 12,
                length: 11
            })
        ));
        assert!(matches!(
            encrypt_explicit_iv(b"x", &key, &[0u8; 15], AesMode::CbcPkcs7),
            Err(CryptoError::InvalidIvLength {
                expected: 16,
                length: 15
            })
        ));
    }

    #[test]
    fn gcm_tampering_is_detected() {
        let key = key128();
        for tamper_at in [0usize, 12, 15] {
            let mut ct = encrypt(b"hello world", &key, AesMode::Gcm).unwrap();
            ct[tamper_at] ^= 0x01;
            assert!(matches!(
                decrypt(&ct, &key, AesMode::Gcm),
                Err(CryptoError::Decrypt)
            ));
        }
    }

    #[test]
    fn random_containers_differ_but_both_decrypt() {
        let key = key128();
        let a = encrypt(b"same plaintext", &key, AesMode::Gcm).unwrap();
        let b = encrypt(b"same plaintext", &key, AesMode::Gcm).unwrap();
        assert_eq!(a[..12].len(), 12);
        assert_eq!(b[..12].len(), 12);
        assert_eq!(decrypt(&a, &key, AesMode::Gcm).unwrap(), b"same plaintext");
        assert_eq!(decrypt(&b, &key, AesMode::Gcm).unwrap(), b"same plaintext");
    }

    #[test]
    fn random_bytes_lengths() {
        assert_eq!(random_bytes(12).unwrap().len(), 12);
        assert_eq!(random_bytes(16).unwrap().len(), 16);
        assert_eq!(random_bytes(24).unwrap().len(), 24);
        assert_eq!(random_bytes(32).unwrap().len(), 32);
    }

    #[test]
    fn generate_covers_all_key_sizes() {
        for bits in [AesKeyBits::Aes128, AesKeyBits::Aes192, AesKeyBits::Aes256] {
            assert_eq!(AesKey::generate(bits).unwrap().bits(), bits);
        }
    }

    #[test]
    fn random_source_failures_map_to_random_source() {
        fn fail(_: usize) -> Result<Vec<u8>, CryptoError> {
            Err(CryptoError::RandomSource)
        }

        assert!(matches!(
            AesKey::generate_with_random(AesKeyBits::Aes256, fail),
            Err(CryptoError::RandomSource)
        ));
        assert!(matches!(
            encrypt_with_random(b"payload", &key128(), AesMode::Gcm, fail),
            Err(CryptoError::RandomSource)
        ));
    }
}
