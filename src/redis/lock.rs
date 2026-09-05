//! Redis 单键租约锁。

mod asynchronous;
mod common;
mod sync;
#[cfg(test)]
mod tests;

#[cfg(feature = "redis-async")]
pub use asynchronous::RedisAsyncLockGuard;
pub use sync::RedisLockGuard;

pub(crate) use common::{acquire_command, lock_ttl_millis, release_command, renew_command, token};
#[cfg(test)]
use common::{
    finish_release, finish_renew, lock_ttl_duration, script_result, token_with_rng, RELEASE_SCRIPT,
    RENEW_SCRIPT,
};
