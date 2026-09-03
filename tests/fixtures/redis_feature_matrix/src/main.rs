#[cfg(any(
    feature = "redis",
    feature = "redis-tokio",
    feature = "redis-serde",
    feature = "redis-tokio-serde",
    feature = "all"
))]
fn compile_sync_api() {
    use axutils::{
        RedisClient, RedisConfig, RedisError, RedisLockGuard, RedisTransaction,
        RedisTransportErrorKind, RedisUtils,
    };

    use axutils::redis::{
        RedisClient as ModuleRedisClient, RedisConfig as ModuleRedisConfig,
        RedisError as ModuleRedisError, RedisLockGuard as ModuleRedisLockGuard,
        RedisTransaction as ModuleRedisTransaction,
        RedisTransportErrorKind as ModuleRedisTransportErrorKind,
    };
    use axutils::utils::redis_utils::RedisUtils as NestedRedisUtils;
    use axutils::utils::RedisUtils as UtilsRedisUtils;

    let config = RedisConfig::single("redis://example.com:6379/0").expect("fixture config");
    let client = RedisClient::new(config).expect("fixture client");

    let _: Result<Option<u8>, RedisError> = client.get("fixture:key");
    let _: Result<(), RedisError> = client.set("fixture:key", 1_u8);

    let _: Option<ModuleRedisClient> = None;
    let _: Option<ModuleRedisConfig> = None;
    let _: Option<ModuleRedisError> = None;
    let _: Option<ModuleRedisTransaction> = None;
    let _: Option<ModuleRedisTransportErrorKind> = None;
    let _: Option<ModuleRedisLockGuard> = None;
    let _: Option<RedisTransaction> = None;
    let _: Option<RedisTransportErrorKind> = None;
    let _: Option<RedisLockGuard> = None;
    let _: Option<UtilsRedisUtils> = None;
    let _: Option<NestedRedisUtils> = None;
    let _ = RedisClient::get::<&str, u8>;
    let _ = RedisClient::get_bytes::<&str>;
    let _ = RedisClient::set::<&str, u8>;
    let _ = RedisClient::set_bytes::<&str, Vec<u8>>;
    let _ = RedisClient::set_with_expiry::<&str, u8>;
    let _ = RedisClient::set_bytes_with_expiry::<&str, Vec<u8>>;
    let _ = RedisClient::set_nx::<&str, u8>;
    let _ = RedisClient::set_nx_with_expiry::<&str, u8>;
    let _ = RedisClient::try_lock::<&str>;
    let _ = RedisClient::set_bytes_nx::<&str, Vec<u8>>;
    let _ = RedisClient::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
    let _ = RedisClient::delete::<&str>;
    let _ = RedisClient::delete_many::<[&str; 1], &str>;
    let _ = RedisClient::exists::<&str>;
    let _ = RedisClient::mget::<[&str; 1], &str, u8>;
    let _ = RedisClient::mget_bytes::<[&str; 1], &str>;
    let _ = RedisClient::mset::<[(&str, u8); 1], &str, u8>;
    let _ = RedisClient::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    let _ = RedisClient::hget::<&str, &str, u8>;
    let _ = RedisClient::hget_bytes::<&str, &str>;
    let _ = RedisClient::hset::<&str, &str, u8>;
    let _ = RedisClient::hset_bytes::<&str, &str, Vec<u8>>;
    let _ = RedisClient::hgetall::<&str, u8>;
    let _ = RedisClient::hgetall_bytes::<&str>;
    let _ = RedisClient::hdel::<&str, &str>;
    let _ = RedisClient::hexists::<&str, &str>;
    let _ = RedisClient::hlen::<&str>;
    let _ = RedisClient::hset_many::<[(&str, u8); 1], &str, &str, u8>;
    let _ = RedisClient::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    let _ = RedisClient::expire::<&str>;
    let _ = RedisClient::pexpire::<&str>;
    let _ = RedisClient::persist::<&str>;
    let _ = RedisClient::ttl::<&str>;
    let _ = RedisClient::pttl::<&str>;
    let _ = RedisClient::incr::<&str>;
    let _ = RedisClient::incr_by::<&str>;
    let _ = RedisClient::decr::<&str>;
    let _ = RedisClient::decr_by::<&str>;
    let _ = RedisClient::lpush::<&str, u8>;
    let _ = RedisClient::rpush::<&str, u8>;
    let _ = RedisClient::lpop::<&str, u8>;
    let _ = RedisClient::rpop::<&str, u8>;
    let _ = RedisClient::lrange::<&str, u8>;
    let _ = RedisClient::sadd::<&str, u8>;
    let _ = RedisClient::srem::<&str, u8>;
    let _ = RedisClient::sismember::<&str, u8>;
    let _ = RedisClient::smembers::<&str, u8>;
    let _ = RedisClient::ping;
    let _ = RedisLockGuard::release;
    let _ = RedisLockGuard::renew;
    let _ = RedisClient::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    let _ = RedisUtils::get::<&str, u8>;
    let _ = RedisUtils::get_bytes::<&str>;
    let _ = RedisUtils::set::<&str, u8>;
    let _ = RedisUtils::set_bytes::<&str, Vec<u8>>;
    let _ = RedisUtils::set_with_expiry::<&str, u8>;
    let _ = RedisUtils::set_bytes_with_expiry::<&str, Vec<u8>>;
    let _ = RedisUtils::set_nx::<&str, u8>;
    let _ = RedisUtils::set_nx_with_expiry::<&str, u8>;
    let _ = RedisUtils::try_lock::<&str>;
    let _ = RedisUtils::set_bytes_nx::<&str, Vec<u8>>;
    let _ = RedisUtils::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
    let _ = RedisUtils::delete::<&str>;
    let _ = RedisUtils::delete_many::<[&str; 1], &str>;
    let _ = RedisUtils::exists::<&str>;
    let _ = RedisUtils::mget::<[&str; 1], &str, u8>;
    let _ = RedisUtils::mget_bytes::<[&str; 1], &str>;
    let _ = RedisUtils::mset::<[(&str, u8); 1], &str, u8>;
    let _ = RedisUtils::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    let _ = RedisUtils::hget::<&str, &str, u8>;
    let _ = RedisUtils::hget_bytes::<&str, &str>;
    let _ = RedisUtils::hset::<&str, &str, u8>;
    let _ = RedisUtils::hset_bytes::<&str, &str, Vec<u8>>;
    let _ = RedisUtils::hgetall::<&str, u8>;
    let _ = RedisUtils::hgetall_bytes::<&str>;
    let _ = RedisUtils::hdel::<&str, &str>;
    let _ = RedisUtils::hexists::<&str, &str>;
    let _ = RedisUtils::hlen::<&str>;
    let _ = RedisUtils::hset_many::<[(&str, u8); 1], &str, &str, u8>;
    let _ = RedisUtils::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    let _ = RedisUtils::expire::<&str>;
    let _ = RedisUtils::pexpire::<&str>;
    let _ = RedisUtils::persist::<&str>;
    let _ = RedisUtils::ttl::<&str>;
    let _ = RedisUtils::pttl::<&str>;
    let _ = RedisUtils::incr::<&str>;
    let _ = RedisUtils::incr_by::<&str>;
    let _ = RedisUtils::decr::<&str>;
    let _ = RedisUtils::decr_by::<&str>;
    let _ = RedisUtils::lpush::<&str, u8>;
    let _ = RedisUtils::rpush::<&str, u8>;
    let _ = RedisUtils::lpop::<&str, u8>;
    let _ = RedisUtils::rpop::<&str, u8>;
    let _ = RedisUtils::lrange::<&str, u8>;
    let _ = RedisUtils::sadd::<&str, u8>;
    let _ = RedisUtils::srem::<&str, u8>;
    let _ = RedisUtils::sismember::<&str, u8>;
    let _ = RedisUtils::smembers::<&str, u8>;
    let _ = RedisUtils::ping;
    let _ = RedisUtils::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    let _ = RedisUtils::is_initialized();
}

