use std::time::Duration;

#[cfg(feature = "redis-cluster")]
use ::redis::cluster_routing::Slot;

use super::{codec, config::RedisConfig, error::RedisError};

/// 在全部输入校验通过后，拒绝 Cluster 多 key 命令被上游拆成非原子的跨 slot 操作。
/// 单机没有 slot 限制；使用上游算法处理二进制 key 与 Redis hash tag。
pub(crate) fn check_key_slots<'a>(
    keys: impl IntoIterator<Item = &'a [u8]>,
    config: &RedisConfig,
) -> Result<(), RedisError> {
    #[cfg(feature = "redis-cluster")]
    if config.is_cluster() {
        // 比较 slot 而非节点地址：同一节点可能同时负责多个仍不可组成原子命令的 slot。
        let mut slots = keys.into_iter().map(Slot::for_key);
        if slots
            .next()
            .is_some_and(|first| slots.any(|slot| slot != first))
        {
            return Err(RedisError::CrossSlot);
        }
    }
    #[cfg(not(feature = "redis-cluster"))]
    let _ = (keys, config);
    Ok(())
}

/// 校验一个非空 key 的字节预算，再复制为命令拥有的参数。
pub(crate) fn key<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    // 先验证借用切片，只有合法输入才分配拥有型参数。
    let value = value.as_ref();
    if value.is_empty() || value.len() > config.max_key_bytes {
        return Err(RedisError::InvalidKey);
    }
    Ok(value.to_vec())
}

/// 校验一个非空 Hash field，沿用 key 字节预算但保留独立错误分类。
pub(crate) fn field<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    // 先验证借用切片，只有合法输入才分配拥有型参数。
    let value = value.as_ref();
    if value.is_empty() || value.len() > config.max_key_bytes {
        return Err(RedisError::InvalidField);
    }
    Ok(value.to_vec())
}

/// 使用当前单值预算执行有界 MessagePack 序列化。
pub(crate) fn encoded<T: serde::Serialize>(
    value: &T,
    config: &RedisConfig,
) -> Result<Vec<u8>, RedisError> {
    // 编码器在写入过程中执行字节预算检查，避免先产生无界编码结果。
    codec::encode(value, config.max_value_bytes)
}

/// 按单值字节预算检查并复制 raw 参数，不改变二进制内容。
pub(crate) fn raw<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    // raw 校验与复制由 codec 统一执行，调用方无需临时编码。
    codec::raw(value, config.max_value_bytes)
}

/// 按既定顺序装配一个固定名称的命令；参数已经由调用方校验。
pub(crate) fn command(name: &'static str, args: impl IntoIterator<Item = Vec<u8>>) -> ::redis::Cmd {
    // 每项保持独立 RESP 参数，不能把二进制值拼接成命令文本。
    let mut command = ::redis::cmd(name);
    for arg in args {
        command.arg(arg);
    }
    command
}

/// 累计批量参数有效载荷字节数，拒绝整数溢出和配置预算超限。
pub(crate) fn add_batch_bytes(
    current: usize,
    additional: usize,
    config: &RedisConfig,
) -> Result<usize, RedisError> {
    // 溢出与超预算统一视为输入过大；返回成功后调用方才推进累计状态。
    let total = current
        .checked_add(additional)
        .ok_or(RedisError::ValueTooLarge {
            limit: config.max_batch_bytes,
        })?;
    if total > config.max_batch_bytes {
        return Err(RedisError::ValueTooLarge {
            limit: config.max_batch_bytes,
        });
    }
    Ok(total)
}

