//! Redis 一次初始化进程级便捷入口。

use std::sync::OnceLock;

use serde::{de::DeserializeOwned, Serialize};

use crate::redis::{RedisClient, RedisConfig, RedisError, RedisLockGuard, RedisTransaction};

#[cfg(all(feature = "redis", feature = "tokio"))]
use crate::redis::RedisAsyncLockGuard;

static REDIS_CLIENT: OnceLock<RedisClient> = OnceLock::new();

/// 单默认 Redis 客户端的进程级便捷入口。
///
/// 必须先成功调用 [`Self::init`]。初始化成功后只能保留第一个客户端，不能 reset、replace
/// 或读取连接 URL/凭据；需要多个配置或可控生命周期时，直接持有多个 [`RedisClient`]。
pub struct RedisUtils;

impl RedisUtils {
    /// 初始化全局 Redis 客户端。
    ///
    /// 该调用只执行本地配置、客户端和惰性连接池构造，不发送 PING 或建立网络连接；非法
    /// 配置不会占用初始化机会，成功后再次调用返回 [`RedisError::AlreadyInitialized`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisConfig, RedisUtils};
    ///
    /// let config = RedisConfig::single("redis://127.0.0.1:6379/0")?;
    /// RedisUtils::init(config)?;
    /// # Ok::<(), axutils::RedisError>(())
    /// ```
    pub fn init(config: RedisConfig) -> Result<(), RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = match RedisClient::new(config) {
            Ok(client) => REDIS_CLIENT
                .set(client)
                .map_err(|_| RedisError::AlreadyInitialized),
            Err(error) => Err(error),
        };
        #[cfg(feature = "tracing")]
        crate::tracing::redis::record_client_init(&result, started);
        result
    }

    /// 返回全局 Redis 客户端是否已经成功初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::is_initialized();
    /// ```
    pub fn is_initialized() -> bool {
        REDIS_CLIENT.get().is_some()
    }

    pub(crate) fn client() -> Result<&'static RedisClient, RedisError> {
        REDIS_CLIENT.get().ok_or(RedisError::NotInitialized)
    }

    /// 读取 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::get::<&str, u8>;
    /// ```
    pub fn get<K: AsRef<[u8]>, T: DeserializeOwned>(key: K) -> Result<Option<T>, RedisError> {
        Self::client()?.get(key)
    }

    /// 读取 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::get_bytes::<&str>;
    /// ```
    pub fn get_bytes<K: AsRef<[u8]>>(key: K) -> Result<Option<Vec<u8>>, RedisError> {
        Self::client()?.get_bytes(key)
    }

    /// 写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set::<&str, u8>;
    /// ```
    pub fn set<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<(), RedisError> {
        Self::client()?.set(key, value)
    }

    /// 写入 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes<K: AsRef<[u8]>, V: AsRef<[u8]>>(key: K, value: V) -> Result<(), RedisError> {
        Self::client()?.set_bytes(key, value)
    }

    /// 使用原子 `SET ... PX` 写入带 TTL 的 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_with_expiry::<&str, u8>;
    /// ```
    pub fn set_with_expiry<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
        ttl: std::time::Duration,
    ) -> Result<(), RedisError> {
        Self::client()?.set_with_expiry(key, value, ttl)
    }

    /// 使用原子 `SET ... PX` 写入带 TTL 的 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_with_expiry::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
        ttl: std::time::Duration,
    ) -> Result<(), RedisError> {
        Self::client()?.set_bytes_with_expiry(key, value, ttl)
    }

    /// 仅在 key 不存在时写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_nx::<&str, u8>;
    /// ```
    pub fn set_nx<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<bool, RedisError> {
        Self::client()?.set_nx(key, value)
    }

    /// 仅在 key 不存在时写入带 TTL 的 MessagePack 值。
    ///
    /// 这是通用的 NX 写入，不会生成锁 token，也不会在 guard 被丢弃时自动释放；需要
    /// 所有权校验的单键租约锁请使用 [`RedisUtils::try_lock`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_nx_with_expiry::<&str, u8>;
    /// ```
    pub fn set_nx_with_expiry<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?.set_nx_with_expiry(key, value, ttl)
    }

    /// 尝试通过全局客户端获取单键租约锁。
    ///
    /// 全局客户端只是连接入口，不是进程内互斥锁；跨进程互斥由 Redis key、不可预测
    /// token 和 TTL 协议保证。返回的 guard 拥有 `RedisClient` clone，可以在该方法返回后
    /// 显式释放或续租。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    /// use std::time::Duration;
    ///
    /// let _ = RedisUtils::try_lock::<&str>;
    /// let _ = Duration::from_secs(30);
    /// ```
    pub fn try_lock<K: AsRef<[u8]>>(
        key: K,
        ttl: std::time::Duration,
    ) -> Result<Option<RedisLockGuard>, RedisError> {
        Self::client()?.try_lock(key, ttl)
    }

    /// 仅在 key 不存在时写入 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_nx::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_nx<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
    ) -> Result<bool, RedisError> {
        Self::client()?.set_bytes_nx(key, value)
    }

    /// 仅在 key 不存在时写入带 TTL 的 raw 值。
    ///
    /// 这是通用的 NX 写入，不会生成锁 token，也不会在 guard 被丢弃时自动释放；需要
    /// 所有权校验的单键租约锁请使用 [`RedisUtils::try_lock`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
    /// ```
    pub fn set_bytes_nx_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?.set_bytes_nx_with_expiry(key, value, ttl)
    }

    /// 删除一个 key。
    ///
    /// 此操作是无条件删除，不会校验租约 token；释放由 [`RedisUtils::try_lock`] 返回的锁
    /// 应使用 guard 的 `release` 方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::delete::<&str>;
    /// ```
    pub fn delete<K: AsRef<[u8]>>(key: K) -> Result<u64, RedisError> {
        Self::client()?.delete(key)
    }

    /// 有界批量删除 key。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::delete_many::<[&str; 1], &str>;
    /// ```
    pub fn delete_many<I, K>(keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        Self::client()?.delete_many(keys)
    }

    /// 判断 key 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::exists::<&str>;
    /// ```
    pub fn exists<K: AsRef<[u8]>>(key: K) -> Result<bool, RedisError> {
        Self::client()?.exists(key)
    }

    /// 按输入顺序批量读取 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mget::<[&str; 1], &str, u8>;
    /// ```
    pub fn mget<I, K, T>(keys: I) -> Result<Vec<Option<T>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
        T: DeserializeOwned,
    {
        Self::client()?.mget(keys)
    }

    /// 按输入顺序批量读取 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mget_bytes::<[&str; 1], &str>;
    /// ```
    pub fn mget_bytes<I, K>(keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        Self::client()?.mget_bytes(keys)
    }

    /// 有界批量写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mset::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub fn mset<I, K, T>(entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, T)>,
        K: AsRef<[u8]>,
        T: Serialize,
    {
        Self::client()?.mset(entries)
    }

    /// 有界批量写入 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub fn mset_bytes<I, K, V>(entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Self::client()?.mset_bytes(entries)
    }

    /// 读取 Hash 中的 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hget::<&str, &str, u8>;
    /// ```
    pub fn hget<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
        field: F,
    ) -> Result<Option<T>, RedisError> {
        Self::client()?.hget(key, field)
    }

    /// 读取 Hash 中的 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hget_bytes::<&str, &str>;
    /// ```
    pub fn hget_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        key: K,
        field: F,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        Self::client()?.hget_bytes(key, field)
    }

    /// 写入一个 MessagePack Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset::<&str, &str, u8>;
    /// ```
    pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        key: K,
        field: F,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.hset(key, field, value)
    }

    /// 写入一个 raw Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_bytes::<&str, &str, Vec<u8>>;
    /// ```
    pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        field: F,
        value: V,
    ) -> Result<u64, RedisError> {
        Self::client()?.hset_bytes(key, field, value)
    }

    /// 读取 Hash 全部 field 和 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hgetall::<&str, u8>;
    /// ```
    pub fn hgetall<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Vec<(Vec<u8>, T)>, RedisError> {
        Self::client()?.hgetall(key)
    }

    /// 读取 Hash 全部 field 和 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hgetall_bytes::<&str>;
    /// ```
    #[allow(clippy::type_complexity)]
    pub fn hgetall_bytes<K: AsRef<[u8]>>(key: K) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
        Self::client()?.hgetall_bytes(key)
    }

    /// 删除一个 Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hdel::<&str, &str>;
    /// ```
    pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(key: K, field: F) -> Result<u64, RedisError> {
        Self::client()?.hdel(key, field)
    }

    /// 判断 Hash field 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hexists::<&str, &str>;
    /// ```
    pub fn hexists<K: AsRef<[u8]>, F: AsRef<[u8]>>(key: K, field: F) -> Result<bool, RedisError> {
        Self::client()?.hexists(key, field)
    }

    /// 返回 Hash field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hlen::<&str>;
    /// ```
    pub fn hlen<K: AsRef<[u8]>>(key: K) -> Result<u64, RedisError> {
        Self::client()?.hlen(key)
    }

    /// 有界批量写入 MessagePack Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_many::<[(&str, u8); 1], &str, &str, u8>;
    /// ```
    pub fn hset_many<I, K, F, T>(key: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, T)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        T: Serialize,
    {
        Self::client()?.hset_many(key, entries)
    }

    /// 有界批量写入 raw Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    /// ```
    pub fn hset_many_bytes<I, K, F, V>(key: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, V)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Self::client()?.hset_many_bytes(key, entries)
    }

    /// 以秒为单位设置 key 的 TTL。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::expire::<&str>;
    /// ```
    pub fn expire<K: AsRef<[u8]>>(key: K, ttl: std::time::Duration) -> Result<bool, RedisError> {
        Self::client()?.expire(key, ttl)
    }

    /// 以毫秒为单位设置 key 的 TTL。
    ///
    /// 此操作是无条件设置 TTL，不会校验租约 token；续租由 [`RedisUtils::try_lock`] 返回的
    /// 锁应使用 guard 的 `renew` 方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::pexpire::<&str>;
    /// ```
    pub fn pexpire<K: AsRef<[u8]>>(key: K, ttl: std::time::Duration) -> Result<bool, RedisError> {
        Self::client()?.pexpire(key, ttl)
    }

    /// 删除 key 的 TTL。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::persist::<&str>;
    /// ```
    pub fn persist<K: AsRef<[u8]>>(key: K) -> Result<bool, RedisError> {
        Self::client()?.persist(key)
    }

    /// 返回 Redis 原生 TTL 秒数。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::ttl::<&str>;
    /// ```
    pub fn ttl<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.ttl(key)
    }

    /// 返回 Redis 原生 TTL 毫秒数。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::pttl::<&str>;
    /// ```
    pub fn pttl<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.pttl(key)
    }

    /// 将 key 作为 Redis 原生十进制整数加一。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::incr::<&str>;
    /// ```
    pub fn incr<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.incr(key)
    }

    /// 将 key 作为 Redis 原生十进制整数增加指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::incr_by::<&str>;
    /// ```
    pub fn incr_by<K: AsRef<[u8]>>(key: K, amount: i64) -> Result<i64, RedisError> {
        Self::client()?.incr_by(key, amount)
    }

    /// 将 key 作为 Redis 原生十进制整数减一。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::decr::<&str>;
    /// ```
    pub fn decr<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.decr(key)
    }

    /// 将 key 作为 Redis 原生十进制整数减少指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::decr_by::<&str>;
    /// ```
    pub fn decr_by<K: AsRef<[u8]>>(key: K, amount: i64) -> Result<i64, RedisError> {
        Self::client()?.decr_by(key, amount)
    }

    /// 从列表左侧压入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lpush::<&str, u8>;
    /// ```
    pub fn lpush<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<u64, RedisError> {
        Self::client()?.lpush(key, value)
    }

    /// 从列表右侧压入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::rpush::<&str, u8>;
    /// ```
    pub fn rpush<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<u64, RedisError> {
        Self::client()?.rpush(key, value)
    }

    /// 从列表左侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lpop::<&str, u8>;
    /// ```
    pub fn lpop<K: AsRef<[u8]>, T: DeserializeOwned>(key: K) -> Result<Option<T>, RedisError> {
        Self::client()?.lpop(key)
    }

    /// 从列表右侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::rpop::<&str, u8>;
    /// ```
    pub fn rpop<K: AsRef<[u8]>, T: DeserializeOwned>(key: K) -> Result<Option<T>, RedisError> {
        Self::client()?.rpop(key)
    }

    /// 读取列表范围。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lrange::<&str, u8>;
    /// ```
    pub fn lrange<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
        start: isize,
        stop: isize,
    ) -> Result<Vec<T>, RedisError> {
        Self::client()?.lrange(key, start, stop)
    }

    /// 向集合加入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::sadd::<&str, u8>;
    /// ```
    pub fn sadd<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<u64, RedisError> {
        Self::client()?.sadd(key, value)
    }

    /// 从集合移除一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::srem::<&str, u8>;
    /// ```
    pub fn srem<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<u64, RedisError> {
        Self::client()?.srem(key, value)
    }

    /// 判断集合是否包含一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::sismember::<&str, u8>;
    /// ```
    pub fn sismember<K: AsRef<[u8]>, T: Serialize>(key: K, value: T) -> Result<bool, RedisError> {
        Self::client()?.sismember(key, value)
    }

    /// 读取集合全部 MessagePack 成员。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::smembers::<&str, u8>;
    /// ```
    pub fn smembers<K: AsRef<[u8]>, T: DeserializeOwned>(key: K) -> Result<Vec<T>, RedisError> {
        Self::client()?.smembers(key)
    }

    /// 向 Redis 发送 `PING`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::ping;
    /// ```
    pub fn ping() -> Result<String, RedisError> {
        Self::client()?.ping()
    }

    /// 使用全局客户端执行单机原子事务。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisError, RedisTransaction, RedisUtils};
    ///
    /// let _ = RedisUtils::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub fn transaction<F>(callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        Self::client()?.transaction(callback)
    }
}

