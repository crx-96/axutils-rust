//! 密钥输入预算、格式分类与 DER 元数据读取；完整密钥有效性仍由签验后端判断。

use jsonwebtoken::errors::{Error, ErrorKind};

use super::super::{EcCurve, JwtError};

/// PEM/DER 密钥输入的最大字节数，限制后端解析前的资源消耗。
pub(super) const MAX_KEY_BYTES: usize = 128 * 1024;
/// HMAC secret 的最大字节数；算法相关的最小长度由配置校验负责。
pub(super) const MAX_HMAC_SECRET_BYTES: usize = 4096;

/// 从 PKCS#1 DER 外形识别的 RSA 密钥种类，不代表完整密码学有效性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RsaDerKind {
    /// 模数及指数符合本库最低约束的公钥外形。
    Public,
    /// 以受支持版本整数开头的私钥外形，剩余字段由后端验证。
    Private,
    /// 未识别、截断或不符合本库最低公钥约束。
    Unknown,
}

/// 拒绝空输入与超预算密钥，只返回固定格式类别，不回显密钥字节。
pub(super) fn validate_key_size(bytes: &[u8], kind: &'static str) -> Result<(), JwtError> {
    // 在解析前分别报告空密钥与资源预算错误，保持现有错误分类。
    if bytes.is_empty() {
        return Err(JwtError::InvalidKey { kind });
    }
    if bytes.len() > MAX_KEY_BYTES {
        return Err(JwtError::InvalidConfig { field: "key_size" });
    }
    Ok(())
}

