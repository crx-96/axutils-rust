use std::fmt;

use super::{EcCurve, JwtAlgorithm, JwtError, JwtSigningKey, JwtVerificationKey, KeyFamily};

pub(crate) const MAX_LEEWAY: u64 = 86_400;
pub(crate) const MAX_CLAIM_STRING_BYTES: usize = 4 * 1024;
pub(crate) const MAX_ALLOWLIST_ITEMS: usize = 32;

/// JWT 标准 claims 验证规则。
///
/// `new()` 默认要求并校验 `exp`，不校验但不强制 `nbf`，也不强制 `aud`、`iss` 或 `sub`；
/// 默认 leeway 为 60 秒。`require_*` 只控制存在性，`with_validate_*` 只控制时间语义校验。
/// 所有 builder 都返回新的配置值，不提供按次 decode 修改验证规则的入口。
///
/// # Examples
///
/// ```
/// use axutils::jwt::JwtValidation;
///
/// let _validation = JwtValidation::new();
/// ```
pub struct JwtValidation {
    pub(crate) validate_exp: bool,
    pub(crate) require_exp: bool,
    pub(crate) validate_nbf: bool,
    pub(crate) require_nbf: bool,
    pub(crate) require_aud: bool,
    pub(crate) require_iss: bool,
    pub(crate) require_sub: bool,
    pub(crate) audience: Option<Vec<String>>,
    pub(crate) issuers: Option<Vec<String>>,
    pub(crate) subject: Option<String>,
    pub(crate) leeway: u64,
}

impl fmt::Debug for JwtValidation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtValidation")
            .field("validate_exp", &self.validate_exp)
            .field("require_exp", &self.require_exp)
            .field("validate_nbf", &self.validate_nbf)
            .field("require_nbf", &self.require_nbf)
            .field("require_aud", &self.require_aud)
            .field("require_iss", &self.require_iss)
            .field("require_sub", &self.require_sub)
            .field(
                "audience_count",
                &self.audience.as_ref().map_or(0, Vec::len),
            )
            .field("issuer_count", &self.issuers.as_ref().map_or(0, Vec::len))
            .field("has_subject", &self.subject.is_some())
            .field("leeway", &self.leeway)
            .finish()
    }
}

impl fmt::Display for JwtValidation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JwtValidation(<redacted allowlists>)")
    }
}

impl JwtValidation {
    /// 创建默认验证规则：要求并校验 `exp`，leeway 为 60 秒。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new();
    /// let _ = validation;
    /// ```
    pub fn new() -> Self {
        Self {
            validate_exp: true,
            require_exp: true,
            validate_nbf: false,
            require_nbf: false,
            require_aud: false,
            require_iss: false,
            require_sub: false,
            audience: None,
            issuers: None,
            subject: None,
            leeway: 60,
        }
    }

    /// 设置是否比较 `exp` 与当前 Unix seconds。
    ///
    /// 字段存在时始终检查为非负整数；传入 `false` 只关闭时间比较。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let _validation = JwtValidation::new().with_validate_exp(false);
    /// ```
    pub fn with_validate_exp(mut self, validate: bool) -> Self {
        self.validate_exp = validate;
        self
    }

    /// 设置是否要求 `exp` 字段存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_require_exp(false);
    /// ```
    pub fn with_require_exp(mut self, require: bool) -> Self {
        self.require_exp = require;
        self
    }

    /// 设置是否比较 `nbf` 与当前 Unix seconds。
    ///
    /// `nbf` 存在时始终检查为非负整数；传入 `false` 只关闭时间比较。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_validate_nbf(true);
    /// ```
    pub fn with_validate_nbf(mut self, validate: bool) -> Self {
        self.validate_nbf = validate;
        self
    }

    /// 设置是否要求 `nbf` 字段存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_require_nbf(true);
    /// ```
    pub fn with_require_nbf(mut self, require: bool) -> Self {
        self.require_nbf = require;
        self
    }

    /// 设置是否要求 `aud` 字段存在。
    ///
    /// 如果没有配置 audience allowlist，只检查 token 的 `aud` 是非空字符串或非空字符串数组。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_require_aud(true);
    /// ```
    pub fn with_require_aud(mut self, require: bool) -> Self {
        self.require_aud = require;
        self
    }

    /// 设置是否要求 `iss` 字段存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_require_iss(true);
    /// ```
    pub fn with_require_iss(mut self, require: bool) -> Self {
        self.require_iss = require;
        self
    }

    /// 设置是否要求 `sub` 字段存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtValidation;
    ///
    /// let _validation = JwtValidation::new().with_require_sub(true);
    /// ```
    pub fn with_require_sub(mut self, require: bool) -> Self {
        self.require_sub = require;
        self
    }

