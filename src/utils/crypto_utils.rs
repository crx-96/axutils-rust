//! `CryptoUtils` 静态工具入口；具体实现见 [`crate::crypto`]。

#[cfg(feature = "base64")]
use crate::Base64Options;
use crate::CryptoError;
#[cfg(any(feature = "base64", feature = "md5"))]
use crate::TextEncoding;
#[cfg(feature = "aes")]
use crate::{AesCipher, AesKey, AesMode};
#[cfg(feature = "aes")]
use std::sync::OnceLock;

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

    /// 初始化进程级 AES 单例，并固定密钥和模式。
    ///
    /// 初始化成功后不能 reset、replace 或读取密钥；全局实例与进程同寿命，正常退出前不会触发
    /// 内部 [`AesKey`] 的 `Drop`，因此密钥不会由本 crate 清零。需要可控生命周期时使用
    /// [`crate::AesCipher`]。
    ///
    /// # Errors
    ///
    /// 已有线程完成初始化时返回 [`CryptoError::AlreadyInitialized`]；并发竞争中未获胜的调用也
    /// 返回该错误。该方法本身不检查密钥长度，因为 `key` 已是有效的 [`AesKey`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesKey, AesMode, CryptoUtils};
    ///
    /// let key = AesKey::from_bytes([0x00; 32]).unwrap();
    /// CryptoUtils::aes_init(key, AesMode::Gcm).unwrap();
    /// assert_eq!(CryptoUtils::aes_mode().unwrap(), AesMode::Gcm);
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_init(key: AesKey, mode: AesMode) -> Result<(), CryptoError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = if AES_CIPHER.get().is_some() {
            Err(CryptoError::AlreadyInitialized)
        } else {
            AES_CIPHER
                .set(AesCipher::new(key, mode))
                .map_err(|_| CryptoError::AlreadyInitialized)
        };
        #[cfg(feature = "tracing")]
        crate::tracing::crypto::record_init("aes_init", &result, started);
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
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 32], AesMode::Gcm).unwrap();
    /// assert!(CryptoUtils::aes_is_initialized());
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_init_from_bytes(key: impl AsRef<[u8]>, mode: AesMode) -> Result<(), CryptoError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
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
        crate::tracing::crypto::record_init("aes_init_from_bytes", &result, started);
        result
    }

    /// 返回进程级 AES 单例是否已经成功初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// assert!(!CryptoUtils::aes_is_initialized());
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// assert!(CryptoUtils::aes_is_initialized());
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_is_initialized() -> bool {
        AES_CIPHER.get().is_some()
    }

    /// 返回进程级 AES 单例初始化时固定的模式。
    ///
    /// # Errors
    ///
    /// 尚未初始化时返回 [`CryptoError::NotInitialized`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::CbcPkcs7).unwrap();
    /// assert_eq!(CryptoUtils::aes_mode().unwrap(), AesMode::CbcPkcs7);
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_mode() -> Result<AesMode, CryptoError> {
        Ok(Self::aes_cipher()?.mode())
    }

    /// 使用进程级 AES 单例随机生成 IV/nonce 并加密，返回包含前置 IV/nonce 的容器。
    ///
    /// 初始化前不会检查输入，而是优先返回 [`CryptoError::NotInitialized`]。GCM 随机 nonce 在
    /// 同一全局密钥下的安全消息数约为 2^32；需要轮换密钥时必须重启进程或改用
    /// [`crate::AesCipher`] 实例。
    ///
    /// # Errors
    ///
    /// 未初始化时返回 [`CryptoError::NotInitialized`]；其余错误语义与 [`crate::AesCipher::encrypt`]
    /// 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 32], AesMode::Gcm).unwrap();
    /// let ciphertext = CryptoUtils::aes_encrypt("hello world").unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt(&ciphertext).unwrap(), b"hello world");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt(plaintext: impl AsRef<[u8]>) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.encrypt(plaintext)
    }

    /// 解密进程级 AES 单例生成的、包含前置 IV/nonce 的容器。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；输入短于最小长度时返回
    /// [`CryptoError::CiphertextTooShort`]；认证失败、篡改或填充非法时统一返回
    /// [`CryptoError::Decrypt`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::CbcPkcs7).unwrap();
    /// let ciphertext = CryptoUtils::aes_encrypt("secret").unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt(&ciphertext).unwrap(), b"secret");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt(input: impl AsRef<[u8]>) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.decrypt(input)
    }

    /// 使用进程级 AES 单例和调用方提供的 IV/nonce 加密，返回不包含 IV/nonce 的密文。
    ///
    /// GCM 下调用方必须保证 nonce 在全局密钥下唯一；CBC 不提供完整性认证。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；其余错误语义与
    /// [`crate::AesCipher::encrypt_with_iv`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let nonce = [0x00; 12];
    /// let ciphertext = CryptoUtils::aes_encrypt_with_iv("hello", &nonce).unwrap();
    /// assert_eq!(ciphertext.len(), 5 + 16);
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt_with_iv(
        plaintext: impl AsRef<[u8]>,
        iv: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.encrypt_with_iv(plaintext, iv)
    }

    /// 使用进程级 AES 单例和调用方提供的 IV/nonce 解密不包含 IV/nonce 的密文。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；其余错误语义与
    /// [`crate::AesCipher::decrypt_with_iv`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let nonce = [0x00; 12];
    /// let ciphertext = CryptoUtils::aes_encrypt_with_iv("hello", &nonce).unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt_with_iv(&ciphertext, &nonce).unwrap(), b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt_with_iv(
        ciphertext: impl AsRef<[u8]>,
        iv: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.decrypt_with_iv(ciphertext, iv)
    }

    /// 使用进程级 AES 单例随机生成 IV/nonce，加密并编码为小写十六进制。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；其余错误语义与
    /// [`crate::AesCipher::encrypt_hex`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = CryptoUtils::aes_encrypt_hex("hello").unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt_hex(&encoded).unwrap(), b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_encrypt_hex(plaintext: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        Self::aes_cipher()?.encrypt_hex(plaintext)
    }

    /// 解码十六进制容器并使用进程级 AES 单例解密。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；初始化后奇数长度或非法字符分别返回
    /// [`CryptoError::OddHexLength`] 或 [`CryptoError::InvalidHex`]，其余错误语义与
    /// [`crate::AesCipher::decrypt_hex`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::CbcPkcs7).unwrap();
    /// let encoded = CryptoUtils::aes_encrypt_hex("hello").unwrap();
    /// assert_eq!(CryptoUtils::aes_decrypt_hex(&encoded).unwrap(), b"hello");
    /// ```
    #[cfg(feature = "aes")]
    pub fn aes_decrypt_hex(input: &str) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.decrypt_hex(input)
    }

    /// 使用进程级 AES 单例加密完整容器，并按 `options` 编码为 Base64。
    ///
    /// 仅在同时启用 `aes` 与 `base64` feature 时提供；`options` 仍可逐次选择，不会被初始化
    /// 固定。
    ///
    /// # Errors
    ///
    /// 初始化前返回 [`CryptoError::NotInitialized`]；其余错误语义与
    /// [`crate::AesCipher::encrypt_base64`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, Base64Options, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = CryptoUtils::aes_encrypt_base64("hello", Base64Options::URL_SAFE_NO_PAD).unwrap();
    /// assert_eq!(
    ///     CryptoUtils::aes_decrypt_base64(&encoded, Base64Options::URL_SAFE_NO_PAD).unwrap(),
    ///     b"hello"
    /// );
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn aes_encrypt_base64(
        plaintext: impl AsRef<[u8]>,
        options: Base64Options,
    ) -> Result<String, CryptoError> {
        Self::aes_cipher()?.encrypt_base64(plaintext, options)
    }

    /// 解码 Base64 容器并使用进程级 AES 单例解密。
    ///
    /// 仅在同时启用 `aes` 与 `base64` feature 时提供；`options` 必须与输入的字母表和填充形式
    /// 一致。
    ///
    /// # Errors
    ///
    /// 初始化前优先返回 [`CryptoError::NotInitialized`]；Base64 输入非法时返回
    /// [`CryptoError::Base64Decode`]，其余错误语义与 [`crate::AesCipher::decrypt_base64`] 相同。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{AesMode, Base64Options, CryptoUtils};
    ///
    /// CryptoUtils::aes_init_from_bytes([0x00; 16], AesMode::Gcm).unwrap();
    /// let encoded = CryptoUtils::aes_encrypt_base64("hello", Base64Options::STANDARD).unwrap();
    /// assert_eq!(
    ///     CryptoUtils::aes_decrypt_base64(&encoded, Base64Options::STANDARD).unwrap(),
    ///     b"hello"
    /// );
    /// ```
    #[cfg(all(feature = "aes", feature = "base64"))]
    pub fn aes_decrypt_base64(input: &str, options: Base64Options) -> Result<Vec<u8>, CryptoError> {
        Self::aes_cipher()?.decrypt_base64(input, options)
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
    fn aes_cipher_facade_roundtrip_both_modes() {
        use crate::{AesCipher, AesMode};
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
        use crate::{AesCipher, AesMode, Base64Options};
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
    fn aes_global_lifecycle_and_paths() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        use crate::{AesCipher, AesKey, AesMode, CryptoError};

        assert!(!CryptoUtils::aes_is_initialized());
        assert!(matches!(
            CryptoUtils::aes_mode(),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_encrypt("payload"),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_decrypt([]),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_encrypt_with_iv("payload", &[0u8; 0]),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_decrypt_with_iv([], &[0u8; 0]),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_encrypt_hex("payload"),
            Err(CryptoError::NotInitialized)
        ));
        assert!(matches!(
            CryptoUtils::aes_decrypt_hex("not-hex"),
            Err(CryptoError::NotInitialized)
        ));
        #[cfg(feature = "base64")]
        {
            assert!(matches!(
                CryptoUtils::aes_encrypt_base64("payload", crate::Base64Options::STANDARD),
                Err(CryptoError::NotInitialized)
            ));
            assert!(matches!(
                CryptoUtils::aes_decrypt_base64("!", crate::Base64Options::STANDARD),
                Err(CryptoError::NotInitialized)
            ));
        }

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
        assert_eq!(CryptoUtils::aes_mode().unwrap(), mode);
        assert!(CryptoUtils::aes_is_initialized());
        assert!(matches!(
            CryptoUtils::aes_init(AesKey::from_bytes([0x55; 16]).unwrap(), AesMode::Gcm),
            Err(CryptoError::AlreadyInitialized)
        ));
        assert_eq!(CryptoUtils::aes_mode().unwrap(), mode);

        let cipher = AesCipher::from_key_bytes(key_bytes, mode).unwrap();
        let container = CryptoUtils::aes_encrypt("container").unwrap();
        assert_eq!(cipher.decrypt(&container).unwrap(), b"container");
        let instance_container = cipher.encrypt("instance container").unwrap();
        assert_eq!(
            CryptoUtils::aes_decrypt(&instance_container).unwrap(),
            b"instance container"
        );

        let iv = vec![0u8; mode.iv_length()];
        let explicit = CryptoUtils::aes_encrypt_with_iv("explicit", &iv).unwrap();
        assert_eq!(cipher.decrypt_with_iv(&explicit, &iv).unwrap(), b"explicit");
        let instance_explicit = cipher.encrypt_with_iv("instance explicit", &iv).unwrap();
        assert_eq!(
            CryptoUtils::aes_decrypt_with_iv(&instance_explicit, &iv).unwrap(),
            b"instance explicit"
        );

        let encoded = CryptoUtils::aes_encrypt_hex("hex").unwrap();
        assert_eq!(cipher.decrypt_hex(&encoded).unwrap(), b"hex");
        let instance_encoded = cipher.encrypt_hex("instance hex").unwrap();
        assert_eq!(
            CryptoUtils::aes_decrypt_hex(&instance_encoded).unwrap(),
            b"instance hex"
        );

        #[cfg(feature = "base64")]
        {
            let encoded =
                CryptoUtils::aes_encrypt_base64("base64", crate::Base64Options::STANDARD_NO_PAD)
                    .unwrap();
            assert_eq!(
                cipher
                    .decrypt_base64(&encoded, crate::Base64Options::STANDARD_NO_PAD)
                    .unwrap(),
                b"base64"
            );
            let instance_encoded = cipher
                .encrypt_base64("instance base64", crate::Base64Options::URL_SAFE)
                .unwrap();
            assert_eq!(
                CryptoUtils::aes_decrypt_base64(&instance_encoded, crate::Base64Options::URL_SAFE)
                    .unwrap(),
                b"instance base64"
            );
        }
    }
}
