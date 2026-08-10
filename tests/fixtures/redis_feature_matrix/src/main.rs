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
    let _ = RedisClient::try_lock::<&str>;
    let _ = RedisLockGuard::release;
    let _ = RedisLockGuard::renew;
    let _ = RedisUtils::try_lock::<&str>;
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

    let config = RedisConfig::single("redis://127.0.0.1:6379/0").expect("fixture config");
    let client = RedisClient::new(config).expect("fixture client");

    let _: Result<Option<u8>, RedisError> = client.get_async("fixture:key").await;
    let _: Result<(), RedisError> = client.set_async("fixture:key", 1_u8).await;
    let _: Option<RedisAsyncLockGuard> = None;
    let _: Option<ModuleRedisAsyncLockGuard> = None;
    let _ = RedisClient::try_lock_async::<&str>;
    let _ = RedisAsyncLockGuard::release;
    let _ = RedisAsyncLockGuard::renew;
    let _ = RedisUtils::try_lock_async::<&str>;
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
    feature = "negative-redis-config"
)))]
fn main() {}
