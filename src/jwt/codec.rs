use std::fmt;

use super::{JwtAlgorithm, JwtConfigParts};

mod decode;
mod encode;
mod error_map;
#[cfg(test)]
mod tests;

pub(crate) const MAX_TOKEN_BYTES: usize = 64 * 1024;

/// 已完成配置校验的 JWT codec。
///
/// 通过消费 [`super::JwtConfig`] 创建。codec 拥有 key 与验证规则，不暴露其内容；同一实例可在
/// 不共享进程级状态的情况下签发和验证 token。
pub struct JwtCodec {
    algorithm: JwtAlgorithm,
    signing_key: Option<super::JwtSigningKey>,
    verification_key: Option<super::JwtVerificationKey>,
    validation: super::JwtValidation,
}

impl fmt::Debug for JwtCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtCodec")
            .field("algorithm", &self.algorithm)
            .field("has_signing_key", &self.signing_key.is_some())
            .field("has_verification_key", &self.verification_key.is_some())
            .field("validation", &self.validation)
            .finish()
    }
}

impl fmt::Display for JwtCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JwtCodec(algorithm={:?}, signing_key={}, verification_key={})",
            self.algorithm,
            if self.signing_key.is_some() {
                "configured"
            } else {
                "absent"
            },
            if self.verification_key.is_some() {
                "configured"
            } else {
                "absent"
            }
        )
    }
}

impl JwtCodec {
    /// 消费已校验的配置创建实例级 codec。
    ///
    /// [`super::JwtConfig::new`] 已在创建配置时验证算法、key 与 claims 规则的组合。本构造不访问
    /// 网络、文件或系统时钟，也不会注册全局状态。
    #[must_use]
    pub fn new(config: super::JwtConfig) -> Self {
        let JwtConfigParts {
            algorithm,
            signing_key,
            verification_key,
            validation,
        } = config.into_parts();
        Self {
            algorithm,
            signing_key,
            verification_key,
            validation,
        }
    }
}
