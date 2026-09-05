use std::fmt;

use jsonwebtoken::EncodingKey;

use super::{
    super::{EcCurve, JwtError, KeyFamily},
    shared::{
        ec_curve_from_private_der, map_backend_key_error, rsa_der_kind, rsa_modulus_bits,
        validate_key_size, validate_pem_label, RsaDerKind, MAX_HMAC_SECRET_BYTES,
    },
};

/// 拥有型 JWT 签名 key。
///
/// 该类型不实现 `Clone` 或可读取 key 内容的 API，`Display`/`Debug` 只显示 key family。
/// HMAC 使用原始 secret bytes；非对称签名只接受对应算法族的私钥格式。opaque DER 的部分
/// 结构检查会延迟到 encode 阶段。
///
/// # Examples
///
/// ```
/// use axutils::jwt::{JwtError, JwtSigningKey};
///
/// let _key = JwtSigningKey::from_hmac_secret([0x11; 32])?;
/// # Ok::<(), JwtError>(())
/// ```
pub struct JwtSigningKey {
    family: KeyFamily,
    backend: EncodingKey,
    rsa_modulus_bits: Option<usize>,
    ec_curve: Option<EcCurve>,
    rsa_der_kind: RsaDerKind,
}

impl fmt::Debug for JwtSigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtSigningKey")
            .field("family", &self.family)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl fmt::Display for JwtSigningKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "JwtSigningKey({:?})", self.family)
    }
}

impl JwtSigningKey {
    /// 从原始 HMAC secret 构造签名 key。
    ///
    /// secret 不能为空且最多 4096 字节；不同 HS 算法所需的最小长度在
    /// [`super::super::JwtConfig::new`] 中按算法检查。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtSigningKey};
    ///
    /// let key = JwtSigningKey::from_hmac_secret([0x11; 32])?;
    /// let _ = format!("{key:?}");
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn from_hmac_secret(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        if bytes.is_empty() || bytes.len() > MAX_HMAC_SECRET_BYTES {
            return Err(JwtError::InvalidKey {
                kind: "hmac_secret",
            });
        }
        Ok(Self {
            family: KeyFamily::Hmac,
            backend: EncodingKey::from_secret(bytes),
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 RSA PKCS#1 或 PKCS#8 私钥 PEM 构造签名 key。
    ///
    /// 只接受 `RSA PRIVATE KEY` 或包含 RSA 私钥的 `PRIVATE KEY` label；公钥 label、证书、
    /// EC 私钥和 Ed25519 私钥都会被拒绝。PEM 输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let pem = std::fs::read("rsa-private.pem")?;
    /// let _key = JwtSigningKey::from_rsa_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_rsa_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "rsa_pem")?;
        validate_pem_label(bytes, &["RSA PRIVATE KEY", "PRIVATE KEY"], "rsa_pem", true)?;
        let backend = EncodingKey::from_rsa_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "rsa_pem"))?;
        let rsa_modulus_bits = rsa_modulus_bits(backend.as_bytes());
        Ok(Self {
            family: KeyFamily::Rsa,
            backend,
            rsa_modulus_bits,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Private,
        })
    }

    /// 从 RSA PKCS#1 私钥 DER 构造签名 key。
    ///
    /// 构造器只执行非空和大小检查；DER 的结构错误会在 encode 时返回 key-format 错误。
    /// 输入语义仍固定为 RSA PKCS#1 私钥 DER；opaque bytes 只表示延迟结构校验，不扩大为
    /// PKCS#8、SPKI 或其他未验证 DER 格式的支持范围。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let _ = JwtSigningKey::from_rsa_der([0u8; 1]);
    /// ```
    pub fn from_rsa_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "rsa_der")?;
        let backend = EncodingKey::from_rsa_der(bytes);
        let rsa_modulus_bits = rsa_modulus_bits(backend.as_bytes());
        Ok(Self {
            family: KeyFamily::Rsa,
            backend,
            rsa_modulus_bits,
            ec_curve: None,
            rsa_der_kind: rsa_der_kind(bytes),
        })
    }

    /// 从 EC PKCS#8 私钥 PEM 构造 ECDSA 签名 key。
    ///
    /// `EC PRIVATE KEY` SEC1 label 不在承诺范围内；只接受 `PRIVATE KEY`，输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let pem = std::fs::read("ec-private-pkcs8.pem")?;
    /// let _key = JwtSigningKey::from_ec_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_ec_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ec_pem")?;
        validate_pem_label(bytes, &["PRIVATE KEY"], "ec_pem", true)?;
        let backend = EncodingKey::from_ec_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "ec_pem"))?;
        let ec_curve = ec_curve_from_private_der(backend.as_bytes());
        Ok(Self {
            family: KeyFamily::Ec,
            backend,
            rsa_modulus_bits: None,
            ec_curve,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 EC PKCS#8 私钥 DER 构造 ECDSA 签名 key。
    ///
    /// DER 结构错误延迟到 encode 阶段；输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let _ = JwtSigningKey::from_ec_der([0u8; 1]);
    /// ```
    pub fn from_ec_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ec_der")?;
        let backend = EncodingKey::from_ec_der(bytes);
        Ok(Self {
            family: KeyFamily::Ec,
            backend,
            rsa_modulus_bits: None,
            ec_curve: ec_curve_from_private_der(bytes),
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 Ed25519 PKCS#8 私钥 PEM 构造 EdDSA 签名 key。
    ///
    /// 只接受 `PRIVATE KEY` 中的 Ed25519 key，不支持 Ed448；输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let pem = std::fs::read("ed25519-private-pkcs8.pem")?;
    /// let _key = JwtSigningKey::from_ed_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_ed_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ed_pem")?;
        validate_pem_label(bytes, &["PRIVATE KEY"], "ed_pem", true)?;
        let backend = EncodingKey::from_ed_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "ed_pem"))?;
        Ok(Self {
            family: KeyFamily::Ed,
            backend,
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 Ed25519 PKCS#8 私钥 DER 构造 EdDSA 签名 key。
    ///
    /// DER 结构错误延迟到 encode 阶段；输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtSigningKey;
    ///
    /// let _ = JwtSigningKey::from_ed_der([0u8; 1]);
    /// ```
    pub fn from_ed_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ed_der")?;
        let backend = EncodingKey::from_ed_der(bytes);
        Ok(Self {
            family: KeyFamily::Ed,
            backend,
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    pub(crate) fn family(&self) -> KeyFamily {
        self.family
    }

    pub(crate) fn backend(&self) -> &EncodingKey {
        &self.backend
    }

    pub(crate) fn rsa_modulus_bits(&self) -> Option<usize> {
        self.rsa_modulus_bits
    }

    pub(crate) fn ec_curve(&self) -> Option<EcCurve> {
        self.ec_curve
    }

    pub(crate) fn is_public_rsa_der(&self) -> bool {
        self.family == KeyFamily::Rsa && self.rsa_der_kind == RsaDerKind::Public
    }
}
