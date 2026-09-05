//! 文本与字节之间的编码/解码抽象。

use std::str;

#[cfg(feature = "encoding_rs")]
use encoding_rs::{DecoderResult, EncoderResult, Encoding};

use crate::crypto::CryptoError;

/// 文本编码；`Utf8` 无需任何 feature，其余变体需要启用 `encoding_rs` feature。
///
/// legacy 变体遵循 WHATWG Encoding Standard：ISO-8859-1/Latin-1 在该标准中映射为
/// windows-1252；`Gbk` 无法输出 GB18030 的 4 字节序列（需要完整覆盖时应使用 `Gb18030`）。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-8，标准库实现，无第三方依赖。
    Utf8,
    /// GBK（简体中文）；无法输出 GB18030 的 4 字节序列。
    #[cfg(feature = "encoding_rs")]
    Gbk,
    /// GB18030（简体中文，覆盖 GBK 的全部字符）。
    #[cfg(feature = "encoding_rs")]
    Gb18030,
    /// Big5（繁体中文）。
    #[cfg(feature = "encoding_rs")]
    Big5,
    /// Shift_JIS（日文）。
    #[cfg(feature = "encoding_rs")]
    ShiftJis,
    /// EUC-KR（韩文）。
    #[cfg(feature = "encoding_rs")]
    EucKr,
    /// windows-1252；WHATWG 标准中 ISO-8859-1/Latin-1 也映射到该编码。
    #[cfg(feature = "encoding_rs")]
    Windows1252,
}

