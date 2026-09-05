use std::sync::OnceLock;

use super::{JwtCodec, JwtConfig, JwtError};
#[cfg(feature = "tracing")]
use crate::telemetry::jwt as jwt_trace;

static JWT_CODEC: OnceLock<JwtCodec> = OnceLock::new();

/// 单一进程级 JWT 签发/验证便捷入口。
///
/// 必须先成功调用 [`Self::init`]；第一个成功配置会永久保留，不能 reset、replace 或热轮换。
/// 需要多组 key 或不同验证规则时，请直接持有多个 [`JwtCodec`] 实例。
pub struct JwtUtils;

impl JwtUtils {
    /// 初始化 JWT 全局 codec。
    ///
    /// `JwtConfig` 会在此调用前完成本地校验；只有完整配置成功占用 `OnceLock` 后才算初始化
    /// 成功。再次初始化返回 [`JwtError::AlreadyInitialized`]，不会覆盖第一组 key。
    pub fn init(config: JwtConfig) -> Result<(), JwtError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = JWT_CODEC
            .set(JwtCodec::new(config))
            .map_err(|_| JwtError::AlreadyInitialized);
        #[cfg(feature = "tracing")]
        jwt_trace::record_client_init(&result, started);
        result
    }

    /// 返回全局 codec 是否已经成功初始化。
    pub fn is_initialized() -> bool {
        JWT_CODEC.get().is_some()
    }

    /// 返回已初始化的全局 codec。
    ///
    /// 未初始化时返回 [`JwtError::NotInitialized`]。返回值只暴露安全的签发和验证操作，不提供
    /// key、token 或原始验证状态的读取入口。
    pub fn codec() -> Result<&'static JwtCodec, JwtError> {
        JWT_CODEC.get().ok_or(JwtError::NotInitialized)
    }
}
