use std::fmt;

use jsonwebtoken::{DecodingKey, EncodingKey};

use super::{EcCurve, JwtError, KeyFamily};

pub(crate) const MAX_KEY_BYTES: usize = 128 * 1024;
const MAX_HMAC_SECRET_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RsaDerKind {
    Public,
    Private,
    Unknown,
}

/// 拥有型 JWT 签名 key。
///
/// 该类型不实现 `Clone` 或可读取 key 内容的 API，`Display`/`Debug` 只显示 key family。
/// HMAC 使用原始 secret bytes；非对称签名只接受对应算法族的私钥格式。opaque DER 的部分
/// 结构检查会延迟到 encode 阶段。
///
/// # Examples
///
/// ```
/// let _key = axutils::JwtSigningKey::from_hmac_secret([0x11; 32])?;
/// # Ok::<(), axutils::JwtError>(())
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
    /// [`super::JwtConfig::new`] 中按算法检查。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::JwtSigningKey;
    ///
    /// let key = JwtSigningKey::from_hmac_secret([0x11; 32])?;
    /// let _ = format!("{key:?}");
    /// # Ok::<(), axutils::JwtError>(())
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
    /// use axutils::JwtSigningKey;
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
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::JwtSigningKey;
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
    /// use axutils::JwtSigningKey;
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
    /// use axutils::JwtSigningKey;
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
    /// use axutils::JwtSigningKey;
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
    /// use axutils::JwtSigningKey;
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

/// 拥有型 JWT 验证 key。
///
/// 验证服务可以只持有公钥，不需要私钥。该类型不实现 `Clone` 或 key 读取 API，
/// `Display`/`Debug` 只显示 key family。RSA 私钥 PEM、EC/Ed 私钥和不符合 Ed25519 32-byte raw 约束的
/// key 会被拒绝；opaque DER 的结构错误会在 decode 阶段稳定返回 key 错误。
///
/// # Examples
///
/// ```
/// let _key = axutils::JwtVerificationKey::from_hmac_secret([0x11; 32])?;
/// # Ok::<(), axutils::JwtError>(())
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
    /// use axutils::JwtVerificationKey;
    ///
    /// let _key = JwtVerificationKey::from_hmac_secret([0x22; 32])?;
    /// # Ok::<(), axutils::JwtError>(())
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
    /// use axutils::JwtVerificationKey;
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
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::JwtVerificationKey;
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
    /// use axutils::JwtVerificationKey;
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
    /// use axutils::JwtVerificationKey;
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
    /// use axutils::JwtVerificationKey;
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
    /// use axutils::JwtVerificationKey;
    ///
    /// let _key = JwtVerificationKey::from_ed_der([0x33; 32])?;
    /// # Ok::<(), axutils::JwtError>(())
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

fn validate_key_size(bytes: &[u8], kind: &'static str) -> Result<(), JwtError> {
    if bytes.is_empty() {
        return Err(JwtError::InvalidKey { kind });
    }
    if bytes.len() > MAX_KEY_BYTES {
        return Err(JwtError::InvalidConfig { field: "key_size" });
    }
    Ok(())
}

fn validate_pem_label(
    bytes: &[u8],
    allowed: &[&str],
    kind: &'static str,
    signing: bool,
) -> Result<(), JwtError> {
    let Some(label) = pem_label(bytes) else {
        return Err(JwtError::UnsupportedKeyFormat { kind });
    };
    if allowed.contains(&label) {
        return Ok(());
    }
    if signing || label.contains("PRIVATE") || label.contains("PUBLIC") {
        Err(JwtError::InvalidKey { kind })
    } else {
        Err(JwtError::UnsupportedKeyFormat { kind })
    }
}

fn pem_label(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let start_marker = "-----BEGIN ";
    let start = text.find(start_marker)? + start_marker.len();
    let end = text[start..].find("-----")?;
    Some(&text[start..start + end])
}

