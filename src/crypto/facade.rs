//! `CryptoUtils` 静态工具入口；具体实现见 [`crate::crypto`]。

#[cfg(feature = "md5")]
use super::md5;
#[cfg(any(feature = "base64", feature = "md5"))]
use super::TextEncoding;
#[cfg(feature = "base64")]
use super::{base64 as base64_codec, Base64Options};
use super::{hex, CryptoError};
#[cfg(feature = "aes")]
use super::{AesCipher, AesKey, AesMode};
#[cfg(feature = "aes")]
use std::sync::OnceLock;
#[cfg(all(feature = "aes", feature = "tracing"))]
use std::time::Instant;

#[cfg(all(feature = "aes", feature = "tracing"))]
use crate::telemetry::crypto as crypto_trace;

/// 内存数据编码、摘要和加解密的静态工具入口。
///
/// 十六进制、Base64、MD5 和 `TextEncoding` 路径仍是无状态的；启用 `aes` 后，AES 静态方法
/// 使用文件内的进程级 `OnceLock`。必须先通过 `CryptoUtils::aes_init` 或
/// `CryptoUtils::aes_init_from_bytes`
/// 成功初始化一次密钥与模式，之后该密钥和模式不可修改，也无法通过本类型读取。
///
/// 全局 `AesCipher` 与进程同寿命，正常退出前不会触发内部 `AesKey` 的 `Drop`，因此全局密钥
/// 不会由本 crate 清零。需要多密钥、多模式或可控密钥生命周期时，请使用可独立销毁的
/// `AesCipher` 实例；`*_from_bytes` 也只会清零本 crate 的密钥副本，调用方持有的
/// 原始数组或 `Vec<u8>` 需要由调用方自行清零。
#[derive(Debug, Clone, Copy, Default)]
pub struct CryptoUtils;

#[cfg(feature = "aes")]
static AES_CIPHER: OnceLock<AesCipher> = OnceLock::new();

