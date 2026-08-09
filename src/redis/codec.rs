use serde::{de::DeserializeOwned, Serialize};

use super::error::RedisError;

/// 将值编码为受限的 MessagePack 字节。
pub(crate) fn encode<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, RedisError> {
    let mut writer = LimitedWriter::new(limit);
    value
        .serialize(&mut rmp_serde::Serializer::new(&mut writer))
        .map_err(|_| {
            if writer.exceeded {
                RedisError::ValueTooLarge { limit }
            } else {
                RedisError::Serialize
            }
        })?;
    Ok(writer.into_inner())
}

/// 将 Redis 返回的 MessagePack 字节解码为拥有型值。
pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, RedisError> {
    if bytes.len() > limit {
        return Err(RedisError::ValueTooLarge { limit });
    }
    rmp_serde::from_slice(bytes).map_err(|_| RedisError::Deserialize)
}

/// 校验 raw 值的大小并复制为拥有型字节。
pub(crate) fn raw(bytes: impl AsRef<[u8]>, limit: usize) -> Result<Vec<u8>, RedisError> {
    let bytes = bytes.as_ref();
    if bytes.len() > limit {
        return Err(RedisError::ValueTooLarge { limit });
    }
    Ok(bytes.to_vec())
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl LimitedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(8 * 1024)),
            limit,
            exceeded: false,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl std::io::Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "serialization limit exceeded",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::{decode, encode, raw, RedisError};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Kind {
        Fixture,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Payload {
        kind: Kind,
        name: String,
        values: Vec<u16>,
        bytes: Vec<u8>,
    }

    #[test]
    fn messagepack_round_trips_nested_values_and_option() {
        let payload = Payload {
            kind: Kind::Fixture,
            name: "fixture".to_owned(),
            values: vec![1, 2, 3],
            bytes: vec![0, 1, 255],
        };
        let encoded = encode(&Some(payload), 1024).expect("encode");
        let decoded: Option<Payload> = decode(&encoded, 1024).expect("decode");
        let decoded = decoded.expect("payload should be present");
        assert_eq!(decoded.kind, Kind::Fixture);
        assert_eq!(decoded.name, "fixture");
        assert_eq!(decoded.bytes, vec![0, 1, 255]);

        let encoded_none = encode(&Option::<Payload>::None, 8).expect("encode nil");
        assert_eq!(decode::<Option<Payload>>(&encoded_none, 8), Ok(None));
    }

    #[test]
    fn serialization_and_raw_values_are_bounded() {
        let error = encode(&"too long", 1).expect_err("limit should reject value");
        assert_eq!(error, RedisError::ValueTooLarge { limit: 1 });
        assert_eq!(raw([1u8, 2], 2).expect("raw"), vec![1, 2]);
        assert_eq!(
            raw([1u8, 2], 1),
            Err(RedisError::ValueTooLarge { limit: 1 })
        );
    }

    #[test]
    fn malformed_messagepack_is_deserialize_error() {
        assert_eq!(decode::<u8>(&[0xc1], 8), Err(RedisError::Deserialize));
    }
}
