use std::fmt;

use serde::de::{self, Deserializer as _, MapAccess, Visitor};

use super::{JwtAlgorithm, JwtError};

pub(crate) fn validate_header_segment(
    encoded: &str,
    expected_algorithm: JwtAlgorithm,
) -> Result<(), JwtError> {
    let decoded = decode_base64url(encoded).ok_or(JwtError::InvalidHeader { field: "base64" })?;
    let mut deserializer = serde_json::Deserializer::from_slice(&decoded);
    let header = deserializer
        .deserialize_any(HeaderVisitor)
        .map_err(|_| JwtError::InvalidHeader { field: "json" })?;
    deserializer
        .end()
        .map_err(|_| JwtError::InvalidHeader { field: "trailing" })?;

    if header.typ.as_deref() != Some("JWT") {
        return Err(JwtError::InvalidHeader { field: "typ" });
    }
    if header.alg.as_deref() != Some(expected_algorithm.name()) {
        return Err(JwtError::InvalidHeader { field: "alg" });
    }
    Ok(())
}

struct HeaderFields {
    alg: Option<String>,
    typ: Option<String>,
}

struct HeaderVisitor;

impl<'de> Visitor<'de> for HeaderVisitor {
    type Value = HeaderFields;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JWT header object")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut header = HeaderFields {
            alg: None,
            typ: None,
        };
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "alg" => {
                    if header.alg.is_some() {
                        return Err(de::Error::custom("duplicate JWT header field"));
                    }
                    header.alg = Some(map.next_value::<String>()?);
                }
                "typ" => {
                    if header.typ.is_some() {
                        return Err(de::Error::custom("duplicate JWT header field"));
                    }
                    header.typ = Some(map.next_value::<String>()?);
                }
                _ => return Err(de::Error::custom("unsupported JWT header field")),
            }
        }
        Ok(header)
    }
}

pub(crate) fn decode_base64url(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if bytes.len() % 4 == 1 || bytes.contains(&b'=') {
        return None;
    }

    let mut output = Vec::with_capacity(bytes.len() / 4 * 3 + 2);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in bytes {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
        if bits == 0 {
            buffer = 0;
        } else {
            buffer &= (1 << bits) - 1;
        }
    }
    if bits != 0 && buffer != 0 {
        return None;
    }
    Some(output)
}
