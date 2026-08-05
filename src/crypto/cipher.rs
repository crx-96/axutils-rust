use std::fmt;

use super::{AesKey, AesKeyBits, AesMode, CryptoError};
#[cfg(feature = "base64")]
use crate::Base64Options;
use ::zeroize::Zeroize;

/// 可独立构建和销毁的 AES 加解密实例。
///
/// `AesCipher` 持有一个不可导出的 [`AesKey`] 和初始化时固定的 [`AesMode`]，适合多密钥、
/// 多模式或需要可控密钥生命周期的场景。实例之间互不覆盖；实例被丢弃时，内部密钥由
/// [`AesKey`] 的 `Drop` 实现清零。
///
/// 这与 [`crate::CryptoUtils`] 的进程级 AES 单例不同：全局单例会与进程同寿命，正常进程退出前
/// 不会触发 `Drop`。`AesCipher` 不提供读取、复制或轮换密钥的方法，也不实现 `Clone`、`Copy`、
/// `Display` 或序列化 trait。
///
/// # Examples
///
/// ```
/// use axutils::{AesCipher, AesMode};
///
/// let cipher = AesCipher::from_key_bytes([0x00; 32], AesMode::Gcm).unwrap();
/// assert_eq!(cipher.key_bits().bit_length(), 256);
/// assert_eq!(cipher.mode(), AesMode::Gcm);
/// ```
pub struct AesCipher {
    key: AesKey,
    mode: AesMode,
}

impl AesCipher {
    /// 使用已经构造好的 [`AesKey`] 创建实例，并固定其加解密模式。
    ///
    /// `key` 会被消费；调用方不能再通过该值读取或复制密钥材料。该方法本身不会失败，
    /// 因为密钥长度已由 [`AesKey`] 保证。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesKey, AesMode};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let cipher = AesCipher::new(key, AesMode::CbcPkcs7);
    /// assert_eq!(cipher.mode(), AesMode::CbcPkcs7);
    /// ```
    pub fn new(key: AesKey, mode: AesMode) -> Self {
        Self { key, mode }
    }

    /// 从 16、24 或 32 字节密钥材料创建实例，并固定其加解密模式。
    ///
    /// `key` 的内容会被复制到内部 [`AesKey`]；本 crate 不能清零调用方仍持有的
    /// `Vec<u8>` 或数组副本，调用方如不再需要该副本应自行清零。
    ///
    /// # Errors
    ///
    /// `key` 长度不是 16、24 或 32 字节时返回 [`CryptoError::InvalidKeyLength`]，且不会创建
    /// 实例。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode, CryptoError};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 24], AesMode::Gcm).unwrap();
    /// assert_eq!(cipher.key_bits().bit_length(), 192);
    /// assert!(matches!(
    ///     AesCipher::from_key_bytes([0x00; 15], AesMode::Gcm),
    ///     Err(CryptoError::InvalidKeyLength { length: 15 })
    /// ));
    /// ```
    pub fn from_key_bytes(key: impl AsRef<[u8]>, mode: AesMode) -> Result<Self, CryptoError> {
        Ok(Self::new(AesKey::from_bytes(key)?, mode))
    }

    /// 返回初始化时固定的 AES 模式。
    ///
    /// 该方法只返回模式，不会暴露密钥材料。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// assert_eq!(cipher.mode(), AesMode::Gcm);
    /// ```
    #[must_use]
    pub fn mode(&self) -> AesMode {
        self.mode
    }

    /// 返回内部密钥的位数，不返回密钥字节。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesKeyBits, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 32], AesMode::Gcm).unwrap();
    /// assert_eq!(cipher.key_bits(), AesKeyBits::Aes256);
    /// ```
    #[must_use]
    pub fn key_bits(&self) -> AesKeyBits {
        self.key.bits()
    }

