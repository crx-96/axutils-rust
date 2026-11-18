use jsonwebtoken::Validation as BackendValidation;
use serde::de::{DeserializeOwned, IgnoredAny};

use super::super::{claims, clock, header};
use super::error_map;
use super::{JwtCodec, MAX_TOKEN_BYTES};
use crate::jwt::JwtError;

impl JwtCodec {
    /// 验证固定算法签名并反序列化已验证的 claims。
    ///
    /// 执行顺序固定为 token/Header/claims 预检、签名验证、标准 claims 检查和泛型反序列化。
    /// token 最多 64 KiB；未配置 verification key 时返回
    /// [`JwtError::MissingVerificationKey`]。启用 `exp` 或 `nbf` 校验时使用当前系统 Unix 时钟，
    /// 本 API 不提供时钟注入。
    pub fn decode<T: DeserializeOwned>(&self, token: &str) -> Result<T, JwtError> {
        self.decode_inner(token, None)
    }

    /// 用确定的 Unix 时间测试时间边界，验签与解析和公开路径共用。
    #[cfg(test)]
    pub(crate) fn decode_at<T: DeserializeOwned>(
        &self,
        token: &str,
        now: u64,
    ) -> Result<T, JwtError> {
        self.decode_inner(token, Some(now))
    }

    /// 先约束未验证输入，随后验证签名和标准项，最后才执行调用方的反序列化。
    fn decode_inner<T: DeserializeOwned>(
        &self,
        token: &str,
        fixed_now: Option<u64>,
    ) -> Result<T, JwtError> {
        if token.len() > MAX_TOKEN_BYTES {
            return Err(JwtError::TokenTooLarge {
                length: token.len(),
                limit: MAX_TOKEN_BYTES,
            });
        }
        // 三个段必须完整；先拒绝结构、编码或资源预算不合法的输入。
        let mut parts = token.split('.');
        let header = parts
            .next()
            .ok_or(JwtError::InvalidHeader { field: "segments" })?;
        let payload = parts
            .next()
            .ok_or(JwtError::InvalidHeader { field: "segments" })?;
        let signature = parts
            .next()
            .ok_or(JwtError::InvalidHeader { field: "segments" })?;
        if parts.next().is_some() {
            return Err(JwtError::InvalidHeader { field: "segments" });
        }
        if header.is_empty() {
            return Err(JwtError::InvalidHeader { field: "missing" });
        }
        if payload.is_empty() || signature.is_empty() {
            return Err(JwtError::InvalidHeader { field: "segments" });
        }

        header::validate_header_segment(header, self.algorithm)?;
        let decoded_payload = claims::decode_payload(payload)?;
        claims::preflight_claims(&decoded_payload)?;

        // 后端只负责固定算法验签，标准项保留本库的严格类型与时间规则。
        let verification_key = self
            .verification_key
            .as_ref()
            .ok_or(JwtError::MissingVerificationKey)?;
        verification_key.validate_for_decode()?;
        let algorithm = self
            .algorithm
            .backend()
            .ok_or(JwtError::InvalidConfig { field: "algorithm" })?;
        let mut backend_validation = BackendValidation::new(algorithm);
        backend_validation.algorithms.clear();
        backend_validation.algorithms.push(algorithm);
        backend_validation.required_spec_claims.clear();
        backend_validation.validate_exp = false;
        backend_validation.validate_nbf = false;
        backend_validation.validate_aud = false;
        backend_validation.aud = None;
        backend_validation.iss = None;
        backend_validation.sub = None;

        // 忽略后端的 claims 返回值。验签成功后，两个读取方都使用预检过的 payload 字节。
        jsonwebtoken::decode::<IgnoredAny>(token, verification_key.backend(), &backend_validation)
            .map_err(|error| error_map::map_decode_error(&error, verification_key.family()))?;
        let now = if self.validation.validate_exp || self.validation.validate_nbf {
            Some(match fixed_now {
                Some(now) => now,
                None => clock::now_seconds()?,
            })
        } else {
            None
        };
        claims::validate_standard_claims(&decoded_payload, &self.validation, now)?;
        // 验签和标准项检查均已成功，按目标类型读取同一 payload 的原始数字。
        serde_json::from_slice(&decoded_payload)
            .map_err(|_| JwtError::InvalidToken { segment: "claims" })
    }
}