    /// 设置允许的单个 audience 值。
    ///
    /// 空字符串、控制字符、重复值和超过 32 项/4 KiB 限制返回 `InvalidConfig`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_audience("api.example.com")?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_audience(self, value: impl AsRef<str>) -> Result<Self, JwtError> {
        self.with_audiences(std::iter::once(value))
    }

    /// 设置 audience allowlist。
    ///
    /// 集合必须非空；空字符串、控制字符、重复值和超过 32 项/单项 4 KiB 限制返回
    /// `InvalidConfig`。输入顺序不影响匹配语义，但保留首次出现顺序作为内部确定性表示。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_audiences(["api", "worker"])?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_audiences<I, S>(mut self, values: I) -> Result<Self, JwtError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.audience = Some(validate_allowlist(values, "audience")?);
        Ok(self)
    }

    /// 设置允许的单个 issuer 值。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_issuer("issuer.example.com")?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_issuer(self, value: impl AsRef<str>) -> Result<Self, JwtError> {
        self.with_issuers(std::iter::once(value))
    }

    /// 设置 issuer allowlist。
    ///
    /// 集合限制与 [`Self::with_audiences`] 相同；token 的 `iss` 仍只接受单个字符串，不接受数组。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_issuers(["issuer-a", "issuer-b"])?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_issuers<I, S>(mut self, values: I) -> Result<Self, JwtError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.issuers = Some(validate_allowlist(values, "issuers")?);
        Ok(self)
    }

    /// 设置期望的单个 subject。
    ///
    /// 空字符串、控制字符和超过 4 KiB 返回 `InvalidConfig`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_subject("user-42")?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_subject(mut self, value: impl AsRef<str>) -> Result<Self, JwtError> {
        self.subject = Some(validate_string(value.as_ref(), "subject")?);
        Ok(self)
    }

    /// 设置时间比较允许的时钟偏差，最大为 86,400 秒。
    ///
    /// `exp + leeway`、`now + leeway` 使用 checked arithmetic，溢出时 token 返回
    /// `InvalidClaim`，不会饱和到另一侧边界。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtValidation};
    ///
    /// let validation = JwtValidation::new().with_leeway(120)?;
    /// let _ = validation;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn with_leeway(mut self, leeway: u64) -> Result<Self, JwtError> {
        if leeway > MAX_LEEWAY {
            return Err(JwtError::InvalidConfig { field: "leeway" });
        }
        self.leeway = leeway;
        Ok(self)
    }

    pub(crate) fn validate_resources(&self) -> Result<(), JwtError> {
        if self.leeway > MAX_LEEWAY {
            return Err(JwtError::InvalidConfig { field: "leeway" });
        }
        if let Some(values) = &self.audience {
            validate_stored_allowlist(values, "audience")?;
        }
        if let Some(values) = &self.issuers {
            validate_stored_allowlist(values, "issuers")?;
        }
        if let Some(value) = &self.subject {
            validate_string(value, "subject")?;
        }
        Ok(())
    }
}

impl Default for JwtValidation {
    fn default() -> Self {
        Self::new()
    }
}

/// 一次 JWT codec 初始化所需的算法、key 和验证配置。
///
/// `JwtConfig` 不提供 encode/decode 方法，也不实现 `Clone`；成功传给 `JwtUtils::init` 后，
/// 内部 key 与验证规则会绑定到进程生命周期。至少需要一个签名或验证 key。
///
/// # Examples
///
/// ```
/// use axutils::jwt::{JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation};
///
/// let key = JwtSigningKey::from_hmac_secret([0x11; 32])?;
/// let _config = JwtConfig::new(
///     JwtAlgorithm::Hs256,
///     Some(key),
///     None,
///     JwtValidation::new(),
/// )?;
/// # Ok::<(), JwtError>(())
/// ```
pub struct JwtConfig {
    pub(crate) algorithm: JwtAlgorithm,
    pub(crate) signing_key: Option<JwtSigningKey>,
    pub(crate) verification_key: Option<JwtVerificationKey>,
    pub(crate) validation: JwtValidation,
}

impl fmt::Debug for JwtConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtConfig")
            .field("algorithm", &self.algorithm)
            .field("has_signing_key", &self.signing_key.is_some())
            .field("has_verification_key", &self.verification_key.is_some())
            .field("validation", &self.validation)
            .finish()
    }
}

impl fmt::Display for JwtConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JwtConfig(algorithm={:?}, signing_key={}, verification_key={})",
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

/// 已校验配置拆出的内部所有权部分。
pub(crate) struct JwtConfigParts {
    pub(crate) algorithm: JwtAlgorithm,
    pub(crate) signing_key: Option<JwtSigningKey>,
    pub(crate) verification_key: Option<JwtVerificationKey>,
    pub(crate) validation: JwtValidation,
}

