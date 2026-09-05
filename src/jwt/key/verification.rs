use std::fmt;

use jsonwebtoken::DecodingKey;

use super::{
    super::{EcCurve, JwtError, KeyFamily},
    shared::{
        ec_curve_from_public_point, map_backend_key_error, rsa_der_kind, rsa_modulus_bits,
        validate_key_size, validate_pem_label, RsaDerKind, MAX_HMAC_SECRET_BYTES, MAX_KEY_BYTES,
    },
};

/// 拥有型 JWT 验证 key。
///
/// 验证服务可以只持有公钥，不需要私钥。该类型不实现 `Clone` 或 key 读取 API，
/// `Display`/`Debug` 只显示 key family。RSA 私钥 PEM、EC/Ed 私钥和不符合 Ed25519 32-byte raw 约束的
/// key 会被拒绝；opaque DER 的结构错误会在 decode 阶段稳定返回 key 错误。
///
/// # Examples
///
/// ```
/// use axutils::jwt::{JwtError, JwtVerificationKey};
///
/// let _key = JwtVerificationKey::from_hmac_secret([0x11; 32])?;
/// # Ok::<(), JwtError>(())
/// ```
pub struct JwtVerificationKey {
    family: KeyFamily,
    backend: DecodingKey,
    rsa_modulus_bits: Option<usize>,
    ec_curve: Option<EcCurve>,
    rsa_der_kind: RsaDerKind,
}

impl fmt::Debug for JwtVerificationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JwtVerificationKey")
            .field("family", &self.family)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl fmt::Display for JwtVerificationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "JwtVerificationKey({:?})", self.family)
    }
}

