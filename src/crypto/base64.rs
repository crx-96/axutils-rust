//! Base64 编解码后端（`base64` crate，feature = `base64`）。

use crate::CryptoError;
use ::base64::{
    alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    Engine,
};

/// Base64 字母表选择。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base64Alphabet {
    /// RFC 4648 §4 标准字母表（含 `+` 与 `/`）。
    Standard,
    /// RFC 4648 §5 URL 与文件名安全字母表（含 `-` 与 `_`）。
    UrlSafe,
}

/// Base64 编解码设置：字母表与是否包含 `=` 填充。
///
/// 编码与解码使用同一份设置；解码严格拒绝与设置不符的填充和非规范尾随比特，不做“自动探测”。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base64Options {
    alphabet: Base64Alphabet,
    padding: bool,
}

impl Base64Options {
    /// 标准字母表 + 有填充。
    pub const STANDARD: Self = Self {
        alphabet: Base64Alphabet::Standard,
        padding: true,
    };
    /// 标准字母表 + 无填充。
    pub const STANDARD_NO_PAD: Self = Self {
        alphabet: Base64Alphabet::Standard,
        padding: false,
    };
    /// URL-safe 字母表 + 有填充。
    pub const URL_SAFE: Self = Self {
        alphabet: Base64Alphabet::UrlSafe,
        padding: true,
    };
    /// URL-safe 字母表 + 无填充。
    pub const URL_SAFE_NO_PAD: Self = Self {
        alphabet: Base64Alphabet::UrlSafe,
        padding: false,
    };

    /// 显式构造字母表与填充组合。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Alphabet, Base64Options};
    ///
    /// let options = Base64Options::new(Base64Alphabet::UrlSafe, false);
    /// assert_eq!(options, Base64Options::URL_SAFE_NO_PAD);
    /// ```
    #[must_use]
    pub fn new(alphabet: Base64Alphabet, padding: bool) -> Self {
        Self { alphabet, padding }
    }

    /// 返回当前使用的字母表。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{Base64Alphabet, Base64Options};
    ///
    /// assert_eq!(Base64Options::STANDARD.alphabet(), Base64Alphabet::Standard);
    /// ```
    #[must_use]
    pub fn alphabet(&self) -> Base64Alphabet {
        self.alphabet
    }

    /// 返回是否包含 `=` 填充。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::Base64Options;
    ///
    /// assert!(!Base64Options::STANDARD_NO_PAD.padding());
    /// ```
    #[must_use]
    pub fn padding(&self) -> bool {
        self.padding
    }

    fn engine(&self) -> GeneralPurpose {
        let alphabet_ref = match self.alphabet {
            Base64Alphabet::Standard => &alphabet::STANDARD,
            Base64Alphabet::UrlSafe => &alphabet::URL_SAFE,
        };
        let padding_mode = if self.padding {
            DecodePaddingMode::RequireCanonical
        } else {
            DecodePaddingMode::RequireNone
        };
        let config = GeneralPurposeConfig::new()
            .with_encode_padding(self.padding)
            .with_decode_padding_mode(padding_mode)
            .with_decode_allow_trailing_bits(false);
        GeneralPurpose::new(alphabet_ref, config)
    }
}

pub(crate) fn encode(input: &[u8], options: Base64Options) -> Result<String, CryptoError> {
    let engine = options.engine();
    let encoded_len = ::base64::encoded_len(input.len(), options.padding()).ok_or(
        CryptoError::OutputTooLarge {
            operation: "base64_encode",
        },
    )?;
    let mut out = Vec::new();
    out.try_reserve_exact(encoded_len)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "base64_encode",
        })?;
    out.resize(encoded_len, 0);
    let written =
        engine
            .encode_slice(input, &mut out)
            .map_err(|_| CryptoError::OutputTooLarge {
                operation: "base64_encode",
            })?;
    out.truncate(written);
    Ok(String::from_utf8(out).expect("base64 alphabets only contain ASCII characters"))
}

pub(crate) fn decode(input: &str, options: Base64Options) -> Result<Vec<u8>, CryptoError> {
    let engine = options.engine();
    let estimate = ::base64::decoded_len_estimate(input.len());
    let mut out = Vec::new();
    out.try_reserve_exact(estimate)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "base64_decode",
        })?;
    out.resize(estimate, 0);
    match engine.decode_slice(input, &mut out) {
        Ok(written) => {
            out.truncate(written);
            Ok(out)
        }
        Err(::base64::DecodeSliceError::OutputSliceTooSmall) => Err(CryptoError::OutputTooLarge {
            operation: "base64_decode",
        }),
        Err(::base64::DecodeSliceError::DecodeError(e)) => Err(CryptoError::Base64Decode {
            position: decode_error_position(&e),
        }),
    }
}

fn decode_error_position(error: &::base64::DecodeError) -> Option<usize> {
    match error {
        ::base64::DecodeError::InvalidByte(position, _) => Some(*position),
        ::base64::DecodeError::InvalidLastSymbol { offset, .. } => Some(*offset),
        ::base64::DecodeError::InvalidLength(_) | ::base64::DecodeError::InvalidPadding => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_test_vectors_standard_padded() {
        let cases: [(&[u8], &str); 7] = [
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(encode(input, Base64Options::STANDARD).unwrap(), expected);
            assert_eq!(decode(expected, Base64Options::STANDARD).unwrap(), input);
        }
    }

    #[test]
    fn no_pad_roundtrip() {
        let encoded = encode(b"foob", Base64Options::STANDARD_NO_PAD).unwrap();
        assert_eq!(encoded, "Zm9vYg");
        assert_eq!(
            decode(&encoded, Base64Options::STANDARD_NO_PAD).unwrap(),
            b"foob"
        );
    }

    #[test]
    fn url_safe_distinguishes_plus_slash() {
        let input: &[u8] = &[0xfb, 0xff, 0xfe];
        let std_encoded = encode(input, Base64Options::STANDARD).unwrap();
        let url_encoded = encode(input, Base64Options::URL_SAFE).unwrap();
        assert!(std_encoded.contains('+') || std_encoded.contains('/'));
        assert!(!url_encoded.contains('+') && !url_encoded.contains('/'));
        assert_eq!(
            decode(&url_encoded, Base64Options::URL_SAFE).unwrap(),
            input
        );
    }

    #[test]
    fn cross_alphabet_decode_is_rejected() {
        let encoded = encode(&[0xfb, 0xff, 0xfe], Base64Options::STANDARD).unwrap();
        assert!(decode(&encoded, Base64Options::URL_SAFE).is_err());
    }

    #[test]
    fn missing_required_padding_is_rejected() {
        assert!(decode("Zm9vYg", Base64Options::STANDARD).is_err());
    }

    #[test]
    fn unexpected_padding_in_no_pad_mode_is_rejected() {
        assert!(decode("Zm9vYg==", Base64Options::STANDARD_NO_PAD).is_err());
    }

    #[test]
    fn illegal_characters_and_nonzero_trailing_bits_are_rejected() {
        assert!(decode("Zm9v!g==", Base64Options::STANDARD).is_err());
        assert!(decode("Zm9vYh==", Base64Options::STANDARD).is_err());
    }

    #[test]
    fn whitespace_and_misplaced_padding_are_rejected() {
        assert!(decode(" Zg==", Base64Options::STANDARD).is_err());
        assert!(decode("Zg==\n", Base64Options::STANDARD).is_err());
        assert!(decode("=Zg==", Base64Options::STANDARD).is_err());
        assert!(decode("Z=g=", Base64Options::STANDARD).is_err());
    }
}