/// 校验 PEM 标签与预期用途；标签通过后仍须交由后端解析实际密钥。
pub(super) fn validate_pem_label(
    bytes: &[u8],
    allowed: &[&str],
    kind: &'static str,
    signing: bool,
) -> Result<(), JwtError> {
    // 无标签属于不支持的格式；已识别的公私钥标签不匹配则属于用途错误。
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

/// 将后端密钥解析错误映射为固定类别，丢弃可能携带输入的诊断内容。
pub(super) fn map_backend_key_error(error: &Error, kind: &'static str) -> JwtError {
    match error.kind() {
        ErrorKind::InvalidEcdsaKey
        | ErrorKind::InvalidEddsaKey
        | ErrorKind::InvalidKeyFormat
        | ErrorKind::InvalidRsaKey(_) => JwtError::UnsupportedKeyFormat { kind },
        _ => JwtError::InvalidKey { kind },
    }
}

/// 读取 PKCS#1 公钥或私钥的有效模数位数；截断或空模数返回 `None`。
pub(super) fn rsa_modulus_bits(bytes: &[u8]) -> Option<usize> {
    // 外层只能包含一个完整 SEQUENCE；私钥先越过版本整数，再读取模数。
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
    // 去掉 DER 正整数的前导零，按首个有效字节计算实际位宽。
    let first_nonzero = modulus.iter().position(|byte| *byte != 0)?;
    let significant = &modulus[first_nonzero..];
    Some((significant.len() - 1) * 8 + (8 - significant[0].leading_zeros() as usize))
}

/// 区分 RSA 公私钥外形，供配置拒绝误作签名密钥的公钥；不替代后端验钥。
pub(super) fn rsa_der_kind(bytes: &[u8]) -> RsaDerKind {
    // 只检查完整顶层序列及最前面的整数，避免越界读取或扫描任意密钥载荷。
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
    // 公钥必须恰有模数和指数，指数为大于 2 的奇数，模数至少 2048 位。
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

/// 从 PKCS#8 AlgorithmIdentifier 读取受支持的命名曲线，忽略私钥载荷中的相同字节。
///
/// 只识别元数据，不接受 SEC1 顶层格式；标量、可选属性及公钥的一致性仍由后端验证。
/// 字段顺序遵循 RFC 5208，EC 算法及命名曲线标识遵循 RFC 5480。
pub(super) fn ec_curve_from_private_der(bytes: &[u8]) -> Option<EcCurve> {
    /// `id-ecPublicKey`（1.2.840.10045.2.1）的 DER OID 值，不包含 tag/length。
    const EC_PUBLIC_KEY_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
    /// `secp256r1`（1.2.840.10045.3.1.7）的 DER OID 值。
    const P256_OID: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
    /// `secp384r1`（1.3.132.0.34）的 DER OID 值。
    const P384_OID: &[u8] = &[0x2b, 0x81, 0x04, 0x00, 0x22];

    // 逐层读取 PKCS#8 的版本、算法和私钥字段；任何截断或错误层级都不推断曲线。
    let (sequence, trailing) = read_der_value_and_rest(bytes, 0x30)?;
    if !trailing.is_empty() {
        return None;
    }
    let (version, rest) = read_der_value_and_rest(sequence, 0x02)?;
    if !matches!(version, [0] | [1]) {
        return None;
    }
    let (identifier, private_key) = read_der_value_and_rest(rest, 0x30)?;
    read_der_value_and_rest(private_key, 0x04)?;

    // 只接受 EC 算法的显式 namedCurve 参数，不能在随机标量或其他算法载荷中查找 OID。
    let (algorithm, parameters) = read_der_value_and_rest(identifier, 0x06)?;
    if algorithm != EC_PUBLIC_KEY_OID {
        return None;
    }
    let (curve, extra_parameters) = read_der_value_and_rest(parameters, 0x06)?;
    if !extra_parameters.is_empty() {
        return None;
    }
    if curve == P256_OID {
        Some(EcCurve::P256)
    } else if curve == P384_OID {
        Some(EcCurve::P384)
    } else {
        None
    }
}

/// 根据未压缩 EC 公钥点的编码长度识别曲线；坐标有效性由验签后端校验。
pub(super) fn ec_curve_from_public_point(bytes: &[u8]) -> Option<EcCurve> {
    match (bytes.first(), bytes.len()) {
        (Some(0x04), 65) => Some(EcCurve::P256),
        (Some(0x04), 97) => Some(EcCurve::P384),
        _ => None,
    }
}

/// 读取 PEM 的首个 BEGIN 标签；这里只定位标签，不尝试解析 Base64 载荷。
fn pem_label(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let start_marker = "-----BEGIN ";
    let start = text.find(start_marker)? + start_marker.len();
    let end = text[start..].find("-----")?;
    Some(&text[start..start + end])
}

/// 从 DER 开头读取指定 tag 的值及未消费尾部；长度溢出、截断或 tag 不符返回 `None`。
fn read_der_value_and_rest(input: &[u8], expected_tag: u8) -> Option<(&[u8], &[u8])> {
    // 至少需要 tag 和长度首字节，后续加法均检查溢出再构造切片。
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

/// 解码 DER 定长长度，返回载荷字节数和长度字段占用字节数；拒绝不定长与越界编码。
fn der_length(input: &[u8]) -> Option<(usize, usize)> {
    // 短长度直接编码；长长度限制为 usize 能表示的字节数，防止后续移位丢失高位。
    let first = *input.first()?;
    if first & 0x80 == 0 {
        return Some((first as usize, 1));
    }
    let count = (first & 0x7f) as usize;
    if count == 0 || count > std::mem::size_of::<usize>() || input.len() < count + 1 {
        return None;
    }
    // 按大端累计长度，调用方还会检查完整 TLV 是否落在输入范围内。
    let mut length = 0usize;
    for byte in &input[1..=count] {
        length = length.checked_shl(8)?.checked_add(*byte as usize)?;
    }
    Some((length, count + 1))
}

#[cfg(test)]
mod tests {
    use super::{ec_curve_from_private_der, EcCurve};

    fn private_key_envelope() -> Vec<u8> {
        vec![
            0x30, 0x1c, 0x02, 0x01, 0x00, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d,
            0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x04, 0x02,
            0x00, 0x00,
        ]
    }

    #[test]
    fn only_named_curve_in_the_ec_algorithm_identifier_is_recognized() {
        let envelope = private_key_envelope();
        assert_eq!(ec_curve_from_private_der(&envelope), Some(EcCurve::P256));
        for (index, value) in [(0, 0x04), (4, 0x02), (5, 0x04), (15, 0x02), (26, 0x03)] {
            let mut malformed = envelope.clone();
            malformed[index] = value;
            assert_eq!(ec_curve_from_private_der(&malformed), None);
        }
        assert_eq!(ec_curve_from_private_der(&envelope[16..26]), None);
    }

    #[test]
    fn truncated_or_trailing_der_does_not_produce_curve_metadata() {
        let envelope = private_key_envelope();
        for length in 0..envelope.len() {
            assert_eq!(ec_curve_from_private_der(&envelope[..length]), None);
        }
        let mut trailing = envelope;
        trailing.push(0);
        assert_eq!(ec_curve_from_private_der(&trailing), None);
    }
}
