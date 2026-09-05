use std::time::Duration;

use super::{MAX_ENDPOINT_BYTES, MAX_TIMEOUT, MIN_TIMEOUT};
use crate::redis::RedisError;

pub(super) fn validate_endpoint(endpoint: &str) -> Result<(), RedisError> {
    if endpoint.is_empty() {
        return Err(RedisError::invalid_config("url"));
    }
    if endpoint.len() > MAX_ENDPOINT_BYTES || endpoint.chars().any(char::is_control) {
        return Err(RedisError::invalid_config("url"));
    }
    if !endpoint.starts_with("redis://") {
        return Err(RedisError::invalid_config("scheme"));
    }
    Ok(())
}

pub(super) fn validate_database(endpoint: &str, cluster: bool) -> Result<(), RedisError> {
    let after_scheme = &endpoint["redis://".len()..];
    let Some(slash) = after_scheme.find('/') else {
        return Ok(());
    };
    let database = &after_scheme[slash + 1..];
    let database = database.split(['?', '#']).next().unwrap_or_default();
    if database.is_empty() {
        return Ok(());
    }
    let Ok(number) = database.parse::<i64>() else {
        return Err(RedisError::invalid_config("database"));
    };
    if number < 0 || (cluster && number != 0) {
        return Err(RedisError::invalid_config("database"));
    }
    Ok(())
}

#[cfg(feature = "redis-cluster")]
pub(super) fn endpoint_credentials(endpoint: &str) -> Option<&str> {
    let authority = endpoint["redis://".len()..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    authority.rfind('@').map(|at| &authority[..at])
}

pub(super) fn bounded(
    value: usize,
    min: usize,
    max: usize,
    field: &'static str,
) -> Result<usize, RedisError> {
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(RedisError::invalid_config(field))
    }
}

pub(super) fn checked_timeout(
    value: Duration,
    field: &'static str,
) -> Result<Duration, RedisError> {
    if (MIN_TIMEOUT..=MAX_TIMEOUT).contains(&value) {
        Ok(value)
    } else {
        Err(RedisError::invalid_config(field))
    }
}
