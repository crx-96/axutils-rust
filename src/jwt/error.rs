use std::fmt;

/// JWT 能力返回的脱敏、稳定错误分类。
///
/// 错误只包含固定字段名、长度、位置和限制值等元数据，不包含 token、claims、secret、
/// 私钥或第三方错误文本。该枚举未来可能增加变体，调用方匹配时应保留 wildcard 分支。
///
/// # Examples
///
/// ```
/// use axutils::jwt::JwtError;
///
/// let error = JwtError::NotInitialized;
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
    /// use axutils::jwt::JwtError;
    ///
    /// let error = JwtError::InvalidConfig { field: "keys" };
    /// assert!(error.to_string().contains("keys"));
    /// ```
    InvalidConfig {
        /// 无效配置的固定字段分类，例如 `"keys"`、`"algorithm"` 或 `"leeway"`；不包含
        /// secret、私钥、token 或其他配置值。
        ///
        /// 调用方可按字段名匹配配置错误，但应保留 wildcard 以兼容未来分类。
        field: &'static str,
    },
    /// key 角色、算法族或 key 参数不匹配。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::InvalidKey { kind: "rsa_pem" };
    /// ```
    InvalidKey {
        /// key 错误的固定分类，例如算法、角色或参数类别；不包含 key、密码或 PEM 文本。
        ///
        /// 调用方可按分类决定修复配置还是拒绝请求，但不能把该字段当作 key 内容。
        kind: &'static str,
    },
    /// key 格式无法被当前后端承诺或解析。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::UnsupportedKeyFormat { kind: "verification_key" };
    /// ```
    UnsupportedKeyFormat {
        /// 当前后端不支持或无法解析的 key 格式固定分类；不包含 key 材料或第三方错误文本。
        ///
        /// 调用方可按格式类别提示使用受支持的 key 表示，并应保留 wildcard 分支。
        kind: &'static str,
    },
    /// 配置没有签名 key。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::MissingSigningKey;
    /// ```
    MissingSigningKey,
    /// 配置没有验证 key。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::MissingVerificationKey;
    /// ```
    MissingVerificationKey,
    /// 全局入口尚未初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::NotInitialized;
    /// ```
    NotInitialized,
    /// 全局入口已经成功初始化，不能替换。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::AlreadyInitialized;
    /// ```
    AlreadyInitialized,
    /// token 超过固定 UTF-8 字节上限。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::TokenTooLarge {
    ///     length: 65 * 1024,
    ///     limit: 64 * 1024,
    /// };
    /// ```
    TokenTooLarge {
        /// token 的 UTF-8 字节长度，不是字符数，也不包含 token 内容。
        length: usize,
        /// token 允许的最大 UTF-8 字节数；与 `length` 使用相同单位。
        limit: usize,
    },
    /// encode 的 claims JSON 超过固定字节上限。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::ClaimsTooLarge {
    ///     length: 33 * 1024,
    ///     limit: 32 * 1024,
    /// };
    /// ```
    ClaimsTooLarge {
        /// encode 前 claims JSON 的 UTF-8 字节长度，不包含 JSON 或 claim 值本身。
        length: usize,
        /// claims JSON 允许的最大 UTF-8 字节数；与 `length` 使用相同单位。
        limit: usize,
    },
    /// 受控 Header 字段、值或结构无效。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::InvalidHeader { field: "alg" };
    /// ```
    InvalidHeader {
        /// Header 结构中无效部分的固定分类，例如 `"segments"`、`"alg"`、`"json"` 或
        /// `"base64"`；不包含 Header 值或 token 内容。
        ///
        /// 调用方可按分类决定拒绝 token 或记录脱敏诊断，不应记录原始 Header。
        field: &'static str,
    },
    /// 已签名 claims 的标准字段类型、时间或 allowlist 不满足配置。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::InvalidClaim { claim: "exp" };
    /// ```
    InvalidClaim {
        /// 无效标准 claim 的名称，例如 `"exp"`、`"nbf"`、`"aud"`、`"iss"` 或 `"sub"`；
        /// 只提供名称，不包含 claim 值。
        ///
        /// 调用方可按 claim 名称匹配校验失败类型，但不能据此推断或记录 payload 内容。
        claim: &'static str,
    },
    /// 要求存在但 token 完全没有的标准 claim。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::MissingRequiredClaim { claim: "sub" };
    /// ```
    MissingRequiredClaim {
        /// token 缺失的标准 claim 名称；只提供固定名称，不包含 token 或其他 claims。
        ///
        /// 调用方可按名称决定补齐声明或拒绝 token，并应保留 wildcard 以兼容新 claim。
        claim: &'static str,
    },
    /// token 的 payload、签名或调用方类型反序列化失败。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtError;
    ///
    /// let _error = JwtError::InvalidToken { segment: "signature" };
    /// ```
    InvalidToken {
        /// token 处理阶段的固定分类，例如 `"claims"`、`"signature"`、`"token"` 或
        /// `"encode"`；不包含 token、payload、签名或第三方错误文本。
        ///
        /// 调用方可按阶段区分输入格式与签名处理失败，但不应记录原始 token。
        segment: &'static str,
    },
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