    /// 使用随机 IV/nonce 加密，返回包含前置 IV/nonce 的容器。
    ///
    /// `Gcm` 的布局为 `nonce || ciphertext || tag`，`CbcPkcs7` 的布局为
    /// `iv || ciphertext`。GCM 随机 nonce 在同一密钥下的安全消息数约为 2^32；CBC 不提供完整性
    /// 认证，只应用于旧系统互操作。
    ///
    /// # Errors
    ///
    /// 随机源不可用时返回 [`CryptoError::RandomSource`]；加密失败时返回
    /// [`CryptoError::Encrypt`]；可检查的容量计算或预留失败时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let ciphertext = cipher.encrypt("hello").unwrap();
    /// assert_eq!(cipher.decrypt(&ciphertext).unwrap(), b"hello");
    /// ```
    pub fn encrypt(&self, plaintext: impl AsRef<[u8]>) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_encrypt_container(plaintext.as_ref(), &self.key, self.mode)
    }

    /// 解密 [`Self::encrypt`] 返回的包含前置 IV/nonce 的容器。
    ///
    /// # Errors
    ///
    /// 输入短于当前模式的绝对最小长度时返回 [`CryptoError::CiphertextTooShort`]；认证失败、
    /// 填充非法、密文被篡改或 CBC 密文不是整块时统一返回 [`CryptoError::Decrypt`]，不区分具体
    /// 原因，以避免暴露 padding oracle 信号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode, CryptoError};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// assert!(matches!(
    ///     cipher.decrypt([0u8; 10]),
    ///     Err(CryptoError::CiphertextTooShort { minimum: 28, length: 10 })
    /// ));
    /// ```
    pub fn decrypt(&self, input: impl AsRef<[u8]>) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_decrypt_container(input.as_ref(), &self.key, self.mode)
    }

    /// 使用调用方提供的 IV/nonce 加密，返回不包含 IV/nonce 的密文。
    ///
    /// GCM 下调用方必须保证 `iv`/nonce 在同一密钥下唯一；CBC 仍不提供完整性认证。
    ///
    /// # Errors
    ///
    /// `iv` 长度不符合当前模式时返回 [`CryptoError::InvalidIvLength`]；加密失败时返回
    /// [`CryptoError::Encrypt`]；可检查的容量计算或预留失败时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let nonce = [0x00; 12];
    /// let ciphertext = cipher.encrypt_with_iv("hello", &nonce).unwrap();
    /// assert_eq!(cipher.decrypt_with_iv(&ciphertext, &nonce).unwrap(), b"hello");
    /// ```
    pub fn encrypt_with_iv(
        &self,
        plaintext: impl AsRef<[u8]>,
        iv: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_encrypt_explicit_iv(plaintext.as_ref(), &self.key, iv, self.mode)
    }

    /// 使用调用方提供的 IV/nonce 解密不包含 IV/nonce 的密文。
    ///
    /// # Errors
    ///
    /// `iv` 长度不符合当前模式时返回 [`CryptoError::InvalidIvLength`]；密文短于模式允许的
    /// 绝对最小长度时返回 [`CryptoError::CiphertextTooShort`]；认证失败、填充非法或密文被篡改时
    /// 返回 [`CryptoError::Decrypt`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let nonce = [0x00; 12];
    /// let ciphertext = cipher.encrypt_with_iv("hello", &nonce).unwrap();
    /// assert_eq!(cipher.decrypt_with_iv(&ciphertext, &nonce).unwrap(), b"hello");
    /// ```
    pub fn decrypt_with_iv(
        &self,
        ciphertext: impl AsRef<[u8]>,
        iv: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_decrypt_explicit_iv(ciphertext.as_ref(), &self.key, iv, self.mode)
    }

    /// 使用随机 IV/nonce 加密并将完整容器编码为小写十六进制。
    ///
    /// # Errors
    ///
    /// 除十六进制编码可能返回 [`CryptoError::OutputTooLarge`] 外，错误语义与
    /// [`Self::encrypt`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = cipher.encrypt_hex("hello").unwrap();
    /// assert_eq!(cipher.decrypt_hex(&encoded).unwrap(), b"hello");
    /// ```
    pub fn encrypt_hex(&self, plaintext: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        let mut ciphertext = self.encrypt(plaintext)?;
        let result = crate::crypto::hex_encode_lower(&ciphertext);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 解码小写或大写十六进制容器并解密。
    ///
    /// # Errors
    ///
    /// 奇数长度或非法字符分别返回 [`CryptoError::OddHexLength`] 或
    /// [`CryptoError::InvalidHex`]；其余错误语义与 [`Self::decrypt`] 相同。解码得到的中间密文
    /// 在返回前会清零。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode, CryptoError};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::CbcPkcs7).unwrap();
    /// let encoded = cipher.encrypt_hex("hello").unwrap();
    /// assert_eq!(cipher.decrypt_hex(&encoded).unwrap(), b"hello");
    /// assert!(matches!(cipher.decrypt_hex("abc"), Err(CryptoError::OddHexLength { .. })));
    /// ```
    pub fn decrypt_hex(&self, input: &str) -> Result<Vec<u8>, CryptoError> {
        let mut ciphertext = crate::crypto::hex_decode(input)?;
        let result = self.decrypt(&ciphertext);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 使用随机 IV/nonce 加密并按 `options` 编码完整容器为 Base64。
    ///
    /// 该方法仅在同时启用 `aes` 与 `base64` feature 时提供；`options` 只控制 Base64 字母表和
    /// 填充形式，不属于密钥或模式配置。
    ///
    /// # Errors
    ///
    /// 错误语义与 [`Self::encrypt`] 相同；Base64 输出容量计算或预留失败时返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode, Base64Options};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = cipher.encrypt_base64("hello", Base64Options::URL_SAFE_NO_PAD).unwrap();
    /// assert_eq!(
    ///     cipher.decrypt_base64(&encoded, Base64Options::URL_SAFE_NO_PAD).unwrap(),
    ///     b"hello"
    /// );
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn encrypt_base64(
        &self,
        plaintext: impl AsRef<[u8]>,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        let mut ciphertext = self.encrypt(plaintext)?;
        let result = crate::crypto::base64_encode(&ciphertext, options);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 解码 Base64 容器并解密。
    ///
    /// 该方法仅在同时启用 `aes` 与 `base64` feature 时提供；`options` 必须与输入的字母表和
    /// 填充形式一致。
    ///
    /// # Errors
    ///
    /// Base64 输入非法时返回 [`CryptoError::Base64Decode`]；其余错误语义与 [`Self::decrypt`]
    /// 相同。解码得到的中间密文在返回前会清零。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesCipher, AesMode, Base64Options};
    ///
    /// let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = cipher.encrypt_base64("hello", Base64Options::STANDARD).unwrap();
    /// assert_eq!(
    ///     cipher.decrypt_base64(&encoded, Base64Options::STANDARD).unwrap(),
    ///     b"hello"
    /// );
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn decrypt_base64(
        &self,
        input: &str,
        options: Base64Options,
    ) -> Result<Vec<u8>, CryptoError> {
        let mut ciphertext = crate::crypto::base64_decode(input, options)?;
        let result = self.decrypt(&ciphertext);
        ciphertext.as_mut_slice().zeroize();
        result
    }
}

impl fmt::Debug for AesCipher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AesCipher")
            .field("key_bits", &self.key.bits())
            .field("mode", &self.mode)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn cipher_is_send_sync_and_debug_is_redacted() {
        assert_send_sync::<AesCipher>();
        let cipher = AesCipher::from_key_bytes([0x99; 16], AesMode::Gcm).unwrap();
        let debug = format!("{cipher:?}");
        assert!(debug.contains("Aes128"));
        assert!(debug.contains("Gcm"));
        assert!(!debug.contains("153"));
    }

    #[test]
    fn key_sizes_and_modes_roundtrip() {
        for (key_len, expected_bits) in [
            (16, AesKeyBits::Aes128),
            (24, AesKeyBits::Aes192),
            (32, AesKeyBits::Aes256),
        ] {
            for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
                let cipher = AesCipher::from_key_bytes(vec![0x11; key_len], mode).unwrap();
                assert_eq!(cipher.key_bits(), expected_bits);
                assert_eq!(cipher.mode(), mode);
                let ciphertext = cipher.encrypt("payload").unwrap();
                assert_eq!(cipher.decrypt(&ciphertext).unwrap(), b"payload");
                let encoded = cipher.encrypt_hex("payload").unwrap();
                assert_eq!(cipher.decrypt_hex(&encoded).unwrap(), b"payload");
            }
        }
    }

    #[test]
    fn from_key_bytes_rejects_invalid_lengths() {
        for length in [0usize, 15, 17, 33] {
            let err = AesCipher::from_key_bytes(vec![0x00; length], AesMode::Gcm).unwrap_err();
            assert_eq!(err, CryptoError::InvalidKeyLength { length });
        }
    }

    #[test]
    fn explicit_iv_errors_and_gcm_tampering_are_handled() {
        let cipher = AesCipher::from_key_bytes([0x22; 16], AesMode::Gcm).unwrap();
        assert!(matches!(
            cipher.encrypt_with_iv("x", &[0u8; 11]),
            Err(CryptoError::InvalidIvLength {
                expected: 12,
                length: 11
            })
        ));
        assert!(matches!(
            cipher.decrypt([0u8; 10]),
            Err(CryptoError::CiphertextTooShort {
                minimum: 28,
                length: 10
            })
        ));

        let iv = [0x33; 12];
        let mut ciphertext = cipher.encrypt_with_iv("authenticated", &iv).unwrap();
        ciphertext[0] ^= 1;
        assert!(matches!(
            cipher.decrypt_with_iv(&ciphertext, &iv),
            Err(CryptoError::Decrypt)
        ));
    }

    #[cfg(feature = "base64")]
    #[test]
    fn base64_options_roundtrip() {
        let cipher = AesCipher::from_key_bytes([0x44; 32], AesMode::CbcPkcs7).unwrap();
        for options in [
            Base64Options::STANDARD,
            Base64Options::STANDARD_NO_PAD,
            Base64Options::URL_SAFE,
            Base64Options::URL_SAFE_NO_PAD,
        ] {
            let encoded = cipher.encrypt_base64("payload", options).unwrap();
            assert_eq!(
                cipher.decrypt_base64(&encoded, options).unwrap(),
                b"payload"
            );
        }
    }
}
