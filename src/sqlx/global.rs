use std::sync::OnceLock;

use super::{SqlxClient, SqlxConfig, SqlxError};
#[cfg(feature = "tracing")]
use crate::telemetry::sqlx as sqlx_trace;

static CLIENT: OnceLock<SqlxClient> = OnceLock::new();

/// SQLx Any 默认客户端的一次初始化全局入口。
///
/// `init_async` 成功后不能 reset/replace；关闭 pool 后仍保持 initialized 状态。需要多个数据库或可控
/// 生命周期时，请直接持有多个 [`SqlxClient`]。
pub struct SqlxUtils;

impl SqlxUtils {
    /// 连接并一次性初始化全局 SQLx client。
    ///
    /// 已初始化时直接返回 [`SqlxError::AlreadyInitialized`]，不会访问传入 URL。连接、配置或
    /// runtime 失败都不占用初始化机会；并发竞争中未赢得 `OnceLock` 的 client 会先关闭。成功
    /// 连接会访问数据库并可能产生网络或 SQLite 文件 I/O。
    pub async fn init_async(config: SqlxConfig) -> Result<(), SqlxError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = if CLIENT.get().is_some() {
            Err(SqlxError::AlreadyInitialized)
        } else {
            match SqlxClient::connect(config).await {
                Ok(client) => match CLIENT.set(client.clone()) {
                    Ok(()) => Ok(()),
                    Err(_) => {
                        let cleanup_result = client.close_async().await;
                        #[cfg(feature = "tracing")]
                        if let Err(error) = &cleanup_result {
                            sqlx_trace::record_init_cleanup(error, started);
                        }
                        let _ = cleanup_result;
                        Err(SqlxError::AlreadyInitialized)
                    }
                },
                Err(error) => Err(error),
            }
        };
        #[cfg(feature = "tracing")]
        sqlx_trace::record_client_init(&result, started);
        result
    }

    /// 返回全局 client 是否已经成功初始化。
    pub fn is_initialized() -> bool {
        CLIENT.get().is_some()
    }

    /// 返回已初始化的全局 SQLx client。
    ///
    /// 未初始化时返回 [`SqlxError::NotInitialized`]。关闭后的 client 仍可取得，但其查询和事务
    /// 操作会保留底层 [`SqlxError::PoolClosed`] 语义。
    pub fn client() -> Result<&'static SqlxClient, SqlxError> {
        CLIENT.get().ok_or(SqlxError::NotInitialized)
    }
}
