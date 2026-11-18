use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{self, DeserializeSeed, Deserializer as _, MapAccess, SeqAccess, Visitor};
use serde_json::{value::RawValue, Deserializer as JsonDeserializer};

use super::config::{JwtValidation, MAX_ALLOWLIST_ITEMS, MAX_CLAIM_STRING_BYTES};
use super::{header, JwtError};

/// Base64URL 解码后的 claims 字节上限。
pub(crate) const MAX_CLAIMS_BYTES: usize = 32 * 1024;
/// 包含根对象在内，同时打开的 JSON 容器上限。
const MAX_CLAIMS_DEPTH: usize = 32;
/// 每个 JSON 对象允许的成员数。
const MAX_OBJECT_MEMBERS: usize = 256;
/// 每个 JSON 数组允许的元素数。
const MAX_ARRAY_ELEMENTS: usize = 256;

/// 签发与验签共用的结构预检，检查完整 JSON 的语法、根类型、资源预算和重复键。
pub(crate) fn preflight_claims(payload: &[u8]) -> Result<(), JwtError> {
    if payload.len() > MAX_CLAIMS_BYTES {
        return Err(JwtError::InvalidToken {
            segment: "claims_size",
        });
    }
    // 先限制源文本中的容器深度，再允许 serde 递归。这样数字的内部表示不占用容器预算。
    check_container_depth(payload)?;
    let mut deserializer = JsonDeserializer::from_slice(payload);
    deserializer
        .deserialize_map(ClaimsVisitor)
        .and_then(|()| deserializer.end())
        .map_err(|_| JwtError::InvalidToken { segment: "claims" })
}

/// 常量空间扫描实际容器的开闭符；完整的分隔符匹配、转义和标量语法由 JSON 解析器检查。
fn check_container_depth(payload: &[u8]) -> Result<(), JwtError> {
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut bytes = payload.iter();
    while let Some(byte) = bytes.next() {
        match byte {
            // 字符串内的反斜线连同下一字节消费，转义引号不会提前结束字符串。
            b'\\' if in_string => {
                bytes.next();
            }
            b'"' => in_string = !in_string,
            b'{' | b'[' if !in_string => {
                depth += 1;
                if depth > MAX_CLAIMS_DEPTH {
                    return Err(JwtError::InvalidToken { segment: "claims" });
                }
            }
            b'}' | b']' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// 在原始深度已受限的前提下，校验各层的成员数、数组长度及解码后的键唯一性。
struct ClaimsVisitor;

impl<'de> DeserializeSeed<'de> for ClaimsVisitor {
    type Value = ();

    /// 子值可以是任意 JSON 类型，由后端负责完整解码和语法检查。
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for ClaimsVisitor {
    type Value = ();

    /// 仅提供结构描述，不在错误中暴露字段值。
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded JSON claims")
    }

    /// 分对象保存已解码的键，因此等价转义是重复键，不同对象内的同名字段相互独立。
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.len() >= MAX_OBJECT_MEMBERS {
                return Err(de::Error::custom("claims object member limit exceeded"));
            }
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate claims object key"));
            }
            map.next_value_seed(ClaimsVisitor)?;
        }
        Ok(())
    }

    /// 逐项检查子树及本数组长度，避免为数组内容构建中间值。
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let mut count = 0;
        while sequence.next_element_seed(ClaimsVisitor)?.is_some() {
            count += 1;
            if count > MAX_ARRAY_ELEMENTS {
                return Err(de::Error::custom("claims array element limit exceeded"));
            }
        }
        Ok(())
    }

    /// 接受已解析的布尔值。
    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    /// 接受已解析的有符号整数。
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    /// 接受已解析的无符号整数。
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    /// 接受已解析的浮点数。
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    /// 接受已校验编码和转义的字符串。
    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    /// 接受 JSON null。
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
}

/// 解码 payload 段，编码错误保留独立于 JSON 语法错误的分类。
pub(crate) fn decode_payload(payload: &str) -> Result<Vec<u8>, JwtError> {
    header::decode_base64url(payload).ok_or(JwtError::InvalidToken {
        segment: "payload_base64",
    })
}

