use serde::Serialize;

use super::super::{commands, config::RedisConfig, error::RedisError};

/// 收集有界 key 列表；输入错误优先于 Cluster slot 校验，空列表保持本地空操作。
pub(super) fn collect_keys<I, K>(keys: I, config: &RedisConfig) -> Result<Vec<Vec<u8>>, RedisError>
where
    I: IntoIterator<Item = K>,
    K: AsRef<[u8]>,
{
    let mut collected = Vec::new();
    let mut total = 0;
    // 逐项检查数量、key 与累计字节；超限后不再消费剩余迭代器。
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
    // 所有既有输入错误检查完毕后才检查跨 slot，发送之前确保不会被上游分拆。
    commands::check_key_slots(collected.iter().map(Vec::as_slice), config)?;
    Ok(collected)
}

/// 将 MessagePack MSET 的 key/value 对按原顺序编码，并共享数量、字节与 slot 边界。
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
    // 数量预算按对计数，只有 key/value 都校验成功才追加完整参数对。
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
    // 值不参与 slot 计算；即使值包含 hash tag，也不能改变路由判断。
    commands::check_key_slots(args.iter().step_by(2).map(Vec::as_slice), config)?;
    Ok(args)
}

/// 收集 raw MSET 参数对，保持字节原样并在全部本地预算检查后校验 key slot。
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
    // 按 key/value 对控制迭代消费和有效载荷累计，不计 RESP 协议开销。
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
    commands::check_key_slots(args.iter().step_by(2).map(Vec::as_slice), config)?;
    Ok(args)
}

/// 收集单个 Hash 的 MessagePack HSET 参数；只有一个 key，不需要多 key slot 校验。
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
    // 空 entries 只校验 key 并返回空操作；非空时将 key 纳入批量字节预算。
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

/// 收集单个 Hash 的 raw HSET 参数；与编码版本保持数量及字节预算的检查顺序。
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
    // 数量只计算 field/value 对，整个 Hash key 的字节仍计入非空批次预算。
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
