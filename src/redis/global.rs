//! 默认 Redis 客户端的进程级生命周期入口。

use std::sync::OnceLock;

use super::{RedisClient, RedisConfig, RedisError, RedisTransportErrorKind};

#[cfg(feature = "tracing")]
use crate::telemetry::redis as redis_trace;

static REDIS_CLIENT: OnceLock<RedisClient> = OnceLock::new();

/// 单默认 Redis 客户端的进程级便捷入口。
///
/// 该入口只管理默认客户端的初始化和访问。命令、事务和锁操作应通过 [`Self::client`]
/// 返回的 [`RedisClient`] 实例调用；需要多个配置或可控生命周期时，直接持有多个
/// `RedisClient`。初始化成功后只能保留第一个客户端，不能 reset、replace 或读取连接
/// URL/凭据。
pub struct RedisUtils;

impl RedisUtils {
    /// 同步验证 Redis 可用后初始化全局客户端。
    pub fn init(config: RedisConfig) -> Result<(), RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = (|| {
            if Self::is_initialized() {
                return Err(RedisError::AlreadyInitialized);
            }
            let client = RedisClient::new(config)?;
            if client.ping()? != "PONG" {
                return Err(RedisError::Transport(RedisTransportErrorKind::Protocol));
            }
            REDIS_CLIENT
                .set(client)
                .map_err(|_| RedisError::AlreadyInitialized)
        })();
        #[cfg(feature = "tracing")]
        redis_trace::record_client_init(&result, started);
        result
    }

    /// 异步验证 Redis 可用后初始化全局客户端，需要 `redis-async`。
    #[cfg(feature = "redis-async")]
    pub async fn init_async(config: RedisConfig) -> Result<(), RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            if Self::is_initialized() {
                return Err(RedisError::AlreadyInitialized);
            }
            let client = RedisClient::new(config)?;
            if client.ping_async().await? != "PONG" {
                return Err(RedisError::Transport(RedisTransportErrorKind::Protocol));
            }
            REDIS_CLIENT
                .set(client)
                .map_err(|_| RedisError::AlreadyInitialized)
        }
        .await;
        #[cfg(feature = "tracing")]
        redis_trace::record_client_init(&result, started);
        result
    }

    /// 返回全局 Redis 客户端是否已经成功初始化。
    pub fn is_initialized() -> bool {
        REDIS_CLIENT.get().is_some()
    }

    /// 返回已初始化的全局 Redis 客户端，不执行网络 I/O。
    pub fn client() -> Result<&'static RedisClient, RedisError> {
        REDIS_CLIENT.get().ok_or(RedisError::NotInitialized)
    }
}
