use std::io;

use jsonwebtoken::Header as BackendHeader;
use serde::Serialize;
use serde_json::{value::RawValue, Serializer as JsonSerializer};

use super::super::claims::{self, MAX_CLAIMS_BYTES};
use super::{ensure_crypto_provider, error_map, JwtCodec, MAX_TOKEN_BYTES};
use crate::jwt::JwtError;

impl JwtCodec {
    /// 用固定算法签发泛型 claims。
    ///
    /// claims 必须是受限 JSON object：序列化结果最多 32 KiB，并经过重复键、深度、成员和数组
    /// 预算预检；生成的 token 最多 64 KiB。未配置 signing key 时返回
    /// [`JwtError::MissingSigningKey`]。返回的 token 不是加密内容，调用方不得在日志或错误中记录
    /// 其中的敏感 claims。签发时无法确定加密 provider 则返回 `InvalidConfig { field: "crypto_provider" }`。
    pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String, JwtError> {
        // 没有签名密钥时直接返回，避免执行调用方的序列化逻辑。
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
        // 结构和资源预算直接检查待签发的 JSON。后端按原文嵌入，只序列化一次业务类型。
        let serialized = writer.bytes;
        claims::preflight_claims(&serialized)?;
        let raw: &RawValue = serde_json::from_slice(&serialized)
            .map_err(|_| JwtError::InvalidToken { segment: "claims" })?;
        let algorithm = self
            .algorithm
            .backend()
            .ok_or(JwtError::InvalidConfig { field: "algorithm" })?;
        let header = BackendHeader::new(algorithm);
        ensure_crypto_provider()?;
        let token = jsonwebtoken::encode(&header, &raw, signing_key.backend())
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

/// 捕获调用方一次序列化的原文，并为超限错误保留至多一个额外字节。
struct BoundedJsonWriter {
    /// 实际捕获的 JSON 字节，长度至多为 limit + 1。
    bytes: Vec<u8>,
    /// 可签发的最大字节数。
    limit: usize,
    /// 区分容量超限与调用方序列化错误，不向外传递原始错误。
    overflowed: bool,
}

impl BoundedJsonWriter {
    /// 按需扩容，小 claims 无需预留整个预算。
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(limit.min(1024)),
            limit,
            overflowed: false,
        }
    }
}

impl io::Write for BoundedJsonWriter {
    /// 写入预算内的片段，超限时留下长度证据并终止序列化。
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

    /// 字节已直接写入内存，无待刷新的数据。
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