#[cfg(any(
    feature = "redis-tokio",
    feature = "redis-tokio-serde",
    feature = "all"
))]
async fn compile_async_api() {
    use axutils::{RedisAsyncLockGuard, RedisClient, RedisConfig, RedisError, RedisUtils};

    use axutils::redis::RedisAsyncLockGuard as ModuleRedisAsyncLockGuard;

    let _ = RedisUtils::init_async;
    let _ = axutils::utils::RedisUtils::init_async;
    let _ = axutils::utils::redis_utils::RedisUtils::init_async;

    let config = RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config");
    let client = RedisClient::new(config).expect("fixture client");

    let _: Result<Option<u8>, RedisError> = client.get_async("fixture:key").await;
    let _: Result<(), RedisError> = client.set_async("fixture:key", 1_u8).await;
    let _: Option<RedisAsyncLockGuard> = None;
    let _: Option<ModuleRedisAsyncLockGuard> = None;
    let _ = RedisClient::get_async::<&str, u8>;
    let _ = RedisClient::get_bytes_async::<&str>;
    let _ = RedisClient::set_async::<&str, u8>;
    let _ = RedisClient::set_bytes_async::<&str, Vec<u8>>;
    let _ = RedisClient::set_with_expiry_async::<&str, u8>;
    let _ = RedisClient::set_bytes_with_expiry_async::<&str, Vec<u8>>;
    let _ = RedisClient::set_nx_async::<&str, u8>;
    let _ = RedisClient::set_nx_with_expiry_async::<&str, u8>;
    let _ = RedisClient::try_lock_async::<&str>;
    let _ = RedisClient::set_bytes_nx_async::<&str, Vec<u8>>;
    let _ = RedisClient::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
    let _ = RedisClient::delete_async::<&str>;
    let _ = RedisClient::delete_many_async::<[&str; 1], &str>;
    let _ = RedisClient::exists_async::<&str>;
    let _ = RedisClient::mget_async::<[&str; 1], &str, u8>;
    let _ = RedisClient::mget_bytes_async::<[&str; 1], &str>;
    let _ = RedisClient::mset_async::<[(&str, u8); 1], &str, u8>;
    let _ = RedisClient::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    let _ = RedisClient::hget_async::<&str, &str, u8>;
    let _ = RedisClient::hget_bytes_async::<&str, &str>;
    let _ = RedisClient::hset_async::<&str, &str, u8>;
    let _ = RedisClient::hset_bytes_async::<&str, &str, Vec<u8>>;
    let _ = RedisClient::hgetall_async::<&str, u8>;
    let _ = RedisClient::hgetall_bytes_async::<&str>;
    let _ = RedisClient::hdel_async::<&str, &str>;
    let _ = RedisClient::hexists_async::<&str, &str>;
    let _ = RedisClient::hlen_async::<&str>;
    let _ = RedisClient::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
    let _ = RedisClient::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    let _ = RedisClient::expire_async::<&str>;
    let _ = RedisClient::pexpire_async::<&str>;
    let _ = RedisClient::persist_async::<&str>;
    let _ = RedisClient::ttl_async::<&str>;
    let _ = RedisClient::pttl_async::<&str>;
    let _ = RedisClient::incr_async::<&str>;
    let _ = RedisClient::incr_by_async::<&str>;
    let _ = RedisClient::decr_async::<&str>;
    let _ = RedisClient::decr_by_async::<&str>;
    let _ = RedisClient::lpush_async::<&str, u8>;
    let _ = RedisClient::rpush_async::<&str, u8>;
    let _ = RedisClient::lpop_async::<&str, u8>;
    let _ = RedisClient::rpop_async::<&str, u8>;
    let _ = RedisClient::lrange_async::<&str, u8>;
    let _ = RedisClient::sadd_async::<&str, u8>;
    let _ = RedisClient::srem_async::<&str, u8>;
    let _ = RedisClient::sismember_async::<&str, u8>;
    let _ = RedisClient::smembers_async::<&str, u8>;
    let _ = RedisClient::ping_async;
    let _ = RedisAsyncLockGuard::release;
    let _ = RedisAsyncLockGuard::renew;
    let _ = RedisClient::transaction_async::<fn(&mut axutils::RedisTransaction) -> Result<(), RedisError>>;
    let _ = RedisUtils::get_async::<&str, u8>;
    let _ = RedisUtils::get_bytes_async::<&str>;
    let _ = RedisUtils::set_async::<&str, u8>;
    let _ = RedisUtils::set_bytes_async::<&str, Vec<u8>>;
    let _ = RedisUtils::set_with_expiry_async::<&str, u8>;
    let _ = RedisUtils::set_bytes_with_expiry_async::<&str, Vec<u8>>;
    let _ = RedisUtils::set_nx_async::<&str, u8>;
    let _ = RedisUtils::set_nx_with_expiry_async::<&str, u8>;
    let _ = RedisUtils::try_lock_async::<&str>;
    let _ = RedisUtils::set_bytes_nx_async::<&str, Vec<u8>>;
    let _ = RedisUtils::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
    let _ = RedisUtils::delete_async::<&str>;
    let _ = RedisUtils::delete_many_async::<[&str; 1], &str>;
    let _ = RedisUtils::exists_async::<&str>;
    let _ = RedisUtils::mget_async::<[&str; 1], &str, u8>;
    let _ = RedisUtils::mget_bytes_async::<[&str; 1], &str>;
    let _ = RedisUtils::mset_async::<[(&str, u8); 1], &str, u8>;
    let _ = RedisUtils::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    let _ = RedisUtils::hget_async::<&str, &str, u8>;
    let _ = RedisUtils::hget_bytes_async::<&str, &str>;
    let _ = RedisUtils::hset_async::<&str, &str, u8>;
    let _ = RedisUtils::hset_bytes_async::<&str, &str, Vec<u8>>;
    let _ = RedisUtils::hgetall_async::<&str, u8>;
    let _ = RedisUtils::hgetall_bytes_async::<&str>;
    let _ = RedisUtils::hdel_async::<&str, &str>;
    let _ = RedisUtils::hexists_async::<&str, &str>;
    let _ = RedisUtils::hlen_async::<&str>;
    let _ = RedisUtils::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
    let _ = RedisUtils::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    let _ = RedisUtils::expire_async::<&str>;
    let _ = RedisUtils::pexpire_async::<&str>;
    let _ = RedisUtils::persist_async::<&str>;
    let _ = RedisUtils::ttl_async::<&str>;
    let _ = RedisUtils::pttl_async::<&str>;
    let _ = RedisUtils::incr_async::<&str>;
    let _ = RedisUtils::incr_by_async::<&str>;
    let _ = RedisUtils::decr_async::<&str>;
    let _ = RedisUtils::decr_by_async::<&str>;
    let _ = RedisUtils::lpush_async::<&str, u8>;
    let _ = RedisUtils::rpush_async::<&str, u8>;
    let _ = RedisUtils::lpop_async::<&str, u8>;
    let _ = RedisUtils::rpop_async::<&str, u8>;
    let _ = RedisUtils::lrange_async::<&str, u8>;
    let _ = RedisUtils::sadd_async::<&str, u8>;
    let _ = RedisUtils::srem_async::<&str, u8>;
    let _ = RedisUtils::sismember_async::<&str, u8>;
    let _ = RedisUtils::smembers_async::<&str, u8>;
    let _ = RedisUtils::ping_async;
    let _ = RedisUtils::transaction_async::<fn(&mut axutils::RedisTransaction) -> Result<(), RedisError>>;
}

