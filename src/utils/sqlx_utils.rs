//! SQLx Any 客户端的一次初始化进程级便捷入口。
//!
//! 该模块只在同时启用 `sqlx` 与 `tokio` feature 时公开。它不复制实例客户端的查询逻辑，
//! 只维护一个不可 reset/replace 的 `OnceLock<SqlxClient>`；调用方需要自行提供 Tokio runtime，
//! 并承担 SQLx Any driver 默认注册的进程级唯一性前提。

use std::sync::OnceLock;

use crate::sqlx::{SqlxClient, SqlxConfig, SqlxError, SqlxQueryResult, SqlxRow, SqlxTransaction};

static CLIENT: OnceLock<SqlxClient> = OnceLock::new();

/// SQLx Any 默认客户端的一次初始化全局入口。
///
/// 所有方法都要求 `sqlx + tokio` feature。`init` 成功后不能 reset/replace，`close_async` 关闭
/// pool 后仍保持 initialized 状态；需要多个数据库或可控生命周期时，请直接持有多个
/// [`SqlxClient`]。查询构造方法只构造 SQLx 查询对象，不检查初始化状态；执行方法才会返回
/// [`SqlxError::NotInitialized`]。
///
/// Any driver 注册是进程级状态，本入口假定本 crate 是进程中唯一的默认 Any driver 注册方。
pub struct SqlxUtils;