/// 累计事务的 RESP 编码字节数，失败时不更新调用方的排队状态。
pub(crate) fn add_transaction_bytes(
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, RedisError> {
    // 溢出与超预算统一视为输入过大；返回成功后调用方才推进累计状态。
    let total = current
        .checked_add(additional)
        .ok_or(RedisError::ValueTooLarge { limit })?;
    if total > limit {
        return Err(RedisError::ValueTooLarge { limit });
    }
    Ok(total)
}

/// 检查已经由后端接收的单值响应大小；此限制不是网络解析器的分配上限。
pub(crate) fn check_value_response(bytes: &[u8], config: &RedisConfig) -> Result<(), RedisError> {
    // 后端已经拥有这些响应字节；这里只决定该值能否进入公开返回结果。
    if bytes.len() > config.max_value_bytes {
        Err(RedisError::ValueTooLarge {
            limit: config.max_value_bytes,
        })
    } else {
        Ok(())
    }
}

/// 先检查单值，再累计多值响应字节预算，保留单值错误优先级。
pub(crate) fn add_response_bytes(
    current: usize,
    bytes: &[u8],
    config: &RedisConfig,
) -> Result<usize, RedisError> {
    // 先维持单值限制，再使用 checked_add 维护整批响应预算。
    check_value_response(bytes, config)?;
    let total = current
        .checked_add(bytes.len())
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

/// 对可在本地计算的非负列表闭区间预检项数；其他区间由响应检查兜底。
pub(crate) fn check_lrange_request(
    start: isize,
    stop: isize,
    config: &RedisConfig,
) -> Result<(), RedisError> {
    // 负下标依赖远端列表长度，无法精确预估；可计算区间按 Redis 闭区间规则计数。
    if start >= 0 && stop >= start {
        let count = stop
            .checked_sub(start)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(RedisError::CollectionTooLarge {
                limit: config.max_collection_items,
            })?;
        if count > config.max_collection_items {
            return Err(RedisError::CollectionTooLarge {
                limit: config.max_collection_items,
            });
        }
    }
    Ok(())
}

/// 将正 duration 向上取整为 Redis TTL 毫秒，拒绝不能表示为 i64 的值。
pub(crate) fn duration_millis(duration: Duration) -> Result<i64, RedisError> {
    if duration.is_zero() {
        return Err(RedisError::invalid_config("ttl"));
    }
    // 向上取整避免正但不足一个单位的 TTL 变成零，并检查后端整数可表示性。
    let nanos = duration.as_nanos();
    let millis = nanos
        .checked_add(999_999)
        .map(|value| value / 1_000_000)
        .ok_or(RedisError::invalid_config("ttl"))?;
    i64::try_from(millis).map_err(|_| RedisError::invalid_config("ttl"))
}

/// 将正 duration 向上取整为 Redis TTL 秒数，拒绝不能表示为 i64 的值。
pub(crate) fn duration_seconds(duration: Duration) -> Result<i64, RedisError> {
    if duration.is_zero() {
        return Err(RedisError::invalid_config("ttl"));
    }
    // 向上取整避免正但不足一个单位的 TTL 变成零，并检查后端整数可表示性。
    let nanos = duration.as_nanos();
    let seconds = nanos
        .checked_add(999_999_999)
        .map(|value| value / 1_000_000_000)
        .ok_or(RedisError::invalid_config("ttl"))?;
    i64::try_from(seconds).map_err(|_| RedisError::invalid_config("ttl"))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use redis_test::{MockCmd, MockRedisConnection};

    use super::{check_lrange_request, command, duration_millis, duration_seconds};
    use crate::redis::{RedisConfig, RedisError};

    #[test]
    fn ttl_conversions_round_up_and_reject_overflow() {
        assert_eq!(duration_millis(Duration::from_nanos(1)).unwrap(), 1);
        assert_eq!(duration_seconds(Duration::from_millis(1)).unwrap(), 1);
        assert_eq!(
            duration_millis(Duration::ZERO),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
        assert_eq!(
            duration_millis(Duration::MAX),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
        assert_eq!(
            duration_seconds(Duration::MAX),
            Err(RedisError::InvalidConfig { field: "ttl" })
        );
    }

    #[test]
    fn lrange_prechecks_only_calculable_nonnegative_ranges() {
        let config = RedisConfig::single("redis://127.0.0.1:6379/0")
            .unwrap()
            .with_max_collection_items(2)
            .unwrap();
        assert_eq!(
            check_lrange_request(0, 2, &config),
            Err(RedisError::CollectionTooLarge { limit: 2 })
        );
        assert!(check_lrange_request(-3, -1, &config).is_ok());
        assert!(check_lrange_request(-3, 1, &config).is_ok());
    }

    #[test]
    fn command_adapter_preserves_binary_arguments_and_response_conversion() {
        let mut connection = MockRedisConnection::new([
            MockCmd::new(
                ::redis::cmd("SET")
                    .arg("binary-key")
                    .arg(&[0_u8, 255, 1][..]),
                Ok(""),
            ),
            MockCmd::new(::redis::cmd("PTTL").arg("binary-key"), Ok(42_i64)),
        ])
        .assert_all_commands_consumed();

        command("SET", [b"binary-key".to_vec(), vec![0_u8, 255, 1]])
            .exec(&mut connection)
            .expect("mock SET command should match");
        let ttl: i64 = command("PTTL", [b"binary-key".to_vec()])
            .query(&mut connection)
            .expect("mock PTTL response should convert");
        assert_eq!(ttl, 42);
    }
}