#[cfg(any(
    feature = "redis-tokio",
    feature = "redis-tokio-serde",
    feature = "all"
))]
fn main() {
    compile_sync_api();
    let _ = compile_async_api;
}

#[cfg(all(
    any(feature = "redis", feature = "redis-serde"),
    not(any(
        feature = "redis-tokio",
        feature = "redis-tokio-serde",
        feature = "all"
    ))
))]
fn main() {
    compile_sync_api();
}

#[cfg(any(feature = "none", feature = "tokio-only"))]
fn main() {}

#[cfg(feature = "negative-no-redis-module")]
fn main() {
    let _ = axutils::redis::RedisClient::new;
}

#[cfg(feature = "negative-no-redis-root")]
fn main() {
    let _ = axutils::RedisClient::new;
}

#[cfg(feature = "negative-no-redis-utils")]
fn main() {
    let _ = axutils::RedisUtils::is_initialized;
}

#[cfg(feature = "negative-tokio-redis-module")]
fn main() {
    let _ = axutils::redis::RedisClient::new;
}

#[cfg(feature = "negative-tokio-redis-root")]
fn main() {
    let _ = axutils::RedisClient::new;
}

#[cfg(feature = "negative-tokio-redis-utils")]
fn main() {
    let _ = axutils::RedisUtils::is_initialized;
}

