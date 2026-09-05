//! AES 密钥类型和密钥长度。

use std::fmt;

use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::random;

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
    /// use axutils::crypto::AesKeyBits;
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
    /// use axutils::crypto::AesKeyBits;
    ///
    /// assert_eq!(AesKeyBits::Aes256.byte_length(), 32);
    /// ```
    #[must_use]
    pub fn byte_length(&self) -> usize {
        self.bit_length() / 8
    }

    pub(super) fn from_byte_length(length: usize) -> Result<Self, CryptoError> {
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
    /// use axutils::crypto::AesKey;
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
    /// use axutils::crypto::{AesKey, AesKeyBits};
    ///
    /// let key = AesKey::generate(AesKeyBits::Aes256).unwrap();
    /// assert_eq!(key.bits(), AesKeyBits::Aes256);
    /// ```
    pub fn generate(bits: AesKeyBits) -> Result<Self, CryptoError> {
        Self::generate_with_random(bits, random::random_bytes)
    }

    pub(super) fn generate_with_random<F>(
        bits: AesKeyBits,
        mut source: F,
    ) -> Result<Self, CryptoError>
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
    /// use axutils::crypto::{AesKey, AesKeyBits};
    ///
    /// let key = AesKey::from_bytes([0x00; 24]).unwrap();
    /// assert_eq!(key.bits(), AesKeyBits::Aes192);
    /// ```
    #[must_use]
    pub fn bits(&self) -> AesKeyBits {
        self.bits
    }

    pub(super) fn key_bytes(&self) -> &[u8] {
        &self.bytes[..self.bits.byte_length()]
    }
}

impl fmt::Debug for AesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AesKey").field("bits", &self.bits).finish()
    }
}

impl Drop for AesKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}
