//! `CryptoUtils` 静态工具入口；具体实现见 [`crate::crypto`]。

#[cfg(feature = "aes")]
use crate::AesKey;
#[cfg(feature = "aes")]
use crate::AesMode;
#[cfg(feature = "base64")]
use crate::Base64Options;
use crate::CryptoError;
#[cfg(any(feature = "base64", feature = "md5"))]
use crate::TextEncoding;
#[cfg(feature = "aes")]
use ::zeroize::Zeroize;

/// 内存数据编码、摘要和加解密的静态工具入口；不保存状态。
#[derive(Debug, Clone, Copy, Default)]
pub struct CryptoUtils;

impl CryptoUtils {
    /// 把字节编码为小写十六进制字符串。
    ///
    /// # Errors
    ///
    /// 输出长度计算溢出或无法预留结果空间时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_encode([0x00, 0xff]).unwrap(), "00ff");
    /// ```
    pub fn hex_encode(input: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        crate::crypto::hex_encode_lower(input.as_ref())
    }

    /// 把字节编码为大写十六进制字符串。
    ///
    /// # Errors
    ///
    /// 与 [`hex_encode`](CryptoUtils::hex_encode) 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_encode_upper([0x00, 0xff]).unwrap(), "00FF");
    /// ```
    pub fn hex_encode_upper(input: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        crate::crypto::hex_encode_upper(input.as_ref())
    }

