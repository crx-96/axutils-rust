//! 基于 `redis-rs` 的有界 Redis 客户端。
//!
//! 该模块仅在 `redis` feature 下导出。同步方法使用惰性 `r2d2` 连接池；同时启用
//! `tokio` feature 时追加带 `_async` 后缀的异步方法。构造客户端或初始化全局入口不会
//! 访问网络，首次命令才会建立连接并返回传输错误。
//!
//! 值 API 使用受限的 `rmp-serde` MessagePack 编解码；需要缓存原始二进制或与其他协议
//! 互操作时使用 `*_bytes` API。第一阶段只接受 `redis://`，不启用 TLS；Cluster 事务
//! 明确返回 [`RedisError::UnsupportedMode`]，不伪装成跨节点原子操作。

mod client;
mod codec;
mod commands;
mod config;
mod error;
mod transaction;

pub use client::RedisClient;
pub use config::RedisConfig;
pub use error::{RedisError, RedisTransportErrorKind};
pub use transaction::RedisTransaction;
