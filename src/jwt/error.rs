use std::fmt;

/// JWT 能力返回的脱敏、稳定错误分类。
///
/// 错误只包含固定字段名、长度、位置和限制值等元数据，不包含 token、claims、secret、
/// 私钥或第三方错误文本。该枚举未来可能增加变体，调用方匹配时应保留 wildcard 分支。
///
/// # Examples
///
/// ```
/// let error = axutils::JwtError::NotInitialized;
/// assert_eq!(error.to_string(), "JWT utility is not initialized");
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtError {
    /// 配置字段或资源限制无效。
    ///
    /// # Examples
    ///
    /// ```
    /// let error = axutils::JwtError::InvalidConfig { field: "keys" };
    /// assert!(error.to_string().contains("keys"));
    /// ```
    InvalidConfig { field: &'static str },
    /// key 角色、算法族或 key 参数不匹配。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::InvalidKey { kind: "rsa_pem" };
    /// ```
    InvalidKey { kind: &'static str },
    /// key 格式无法被当前后端承诺或解析。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::UnsupportedKeyFormat { kind: "verification_key" };
    /// ```
    UnsupportedKeyFormat { kind: &'static str },
    /// 配置没有签名 key。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::MissingSigningKey;
    /// ```
    MissingSigningKey,
    /// 配置没有验证 key。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::MissingVerificationKey;
    /// ```
    MissingVerificationKey,
    /// 全局入口尚未初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::NotInitialized;
    /// ```
    NotInitialized,
    /// 全局入口已经成功初始化，不能替换。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::AlreadyInitialized;
    /// ```
    AlreadyInitialized,
    /// token 超过固定 UTF-8 字节上限。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::TokenTooLarge {
    ///     length: 65 * 1024,
    ///     limit: 64 * 1024,
    /// };
    /// ```
    TokenTooLarge { length: usize, limit: usize },
    /// encode 的 claims JSON 超过固定字节上限。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::ClaimsTooLarge {
    ///     length: 33 * 1024,
    ///     limit: 32 * 1024,
    /// };
    /// ```
    ClaimsTooLarge { length: usize, limit: usize },
    /// 受控 Header 字段、值或结构无效。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::InvalidHeader { field: "alg" };
    /// ```
    InvalidHeader { field: &'static str },
    /// 已签名 claims 的标准字段类型、时间或 allowlist 不满足配置。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::InvalidClaim { claim: "exp" };
    /// ```
    InvalidClaim { claim: &'static str },
    /// 要求存在但 token 完全没有的标准 claim。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::MissingRequiredClaim { claim: "sub" };
    /// ```
    MissingRequiredClaim { claim: &'static str },
    /// token 的 payload、签名或调用方类型反序列化失败。
    ///
    /// # Examples
    ///
    /// ```
    /// let _error = axutils::JwtError::InvalidToken { segment: "signature" };
    /// ```
    InvalidToken { segment: &'static str },
}

impl fmt::Display for JwtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid JWT configuration: {field}")
            }
            Self::InvalidKey { kind } => write!(formatter, "invalid JWT key: {kind}"),
            Self::UnsupportedKeyFormat { kind } => {
                write!(formatter, "unsupported JWT key format: {kind}")
            }
            Self::MissingSigningKey => formatter.write_str("JWT signing key is not configured"),
            Self::MissingVerificationKey => {
                formatter.write_str("JWT verification key is not configured")
            }
            Self::NotInitialized => formatter.write_str("JWT utility is not initialized"),
            Self::AlreadyInitialized => formatter.write_str("JWT utility is already initialized"),
            Self::TokenTooLarge { length, limit } => {
                write!(
                    formatter,
                    "JWT token exceeds the {limit}-byte limit (length {length})"
                )
            }
            Self::ClaimsTooLarge { length, limit } => write!(
                formatter,
                "JWT claims exceed the {limit}-byte limit (length {length})"
            ),
            Self::InvalidHeader { field } => write!(formatter, "invalid JWT header: {field}"),
            Self::InvalidClaim { claim } => write!(formatter, "invalid JWT claim: {claim}"),
            Self::MissingRequiredClaim { claim } => {
                write!(formatter, "missing required JWT claim: {claim}")
            }
            Self::InvalidToken { segment } => write!(formatter, "invalid JWT token: {segment}"),
        }
    }
}

impl std::error::Error for JwtError {}