impl CryptoUtils {
    #[cfg(feature = "aes")]
    fn aes_cipher() -> Result<&'static AesCipher, CryptoError> {
        AES_CIPHER.get().ok_or(CryptoError::NotInitialized)
    }

    /// 把字节编码为小写十六进制字符串。
    ///
    /// # Errors
    ///
    /// 输出长度计算溢出或无法预留结果空间时返回 [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_encode([0x00, 0xff]).unwrap(), "00ff");
    /// ```
    pub fn hex_encode(input: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        hex::encode_lower(input.as_ref())
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
    /// use axutils::utils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_encode_upper([0x00, 0xff]).unwrap(), "00FF");
    /// ```
    pub fn hex_encode_upper(input: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        hex::encode_upper(input.as_ref())
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
    /// use axutils::utils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::hex_decode("00Ff").unwrap(), vec![0x00, 0xff]);
    /// assert!(CryptoUtils::hex_decode("0x0f").is_err());
    /// ```
    pub fn hex_decode(input: &str) -> Result<Vec<u8>, CryptoError> {
        hex::decode(input)
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
    /// use axutils::{crypto::Base64Options, utils::CryptoUtils};
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
        base64_codec::encode(input.as_ref(), options)
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
    /// use axutils::{crypto::{Base64Options, TextEncoding}, utils::CryptoUtils};
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
        base64_codec::encode(&bytes, options)
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
    /// use axutils::{crypto::Base64Options, utils::CryptoUtils};
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
        base64_codec::decode(input, options)
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
    /// use axutils::{crypto::{Base64Options, TextEncoding}, utils::CryptoUtils};
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
        let bytes = base64_codec::decode(input, options)?;
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
    /// use axutils::utils::CryptoUtils;
    ///
    /// let digest = CryptoUtils::md5("abc");
    /// assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5(input: impl AsRef<[u8]>) -> [u8; 16] {
        md5::digest(input.as_ref())
    }

    /// 计算 MD5 摘要，返回 32 字符小写十六进制字符串；大写结果可用
    /// `CryptoUtils::hex_encode_upper(CryptoUtils::md5(x))?` 组合得到。
    ///
    /// 安全提示与 [`md5`](CryptoUtils::md5) 相同：不可用于对抗性场景。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::CryptoUtils;
    ///
    /// assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_hex(input: impl AsRef<[u8]>) -> String {
        let digest = md5::digest(input.as_ref());
        hex::encode_lower_fixed(&digest)
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
    /// use axutils::{crypto::TextEncoding, utils::CryptoUtils};
    ///
    /// let digest = CryptoUtils::md5_text("abc", TextEncoding::Utf8).unwrap();
    /// assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_text(text: &str, encoding: TextEncoding) -> Result<[u8; 16], CryptoError> {
        let bytes = encoding.encode(text)?;
        Ok(md5::digest(&bytes))
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
    /// use axutils::{crypto::TextEncoding, utils::CryptoUtils};
    ///
    /// let hex = CryptoUtils::md5_hex_text("abc", TextEncoding::Utf8).unwrap();
    /// assert_eq!(hex, "900150983cd24fb0d6963f7d28e17f72");
    /// ```
    #[cfg(feature = "md5")]
    pub fn md5_hex_text(text: &str, encoding: TextEncoding) -> Result<String, CryptoError> {
        let bytes = encoding.encode(text)?;
        let digest = md5::digest(&bytes);
        Ok(hex::encode_lower_fixed(&digest))
    }

    /// 初始化进程级 AES 单例，并固定密钥和模式。
    ///
    /// 初始化成功后不能 reset、replace 或读取密钥；全局实例与进程同寿命，正常退出前不会触发
    /// 内部 [`AesKey`] 的 `Drop`，因此密钥不会由本 crate 清零。需要可控生命周期时使用
    /// [`crate::crypto::AesCipher`]。
    ///
    /// # Errors
    ///
    /// 已有线程完成初始化时返回 [`CryptoError::AlreadyInitialized`]；并发竞争中未获胜的调用也
    /// 返回该错误。该方法本身不检查密钥长度，因为 `key` 已是有效的 [`AesKey`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{crypto::{AesKey, AesMode}, utils::CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 32]).unwrap();
    /// CryptoUtils::aes_init(key, AesMode::Gcm).unwrap();
    /// assert_eq!(CryptoUtils::cipher().unwrap().mode(), AesMode::Gcm);
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_init(key: AesKey, mode: AesMode) -> Result<(), CryptoError> {
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = if AES_CIPHER.get().is_some() {
            Err(CryptoError::AlreadyInitialized)
        } else {
            AES_CIPHER
                .set(AesCipher::new(key, mode))
                .map_err(|_| CryptoError::AlreadyInitialized)
        };
        #[cfg(feature = "tracing")]
        crypto_trace::record_init("aes_init", &result, started);
        result
    }

    /// 从 16、24 或 32 字节密钥材料初始化进程级 AES 单例。
    ///
    /// 输入会被复制到内部 [`AesKey`]；本 crate 不能清零调用方仍持有的 `Vec<u8>` 或数组副本，
    /// 调用方如不再需要该副本应自行清零。成功后密钥和模式不可修改，进程退出前全局密钥不会由
    /// 本 crate 清零。
    ///
    /// # Errors
    ///
    /// 已初始化时优先返回 [`CryptoError::AlreadyInitialized`]；未初始化时，密钥长度非法返回
    /// [`CryptoError::InvalidKeyLength`]，且不会占用单例。并发竞争中未获胜的有效初始化返回
    /// [`CryptoError::AlreadyInitialized`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{crypto::AesMode, utils::CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 32], AesMode::Gcm).unwrap();
    /// assert!(CryptoUtils::aes_is_initialized());
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_init_from_bytes(key: impl AsRef<[u8]>, mode: AesMode) -> Result<(), CryptoError> {
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = if AES_CIPHER.get().is_some() {
            Err(CryptoError::AlreadyInitialized)
        } else {
            match AesKey::from_bytes(key) {
                Ok(key) => AES_CIPHER
                    .set(AesCipher::new(key, mode))
                    .map_err(|_| CryptoError::AlreadyInitialized),
                Err(error) => Err(error),
            }
        };
        #[cfg(feature = "tracing")]
        crypto_trace::record_init("aes_init_from_bytes", &result, started);
        result
    }

    /// 返回进程级 AES 单例是否已经成功初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{crypto::AesMode, utils::CryptoUtils};
    ///
    /// assert!(!CryptoUtils::aes_is_initialized());
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// assert!(CryptoUtils::aes_is_initialized());
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_is_initialized() -> bool {
        AES_CIPHER.get().is_some()
    }

    /// 返回已初始化的进程级 AES cipher。
    ///
    /// 全局 cipher 只能成功初始化一次，且密钥会常驻至进程退出。多密钥或需要可控密钥
    /// 生命周期的场景应直接持有 [`AesCipher`] 实例。
    ///
    /// # Errors
    ///
    /// 尚未初始化时返回 [`CryptoError::NotInitialized`]。
    #[cfg(feature = "aes")]
    pub fn cipher() -> Result<&'static AesCipher, CryptoError> {
        Self::aes_cipher()
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
        use crate::crypto::Base64Options;
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
    fn aes_cipher_facade_roundtrip_both_modes() {
        use crate::crypto::{AesCipher, AesMode};
        for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
            let cipher = AesCipher::from_key_bytes([0x01; 16], mode).unwrap();
            let ct = cipher.encrypt("payload").unwrap();
            assert_eq!(cipher.decrypt(&ct).unwrap(), b"payload");
            let hex = cipher.encrypt_hex("payload").unwrap();
            assert_eq!(cipher.decrypt_hex(&hex).unwrap(), b"payload");
        }
    }

    #[cfg(all(feature = "aes", feature = "base64"))]
    #[test]
    fn aes_cipher_base64_roundtrip() {
        use crate::crypto::{AesCipher, AesMode, Base64Options};
        let cipher = AesCipher::from_key_bytes([0x02; 32], AesMode::Gcm).unwrap();
        let text = cipher
            .encrypt_base64("payload", Base64Options::URL_SAFE_NO_PAD)
            .unwrap();
        let plaintext = cipher
            .decrypt_base64(&text, Base64Options::URL_SAFE_NO_PAD)
            .unwrap();
        assert_eq!(plaintext, b"payload");
    }

    #[cfg(feature = "aes")]
    #[test]
    fn aes_global_lifecycle_exposes_cipher_only() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        use crate::crypto::{AesKey, AesMode, CryptoError};

        assert!(!CryptoUtils::aes_is_initialized());
        assert!(matches!(
            CryptoUtils::cipher(),
            Err(CryptoError::NotInitialized)
        ));

        assert!(matches!(
            CryptoUtils::aes_init_from_bytes([0u8; 15], AesMode::Gcm),
            Err(CryptoError::InvalidKeyLength { length: 15 })
        ));
        assert!(!CryptoUtils::aes_is_initialized());

        let barrier = Arc::new(Barrier::new(3));
        let first_barrier = Arc::clone(&barrier);
        let first = thread::spawn(move || {
            first_barrier.wait();
            CryptoUtils::aes_init_from_bytes([0x11; 16], AesMode::Gcm)
        });
        let second_barrier = Arc::clone(&barrier);
        let second = thread::spawn(move || {
            second_barrier.wait();
            CryptoUtils::aes_init_from_bytes([0x22; 16], AesMode::CbcPkcs7)
        });
        barrier.wait();
        let first_result = first.join().unwrap();
        let second_result = second.join().unwrap();
        assert_eq!(
            [first_result.is_ok(), second_result.is_ok()]
                .into_iter()
                .filter(|success| *success)
                .count(),
            1
        );

        let (mode, key_bytes) = if first_result.is_ok() {
            (AesMode::Gcm, [0x11; 16])
        } else {
            (AesMode::CbcPkcs7, [0x22; 16])
        };
        assert_eq!(CryptoUtils::cipher().unwrap().mode(), mode);
        assert!(CryptoUtils::aes_is_initialized());
        assert!(matches!(
            CryptoUtils::aes_init(AesKey::from_bytes([0x55; 16]).unwrap(), AesMode::Gcm),
            Err(CryptoError::AlreadyInitialized)
        ));
        assert_eq!(CryptoUtils::cipher().unwrap().mode(), mode);
        let cipher = CryptoUtils::cipher().unwrap();
        assert_eq!(cipher.mode(), mode);
        assert_eq!(cipher.key_bits().bit_length(), key_bytes.len() * 8);
    }
}
