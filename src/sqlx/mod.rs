//! 基于 SQLx `AnyPool` 的有界异步数据库客户端。
//!
//! 该模块由 `sqlx-postgres`、`sqlx-mysql`、`sqlx-sqlite` 中任一 driver feature 公开；
//! `sqlx` 是一次启用三种 driver 的聚合入口。配置阶段只做本地 URL/边界校验；连接、查询、事务和关闭都要求
//! 调用方已经运行在 Tokio runtime 中，本 crate 不创建 runtime、不调用 `block_on`，首版也不
//! 配置 TLS。
//!
//! [`SqlxClient`] 是可 clone 的实例级连接池入口；[`crate::utils::SqlxUtils`] 只提供一次初始化的
//! 生命周期、状态查询和实例访问入口。查询对象、行、结果和事务保留 SQLx 原生类型语义，调用方需要直接依赖匹配的
//! SQLx 0.9.x 版本以使用 `.bind(...)`、`FromRow`、`QueryBuilder` 和事务的 `&mut *tx`。

mod client;
mod config;
mod driver;
mod error;
pub(crate) mod global;

use sqlx::any::{AnyQueryResult, AnyRow};
use sqlx::{Any, Transaction};

pub use client::SqlxClient;
pub use config::SqlxConfig;
pub use error::{SqlxError, SqlxTransportErrorKind};

/// SQLx Any driver 返回的原生结果行。
pub type SqlxRow = AnyRow;

/// SQLx Any driver 返回的原生影响行数结果。
pub type SqlxQueryResult = AnyQueryResult;

/// SQLx Any driver 的原生事务类型。
pub type SqlxTransaction<'a> = Transaction<'a, Any>;
