use std::sync::OnceLock;

use crate::jwt::{JwtCodec, JwtConfig, JwtError};

static JWT_CODEC: OnceLock<JwtCodec> = OnceLock::new();

/// 单一进程级 JWT 签发/验证便捷入口。
///
/// 必须先成功调用 [`Self::init`]；第一个成功配置会永久保留，不能 reset、replace 或热轮换。
/// 需要多组 key 或不同验证规则时，本期 API 不提供可轮换实例入口，应在进程/服务边界隔离。
///
/// # Examples
///
/// ```
/// let _utils = axutils::JwtUtils;
/// ```
pub struct JwtUtils;

impl JwtUtils {
    /// 初始化 JWT 全局 codec。
    ///
    /// `JwtConfig` 会在此调用前完成本地校验；只有完整配置成功占用 `OnceLock` 后才算初始化
    /// 成功。再次初始化返回 [`JwtError::AlreadyInitialized`]，不会覆盖第一组 key。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtUtils, JwtValidation};
    ///
    /// let config = JwtConfig::new(
    ///     JwtAlgorithm::Hs256,
    ///     Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    ///     None,
    ///     JwtValidation::new(),
    /// )?;
    /// JwtUtils::init(config)?;
    /// # Ok::<(), axutils::JwtError>(())
    /// ```
    pub fn init(config: JwtConfig) -> Result<(), JwtError> {
        JWT_CODEC
            .set(JwtCodec::from_config(config))
            .map_err(|_| JwtError::AlreadyInitialized)
    }

    /// 返回全局 codec 是否已经成功初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::JwtUtils;
    ///
    /// let _initialized = JwtUtils::is_initialized();
    /// ```
    pub fn is_initialized() -> bool {
        JWT_CODEC.get().is_some()
    }

    /// 使用固定配置签发泛型 claims。
    ///
    /// claims 必须序列化为 JSON object，序列化 JSON 最多 32 KiB，最终 token 最多 64 KiB。
    /// 该示例只做函数类型检查，不执行不可重置的全局初始化或签发。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{JwtError, JwtUtils};
    ///
    /// #[derive(serde::Serialize)]
    /// struct Claims { exp: u64 }
    /// let _encode: fn(&Claims) -> Result<String, JwtError> = JwtUtils::encode::<Claims>;
    /// ```
    pub fn encode<T: serde::Serialize>(claims: &T) -> Result<String, JwtError> {
        JWT_CODEC
            .get()
            .ok_or(JwtError::NotInitialized)?
            .encode(claims)
    }

    /// 验证固定算法签名并将已验证 claims 反序列化为调用方类型。
    ///
    /// 该方法不接收按次验证策略；执行顺序固定为 token/Header/claims 预检、签名验证、标准
    /// claims 检查和泛型反序列化。该示例只做函数类型检查，不执行全局状态调用。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{JwtError, JwtUtils};
    ///
    /// #[derive(serde::Deserialize)]
    /// struct Claims { exp: u64 }
    /// let _decode: fn(&str) -> Result<Claims, JwtError> = JwtUtils::decode::<Claims>;
    /// ```
    pub fn decode<T: serde::de::DeserializeOwned>(token: &str) -> Result<T, JwtError> {
        JWT_CODEC
            .get()
            .ok_or(JwtError::NotInitialized)?
            .decode(token)
    }
}