impl JwtVerificationKey {
    /// 从原始 HMAC secret 构造验证 key。
    ///
    /// secret 不能为空且最多 4096 字节；算法对应的最小长度在配置构造阶段检查。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtVerificationKey};
    ///
    /// let _key = JwtVerificationKey::from_hmac_secret([0x22; 32])?;
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
            backend: DecodingKey::from_secret(bytes),
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 RSA PKCS#1 或 SubjectPublicKeyInfo 公钥 PEM 构造验证 key。
    ///
    /// 只接受 `RSA PUBLIC KEY` 或包含 RSA 公钥的 `PUBLIC KEY` label，RSA 私钥 label 在构造期
    /// 拒绝；输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::{JwtError, JwtVerificationKey};
    ///
    /// let pem = std::fs::read("rsa-public.pem")?;
    /// let _key = JwtVerificationKey::from_rsa_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_rsa_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "rsa_pem")?;
        validate_pem_label(bytes, &["RSA PUBLIC KEY", "PUBLIC KEY"], "rsa_pem", false)?;
        let backend = DecodingKey::from_rsa_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "rsa_pem"))?;
        let rsa_modulus_bits = backend.try_get_as_bytes().ok().and_then(rsa_modulus_bits);
        Ok(Self {
            family: KeyFamily::Rsa,
            backend,
            rsa_modulus_bits,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Public,
        })
    }

    /// 从 RSA PKCS#1 公钥 DER 构造验证 key。
    ///
    /// 构造器只执行非空和大小检查；DER 结构和角色错误延迟到 decode 阶段。
    /// 输入语义仍固定为 RSA PKCS#1 公钥 DER；opaque bytes 只表示延迟结构校验，不扩大为
    /// PKCS#8、SPKI 或其他未验证 DER 格式的支持范围。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtVerificationKey};
    ///
    /// let _ = JwtVerificationKey::from_rsa_der([0u8; 1]);
    /// ```
    pub fn from_rsa_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "rsa_der")?;
        let backend = DecodingKey::from_rsa_der(bytes);
        let rsa_modulus_bits = rsa_modulus_bits(bytes);
        Ok(Self {
            family: KeyFamily::Rsa,
            backend,
            rsa_modulus_bits,
            ec_curve: None,
            rsa_der_kind: rsa_der_kind(bytes),
        })
    }

    /// 从 ECDSA 公钥 PEM 构造验证 key。
    ///
    /// 只接受 PKCS#8 `PUBLIC KEY`，不接受私钥或 SEC1 私钥 label；输入最多 128 KiB。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::JwtVerificationKey;
    ///
    /// let pem = std::fs::read("ec-public.pem")?;
    /// let _key = JwtVerificationKey::from_ec_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_ec_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ec_pem")?;
        validate_pem_label(bytes, &["PUBLIC KEY"], "ec_pem", false)?;
        let backend = DecodingKey::from_ec_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "ec_pem"))?;
        let ec_curve =
            ec_curve_from_public_point(backend.try_get_as_bytes().ok().unwrap_or_default());
        Ok(Self {
            family: KeyFamily::Ec,
            backend,
            rsa_modulus_bits: None,
            ec_curve,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 SEC1 encoded public-point bytes 构造 ECDSA 验证 key。
    ///
    /// 这里的 DER 构造器名称保持与后端一致，但输入语义固定为 SEC1 public-point bytes；
    /// 未被探针证明的 SubjectPublicKeyInfo DER 不在承诺范围内。结构错误延迟到 decode 阶段。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::JwtVerificationKey;
    ///
    /// let _ = JwtVerificationKey::from_ec_der([0u8; 1]);
    /// ```
    pub fn from_ec_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ec_der")?;
        let backend = DecodingKey::from_ec_der(bytes);
        let ec_curve = ec_curve_from_public_point(bytes);
        Ok(Self {
            family: KeyFamily::Ec,
            backend,
            rsa_modulus_bits: None,
            ec_curve,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从 Ed25519 公钥 PEM 构造 EdDSA 验证 key。
    ///
    /// 只接受 Ed25519 `PUBLIC KEY`，不支持 Ed448、私钥或证书。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::jwt::JwtVerificationKey;
    ///
    /// let pem = std::fs::read("ed25519-public.pem")?;
    /// let _key = JwtVerificationKey::from_ed_pem(pem)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_ed_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        validate_key_size(bytes, "ed_pem")?;
        validate_pem_label(bytes, &["PUBLIC KEY"], "ed_pem", false)?;
        let backend = DecodingKey::from_ed_pem(bytes)
            .map_err(|error| map_backend_key_error(&error, "ed_pem"))?;
        let length = backend
            .try_get_as_bytes()
            .map(|bytes| bytes.len())
            .unwrap_or_default();
        if length != 32 {
            return Err(JwtError::UnsupportedKeyFormat {
                kind: "ed_public_pem",
            });
        }
        Ok(Self {
            family: KeyFamily::Ed,
            backend,
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    /// 从恰好 32-byte 的 Ed25519 raw 公钥构造 EdDSA 验证 key。
    ///
    /// 该长度检查在调用后端前执行，避免后端 verifier 对短 key 切片造成 panic；0、31、33
    /// 和其他长度均返回 [`JwtError::UnsupportedKeyFormat`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::jwt::{JwtError, JwtVerificationKey};
    ///
    /// let _key = JwtVerificationKey::from_ed_der([0x33; 32])?;
    /// # Ok::<(), JwtError>(())
    /// ```
    pub fn from_ed_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError> {
        let bytes = input.as_ref();
        if bytes.len() > MAX_KEY_BYTES {
            return Err(JwtError::InvalidConfig { field: "key_size" });
        }
        if bytes.len() != 32 {
            return Err(JwtError::UnsupportedKeyFormat {
                kind: "ed_public_der",
            });
        }
        Ok(Self {
            family: KeyFamily::Ed,
            backend: DecodingKey::from_ed_der(bytes),
            rsa_modulus_bits: None,
            ec_curve: None,
            rsa_der_kind: RsaDerKind::Unknown,
        })
    }

    pub(crate) fn family(&self) -> KeyFamily {
        self.family
    }

    pub(crate) fn backend(&self) -> &DecodingKey {
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

    pub(crate) fn validate_for_decode(&self) -> Result<(), JwtError> {
        if self.family == KeyFamily::Rsa && self.rsa_der_kind != RsaDerKind::Public {
            return Err(JwtError::UnsupportedKeyFormat {
                kind: "verification_key",
            });
        }
        Ok(())
    }
}
