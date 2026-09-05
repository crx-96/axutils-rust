use serde::Serialize;

use super::super::{commands, config::RedisConfig, error::RedisError};

pub(super) fn collect_keys<I, K>(keys: I, config: &RedisConfig) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = K>,
    K: AsRef<[u8]>,
{
    let mut collected = Vec::new();
    let mut total = 0;
    for key_value in keys {
        if collected.len() >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        collected.push(key_value);
    }
    Ok(collected)
}

pub(super) fn collect_value_pairs<I, K, T>(
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (K, T)>,
    K: AsRef<[u8]>,
    T: Serialize,
{
    let mut args = Vec::new();
    let mut total = 0;
    for (key_value, value) in entries {
        if args.len() / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        let value = commands::encoded(&value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(key_value);
        args.push(value);
    }
    Ok(args)
}

pub(super) fn collect_raw_pairs<I, K, V>(
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    let mut args = Vec::new();
    let mut total = 0;
    for (key_value, value) in entries {
        if args.len() / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let key_value = commands::key(key_value, config)?;
        let value = commands::raw(value, config)?;
        total = commands::add_batch_bytes(total, key_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(key_value);
        args.push(value);
    }
    Ok(args)
}

#[cfg(feature = "redis-async")]
pub(super) fn collect_hash_pairs<I, K, F, T>(
    key_value: K,
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (F, T)>,
    K: AsRef<[u8]>,
    F: AsRef<[u8]>,
    T: Serialize,
{
    let key_value = commands::key(key_value, config)?;
    let mut args = vec![key_value];
    let mut total = args[0].len();
    for (field_value, value) in entries {
        if (args.len() - 1) / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let field_value = commands::field(field_value, config)?;
        let value = commands::encoded(&value, config)?;
        total = commands::add_batch_bytes(total, field_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(field_value);
        args.push(value);
    }
    Ok(args)
}

#[cfg(feature = "redis-async")]
pub(super) fn collect_hash_raw_pairs<I, K, F, V>(
    key_value: K,
    entries: I,
    config: &RedisConfig,
) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = (F, V)>,
    K: AsRef<[u8]>,
    F: AsRef<[u8]>,
    V: AsRef<[u8]>,
{
    let key_value = commands::key(key_value, config)?;
    let mut args = vec![key_value];
    let mut total = args[0].len();
    for (field_value, value) in entries {
        if (args.len() - 1) / 2 >= config.max_batch_items {
            return Err(RedisError::ValueTooLarge {
                limit: config.max_batch_items,
            });
        }
        let field_value = commands::field(field_value, config)?;
        let value = commands::raw(value, config)?;
        total = commands::add_batch_bytes(total, field_value.len(), config)?;
        total = commands::add_batch_bytes(total, value.len(), config)?;
        args.push(field_value);
        args.push(value);
    }
    Ok(args)
}