#[cfg(feature = "negative-redis-async")]
fn main() {
    let _ = axutils::RedisClient::get_async;
}

#[cfg(feature = "negative-redis-async-lock")]
fn main() {
    let _ = axutils::RedisAsyncLockGuard::release;
}

#[cfg(feature = "negative-redis-utils-async")]
fn main() {
    let _ = axutils::RedisUtils::get_async;
}

#[cfg(feature = "negative-redis-config")]
fn main() {
    let _ = axutils::config::ConfigLoader::new;
}

#[cfg(feature = "negative-redis-utils-init-async")]
fn main() {
    let _ = axutils::RedisUtils::init_async;
}

#[cfg(not(any(
    feature = "none",
    feature = "tokio-only",
    feature = "redis",
    feature = "redis-tokio",
    feature = "redis-serde",
    feature = "redis-tokio-serde",
    feature = "all",
    feature = "negative-no-redis-module",
    feature = "negative-no-redis-root",
    feature = "negative-no-redis-utils",
    feature = "negative-tokio-redis-module",
    feature = "negative-tokio-redis-root",
    feature = "negative-tokio-redis-utils",
    feature = "negative-redis-async",
    feature = "negative-redis-async-lock",
    feature = "negative-redis-utils-async",
    feature = "negative-redis-utils-init-async",
    feature = "negative-redis-config"
)))]
fn main() {}
