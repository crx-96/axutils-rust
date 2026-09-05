//! 十六进制编解码；不依赖任何第三方 crate，默认可用。

use crate::crypto::CryptoError;

const LOWER: &[u8; 16] = b"0123456789abcdef";
const UPPER: &[u8; 16] = b"0123456789ABCDEF";

fn encode(input: &[u8], table: &[u8; 16]) -> Result<String, CryptoError> {
    let len = input
        .len()
        .checked_mul(2)
        .ok_or(CryptoError::OutputTooLarge {
            operation: "hex_encode",
        })?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "hex_encode",
        })?;
    for &byte in input {
        out.push(table[(byte >> 4) as usize]);
        out.push(table[(byte & 0x0f) as usize]);
    }
    Ok(String::from_utf8(out).expect("hex table only contains ASCII characters"))
}

pub(crate) fn encode_lower(input: &[u8]) -> Result<String, CryptoError> {
    encode(input, LOWER)
}

pub(crate) fn encode_upper(input: &[u8]) -> Result<String, CryptoError> {
    encode(input, UPPER)
}

#[cfg(feature = "md5")]
pub(crate) fn encode_lower_fixed(input: &[u8; 16]) -> String {
    let mut out = String::with_capacity(32);
    for &byte in input {
        out.push(LOWER[(byte >> 4) as usize] as char);
        out.push(LOWER[(byte & 0x0f) as usize] as char);
    }
    out
}

pub(crate) fn decode(input: &str) -> Result<Vec<u8>, CryptoError> {
    let bytes = input.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(CryptoError::OddHexLength {
            length: bytes.len(),
        });
    }
    for (position, &b) in bytes.iter().enumerate() {
        if !b.is_ascii_hexdigit() {
            return Err(CryptoError::InvalidHex { position });
        }
    }
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len() / 2)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "hex_decode",
        })?;
    for chunk in bytes.chunks_exact(2) {
        let hi = hex_value(chunk[0]);
        let lo = hex_value(chunk[1]);
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_value(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => unreachable!("validated as ASCII hex digit before calling hex_value"),
    }
}

#[cfg(test)]
mod tests {
    use super as hex_codec;
    use crate::crypto::CryptoError;

    #[test]
    fn encode_lower_and_upper() {
        assert_eq!(hex_codec::encode_lower(&[]).unwrap(), "");
        assert_eq!(hex_codec::encode_lower(&[0x00, 0xff]).unwrap(), "00ff");
        assert_eq!(hex_codec::encode_upper(&[0x00, 0xff]).unwrap(), "00FF");
    }

    #[test]
    fn decode_accepts_mixed_case() {
        assert_eq!(hex_codec::decode("00Ff").unwrap(), vec![0x00, 0xff]);
        assert_eq!(hex_codec::decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn decode_rejects_odd_length() {
        assert_eq!(
            hex_codec::decode("abc").unwrap_err(),
            CryptoError::OddHexLength { length: 3 }
        );
    }

    #[test]
    fn decode_rejects_invalid_char() {
        assert_eq!(
            hex_codec::decode("zz").unwrap_err(),
            CryptoError::InvalidHex { position: 0 }
        );
    }

    #[test]
    fn decode_rejects_whitespace_and_0x_prefix() {
        assert!(hex_codec::decode("0x0f").is_err());
        assert!(hex_codec::decode("0f 0f").is_err());
        assert!(hex_codec::decode(" 0f").is_err());
    }

    #[test]
    fn roundtrip_all_bytes() {
        let input: Vec<u8> = (0u8..=255).collect();
        let encoded = hex_codec::encode_lower(&input).unwrap();
        assert_eq!(hex_codec::decode(&encoded).unwrap(), input);
    }
}
