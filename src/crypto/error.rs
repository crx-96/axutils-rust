//! `CryptoUtils` 统一错误类型；不回显任何明文、密文、密钥、IV 或原始文本内容。

use std::fmt;

/// `CryptoUtils` 与 `crypto` 模块下全部能力共享的错误类型。
///
/// 出于安全考虑，本类型的字段只包含长度、位置偏移和编码名称，**绝不**包含明文、密文、密钥、
/// IV、摘要或原始文本片段；`Debug` 输出同样满足该约束。
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// 十六进制字符串长度为奇数。
    OddHexLength {
        /// 输入字符串的长度。
        length: usize,
    },
    /// 十六进制字符串在该字节位置上不是合法的 `0-9a-fA-F`。
    InvalidHex {
        /// 从 0 开始的非法字符字节偏移。
        position: usize,
    },
    /// 字节序列不是目标编码的合法文本（含默认可用的 UTF-8 校验失败）。
    TextDecodeInvalid {
        /// 目标编码名称，参见 [`TextEncoding::as_str`](crate::TextEncoding::as_str)。
        encoding: &'static str,
        /// 从 0 开始的失败字节偏移；无法提供可靠偏移时为 `None`。
        position: Option<usize>,
    },
    /// 输出长度计算溢出，或无法为指定操作预留结果空间。
    OutputTooLarge {
        /// 触发该错误的内部操作名称。
        operation: &'static str,
    },

    /// 文本无法用目标编码表示（例如 GBK 无法表示的字符）。UTF-8 编码永不失败，因此该变体
    /// 只在启用 `encoding_rs` feature 时存在。
    #[cfg(feature = "encoding_rs")]
    TextEncodeUnmappable {
        /// 目标编码名称。
        encoding: &'static str,
        /// `encoding_rs` 返回的已读取 UTF-8 字节数（从 0 开始）。
        position: usize,
    },

    /// Base64 输入含非法字符、长度非法、非规范尾随比特或与填充设置不符。
    #[cfg(feature = "base64")]
    Base64Decode {
        /// 上游提供可靠偏移时的 ASCII 输入偏移；否则为 `None`。
        position: Option<usize>,
    },

    /// 密钥长度不是 16、24 或 32 字节。
    #[cfg(feature = "aes")]
    InvalidKeyLength {
        /// 实际提供的密钥长度。
        length: usize,
    },
    /// 显式 IV/nonce 长度与所选模式不匹配。
    #[cfg(feature = "aes")]
    InvalidIvLength {
        /// 所选模式要求的 IV/nonce 长度。
        expected: usize,
        /// 实际提供的 IV/nonce 长度。
        length: usize,
    },
    /// 全局 AES 单例尚未初始化。
    #[cfg(feature = "aes")]
    NotInitialized,
    /// 全局 AES 单例已经初始化，不能再次替换。
    #[cfg(feature = "aes")]
    AlreadyInitialized,
    /// 容器输入短于当前调用形态的绝对最小长度。
    #[cfg(feature = "aes")]
    CiphertextTooShort {
        /// 当前调用形态下的绝对最小长度。
        minimum: usize,
        /// 实际输入长度。
        length: usize,
    },
    /// 解密失败：认证标签校验失败、填充非法或密文被篡改，**不区分具体原因**。
    #[cfg(feature = "aes")]
    Decrypt,
    /// 加密失败；不区分具体的上游原因。
    #[cfg(feature = "aes")]
    Encrypt,
    /// 操作系统随机源不可用。
    #[cfg(feature = "aes")]
    RandomSource,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddHexLength { length } => write!(f, "hex string has odd length {length}"),
            Self::InvalidHex { position } => {
                write!(f, "invalid hex character at byte position {position}")
            }
            Self::TextDecodeInvalid { encoding, position } => match position {
                Some(pos) => write!(f, "invalid {encoding} byte sequence at position {pos}"),
                None => write!(f, "invalid {encoding} byte sequence"),
            },
            Self::OutputTooLarge { operation } => {
                write!(
                    f,
                    "output too large to allocate for operation `{operation}`"
                )
            }
            #[cfg(feature = "encoding_rs")]
            Self::TextEncodeUnmappable { encoding, position } => {
                write!(
                    f,
                    "text is not representable in {encoding} at position {position}"
                )
            }
            #[cfg(feature = "base64")]
            Self::Base64Decode { position } => match position {
                Some(pos) => write!(f, "invalid base64 input at position {pos}"),
                None => write!(f, "invalid base64 input"),
            },
            #[cfg(feature = "aes")]
            Self::InvalidKeyLength { length } => {
                write!(
                    f,
                    "invalid AES key length {length}; expected 16, 24 or 32 bytes"
                )
            }
            #[cfg(feature = "aes")]
            Self::InvalidIvLength { expected, length } => {
                write!(
                    f,
                    "invalid IV/nonce length {length}; expected {expected} bytes"
                )
            }
            #[cfg(feature = "aes")]
            Self::NotInitialized => write!(f, "AES cipher is not initialized"),
            #[cfg(feature = "aes")]
            Self::AlreadyInitialized => write!(f, "AES cipher is already initialized"),
            #[cfg(feature = "aes")]
            Self::CiphertextTooShort { minimum, length } => {
                write!(f, "ciphertext too short: {length} bytes, minimum {minimum}")
            }
            #[cfg(feature = "aes")]
            Self::Decrypt => write!(f, "decryption failed"),
            #[cfg(feature = "aes")]
            Self::Encrypt => write!(f, "encryption failed"),
            #[cfg(feature = "aes")]
            Self::RandomSource => write!(f, "operating system random source unavailable"),
        }
    }
}

impl std::error::Error for CryptoError {}

#[cfg(test)]
mod tests {
    use super::CryptoError;

    #[test]
    fn display_does_not_echo_sentinel_content() {
        let err = CryptoError::TextDecodeInvalid {
            encoding: "UTF-8",
            position: Some(3),
        };
        let rendered = format!("{err}");
        assert!(!rendered.contains("SENTINEL_SECRET"));
        let debug = format!("{err:?}");
        assert!(!debug.contains("SENTINEL_SECRET"));
    }

    #[test]
    fn odd_hex_length_message_contains_length() {
        let err = CryptoError::OddHexLength { length: 3 };
        assert_eq!(format!("{err}"), "hex string has odd length 3");
    }

    #[cfg(feature = "aes")]
    #[test]
    fn aes_initialization_errors_do_not_echo_sentinel_content() {
        for err in [CryptoError::NotInitialized, CryptoError::AlreadyInitialized] {
            let display = format!("{err}");
            let debug = format!("{err:?}");
            assert!(!display.contains("SENTINEL_SECRET"));
            assert!(!debug.contains("SENTINEL_SECRET"));
        }
    }
}