impl SqlxUtils {
    /// 连接并一次性初始化全局 SQLx client。
    ///
    /// 已初始化时会先返回 [`SqlxError::AlreadyInitialized`]，不会再次连接传入的 URL。连接失败、
    /// 配置失败或 runtime 缺失都不会占用初始化机会；并发初始化中未赢得 `OnceLock` 的 client
    /// 会在返回前优雅关闭；清理失败不会改变公开结果，仍返回
    /// [`SqlxError::AlreadyInitialized`]，并在启用 `tracing` 时记录脱敏的清理错误类别。成功
    /// 连接会访问数据库并可能产生网络或 SQLite 文件 I/O。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// axutils::SqlxUtils::init(axutils::SqlxConfig::new("sqlite::memory:")?).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn init(config: SqlxConfig) -> Result<(), SqlxError> {
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
                            crate::tracing::sqlx::record_init_cleanup(error, started);
                        }
                        let _ = cleanup_result;
                        Err(SqlxError::AlreadyInitialized)
                    }
                },
                Err(error) => Err(error),
            }
        };
        #[cfg(feature = "tracing")]
        crate::tracing::sqlx::record_client_init(&result, started);
        result
    }

    /// 返回全局 client 是否已经成功初始化。
    ///
    /// `true` 只表示 `OnceLock` 已写入，不代表远端数据库健康，也不会因为 pool 被关闭而恢复
    /// 为 `false`。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// let _ = axutils::SqlxUtils::is_initialized();
    /// # }
    /// ```
    pub fn is_initialized() -> bool {
        CLIENT.get().is_some()
    }

    /// 创建固定为 SQLx `Any` 后端的查询对象，不检查全局初始化状态。
    ///
    /// SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方审计后用
    /// `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// let _query = axutils::SqlxUtils::query("SELECT 1");
    /// # }
    /// ```
    pub fn query<'q>(
        sql: impl sqlx::SqlSafeStr,
    ) -> sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments> {
        sqlx::query::<sqlx::Any>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、映射到 `T` 的查询对象，不访问全局 client。
    ///
    /// SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方审计后用
    /// `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// let _query = axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1");
    /// # }
    /// ```
    pub fn query_as<'q, T>(
        sql: impl sqlx::SqlSafeStr,
    ) -> sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments>
    where
        T: for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        sqlx::query_as::<sqlx::Any, T>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、读取第一列为 `T` 的查询对象，不访问全局 client。
    ///
    /// SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方审计后用
    /// `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// let _query = axutils::SqlxUtils::query_scalar::<i64>("SELECT 1");
    /// # }
    /// ```
    pub fn query_scalar<'q, T>(
        sql: impl sqlx::SqlSafeStr,
    ) -> sqlx::query::QueryScalar<'q, sqlx::Any, T, sqlx::any::AnyArguments>
    where
        (T,): for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        sqlx::query_scalar::<sqlx::Any, T>(sql)
    }

    /// 在全局 client 上执行一个 Query。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// axutils::SqlxUtils::execute_async(axutils::SqlxUtils::query("SELECT 1")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_async<'q>(
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments>,
    ) -> Result<SqlxQueryResult, SqlxError> {
        client()?.execute_async(query).await
    }

    /// 在全局 client 上读取一个原生 row。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _row = axutils::SqlxUtils::fetch_one_async(axutils::SqlxUtils::query("SELECT 1")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_async<'q>(
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments>,
    ) -> Result<SqlxRow, SqlxError> {
        client()?.fetch_one_async(query).await
    }

    /// 在全局 client 上读取一个映射为 `T` 的 row。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _row: (i64,) = axutils::SqlxUtils::fetch_one_as_async(
    ///     axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1"),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_as_async<'q, T>(
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        client()?.fetch_one_as_async(query).await
    }

    /// 在全局 client 上最多读取一个原生 row。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _row = axutils::SqlxUtils::fetch_optional_async(
    ///     axutils::SqlxUtils::query("SELECT 1 WHERE 0"),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_async<'q>(
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments>,
    ) -> Result<Option<SqlxRow>, SqlxError> {
        client()?.fetch_optional_async(query).await
    }

    /// 在全局 client 上最多读取一个映射为 `T` 的 row。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _row: Option<(i64,)> = axutils::SqlxUtils::fetch_optional_as_async(
    ///     axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1 WHERE 0"),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_as_async<'q, T>(
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments>,
    ) -> Result<Option<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        client()?.fetch_optional_as_async(query).await
    }

    /// 在全局 client 上逐行读取原生 rows，并执行配置的行数上限。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _rows = axutils::SqlxUtils::fetch_all_async(axutils::SqlxUtils::query("SELECT 1")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_async<'q>(
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments>,
    ) -> Result<Vec<SqlxRow>, SqlxError> {
        client()?.fetch_all_async(query).await
    }

    /// 在全局 client 上逐行读取映射为 `T` 的 rows，并执行配置的行数上限。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _rows: Vec<(i64,)> = axutils::SqlxUtils::fetch_all_as_async(
    ///     axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1"),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_as_async<'q, T>(
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments>,
    ) -> Result<Vec<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        client()?.fetch_all_as_async(query).await
    }

    /// 在全局 client 上读取一个标量。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// let _value: i64 = axutils::SqlxUtils::fetch_scalar_async(
    ///     axutils::SqlxUtils::query_scalar::<i64>("SELECT 1"),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_scalar_async<'q, T>(
        query: sqlx::query::QueryScalar<'q, sqlx::Any, T, sqlx::any::AnyArguments>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin,
        (T,): for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        client()?.fetch_scalar_async(query).await
    }

    /// 在全局 client 上开启原生 SQLx Any 事务。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut tx = axutils::SqlxUtils::begin_async().await?;
    /// sqlx::query::<sqlx::Any>("SELECT 1").execute(&mut *tx).await?;
    /// tx.rollback().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn begin_async() -> Result<SqlxTransaction<'static>, SqlxError> {
        client()?.begin_async().await
    }

    /// 关闭全局 pool，但不清除初始化状态。
    ///
    /// 关闭后 `is_initialized()` 仍为 `true`，后续执行会返回 [`SqlxError::PoolClosed`]，且不能
    /// 通过再次调用 `init` 重新初始化。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// axutils::SqlxUtils::close_async().await?;
    /// assert!(axutils::SqlxUtils::is_initialized());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close_async() -> Result<(), SqlxError> {
        client()?.close_async().await
    }
}

fn client() -> Result<&'static SqlxClient, SqlxError> {
    CLIENT.get().ok_or(SqlxError::NotInitialized)
}
