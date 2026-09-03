//! 基于 `redis-rs` 的有界 Redis 客户端。
//!
//! 该模块仅在 `redis` feature 下导出。同步方法使用惰性 `r2d2` 连接池；同时启用
//! `tokio` feature 时追加带 `_async` 后缀的异步方法。`RedisConfig` 与 `RedisClient::new` 只做
//! 本地惰性构造，普通首次命令才可能建立连接并返回传输错误；`RedisUtils::init` 与
//! `RedisUtils::init_async` 是例外，它们分别同步或在调用方 Tokio runtime 中执行 `PING` 并要求
//! `PONG` 后才写入共用的全局单例。
//!
//! 值 API 使用受限的 `rmp-serde` MessagePack 编解码；需要缓存原始二进制或与其他协议
//! 互操作时使用 `*_bytes` API。单键租约锁使用 OS CSPRNG token、TTL 和单 key Lua `EVAL`
//! 校验释放/续租；它适用于同一 Redis 逻辑主节点或 Cluster 拓扑，不是 Redlock，也不提供
//! fencing token。第一阶段只接受 `redis://`，不启用 TLS；Cluster 事务明确返回
//! [`RedisError::UnsupportedMode`]，不伪装成跨节点原子操作。

mod client;
mod codec;
mod commands;
mod config;
mod error;
mod lock;
mod transaction;

pub use client::RedisClient;
pub use config::RedisConfig;
pub use error::{RedisError, RedisTransportErrorKind};
#[cfg(all(feature = "redis", feature = "tokio"))]
pub use lock::RedisAsyncLockGuard;
pub use lock::RedisLockGuard;
pub use transaction::RedisTransaction;
