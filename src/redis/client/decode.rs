use ::redis::{FromRedisValue, ParsingError, Value};
use serde::de::DeserializeOwned;

use super::super::{codec, commands, config::RedisConfig, error::RedisError};

/// 检查所有存在值的单值与累计响应预算，保留缺失位置及原始二进制内容。
pub(super) fn check_optional_values(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<Vec<u8>>>, RedisError> {
    // 累计只包含实际存在的有效载荷，None 不增加字节但保留结果位置。
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
/// 先完成全响应预算检查，再解码异步 MGET 结果，保留现有错误优先级。
pub(super) fn decode_optional_values<T: DeserializeOwned>(
    values: Vec<Option<Vec<u8>>>,
    config: &RedisConfig,
) -> Result<Vec<Option<T>>, RedisError> {
    // 先判预算再反序列化，避免异步路径的错误分类随解码顺序改变。
    check_optional_values(values, config)?
        .into_iter()
        .map(|value| {
            value
                .map(|bytes| codec::decode(&bytes, config.max_value_bytes))
                .transpose()
        })
        .collect()
}

/// 按线序排列的二进制 field/value，不排序也不合并同名 field。
type HashEntries = Vec<(Vec<u8>, Vec<u8>)>;

/// HGETALL 的协议回复，在命令执行层解码，使协议错误沿用驱动的连接处置和诊断路径。
pub(super) struct HashReply(
    /// 已校验形状和元素类型，尚待应用客户端预算的键值对。
    pub(super) HashEntries,
);

impl FromRedisValue for HashReply {
    /// 只接受 RESP2 的偶数扁平数组和 RESP3 map，直接转移两端字节的所有权。
    fn from_redis_value(reply: Value) -> Result<Self, ParsingError> {
        // 两种容器共用元素解码；按 wire 顺序收集，结构错误先于客户端预算错误。
        let entries = match without_attributes(reply) {
            Value::Map(pairs) => pairs.into_iter().map(hash_pair).collect::<Result<_, _>>()?,
            Value::Array(items) if items.len().is_multiple_of(2) => {
                let mut items = items.into_iter();
                let mut entries = Vec::with_capacity(items.len() / 2);
                while let (Some(field), Some(value)) = (items.next(), items.next()) {
                    entries.push(hash_pair((field, value))?);
                }
                entries
            }
            _ => return Err(ParsingError::from("invalid HGETALL reply shape")),
        };
        Ok(Self(entries))
    }
}

/// RESP3 属性属于元数据，可装饰整条回复，也可单独装饰 field 或 value。
fn without_attributes(mut reply: Value) -> Value {
    while let Value::Attribute { data, .. } = reply {
        reply = *data;
    }
    reply
}

/// 保留原始二进制内容，拒绝将整数、集合或 null 隐式转换成字节。
fn hash_pair((field, value): (Value, Value)) -> Result<(Vec<u8>, Vec<u8>), ParsingError> {
    match (without_attributes(field), without_attributes(value)) {
        (Value::BulkString(field), Value::BulkString(value)) => Ok((field, value)),
        _ => Err(ParsingError::from("expected HGETALL bulk strings")),
    }
}

/// 检查整个 HGETALL 响应的项数、field 和字节预算，成功后按原序返回。
pub(super) fn check_hash_entries(
    entries: HashEntries,
    config: &RedisConfig,
) -> Result<HashEntries, RedisError> {
    // 驱动已检查配对形状，先判项数，再检查每对的 field、单值和累计字节。
    if entries.len() > config.max_collection_items {
        return Err(RedisError::CollectionTooLarge {
            limit: config.max_collection_items,
        });
    }
    let mut response_bytes = 0;
    for (field, value) in &entries {
        if field.is_empty() || field.len() > config.max_key_bytes {
            return Err(RedisError::InvalidField);
        }
        response_bytes = add_response_part(response_bytes, field.len(), config)?;
        response_bytes = commands::add_response_bytes(response_bytes, value, config)?;
    }
    Ok(entries)
}

/// 按原顺序检查集合项数和响应字节，再逐项解码，失败时不返回部分结果。
pub(super) fn decode_collection<T: DeserializeOwned>(
    values: Vec<Vec<u8>>,
    config: &RedisConfig,
) -> Result<Vec<T>, RedisError> {
    // 数量超限优先于逐项字节及 MessagePack 检查。
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

/// 将 Hash field 字节计入累计响应预算；field 大小已按 key 限制单独校验。
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