impl TextEncoding {
    /// 返回编码名称，与 WHATWG Encoding Standard 的标签一致。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::TextEncoding;
    ///
    /// assert_eq!(TextEncoding::Utf8.as_str(), "UTF-8");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            #[cfg(feature = "encoding_rs")]
            Self::Gbk => "GBK",
            #[cfg(feature = "encoding_rs")]
            Self::Gb18030 => "gb18030",
            #[cfg(feature = "encoding_rs")]
            Self::Big5 => "Big5",
            #[cfg(feature = "encoding_rs")]
            Self::ShiftJis => "Shift_JIS",
            #[cfg(feature = "encoding_rs")]
            Self::EucKr => "EUC-KR",
            #[cfg(feature = "encoding_rs")]
            Self::Windows1252 => "windows-1252",
        }
    }

    /// 把 `text` 按本编码编码为字节序列。
    ///
    /// # Errors
    ///
    /// UTF-8 编码永不因内容失败，但可检查的容量失败返回 [`CryptoError::OutputTooLarge`]；
    /// legacy 编码遇到无法表示的字符时返回 `CryptoError::TextEncodeUnmappable`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::TextEncoding;
    ///
    /// let bytes = TextEncoding::Utf8.encode("hello").unwrap();
    /// assert_eq!(bytes, b"hello");
    /// ```
    pub fn encode(&self, text: &str) -> Result<Vec<u8>, CryptoError> {
        match self {
            Self::Utf8 => {
                let mut out = Vec::new();
                out.try_reserve_exact(text.len())
                    .map_err(|_| CryptoError::OutputTooLarge {
                        operation: "text_encode_utf8",
                    })?;
                out.extend_from_slice(text.as_bytes());
                Ok(out)
            }
            #[cfg(feature = "encoding_rs")]
            other => other.encode_legacy(text),
        }
    }

    /// 把字节序列按本编码解码为 `String`。
    ///
    /// # Errors
    ///
    /// 字节序列不是合法文本时返回 [`CryptoError::TextDecodeInvalid`]；可检查的容量失败返回
    /// [`CryptoError::OutputTooLarge`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::crypto::TextEncoding;
    ///
    /// let text = TextEncoding::Utf8.decode(b"hello").unwrap();
    /// assert_eq!(text, "hello");
    /// assert!(TextEncoding::Utf8.decode(&[0xff, 0xfe]).is_err());
    /// ```
    pub fn decode(&self, bytes: impl AsRef<[u8]>) -> Result<String, CryptoError> {
        let bytes = bytes.as_ref();
        match self {
            Self::Utf8 => match str::from_utf8(bytes) {
                Ok(s) => {
                    let mut out = String::new();
                    out.try_reserve_exact(s.len())
                        .map_err(|_| CryptoError::OutputTooLarge {
                            operation: "text_decode_utf8",
                        })?;
                    out.push_str(s);
                    Ok(out)
                }
                Err(e) => Err(CryptoError::TextDecodeInvalid {
                    encoding: "UTF-8",
                    position: Some(e.valid_up_to()),
                }),
            },
            #[cfg(feature = "encoding_rs")]
            other => other.decode_legacy(bytes),
        }
    }

    #[cfg(feature = "encoding_rs")]
    fn whatwg(&self) -> &'static Encoding {
        match self {
            Self::Utf8 => encoding_rs::UTF_8,
            Self::Gbk => encoding_rs::GBK,
            Self::Gb18030 => encoding_rs::GB18030,
            Self::Big5 => encoding_rs::BIG5,
            Self::ShiftJis => encoding_rs::SHIFT_JIS,
            Self::EucKr => encoding_rs::EUC_KR,
            Self::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }

    #[cfg(feature = "encoding_rs")]
    fn encode_legacy(&self, text: &str) -> Result<Vec<u8>, CryptoError> {
        let encoding = self.as_str();
        let mut encoder = self.whatwg().new_encoder();
        let max_len = encoder
            .max_buffer_length_from_utf8_without_replacement(text.len())
            .ok_or(CryptoError::OutputTooLarge {
                operation: "text_encode_legacy",
            })?;
        let mut out = Vec::new();
        out.try_reserve_exact(max_len)
            .map_err(|_| CryptoError::OutputTooLarge {
                operation: "text_encode_legacy",
            })?;
        out.resize(max_len, 0);
        let (result, read, written) =
            encoder.encode_from_utf8_without_replacement(text, &mut out, true);
        match result {
            EncoderResult::InputEmpty => {
                out.truncate(written);
                Ok(out)
            }
            EncoderResult::Unmappable(_) => Err(CryptoError::TextEncodeUnmappable {
                encoding,
                position: read,
            }),
            EncoderResult::OutputFull => Err(CryptoError::OutputTooLarge {
                operation: "text_encode_legacy",
            }),
        }
    }

    #[cfg(feature = "encoding_rs")]
    fn decode_legacy(&self, bytes: &[u8]) -> Result<String, CryptoError> {
        let encoding = self.as_str();
        let mut decoder = self.whatwg().new_decoder_without_bom_handling();
        let max_len = decoder
            .max_utf8_buffer_length_without_replacement(bytes.len())
            .ok_or(CryptoError::OutputTooLarge {
                operation: "text_decode_legacy",
            })?;
        let mut out = String::new();
        out.try_reserve_exact(max_len)
            .map_err(|_| CryptoError::OutputTooLarge {
                operation: "text_decode_legacy",
            })?;
        let (result, _read) = decoder.decode_to_string_without_replacement(bytes, &mut out, true);
        match result {
            DecoderResult::InputEmpty => Ok(out),
            DecoderResult::Malformed(_, _) => Err(CryptoError::TextDecodeInvalid {
                encoding,
                position: None,
            }),
            DecoderResult::OutputFull => Err(CryptoError::OutputTooLarge {
                operation: "text_decode_legacy",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TextEncoding;
    use crate::crypto::CryptoError;

    #[test]
    fn utf8_roundtrip() {
        let bytes = TextEncoding::Utf8.encode("你好").unwrap();
        assert_eq!(TextEncoding::Utf8.decode(&bytes).unwrap(), "你好");
    }

    #[test]
    fn utf8_decode_reports_valid_up_to() {
        let err = TextEncoding::Utf8.decode([b'a', 0xff]).unwrap_err();
        assert_eq!(
            err,
            CryptoError::TextDecodeInvalid {
                encoding: "UTF-8",
                position: Some(1)
            }
        );
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn legacy_variants_roundtrip() {
        let cases = [
            (TextEncoding::Gbk, "简体中文"),
            (TextEncoding::Gb18030, "简体中文𠀀"),
            (TextEncoding::Big5, "繁體中文"),
            (TextEncoding::ShiftJis, "日本語"),
            (TextEncoding::EucKr, "한국어"),
            (TextEncoding::Windows1252, "café"),
        ];
        for (encoding, text) in cases {
            let bytes = encoding.encode(text).unwrap();
            assert_eq!(
                encoding.decode(&bytes).unwrap(),
                text,
                "{}",
                encoding.as_str()
            );
        }
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn gbk_cannot_encode_gb18030_only_characters() {
        let err = TextEncoding::Gbk.encode("𠀀").unwrap_err();
        assert!(matches!(
            err,
            CryptoError::TextEncodeUnmappable {
                encoding: "GBK",
                ..
            }
        ));
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn unmappable_position_counts_utf8_input_bytes() {
        let err = TextEncoding::Gbk.encode("你𠀀").unwrap_err();
        assert_eq!(
            err,
            CryptoError::TextEncodeUnmappable {
                encoding: "GBK",
                position: "你𠀀".len(),
            }
        );
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn legacy_decode_rejects_malformed_bytes() {
        let err = TextEncoding::Gbk.decode([0xff, 0xff]).unwrap_err();
        assert!(matches!(
            err,
            CryptoError::TextDecodeInvalid {
                encoding: "GBK",
                position: None
            }
        ));
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn as_str_matches_whatwg_labels() {
        assert_eq!(TextEncoding::Gbk.as_str(), "GBK");
        assert_eq!(TextEncoding::Gb18030.as_str(), "gb18030");
        assert_eq!(TextEncoding::Big5.as_str(), "Big5");
        assert_eq!(TextEncoding::ShiftJis.as_str(), "Shift_JIS");
        assert_eq!(TextEncoding::EucKr.as_str(), "EUC-KR");
        assert_eq!(TextEncoding::Windows1252.as_str(), "windows-1252");
    }

    #[cfg(feature = "encoding_rs")]
    #[test]
    fn bom_is_not_stripped_or_added() {
        let bytes = TextEncoding::Utf8.encode("\u{feff}hi").unwrap();
        assert_eq!(bytes, "\u{feff}hi".as_bytes());
        let decoded = TextEncoding::Utf8.decode(&bytes).unwrap();
        assert!(decoded.starts_with('\u{feff}'));
    }
}