impl JwtConfig {
    /// 构造一次初始化配置并完成算法/key、资源和 validation allowlist 校验。
    ///
    /// `signing_key` 和 `verification_key` 可以分别只提供一侧，以支持只签发或只验证的服务；
    /// 两者同时缺失返回 `InvalidConfig`。HMAC secret 最小长度为 HS256=32、HS384=48、
    /// HS512=64 字节，RSA modulus 小于 2048 bit 被拒绝，key 输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey};
    ///
    /// let signing = JwtSigningKey::from_hmac_secret([0x11; 32])?;
    /// let verifying = JwtVerificationKey::from_hmac_secret([0x11; 32])?;
    /// let config = JwtConfig::new(
    ///     JwtAlgorithm::Hs256,
    ///     Some(signing),
    ///     Some(verifying),
    ///     JwtValidation::new(),
    /// )?;
    /// let _ = format!("{config:?}");
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn new(
        algorithm: JwtAlgorithm,
        signing_key: Option<JwtSigningKey>,
        verification_key: Option<JwtVerificationKey>,
        validation: JwtValidation,
    ) -> Result<Self, JwtError> {
        if signing_key.is_none() && verification_key.is_none() {
            return Err(JwtError::InvalidConfig { field: "keys" });
        }
        if algorithm.backend().is_none() {
            return Err(JwtError::InvalidConfig { field: "algorithm" });
        }
        validation.validate_resources()?;
        let family = algorithm.family();
        if let Some(key) = signing_key.as_ref() {
            validate_key_for_algorithm(
                key.family(),
                key.rsa_modulus_bits(),
                key.ec_curve(),
                key.is_public_rsa_der(),
                family,
                algorithm,
                true,
            )?;
        }
        if let Some(key) = verification_key.as_ref() {
            validate_key_for_algorithm(
                key.family(),
                key.rsa_modulus_bits(),
                key.ec_curve(),
                key.is_public_rsa_der(),
                family,
                algorithm,
                false,
            )?;
        }
        if let Some(minimum) = algorithm.hmac_minimum_secret_length() {
            if let Some(key) = signing_key.as_ref() {
                if key.backend().as_bytes().len() < minimum {
                    return Err(JwtError::InvalidKey {
                        kind: "hmac_secret_length",
                    });
                }
            }
            if let Some(key) = verification_key.as_ref() {
                if key
                    .backend()
                    .try_get_as_bytes()
                    .map_or(true, |bytes| bytes.len() < minimum)
                {
                    return Err(JwtError::InvalidKey {
                        kind: "hmac_secret_length",
                    });
                }
            }
        }
        Ok(Self {
            algorithm,
            signing_key,
            verification_key,
            validation,
        })
    }

    pub(crate) fn into_parts(self) -> JwtConfigParts {
        JwtConfigParts {
            algorithm: self.algorithm,
            signing_key: self.signing_key,
            verification_key: self.verification_key,
            validation: self.validation,
        }
    }
}

fn validate_key_for_algorithm(
    key_family: KeyFamily,
    modulus_bits: Option<usize>,
    ec_curve: Option<EcCurve>,
    public_rsa_der: bool,
    expected_family: KeyFamily,
    algorithm: JwtAlgorithm,
    signing: bool,
) -> Result<(), JwtError> {
    if key_family != expected_family {
        return Err(JwtError::InvalidKey {
            kind: if signing {
                "signing_algorithm_key"
            } else {
                "verification_algorithm_key"
            },
        });
    }
    if signing && public_rsa_der {
        return Err(JwtError::InvalidKey {
            kind: "signing_key_role",
        });
    }
    if expected_family == KeyFamily::Rsa {
        if let Some(bits) = modulus_bits {
            if bits < 2048 {
                return Err(JwtError::InvalidKey {
                    kind: "rsa_modulus_bits",
                });
            }
        }
    }
    if expected_family == KeyFamily::Ec {
        let expected_curve = match algorithm {
            JwtAlgorithm::Es256 => Some(EcCurve::P256),
            JwtAlgorithm::Es384 => Some(EcCurve::P384),
            _ => None,
        };
        if let (Some(actual), Some(expected)) = (ec_curve, expected_curve) {
            if actual != expected {
                return Err(JwtError::InvalidKey { kind: "ec_curve" });
            }
        }
    }
    if algorithm.backend().is_none() {
        return Err(JwtError::InvalidConfig { field: "algorithm" });
    }
    Ok(())
}

fn validate_allowlist<I, S>(values: I, field: &'static str) -> Result<Vec<String>, JwtError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut result = Vec::new();
    for value in values {
        if result.len() >= MAX_ALLOWLIST_ITEMS {
            return Err(JwtError::InvalidConfig { field });
        }
        let value = validate_string(value.as_ref(), field)?;
        if result.iter().any(|existing| existing == &value) {
            return Err(JwtError::InvalidConfig { field });
        }
        result.push(value);
    }
    if result.is_empty() {
        return Err(JwtError::InvalidConfig { field });
    }
    Ok(result)
}

fn validate_stored_allowlist(values: &[String], field: &'static str) -> Result<(), JwtError> {
    if values.is_empty() || values.len() > MAX_ALLOWLIST_ITEMS {
        return Err(JwtError::InvalidConfig { field });
    }
    for (index, value) in values.iter().enumerate() {
        validate_string(value, field)?;
        if values[..index].iter().any(|existing| existing == value) {
            return Err(JwtError::InvalidConfig { field });
        }
    }
    Ok(())
}

fn validate_string(value: &str, field: &'static str) -> Result<String, JwtError> {
    if value.is_empty()
        || value.len() > MAX_CLAIM_STRING_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(JwtError::InvalidConfig { field });
    }
    Ok(value.to_owned())
}
