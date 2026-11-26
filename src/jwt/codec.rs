use std::{fmt, sync::OnceLock};

use jsonwebtoken::crypto::CryptoProvider;

use super::{JwtAlgorithm, JwtConfigParts, JwtError};

mod decode;
mod encode;
mod error_map;
#[cfg(test)]
mod tests;

/// 编解码允许的最大紧凑 JWT 字节数。
pub(crate) const MAX_TOKEN_BYTES: usize = 64 * 1024;

/// 在调用后端签发或验签前确认进程级 provider 已就绪。
fn ensure_crypto_provider() -> Result<(), JwtError> {
    // 双 provider 且宿主尚未选择时，保留选择机会，下一次操作重新查询。
    CryptoProvider::try_get_default()
        .map(|_| ())
        .ok_or(JwtError::InvalidConfig {
            field: "crypto_provider",
        })
}

/// 已完成配置校验的 JWT codec。
///
/// 通过消费 [`super::JwtConfig`] 创建。每个实例独立持有 key 与验证规则，签发和验签使用
/// `jsonwebtoken` 的进程级加密 provider。
pub struct JwtCodec {
    /// 签发和验签共用的固定算法。
    algorithm: JwtAlgorithm,
    /// 签发使用的可选私钥或共享密钥。
    signing_key: Option<super::JwtSigningKey>,
    /// 验签使用的可选公钥或共享密钥。
    verification_key: Option<super::JwtVerificationKey>,
    /// 本实例的算法和密钥固定，首次验签时缓存公钥格式检查结果（包括失败）。
    verification_key_format: OnceLock<Result<(), JwtError>>,
    /// 签名通过后应用的标准 claims 规则。
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
            verification_key_format: OnceLock::new(),
            validation,
        }
    }
}