/// 从已验签、已预检的同一 payload 借用顶层字段原文，依次校验标准项。
pub(crate) fn validate_standard_claims(
    payload: &[u8],
    validation: &JwtValidation,
    now: Option<u64>,
) -> Result<(), JwtError> {
    // 键已在预检中排重。保留每个值的 JSON 类型及原文，包括显式 null 和宽整数。
    let object: BTreeMap<String, &RawValue> = serde_json::from_slice(payload)
        .map_err(|_| JwtError::InvalidToken { segment: "claims" })?;

    // 即使关闭时间比较，存在的 NumericDate 也必须具有非负整数类型。
    let exp = match object.get("exp") {
        None if validation.require_exp => {
            return Err(JwtError::MissingRequiredClaim { claim: "exp" });
        }
        None => None,
        Some(value) => Some(numeric_date(value, "exp")?),
    };
    if validation.validate_exp {
        if let Some(exp) = exp {
            let now = now.ok_or(JwtError::InvalidClaim { claim: "exp" })?;
            let latest = exp
                .checked_add(validation.leeway)
                .ok_or(JwtError::InvalidClaim { claim: "exp" })?;
            if latest < now {
                return Err(JwtError::InvalidClaim { claim: "exp" });
            }
        }
    }

    // 起效时间使用同一时钟和宽限期，边界加法溢出归入该声明错误。
    let nbf = match object.get("nbf") {
        None if validation.require_nbf => {
            return Err(JwtError::MissingRequiredClaim { claim: "nbf" });
        }
        None => None,
        Some(value) => Some(numeric_date(value, "nbf")?),
    };
    if validation.validate_nbf {
        if let Some(nbf) = nbf {
            let now = now.ok_or(JwtError::InvalidClaim { claim: "nbf" })?;
            let earliest = now
                .checked_add(validation.leeway)
                .ok_or(JwtError::InvalidClaim { claim: "nbf" })?;
            if nbf > earliest {
                return Err(JwtError::InvalidClaim { claim: "nbf" });
            }
        }
    }

    // audience 允许单个字符串或字符串数组，与配置至少匹配一项。
    let audience = match object.get("aud") {
        None if validation.require_aud => {
            return Err(JwtError::MissingRequiredClaim { claim: "aud" });
        }
        None => None,
        Some(value) => Some(audience_values(value)?),
    };
    if let (Some(values), Some(expected)) = (&audience, &validation.audience) {
        if !values.iter().any(|value| expected.contains(value)) {
            return Err(JwtError::InvalidClaim { claim: "aud" });
        }
    }

    // issuer 和 subject 分别检查存在性、字符串约束与配置的精确匹配。
    let issuer = match object.get("iss") {
        None if validation.require_iss => {
            return Err(JwtError::MissingRequiredClaim { claim: "iss" });
        }
        None => None,
        Some(value) => Some(string_claim(value, "iss")?),
    };
    if let (Some(issuer), Some(expected)) = (issuer, &validation.issuers) {
        if !expected.contains(&issuer) {
            return Err(JwtError::InvalidClaim { claim: "iss" });
        }
    }

    let subject = match object.get("sub") {
        None if validation.require_sub => {
            return Err(JwtError::MissingRequiredClaim { claim: "sub" });
        }
        None => None,
        Some(value) => Some(string_claim(value, "sub")?),
    };
    if let (Some(subject), Some(expected)) = (subject, validation.subject.as_deref()) {
        if subject != expected {
            return Err(JwtError::InvalidClaim { claim: "sub" });
        }
    }

    Ok(())
}

/// 从原始 JSON 数字读取 u64，不对小数、指数或超范围整数做舍入。
fn numeric_date(value: &RawValue, claim: &'static str) -> Result<u64, JwtError> {
    serde_json::from_str(value.get()).map_err(|_| JwtError::InvalidClaim { claim })
}

/// 解码标准字符串声明，在解码转义后检查字节预算和控制字符。
fn string_claim(value: &RawValue, claim: &'static str) -> Result<String, JwtError> {
    let string: String =
        serde_json::from_str(value.get()).map_err(|_| JwtError::InvalidClaim { claim })?;
    if string.is_empty()
        || string.len() > MAX_CLAIM_STRING_BYTES
        || string.chars().any(char::is_control)
    {
        return Err(JwtError::InvalidClaim { claim });
    }
    Ok(string)
}

/// 校验 audience 的两种 JSON 形式，对数组逐项应用相同的字符串规则。
fn audience_values(value: &RawValue) -> Result<Vec<String>, JwtError> {
    if !value.get().starts_with('[') {
        return Ok(vec![string_claim(value, "aud")?]);
    }
    // 结构预检已限制数组长度，这里另行应用 audience 的更小上限。
    let values: Vec<&RawValue> =
        serde_json::from_str(value.get()).map_err(|_| JwtError::InvalidClaim { claim: "aud" })?;
    if values.is_empty() || values.len() > MAX_ALLOWLIST_ITEMS {
        return Err(JwtError::InvalidClaim { claim: "aud" });
    }
    values
        .into_iter()
        .map(|value| string_claim(value, "aud"))
        .collect()
}
