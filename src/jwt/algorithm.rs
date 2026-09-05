use jsonwebtoken::Algorithm;

/// JWT 首期支持的签名算法。
///
/// 该枚举是本 crate 的稳定抽象，不把 `jsonwebtoken` 的算法类型暴露给调用方。
/// Header 中的算法始终必须与初始化时的值完全一致；不支持 `none`、ES512、P-521 和
/// Ed448。枚举使用 `#[non_exhaustive]`，调用方匹配时应保留 wildcard 分支。
///
/// # Examples
///
/// ```
/// use axutils::jwt::JwtAlgorithm;
///
/// let algorithm = JwtAlgorithm::Hs256;
/// assert_eq!(algorithm, JwtAlgorithm::Hs256);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JwtAlgorithm {
    /// HMAC with SHA-256.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Hs256;
    /// ```
    Hs256,
    /// HMAC with SHA-384.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Hs384;
    /// ```
    Hs384,
    /// HMAC with SHA-512.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Hs512;
    /// ```
    Hs512,
    /// RSA PKCS#1 v1.5 with SHA-256.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Rs256;
    /// ```
    Rs256,
    /// RSA PKCS#1 v1.5 with SHA-384.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Rs384;
    /// ```
    Rs384,
    /// RSA PKCS#1 v1.5 with SHA-512.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Rs512;
    /// ```
    Rs512,
    /// RSA-PSS with SHA-256.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Ps256;
    /// ```
    Ps256,
    /// RSA-PSS with SHA-384.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Ps384;
    /// ```
    Ps384,
    /// RSA-PSS with SHA-512.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Ps512;
    /// ```
    Ps512,
    /// ECDSA P-256 with SHA-256.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Es256;
    /// ```
    Es256,
    /// ECDSA P-384 with SHA-384.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Es384;
    /// ```
    Es384,
    /// Ed25519 EdDSA. Ed448 is not supported.
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtAlgorithm;
    ///
    /// let _algorithm = JwtAlgorithm::Ed25519;
    /// ```
    Ed25519,
}

/// The key family needed by a supported algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyFamily {
    Hmac,
    Rsa,
    Ec,
    Ed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EcCurve {
    P256,
    P384,
}

impl JwtAlgorithm {
    #[allow(unreachable_patterns)]
    pub(crate) fn backend(self) -> Option<Algorithm> {
        Some(match self {
            Self::Hs256 => Algorithm::HS256,
            Self::Hs384 => Algorithm::HS384,
            Self::Hs512 => Algorithm::HS512,
            Self::Rs256 => Algorithm::RS256,
            Self::Rs384 => Algorithm::RS384,
            Self::Rs512 => Algorithm::RS512,
            Self::Ps256 => Algorithm::PS256,
            Self::Ps384 => Algorithm::PS384,
            Self::Ps512 => Algorithm::PS512,
            Self::Es256 => Algorithm::ES256,
            Self::Es384 => Algorithm::ES384,
            Self::Ed25519 => Algorithm::EdDSA,
            _ => return None,
        })
    }

    #[allow(unreachable_patterns)]
    pub(crate) fn family(self) -> KeyFamily {
        match self {
            Self::Hs256 | Self::Hs384 | Self::Hs512 => KeyFamily::Hmac,
            Self::Rs256 | Self::Rs384 | Self::Rs512 | Self::Ps256 | Self::Ps384 | Self::Ps512 => {
                KeyFamily::Rsa
            }
            Self::Es256 | Self::Es384 => KeyFamily::Ec,
            Self::Ed25519 => KeyFamily::Ed,
            _ => KeyFamily::Hmac,
        }
    }

    #[allow(unreachable_patterns)]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Hs256 => "HS256",
            Self::Hs384 => "HS384",
            Self::Hs512 => "HS512",
            Self::Rs256 => "RS256",
            Self::Rs384 => "RS384",
            Self::Rs512 => "RS512",
            Self::Ps256 => "PS256",
            Self::Ps384 => "PS384",
            Self::Ps512 => "PS512",
            Self::Es256 => "ES256",
            Self::Es384 => "ES384",
            Self::Ed25519 => "EdDSA",
            _ => "unknown",
        }
    }

    pub(crate) fn hmac_minimum_secret_length(self) -> Option<usize> {
        match self {
            Self::Hs256 => Some(32),
            Self::Hs384 => Some(48),
            Self::Hs512 => Some(64),
            _ => None,
        }
    }
}
