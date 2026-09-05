use serde::de::DeserializeOwned;

use super::super::{
    codec, commands,
    config::RedisConfig,
    error::{RedisError, RedisTransportErrorKind},
};

#[cfg(any(test, feature = "redis-async"))]
pub(super) fn check_optional_values(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<Vec<u8>>>, RedisError> {
    let mut response_bytes = 0;
    values
        .into_iter()
        .map(|value| {
            value
                .map(|bytes| {
                    response_bytes = commands::add_response_bytes(response_bytes, &bytes, config)?;
                    Ok(bytes)
                })
                .transpose()
        })
        .collect()
}

#[cfg(feature = "redis-async")]
#[cfg(feature = "redis-async")]
pub(super) fn decode_optional_values<T: DeserializeOwned>(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<T>>, RedisError> {
    check_optional_values(values, config)?
        .into_iter()
        .map(|value| {
            value
                .map(|bytes| codec::decode(&bytes, config.max_value_bytes))
                .transpose()
        })
        .collect()
}

#[allow(clippy::type_complexity)]
pub(super) fn decode_hash_entries(
    flat: Vec<Vec<u8>>,
    config: &RedisConfig,
) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
    if !flat.len().is_multiple_of(2) {
        return Err(RedisError::Transport(RedisTransportErrorKind::Protocol));
    }
    let count = flat.len() / 2;
    if count > config.max_collection_items {
        return Err(RedisError::CollectionTooLarge {
            limit: config.max_collection_items,
        });
    }
    let mut response_bytes = 0;
    let mut entries = Vec::with_capacity(count);
    let mut values = flat.into_iter();
    for _ in 0..count {
        let Some(field_value) = values.next() else {
            return Err(RedisError::Transport(RedisTransportErrorKind::Protocol));
        };
        if field_value.is_empty() || field_value.len() > config.max_key_bytes {
            return Err(RedisError::InvalidField);
        }
        response_bytes = add_response_part(response_bytes, field_value.len(), config)?;
        let Some(value) = values.next() else {
            return Err(RedisError::Transport(RedisTransportErrorKind::Protocol));
        };
        response_bytes = commands::add_response_bytes(response_bytes, &value, config)?;
        entries.push((field_value, value));
    }
    Ok(entries)
}

pub(super) fn decode_collection<T: DeserializeOwned>(
    values: Vec<Vec<u8>>,
    config: &RedisConfig,
) -> Result<Vec<T>, RedisError> {
    if values.len() > config.max_collection_items {
        return Err(RedisError::CollectionTooLarge {
            limit: config.max_collection_items,
        });
    }
    let mut response_bytes = 0;
    values
        .into_iter()
        .map(|bytes| {
            response_bytes = commands::add_response_bytes(response_bytes, &bytes, config)?;
            codec::decode(&bytes, config.max_value_bytes)
        })
        .collect()
}

fn add_response_part(
    current: usize,
    bytes: usize,
    config: &RedisConfig,
) -> Result<usize, RedisError> {
    let total = current
        .checked_add(bytes)
        .ok_or(RedisError::ResponseTooLarge {
            limit: config.max_response_bytes,
        })?;
    if total > config.max_response_bytes {
        return Err(RedisError::ResponseTooLarge {
            limit: config.max_response_bytes,
        });
    }
    Ok(total)
}