    /// 把十六进制字符串解码为字节；同时接受大小写，拒绝空白、`0x` 前缀和奇数长度。
    ///
    /// # Errors
    ///
    /// 长度为奇数时返回 [`CryptoError::OddHexLength`]；含非法字符时返回
    /// [`CryptoError::InvalidHex`]；容量计算失败时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_decode("00Ff").unwrap(), vec![0x00, 0xff]);
    /// assert!(CryptoUtils::hex_decode("0x0f").is_err());
    /// ```
    pub fn hex_decode(input: &str) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::hex_decode(input)
    }

    /// 把字节按 `options` 指定的字母表与填充设置编码为 Base64 字符串。
    ///
    /// # Errors
    ///
    /// 输出长度计算溢出或无法预留结果空间时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Options, CryptoUtils};
    ///
    /// assert_eq!(
    ///     CryptoUtils::base64_encode("foobar", Base64Options::STANDARD).unwrap(),
    ///     "Zm9vYmFy"
    /// );
    /// ```
    #[cfg(feature = "base64")]
    pub fn base64_encode(
        input: impl AsRef<[u8]>,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        crate::crypto::base64_encode(input.as_ref(), options)
    }

    /// 先按 `encoding` 把 `text` 编码为字节，再按 `options` 编码为 Base64 字符串。
    ///
    /// # Errors
    ///
    /// 文本编码失败时返回对应的 [`CryptoError`]；Base64 编码失败时返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Options, CryptoUtils, TextEncoding};
    ///
    /// let encoded =
    ///     CryptoUtils::base64_encode_text("foobar", TextEncoding::Utf8, Base64Options::STANDARD)
    ///         .unwrap();
    /// assert_eq!(encoded, "Zm9vYmFy");
    /// ```
    #[cfg(feature = "base64")]
    pub fn base64_encode_text(
        text: &str,
        encoding: TextEncoding,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        let bytes = encoding.encode(text)?;
        crate::crypto::base64_encode(&bytes, options)
    }

    /// 按 `options` 指定的字母表与填充设置严格解码 Base64 字符串为字节。
    ///
    /// 解码严格：填充、字母表和尾随比特必须与 `options` 完全一致，否则返回错误；对方系统产生
    /// 无填充 Base64 时应改用 `Base64Options::*_NO_PAD`。
    ///
    /// # Errors
    ///
    /// 输入不合法或与 `options` 不符时返回 [`CryptoError::Base64Decode`]；容量计算失败时返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Options, CryptoUtils};
    ///
    /// assert_eq!(
    ///     CryptoUtils::base64_decode("Zm9vYmFy", Base64Options::STANDARD).unwrap(),
    ///     b"foobar"
    /// );
    /// // "Zm9vYg==" carries `=` padding, so it is rejected under the no-padding setting.
    /// assert!(CryptoUtils::base64_decode("Zm9vYg==", Base64Options::STANDARD_NO_PAD).is_err());
    /// ```
    #[cfg(feature = "base64")]
    pub fn base64_decode(input: &str, options: Base64Options) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::base64_decode(input, options)
    }

    /// 解码 Base64 字符串为字节，再按 `encoding` 解码为文本。
    ///
    /// # Errors
    ///
    /// Base64 解码失败时返回 [`CryptoError::Base64Decode`]；文本解码失败时返回
    /// [`CryptoError::TextDecodeInvalid`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Options, CryptoUtils, TextEncoding};
    ///
    /// let text = CryptoUtils::base64_decode_text("Zm9vYmFy", TextEncoding::Utf8, Base64Options::STANDARD)
    ///     .unwrap();
    /// assert_eq!(text, "foobar");
    /// ```
    #[cfg(feature = "base64")]
    pub fn base64_decode_text(
        input: &str,
        encoding: TextEncoding,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        let bytes = crate::crypto::base64_decode(input, options)?;
        encoding.decode(bytes)
    }

    /// 计算 MD5 摘要，返回 16 字节原始摘要。
    ///
    /// MD5 是摘要算法，不是加密，不可逆；已存在实用碰撞攻击，**禁止**用于密码存储、数字签名、
    /// 证书、防篡改校验、内容寻址或任何对抗性场景；仅适用于与既有系统对接、且输入不受攻击者
    /// 控制的非对抗性一致性校验（如内部缓存键、去重）。需要抗碰撞性的输入应改用现代摘要算法
    /// （如 SHA-2），本 crate 首期不提供。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::CryptoUtils;
    ///
    /// let digest = CryptoUtils::md5("abc");
    /// assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5(input: impl AsRef<[u8]>) -> [u8; 16] {
        crate::crypto::md5_digest(input.as_ref())
    }

    /// 计算 MD5 摘要，返回 32 字符小写十六进制字符串；大写结果可用
    /// `CryptoUtils::hex_encode_upper(CryptoUtils::md5(x))?` 组合得到。
    ///
    /// 安全提示与 [`md5`](CryptoUtils::md5) 相同：不可用于对抗性场景。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_hex(input: impl AsRef<[u8]>) -> String {
        let digest = crate::crypto::md5_digest(input.as_ref());
        crate::crypto::encode_lower_fixed(&digest)
    }

    /// 先按 `encoding` 把 `text` 编码为字节，再计算 MD5 摘要。
    ///
    /// # Errors
    ///
    /// 文本编码失败时返回对应的 [`CryptoError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{CryptoUtils, TextEncoding};
    ///
    /// let digest = CryptoUtils::md5_text("abc", TextEncoding::Utf8).unwrap();
    /// assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_text(text: &str, encoding: TextEncoding) -> Result<[u8; 16], CryptoError> {
        let bytes = encoding.encode(text)?;
        Ok(crate::crypto::md5_digest(&bytes))
    }

    /// 先按 `encoding` 把 `text` 编码为字节，再计算 32 字符小写十六进制 MD5 摘要。
    ///
    /// # Errors
    ///
    /// 文本编码失败时返回对应的 [`CryptoError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{CryptoUtils, TextEncoding};
    ///
    /// let hex = CryptoUtils::md5_hex_text("abc", TextEncoding::Utf8).unwrap();
    /// assert_eq!(hex, "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_hex_text(text: &str, encoding: TextEncoding) -> Result<String, CryptoError> {
        let bytes = encoding.encode(text)?;
        let digest = crate::crypto::md5_digest(&bytes);
        Ok(crate::crypto::encode_lower_fixed(&digest))
    }

    /// 使用随机 IV/nonce 加密，输出布局为 `iv || 密文(|| tag)`（`Gcm` 附带 16 字节 tag）。
    ///
    /// 同一密钥下 GCM 随机 96-bit nonce 的安全消息数上限约为 2^32；长期高频场景应自行轮换密钥。
    ///
    /// # Errors
    ///
    /// 操作系统随机源不可用时返回 [`CryptoError::RandomSource`]；加密失败时返回
    /// [`CryptoError::Encrypt`]；容量计算失败时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 32]).unwrap();
    /// let ciphertext = CryptoUtils::aes_encrypt("hello world", &key, AesMode::Gcm).unwrap();
    /// let plaintext = CryptoUtils::aes_decrypt(&ciphertext, &key, AesMode::Gcm).unwrap();
    /// assert_eq!(plaintext, b"hello world");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt(
        plaintext: impl AsRef<[u8]>,
        key: &AesKey,
        mode: AesMode,
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_encrypt_container(plaintext.as_ref(), key, mode)
    }

    /// 解密 [`aes_encrypt`](CryptoUtils::aes_encrypt) 的完整输出（含前置 IV/nonce）。
    ///
    /// 直接传入 `&str` 时按其 UTF-8 字节处理，不会自动把十六进制/Base64 文本解码；文本密文应
    /// 使用 [`aes_decrypt_hex`](CryptoUtils::aes_decrypt_hex) 或（启用 `base64` 后）
    /// [`aes_decrypt_base64`](CryptoUtils::aes_decrypt_base64)。解密到字符串时对结果调用
    /// [`TextEncoding::decode`](crate::TextEncoding::decode)。
    ///
    /// # Errors
    ///
    /// 输入短于最小长度时返回 [`CryptoError::CiphertextTooShort`]；认证失败、篡改或填充非法时
    /// 统一返回 [`CryptoError::Decrypt`]（不区分具体原因，避免 padding oracle）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let ciphertext = CryptoUtils::aes_encrypt("secret", &key, AesMode::CbcPkcs7).unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt(&ciphertext, &key, AesMode::CbcPkcs7).unwrap(), b"secret");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt(
        input: impl AsRef<[u8]>,
        key: &AesKey,
        mode: AesMode,
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_decrypt_container(input.as_ref(), key, mode)
    }

    /// 互操作路径：IV/nonce 由调用方提供，输出**不含** IV。
    ///
    /// **警告**：GCM 下重用 nonce 会破坏机密性与完整性，可能导致多条消息的明文恢复或伪造；
    /// 调用方必须自行保证每次调用的 `iv` 唯一。
    ///
    /// # Errors
    ///
    /// `iv` 长度与 `mode` 不符时返回 [`CryptoError::InvalidIvLength`]；加密失败时返回
    /// [`CryptoError::Encrypt`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let iv = [0x00; 12];
    /// let ciphertext =
    ///     CryptoUtils::aes_encrypt_with_iv("hello", &key, &iv, AesMode::Gcm).unwrap();
    /// assert_eq!(ciphertext.len(), 5 + 16);
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt_with_iv(
        plaintext: impl AsRef<[u8]>,
        key: &AesKey,
        iv: &[u8],
        mode: AesMode,
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_encrypt_explicit_iv(plaintext.as_ref(), key, iv, mode)
    }

    /// 互操作路径：解密调用方提供 IV/nonce 加密的密文（输入**不含** IV）。
    ///
    /// # Errors
    ///
    /// `iv` 长度与 `mode` 不符时返回 [`CryptoError::InvalidIvLength`]；输入短于最小长度时返回
    /// [`CryptoError::CiphertextTooShort`]；认证失败、篡改或填充非法时统一返回
    /// [`CryptoError::Decrypt`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let iv = [0x00; 12];
    /// let ciphertext =
    ///     CryptoUtils::aes_encrypt_with_iv("hello", &key, &iv, AesMode::Gcm).unwrap();
    /// let plaintext =
    ///     CryptoUtils::aes_decrypt_with_iv(&ciphertext, &key, &iv, AesMode::Gcm).unwrap();
    /// assert_eq!(plaintext, b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt_with_iv(
        ciphertext: impl AsRef<[u8]>,
        key: &AesKey,
        iv: &[u8],
        mode: AesMode,
    ) -> Result<Vec<u8>, CryptoError> {
        crate::crypto::aes_decrypt_explicit_iv(ciphertext.as_ref(), key, iv, mode)
    }

    /// 等价于 `hex_encode(aes_encrypt(..)?)`；使用随机 IV/nonce。
    ///
    /// # Errors
    ///
    /// 与 [`aes_encrypt`](CryptoUtils::aes_encrypt) 相同；十六进制编码的容量错误继续返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let hex = CryptoUtils::aes_encrypt_hex("hello", &key, AesMode::Gcm).unwrap();
    /// let plaintext = CryptoUtils::aes_decrypt_hex(&hex, &key, AesMode::Gcm).unwrap();
    /// assert_eq!(plaintext, b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt_hex(
        plaintext: impl AsRef<[u8]>,
        key: &AesKey,
        mode: AesMode,
    ) -> Result<String, CryptoError> {
        let mut ciphertext = crate::crypto::aes_encrypt_container(plaintext.as_ref(), key, mode)?;
        let result = crate::crypto::hex_encode_lower(&ciphertext);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 等价于 `aes_decrypt(hex_decode(..)?, ..)`。
    ///
    /// # Errors
    ///
    /// 十六进制解码失败时返回 [`CryptoError::OddHexLength`]/[`CryptoError::InvalidHex`]；其余
    /// 与 [`aes_decrypt`](CryptoUtils::aes_decrypt) 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let hex = CryptoUtils::aes_encrypt_hex("hello", &key, AesMode::CbcPkcs7).unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt_hex(&hex, &key, AesMode::CbcPkcs7).unwrap(), b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt_hex(
        input: &str,
        key: &AesKey,
        mode: AesMode,
    ) -> Result<Vec<u8>, CryptoError> {
        let mut ciphertext = crate::crypto::hex_decode(input)?;
        let result = crate::crypto::aes_decrypt_container(&ciphertext, key, mode);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 等价于 `base64_encode(aes_encrypt(..)?, options)`；使用随机 IV/nonce。仅在同时启用
    /// `aes` 与 `base64` feature 时提供。
    ///
    /// # Errors
    ///
    /// 与 [`aes_encrypt`](CryptoUtils::aes_encrypt) 相同；Base64 编码的容量错误继续返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, Base64Options, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let text =
    ///     CryptoUtils::aes_encrypt_base64("hello", &key, AesMode::Gcm, Base64Options::STANDARD)
    ///         .unwrap();
    /// let plaintext =
    ///     CryptoUtils::aes_decrypt_base64(&text, &key, AesMode::Gcm, Base64Options::STANDARD)
    ///         .unwrap();
    /// assert_eq!(plaintext, b"hello");
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn aes_encrypt_base64(
        plaintext: impl AsRef<[u8]>,
        key: &AesKey,
        mode: AesMode,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        let mut ciphertext = crate::crypto::aes_encrypt_container(plaintext.as_ref(), key, mode)?;
        let result = crate::crypto::base64_encode(&ciphertext, options);
        ciphertext.as_mut_slice().zeroize();
        result
    }

    /// 等价于 `aes_decrypt(base64_decode(.., options)?, ..)`。仅在同时启用 `aes` 与 `base64`
    /// feature 时提供。
    ///
    /// # Errors
    ///
    /// Base64 解码失败时返回 [`CryptoError::Base64Decode`]；其余与
    /// [`aes_decrypt`](CryptoUtils::aes_decrypt) 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, Base64Options, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 16]).unwrap();
    /// let text = CryptoUtils::aes_encrypt_base64(
    ///     "hello",
    ///     &key,
    ///     AesMode::CbcPkcs7,
    ///     Base64Options::STANDARD,
    /// )
    /// .unwrap();
    /// let plaintext = CryptoUtils::aes_decrypt_base64(
    ///     &text,
    ///     &key,
    ///     AesMode::CbcPkcs7,
    ///     Base64Options::STANDARD,
    /// )
    /// .unwrap();
    /// assert_eq!(plaintext, b"hello");
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn aes_decrypt_base64(
        input: &str,
        key: &AesKey,
        mode: AesMode,
        options: Base64Options,
    ) -> Result<Vec<u8>, CryptoError> {
        let mut ciphertext = crate::crypto::base64_decode(input, options)?;
        let result = crate::crypto::aes_decrypt_container(&ciphertext, key, mode);
        ciphertext.as_mut_slice().zeroize();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::CryptoUtils;

    #[test]
    fn hex_roundtrip() {
        let encoded = CryptoUtils::hex_encode([0x00, 0xff]).unwrap();
        assert_eq!(encoded, "00ff");
        assert_eq!(CryptoUtils::hex_decode(&encoded).unwrap(), vec![0x00, 0xff]);
    }

    #[test]
    fn hex_encode_upper_case() {
        assert_eq!(CryptoUtils::hex_encode_upper([0x00, 0xff]).unwrap(), "00FF");
    }

    #[cfg(feature = "base64")]
    #[test]
    fn base64_facade_roundtrip() {
        use crate::Base64Options;
        let encoded = CryptoUtils::base64_encode("foobar", Base64Options::STANDARD).unwrap();
        assert_eq!(encoded, "Zm9vYmFy");
        assert_eq!(
            CryptoUtils::base64_decode(&encoded, Base64Options::STANDARD).unwrap(),
            b"foobar"
        );
    }

    #[cfg(feature = "md5")]
    #[test]
    fn md5_facade_matches_hex() {
        let digest = CryptoUtils::md5("abc");
        assert_eq!(
            CryptoUtils::hex_encode(digest).unwrap(),
            CryptoUtils::md5_hex("abc")
        );
    }

    #[cfg(feature = "aes")]
    #[test]
    fn aes_facade_roundtrip_both_modes() {
        use crate::{AesKey, AesMode};
        let key = AesKey::from_bytes([0x01; 16]).unwrap();
        for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
            let ct = CryptoUtils::aes_encrypt("payload", &key, mode).unwrap();
            assert_eq!(
                CryptoUtils::aes_decrypt(&ct, &key, mode).unwrap(),
                b"payload"
            );
            let hex = CryptoUtils::aes_encrypt_hex("payload", &key, mode).unwrap();
            assert_eq!(
                CryptoUtils::aes_decrypt_hex(&hex, &key, mode).unwrap(),
                b"payload"
            );
        }
    }

    #[cfg(all(feature = "aes", feature = "base64"))]
    #[test]
    fn aes_base64_facade_roundtrip() {
        use crate::{AesKey, AesMode, Base64Options};
        let key = AesKey::from_bytes([0x02; 32]).unwrap();
        let text = CryptoUtils::aes_encrypt_base64(
            "payload",
            &key,
            AesMode::Gcm,
            Base64Options::URL_SAFE_NO_PAD,
        )
        .unwrap();
        let plaintext = CryptoUtils::aes_decrypt_base64(
            &text,
            &key,
            AesMode::Gcm,
            Base64Options::URL_SAFE_NO_PAD,
        )
        .unwrap();
        assert_eq!(plaintext, b"payload");
    }
}