#[cfg(all(feature = "redis", feature = "tokio"))]
impl RedisUtils {
    /// 异步读取 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::get_async::<&str, u8>;
    /// ```
    pub async fn get_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Option<T>, RedisError> {
        Self::client()?.get_async(key).await
    }

    /// 异步读取 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::get_bytes_async::<&str>;
    /// ```
    pub async fn get_bytes_async<K: AsRef<[u8]>>(key: K) -> Result<Option<Vec<u8>>, RedisError> {
        Self::client()?.get_bytes_async(key).await
    }

    /// 异步写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_async::<&str, u8>;
    /// ```
    pub async fn set_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<(), RedisError> {
        Self::client()?.set_async(key, value).await
    }

    /// 异步写入 raw 字节。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
    ) -> Result<(), RedisError> {
        Self::client()?.set_bytes_async(key, value).await
    }

    /// 异步原子写入带 TTL 的 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_with_expiry_async::<&str, u8>;
    /// ```
    pub async fn set_with_expiry_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
        ttl: std::time::Duration,
    ) -> Result<(), RedisError> {
        Self::client()?.set_with_expiry_async(key, value, ttl).await
    }

    /// 异步原子写入带 TTL 的 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_with_expiry_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_with_expiry_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
        ttl: std::time::Duration,
    ) -> Result<(), RedisError> {
        Self::client()?
            .set_bytes_with_expiry_async(key, value, ttl)
            .await
    }

    /// 异步仅在 key 不存在时写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_nx_async::<&str, u8>;
    /// ```
    pub async fn set_nx_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<bool, RedisError> {
        Self::client()?.set_nx_async(key, value).await
    }

    /// 异步仅在 key 不存在时写入带 TTL 的 MessagePack 值。
    ///
    /// 这是通用的 NX 写入，不会生成锁 token，也不会在 guard 被丢弃时自动释放；需要
    /// 所有权校验的单键租约锁请使用 [`RedisUtils::try_lock_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_nx_with_expiry_async::<&str, u8>;
    /// ```
    pub async fn set_nx_with_expiry_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?
            .set_nx_with_expiry_async(key, value, ttl)
            .await
    }

    /// 异步尝试通过全局客户端获取单键租约锁。
    ///
    /// 全局客户端只是连接入口，不是进程内互斥锁；跨进程互斥由 Redis key、不可预测
    /// token 和 TTL 协议保证。返回的 guard 拥有 `RedisClient` clone；异步 guard 的 `Drop`
    /// 不会发起网络操作，正常路径必须显式 `await release()`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::try_lock_async::<&str>;
    /// ```
    pub async fn try_lock_async<K: AsRef<[u8]>>(
        key: K,
        ttl: std::time::Duration,
    ) -> Result<Option<RedisAsyncLockGuard>, RedisError> {
        Self::client()?.try_lock_async(key, ttl).await
    }

    /// 异步仅在 key 不存在时写入 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_nx_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_nx_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
    ) -> Result<bool, RedisError> {
        Self::client()?.set_bytes_nx_async(key, value).await
    }

    /// 异步仅在 key 不存在时写入带 TTL 的 raw 值。
    ///
    /// 这是通用的 NX 写入，不会生成锁 token，也不会在 guard 被丢弃时自动释放；需要
    /// 所有权校验的单键租约锁请使用 [`RedisUtils::try_lock_async`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
    /// ```
    pub async fn set_bytes_nx_with_expiry_async<K: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        value: V,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?
            .set_bytes_nx_with_expiry_async(key, value, ttl)
            .await
    }

    /// 异步删除一个 key。
    ///
    /// 此操作是无条件删除，不会校验租约 token；释放由 [`RedisUtils::try_lock_async`] 返回
    /// 的锁应使用 guard 的 `release` 方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::delete_async::<&str>;
    /// ```
    pub async fn delete_async<K: AsRef<[u8]>>(key: K) -> Result<u64, RedisError> {
        Self::client()?.delete_async(key).await
    }

    /// 异步有界批量删除 key。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::delete_many_async::<[&str; 1], &str>;
    /// ```
    pub async fn delete_many_async<I, K>(keys: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        Self::client()?.delete_many_async(keys).await
    }

    /// 异步判断 key 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::exists_async::<&str>;
    /// ```
    pub async fn exists_async<K: AsRef<[u8]>>(key: K) -> Result<bool, RedisError> {
        Self::client()?.exists_async(key).await
    }

    /// 异步按输入顺序批量读取 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mget_async::<[&str; 1], &str, u8>;
    /// ```
    pub async fn mget_async<I, K, T>(keys: I) -> Result<Vec<Option<T>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
        T: DeserializeOwned,
    {
        Self::client()?.mget_async(keys).await
    }

    /// 异步按输入顺序批量读取 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mget_bytes_async::<[&str; 1], &str>;
    /// ```
    pub async fn mget_bytes_async<I, K>(keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        Self::client()?.mget_bytes_async(keys).await
    }

    /// 异步有界批量写入 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mset_async::<[(&str, u8); 1], &str, u8>;
    /// ```
    pub async fn mset_async<I, K, T>(entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, T)>,
        K: AsRef<[u8]>,
        T: Serialize,
    {
        Self::client()?.mset_async(entries).await
    }

    /// 异步有界批量写入 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
    /// ```
    pub async fn mset_bytes_async<I, K, V>(entries: I) -> Result<(), RedisError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Self::client()?.mset_bytes_async(entries).await
    }

    /// 异步读取 Hash 中的 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hget_async::<&str, &str, u8>;
    /// ```
    pub async fn hget_async<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
        field: F,
    ) -> Result<Option<T>, RedisError> {
        Self::client()?.hget_async(key, field).await
    }

    /// 异步读取 Hash 中的 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hget_bytes_async::<&str, &str>;
    /// ```
    pub async fn hget_bytes_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        key: K,
        field: F,
    ) -> Result<Option<Vec<u8>>, RedisError> {
        Self::client()?.hget_bytes_async(key, field).await
    }

    /// 异步写入一个 MessagePack Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_async::<&str, &str, u8>;
    /// ```
    pub async fn hset_async<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(
        key: K,
        field: F,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.hset_async(key, field, value).await
    }

    /// 异步写入一个 raw Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_bytes_async::<&str, &str, Vec<u8>>;
    /// ```
    pub async fn hset_bytes_async<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(
        key: K,
        field: F,
        value: V,
    ) -> Result<u64, RedisError> {
        Self::client()?.hset_bytes_async(key, field, value).await
    }

    /// 异步读取 Hash 全部 field 和 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hgetall_async::<&str, u8>;
    /// ```
    pub async fn hgetall_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Vec<(Vec<u8>, T)>, RedisError> {
        Self::client()?.hgetall_async(key).await
    }

    /// 异步读取 Hash 全部 field 和 raw 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hgetall_bytes_async::<&str>;
    /// ```
    pub async fn hgetall_bytes_async<K: AsRef<[u8]>>(
        key: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError> {
        Self::client()?.hgetall_bytes_async(key).await
    }

    /// 异步删除一个 Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hdel_async::<&str, &str>;
    /// ```
    pub async fn hdel_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        key: K,
        field: F,
    ) -> Result<u64, RedisError> {
        Self::client()?.hdel_async(key, field).await
    }

    /// 异步判断 Hash field 是否存在。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hexists_async::<&str, &str>;
    /// ```
    pub async fn hexists_async<K: AsRef<[u8]>, F: AsRef<[u8]>>(
        key: K,
        field: F,
    ) -> Result<bool, RedisError> {
        Self::client()?.hexists_async(key, field).await
    }

    /// 异步返回 Hash field 数量。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hlen_async::<&str>;
    /// ```
    pub async fn hlen_async<K: AsRef<[u8]>>(key: K) -> Result<u64, RedisError> {
        Self::client()?.hlen_async(key).await
    }

    /// 异步有界批量写入 MessagePack Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
    /// ```
    pub async fn hset_many_async<I, K, F, T>(key: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, T)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        T: Serialize,
    {
        Self::client()?.hset_many_async(key, entries).await
    }

    /// 异步有界批量写入 raw Hash field。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
    /// ```
    pub async fn hset_many_bytes_async<I, K, F, V>(key: K, entries: I) -> Result<u64, RedisError>
    where
        I: IntoIterator<Item = (F, V)>,
        K: AsRef<[u8]>,
        F: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        Self::client()?.hset_many_bytes_async(key, entries).await
    }

    /// 异步以秒为单位设置 key 的 TTL。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::expire_async::<&str>;
    /// ```
    pub async fn expire_async<K: AsRef<[u8]>>(
        key: K,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?.expire_async(key, ttl).await
    }

    /// 异步以毫秒为单位设置 key 的 TTL。
    ///
    /// 此操作是无条件设置 TTL，不会校验租约 token；续租由 [`RedisUtils::try_lock_async`] 返回
    /// 的锁应使用 guard 的 `renew` 方法。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::pexpire_async::<&str>;
    /// ```
    pub async fn pexpire_async<K: AsRef<[u8]>>(
        key: K,
        ttl: std::time::Duration,
    ) -> Result<bool, RedisError> {
        Self::client()?.pexpire_async(key, ttl).await
    }

    /// 异步删除 key 的 TTL。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::persist_async::<&str>;
    /// ```
    pub async fn persist_async<K: AsRef<[u8]>>(key: K) -> Result<bool, RedisError> {
        Self::client()?.persist_async(key).await
    }

    /// 异步返回 Redis 原生 TTL 秒数。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::ttl_async::<&str>;
    /// ```
    pub async fn ttl_async<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.ttl_async(key).await
    }

    /// 异步返回 Redis 原生 TTL 毫秒数。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::pttl_async::<&str>;
    /// ```
    pub async fn pttl_async<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.pttl_async(key).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数加一。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::incr_async::<&str>;
    /// ```
    pub async fn incr_async<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.incr_async(key).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数增加指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::incr_by_async::<&str>;
    /// ```
    pub async fn incr_by_async<K: AsRef<[u8]>>(key: K, amount: i64) -> Result<i64, RedisError> {
        Self::client()?.incr_by_async(key, amount).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数减一。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::decr_async::<&str>;
    /// ```
    pub async fn decr_async<K: AsRef<[u8]>>(key: K) -> Result<i64, RedisError> {
        Self::client()?.decr_async(key).await
    }

    /// 异步将 key 作为 Redis 原生十进制整数减少指定值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::decr_by_async::<&str>;
    /// ```
    pub async fn decr_by_async<K: AsRef<[u8]>>(key: K, amount: i64) -> Result<i64, RedisError> {
        Self::client()?.decr_by_async(key, amount).await
    }

    /// 异步从列表左侧压入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lpush_async::<&str, u8>;
    /// ```
    pub async fn lpush_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.lpush_async(key, value).await
    }

    /// 异步从列表右侧压入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::rpush_async::<&str, u8>;
    /// ```
    pub async fn rpush_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.rpush_async(key, value).await
    }

    /// 异步从列表左侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lpop_async::<&str, u8>;
    /// ```
    pub async fn lpop_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Option<T>, RedisError> {
        Self::client()?.lpop_async(key).await
    }

    /// 异步从列表右侧弹出一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::rpop_async::<&str, u8>;
    /// ```
    pub async fn rpop_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Option<T>, RedisError> {
        Self::client()?.rpop_async(key).await
    }

    /// 异步读取列表范围。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::lrange_async::<&str, u8>;
    /// ```
    pub async fn lrange_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
        start: isize,
        stop: isize,
    ) -> Result<Vec<T>, RedisError> {
        Self::client()?.lrange_async(key, start, stop).await
    }

    /// 异步向集合加入一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::sadd_async::<&str, u8>;
    /// ```
    pub async fn sadd_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.sadd_async(key, value).await
    }

    /// 异步从集合移除一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::srem_async::<&str, u8>;
    /// ```
    pub async fn srem_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<u64, RedisError> {
        Self::client()?.srem_async(key, value).await
    }

    /// 异步判断集合是否包含一个 MessagePack 值。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::sismember_async::<&str, u8>;
    /// ```
    pub async fn sismember_async<K: AsRef<[u8]>, T: Serialize>(
        key: K,
        value: T,
    ) -> Result<bool, RedisError> {
        Self::client()?.sismember_async(key, value).await
    }

    /// 异步读取集合全部 MessagePack 成员。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::smembers_async::<&str, u8>;
    /// ```
    pub async fn smembers_async<K: AsRef<[u8]>, T: DeserializeOwned>(
        key: K,
    ) -> Result<Vec<T>, RedisError> {
        Self::client()?.smembers_async(key).await
    }

    /// 异步向 Redis 发送 `PING`。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::RedisUtils;
    ///
    /// let _ = RedisUtils::ping_async;
    /// ```
    pub async fn ping_async() -> Result<String, RedisError> {
        Self::client()?.ping_async().await
    }

    /// 异步执行单机原子事务；callback 只负责同步排队，不接受 async callback。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::{RedisError, RedisTransaction, RedisUtils};
    ///
    /// let _ = RedisUtils::transaction_async::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub async fn transaction_async<F>(callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        Self::client()?.transaction_async(callback).await
    }
}
