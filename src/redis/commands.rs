use std::time::Duration;

use super::{codec, config::RedisConfig, error::RedisError};

pub(crate) fn key<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    let value = value.as_ref();
    if value.is_empty() || value.len() > config.max_key_bytes {
        return Err(RedisError::InvalidKey);
    }
    Ok(value.to_vec())
}

pub(crate) fn field<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    let value = value.as_ref();
    if value.is_empty() || value.len() > config.max_key_bytes {
        return Err(RedisError::InvalidField);
    }
    Ok(value.to_vec())
}

pub(crate) fn encoded<T: serde::Serialize>(
    value: &T,
    config: &RedisConfig,
) -> Result<Vec<u8>, RedisError> {
    codec::encode(value, config.max_value_bytes)
}

pub(crate) fn raw<T: AsRef<[u8]>>(value: T, config: &RedisConfig) -> Result<Vec<u8>, RedisError> {
    codec::raw(value, config.max_value_bytes)
}

pub(crate) fn command(name: &'static str, args: impl IntoIterator<Item = Vec<u8>>) -> ::redis::Cmd {
    let mut command = ::redis::cmd(name);
    for arg in args {
        command.arg(arg);
    }
    command
}

pub(crate) fn add_batch_bytes(
    current: usize,
    additional: usize,
    config: &RedisConfig,
) -> Result<usize, RedisError> {
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

pub(crate) fn add_transaction_bytes(
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, RedisError> {
    let total = current
        .checked_add(additional)
        .ok_or(RedisError::ValueTooLarge { limit })?;
    if total > limit {
        return Err(RedisError::ValueTooLarge { limit });
    }
    Ok(total)
}

pub(crate) fn check_value_response(bytes: &[u8], config: &RedisConfig) -> Result<(), RedisError> {
    if bytes.len() > config.max_value_bytes {
        Err(RedisError::ValueTooLarge {
            limit: config.max_value_bytes,
        })
    } else {
        Ok(())
    }
}

pub(crate) fn add_response_bytes(
    current: usize,
    bytes: &[u8],
    config: &RedisConfig,
) -> Result<usize, RedisError> {
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

pub(crate) fn check_lrange_request(
    start: isize,
    stop: isize,
    config: &RedisConfig,
) -> Result<(), RedisError> {
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

pub(crate) fn duration_millis(duration: Duration) -> Result<i64, RedisError> {
    if duration.is_zero() {
        return Err(RedisError::invalid_config("ttl"));
    }
    let nanos = duration.as_nanos();
    let millis = nanos
        .checked_add(999_999)
        .map(|value| value / 1_000_000)
        .ok_or(RedisError::invalid_config("ttl"))?;
    i64::try_from(millis).map_err(|_| RedisError::invalid_config("ttl"))
}

pub(crate) fn duration_seconds(duration: Duration) -> Result<i64, RedisError> {
    if duration.is_zero() {
        return Err(RedisError::invalid_config("ttl"));
    }
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
