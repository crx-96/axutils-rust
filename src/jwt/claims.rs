use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

use super::config::{JwtValidation, MAX_ALLOWLIST_ITEMS, MAX_CLAIM_STRING_BYTES};
use super::header::decode_base64url;
use super::JwtError;

pub(crate) const MAX_CLAIMS_BYTES: usize = 32 * 1024;
pub(crate) const MAX_CLAIMS_DEPTH: usize = 32;
pub(crate) const MAX_OBJECT_MEMBERS: usize = 256;
pub(crate) const MAX_ARRAY_ELEMENTS: usize = 256;

pub(crate) fn preflight_claims(payload: &[u8]) -> Result<(), JwtError> {
    if payload.len() > MAX_CLAIMS_BYTES {
        return Err(JwtError::InvalidToken {
            segment: "claims_size",
        });
    }
    let mut deserializer = serde_json::Deserializer::from_slice(payload);
    ClaimsSeed {
        depth: 1,
        root: true,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| JwtError::InvalidToken { segment: "claims" })?;
    deserializer
        .end()
        .map_err(|_| JwtError::InvalidToken { segment: "claims" })
}

struct ClaimsSeed {
    depth: usize,
    root: bool,
}

impl<'de> DeserializeSeed<'de> for ClaimsSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ClaimsVisitor {
            depth: self.depth,
            root: self.root,
        })
    }
}

struct ClaimsVisitor {
    depth: usize,
    root: bool,
}

impl<'de> Visitor<'de> for ClaimsVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON claims object")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        if self.depth > MAX_CLAIMS_DEPTH {
            return Err(de::Error::custom("claims nesting limit exceeded"));
        }
        let mut keys = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if keys.len() >= MAX_OBJECT_MEMBERS {
                return Err(de::Error::custom("claims object member limit exceeded"));
            }
            if keys.iter().any(|existing| existing == &key) {
                return Err(de::Error::custom("duplicate claims object key"));
            }
            keys.push(key);
            map.next_value_seed(ClaimsSeed {
                depth: self.depth + 1,
                root: false,
            })?;
        }
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        if self.root {
            return Err(de::Error::custom("claims root must be an object"));
        }
        if self.depth > MAX_CLAIMS_DEPTH {
            return Err(de::Error::custom("claims nesting limit exceeded"));
        }
        let mut count = 0;
        while sequence
            .next_element_seed::<ClaimsSeed>(ClaimsSeed {
                depth: self.depth + 1,
                root: false,
            })?
            .is_some()
        {
            count += 1;
            if count > MAX_ARRAY_ELEMENTS {
                return Err(de::Error::custom("claims array element limit exceeded"));
            }
        }
        Ok(())
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.scalar()
    }
}

impl ClaimsVisitor {
    fn scalar<E>(self) -> Result<(), E>
    where
        E: de::Error,
    {
        if self.root {
            Err(E::custom("claims root must be an object"))
        } else {
            Ok(())
        }
    }
}

pub(crate) fn decode_payload(payload: &str) -> Result<Vec<u8>, JwtError> {
    decode_base64url(payload).ok_or(JwtError::InvalidToken {
        segment: "payload_base64",
    })
}

pub(crate) fn check_value_resources(value: &Value) -> Result<(), JwtError> {
    check_value(value, 1, true)
}

fn check_value(value: &Value, depth: usize, root: bool) -> Result<(), JwtError> {
    match value {
        Value::Object(map) => {
            if depth > MAX_CLAIMS_DEPTH || map.len() > MAX_OBJECT_MEMBERS {
                return Err(JwtError::InvalidToken {
                    segment: "claims_size",
                });
            }
            for child in map.values() {
                check_value(child, depth + 1, false)?;
            }
        }
        Value::Array(values) => {
            if root || depth > MAX_CLAIMS_DEPTH || values.len() > MAX_ARRAY_ELEMENTS {
                return Err(JwtError::InvalidToken {
                    segment: "claims_size",
                });
            }
            for child in values {
                check_value(child, depth + 1, false)?;
            }
        }
        _ if root => {
            return Err(JwtError::InvalidToken {
                segment: "claims_root",
            })
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn validate_standard_claims(
    value: &Value,
    validation: &JwtValidation,
    now: Option<u64>,
) -> Result<(), JwtError> {
    let object = value.as_object().ok_or(JwtError::InvalidToken {
        segment: "claims_root",
    })?;

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

    let audience = match object.get("aud") {
        None if validation.require_aud => {
            return Err(JwtError::MissingRequiredClaim { claim: "aud" });
        }
        None => None,
        Some(value) => Some(audience_values(value)?),
    };
    if let (Some(values), Some(expected)) = (&audience, &validation.audience) {
        if !values
            .iter()
            .any(|value| expected.iter().any(|item| item == value))
        {
            return Err(JwtError::InvalidClaim { claim: "aud" });
        }
    }

    let issuer = match object.get("iss") {
        None if validation.require_iss => {
            return Err(JwtError::MissingRequiredClaim { claim: "iss" });
        }
        None => None,
        Some(value) => Some(string_claim(value, "iss")?),
    };
    if let (Some(issuer), Some(expected)) = (issuer, &validation.issuers) {
        if !expected.iter().any(|item| item == issuer) {
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

fn numeric_date(value: &Value, claim: &'static str) -> Result<u64, JwtError> {
    value.as_u64().ok_or(JwtError::InvalidClaim { claim })
}

fn string_claim<'a>(value: &'a Value, claim: &'static str) -> Result<&'a str, JwtError> {
    let string = value.as_str().ok_or(JwtError::InvalidClaim { claim })?;
    if string.is_empty()
        || string.len() > MAX_CLAIM_STRING_BYTES
        || string.chars().any(char::is_control)
    {
        return Err(JwtError::InvalidClaim { claim });
    }
    Ok(string)
}

fn audience_values(value: &Value) -> Result<Vec<&str>, JwtError> {
    match value {
        Value::String(_) => Ok(vec![string_claim(value, "aud")?]),
        Value::Array(values) => {
            if values.is_empty() || values.len() > MAX_ALLOWLIST_ITEMS {
                return Err(JwtError::InvalidClaim { claim: "aud" });
            }
            values
                .iter()
                .map(|value| string_claim(value, "aud"))
                .collect()
        }
        _ => Err(JwtError::InvalidClaim { claim: "aud" }),
    }
}