fn map_backend_key_error(error: &jsonwebtoken::errors::Error, kind: &'static str) -> JwtError {
    use jsonwebtoken::errors::ErrorKind;

    match error.kind() {
        ErrorKind::InvalidEcdsaKey
        | ErrorKind::InvalidEddsaKey
        | ErrorKind::InvalidKeyFormat
        | ErrorKind::InvalidRsaKey(_) => JwtError::UnsupportedKeyFormat { kind },
        _ => JwtError::InvalidKey { kind },
    }
}

fn rsa_modulus_bits(bytes: &[u8]) -> Option<usize> {
    let (sequence, remainder) = read_der_value_and_rest(bytes, 0x30)?;
    if !remainder.is_empty() {
        return None;
    }
    let (first, rest) = read_der_value_and_rest(sequence, 0x02)?;
    let modulus = if first.len() == 1 && (first[0] == 0 || first[0] == 1) {
        read_der_value_and_rest(rest, 0x02)?.0
    } else {
        first
    };
    let first_nonzero = modulus.iter().position(|byte| *byte != 0)?;
    let significant = &modulus[first_nonzero..];
    Some((significant.len() - 1) * 8 + (8 - significant[0].leading_zeros() as usize))
}

fn rsa_der_kind(bytes: &[u8]) -> RsaDerKind {
    let Some((sequence, remainder)) = read_der_value_and_rest(bytes, 0x30) else {
        return RsaDerKind::Unknown;
    };
    if !remainder.is_empty() {
        return RsaDerKind::Unknown;
    }
    let Some((first, rest)) = read_der_value_and_rest(sequence, 0x02) else {
        return RsaDerKind::Unknown;
    };
    let Some((second, remainder)) = read_der_value_and_rest(rest, 0x02) else {
        return RsaDerKind::Unknown;
    };
    if first.len() == 1 && (first[0] == 0 || first[0] == 1) {
        return RsaDerKind::Private;
    }
    let Some(modulus_bits) = rsa_modulus_bits(bytes) else {
        return RsaDerKind::Unknown;
    };
    let exponent = second
        .iter()
        .position(|byte| *byte != 0)
        .map(|index| &second[index..]);
    let exponent_is_usable = exponent.is_some_and(|exponent| {
        exponent.last().is_some_and(|byte| byte & 1 == 1) && (exponent.len() > 1 || exponent[0] > 2)
    }) && remainder.is_empty();
    if modulus_bits >= 2048 && exponent_is_usable {
        RsaDerKind::Public
    } else {
        RsaDerKind::Unknown
    }
}

fn ec_curve_from_private_der(bytes: &[u8]) -> Option<EcCurve> {
    const P256_OID: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
    const P384_OID: &[u8] = &[0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x22];
    if bytes
        .windows(P256_OID.len())
        .any(|window| window == P256_OID)
    {
        Some(EcCurve::P256)
    } else if bytes
        .windows(P384_OID.len())
        .any(|window| window == P384_OID)
    {
        Some(EcCurve::P384)
    } else {
        None
    }
}

fn ec_curve_from_public_point(bytes: &[u8]) -> Option<EcCurve> {
    match (bytes.first(), bytes.len()) {
        (Some(0x04), 65) => Some(EcCurve::P256),
        (Some(0x04), 97) => Some(EcCurve::P384),
        _ => None,
    }
}

fn read_der_value_and_rest(input: &[u8], expected_tag: u8) -> Option<(&[u8], &[u8])> {
    if input.len() < 2 || input[0] != expected_tag {
        return None;
    }
    let (length, header_size) = der_length(&input[1..])?;
    let end = header_size.checked_add(length)?.checked_add(1)?;
    if end > input.len() {
        return None;
    }
    Some((&input[1 + header_size..end], &input[end..]))
}

fn der_length(input: &[u8]) -> Option<(usize, usize)> {
    let first = *input.first()?;
    if first & 0x80 == 0 {
        return Some((first as usize, 1));
    }
    let count = (first & 0x7f) as usize;
    if count == 0 || count > std::mem::size_of::<usize>() || input.len() < count + 1 {
        return None;
    }
    let mut length = 0usize;
    for byte in &input[1..=count] {
        length = length.checked_shl(8)?.checked_add(*byte as usize)?;
    }
    Some((length, count + 1))
}
