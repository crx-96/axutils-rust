use std::io;

use jsonwebtoken::Header as BackendHeader;
use serde::Serialize;
use serde_json::{Serializer as JsonSerializer, Value};

use super::super::claims::{self, MAX_CLAIMS_BYTES};
use super::error_map;
use super::{JwtCodec, MAX_TOKEN_BYTES};
use crate::jwt::JwtError;

impl JwtCodec {
    /// 用固定算法签发泛型 claims。
    ///
    /// claims 必须是受限 JSON object：序列化结果最多 32 KiB，并经过重复键、深度、成员和数组
    /// 预算预检；生成的 token 最多 64 KiB。未配置 signing key 时返回
    /// [`JwtError::MissingSigningKey`]。返回的 token 不是加密内容，调用方不得在日志或错误中记录
    /// 其中的敏感 claims。
    pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String, JwtError> {
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or(JwtError::MissingSigningKey)?;
        let mut writer = BoundedJsonWriter::new(MAX_CLAIMS_BYTES);
        let serialization_result = {
            let mut serializer = JsonSerializer::new(&mut writer);
            claims.serialize(&mut serializer)
        };
        if serialization_result.is_err() {
            if writer.overflowed {
                return Err(JwtError::ClaimsTooLarge {
                    length: writer.bytes.len(),
                    limit: MAX_CLAIMS_BYTES,
                });
            }
            return Err(JwtError::InvalidToken { segment: "claims" });
        }
        let serialized = writer.bytes;
        claims::preflight_claims(&serialized)?;
        let value: Value = serde_json::from_slice(&serialized)
            .map_err(|_| JwtError::InvalidToken { segment: "claims" })?;
        claims::check_value_resources(&value)?;

        let algorithm = self
            .algorithm
            .backend()
            .ok_or(JwtError::InvalidConfig { field: "algorithm" })?;
        let header = BackendHeader::new(algorithm);
        let token = jsonwebtoken::encode(&header, &value, signing_key.backend())
            .map_err(|error| error_map::map_encode_error(&error))?;
        if token.len() > MAX_TOKEN_BYTES {
            return Err(JwtError::TokenTooLarge {
                length: token.len(),
                limit: MAX_TOKEN_BYTES,
            });
        }
        Ok(token)
    }
}

struct BoundedJsonWriter {
    bytes: Vec<u8>,
    limit: usize,
    overflowed: bool,
}

impl BoundedJsonWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(1024)),
            limit,
            overflowed: false,
        }
    }
}

impl io::Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let capture_limit = self.limit.saturating_add(1);
        let remaining = capture_limit.saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            self.bytes.extend_from_slice(&bytes[..remaining]);
            self.overflowed = true;
            return Err(io::Error::other("bounded JSON writer limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() > self.limit {
            self.overflowed = true;
            return Err(io::Error::other("bounded JSON writer limit exceeded"));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
