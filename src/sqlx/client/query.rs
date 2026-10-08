//! 查询构造、执行与有界结果收集；连接池生命周期由父模块维护。

use futures_util::StreamExt;
use sqlx::{
    any::{AnyArguments, AnyQueryResult},
    query::{Query, QueryAs, QueryScalar},
    Any, FromRow, SqlSafeStr,
};

use super::{ensure_runtime, SqlxClient};
use crate::sqlx::{SqlxError, SqlxRow};
#[cfg(feature = "tracing")]
use crate::telemetry::sqlx as sqlx_trace;

impl SqlxClient {
    /// 创建固定为 SQLx `Any` 后端的参数化查询对象。
    ///
    /// 该方法只调用 SQLx 原生构造函数，不访问数据库。调用方继续使用 SQLx 的 `.bind(...)`、
    /// `.persistent(...)` 等链式 API。SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方
    /// 审计后用 `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example() -> Result<(), SqlxError> {
    /// let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:")?).await?;
    /// let _query = client.query("SELECT 1");
    /// # Ok(())
    /// # }
    /// ```
    pub fn query<'q>(&self, sql: impl SqlSafeStr) -> Query<'q, Any, AnyArguments> {
        // 仅构造绑定入口；SQL 安全标记及参数编码继续由上游 API 约束。
        sqlx::query::<Any>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、映射到 `T` 的查询对象。
    ///
    /// 该方法不执行 SQL；`T` 的 `FromRow`、类型兼容性和参数绑定仍由 SQLx 负责。
    /// SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方审计后用
    /// `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example() -> Result<(), SqlxError> {
    /// let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:")?).await?;
    /// let _query = client.query_as::<(i64,)>("SELECT 1");
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_as<'q, T>(&self, sql: impl SqlSafeStr) -> QueryAs<'q, Any, T, AnyArguments>
    where
        T: for<'r> FromRow<'r, SqlxRow>,
    {
        // 保留调用方的 FromRow 映射，不在构造时取得连接或执行 SQL。
        sqlx::query_as::<Any, T>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、读取第一列为 `T` 的查询对象。
    ///
    /// 该方法不执行 SQL；标量的 `Decode`/`Type` 兼容性仍由 SQLx 负责。
    /// SQLx 0.9 默认只接受静态 SQL 字面量；动态 SQL 必须由调用方审计后用
    /// `sqlx::AssertSqlSafe` 显式标记，这个标记不会替调用方做转义或注入检查。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example() -> Result<(), SqlxError> {
    /// let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:")?).await?;
    /// let _query = client.query_scalar::<i64>("SELECT 1");
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_scalar<'q, T>(&self, sql: impl SqlSafeStr) -> QueryScalar<'q, Any, T, AnyArguments>
    where
        (T,): for<'r> FromRow<'r, SqlxRow>,
    {
        // 标量仍按上游第一列解码契约处理，此时不进行 I/O。
        sqlx::query_scalar::<Any, T>(sql)
    }

    /// 执行一个 SQLx `Query` 并返回受 SQLx 定义的影响行数结果。
    ///
    /// SQL 文本和参数仍由 SQLx 处理；底层错误会映射为不含原始 SQL/URL/数据库消息的
    /// [`SqlxError`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// client.execute_async(client.query("CREATE TABLE items (id INTEGER)")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_async<'q>(
        &self,
        query: Query<'q, Any, AnyArguments>,
    ) -> Result<AnyQueryResult, SqlxError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .execute(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event("execute", self.driver, 0, 0, &result, started);
        result
    }

    /// 读取一个原生 SQLx row；没有结果时返回 [`SqlxError::RowNotFound`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let row = client.fetch_one_async(client.query("SELECT 1")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_async<'q>(
        &self,
        query: Query<'q, Any, AnyArguments>,
    ) -> Result<SqlxRow, SqlxError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .fetch_one(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_one",
            self.driver,
            usize::from(result.is_ok()),
            0,
            &result,
            started,
        );
        result
    }

    /// 读取一个映射为 `T` 的 row；没有结果时返回 [`SqlxError::RowNotFound`]。
    ///
    /// `T` 必须实现 `FromRow`，并满足 SQLx 异步查询所需的 `Send + Unpin`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let row: (i64,) = client.fetch_one_as_async(client.query_as::<(i64,)>("SELECT 1")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_as_async<'q, T>(
        &self,
        query: QueryAs<'q, Any, T, AnyArguments>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin + for<'r> FromRow<'r, SqlxRow>,
    {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .fetch_one(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_one_as",
            self.driver,
            usize::from(result.is_ok()),
            0,
            &result,
            started,
        );
        result
    }

    /// 最多读取一个原生 row；没有结果时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let row = client.fetch_optional_async(client.query("SELECT 1 WHERE 0")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_async<'q>(
        &self,
        query: Query<'q, Any, AnyArguments>,
    ) -> Result<Option<SqlxRow>, SqlxError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_optional",
            self.driver,
            usize::from(result.as_ref().ok().is_some_and(Option::is_some)),
            0,
            &result,
            started,
        );
        result
    }

    /// 最多读取一个映射为 `T` 的 row；没有结果时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let row: Option<(i64,)> = client
    ///     .fetch_optional_as_async(client.query_as::<(i64,)>("SELECT 1 WHERE 0"))
    ///     .await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_as_async<'q, T>(
        &self,
        query: QueryAs<'q, Any, T, AnyArguments>,
    ) -> Result<Option<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> FromRow<'r, SqlxRow>,
    {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_optional_as",
            self.driver,
            usize::from(result.as_ref().ok().is_some_and(Option::is_some)),
            0,
            &result,
            started,
        );
        result
    }

    /// 逐行收集原生 row，并在消费第 `max_rows + 1` 行时返回 [`SqlxError::RowLimitExceeded`]。
    ///
    /// 不会调用无界的 SQLx `fetch_all`；刚好达到上限仍成功，超限后立即停止 stream 并释放连接。
    /// 上限只限制返回行数，不限制单行字段大小。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let rows = client.fetch_all_async(client.query("SELECT 1")).await?;
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_async<'q>(
        &self,
        query: Query<'q, Any, AnyArguments>,
    ) -> Result<Vec<SqlxRow>, SqlxError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            // 多读一行仅用于识别超限，不能把刚好达到配置上限视为失败。
            let sentinel_limit = self
                .max_rows
                .checked_add(1)
                .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
            let mut stream = query.fetch(&self.pool);
            let mut rows = Vec::new();

            // 逐行消费，不调用无界 fetch_all；错误和超限会丢弃 stream 并归还连接。
            while let Some(result) = stream.next().await {
                // 先保留 SQLx 本行解码错误，再判断它是否是超出预算的 sentinel 行。
                let row = result.map_err(|error| SqlxError::from_upstream(&error))?;
                let seen = rows
                    .len()
                    .checked_add(1)
                    .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
                if seen == sentinel_limit {
                    return Err(SqlxError::RowLimitExceeded {
                        limit: self.max_rows,
                    });
                }
                rows.push(row);
            }
            Ok(rows)
        }
        .await;
        // 仅记录可安全观察的数量；SQL、参数、行内容和原始错误不进入事件。
        #[cfg(feature = "tracing")]
        let observed_rows = match &result {
            Ok(rows) => rows.len(),
            Err(SqlxError::RowLimitExceeded { limit }) => *limit,
            Err(_) => 0,
        };
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_all",
            self.driver,
            observed_rows,
            self.max_rows,
            &result,
            started,
        );
        result
    }

    /// 逐行收集映射为 `T` 的结果，并在消费第 `max_rows + 1` 行时返回限制错误。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let rows: Vec<(i64,)> = client
    ///     .fetch_all_as_async(client.query_as::<(i64,)>("SELECT 1"))
    ///     .await?;
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_as_async<'q, T>(
        &self,
        query: QueryAs<'q, Any, T, AnyArguments>,
    ) -> Result<Vec<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> FromRow<'r, SqlxRow>,
    {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            // 多读一行仅用于识别超限，不能把刚好达到配置上限视为失败。
            let sentinel_limit = self
                .max_rows
                .checked_add(1)
                .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
            let mut stream = query.fetch(&self.pool);
            let mut rows = Vec::new();

            // 逐行消费，不调用无界 fetch_all；错误和超限会丢弃 stream 并归还连接。
            while let Some(result) = stream.next().await {
                // 先保留 SQLx 本行解码错误，再判断它是否是超出预算的 sentinel 行。
                let row = result.map_err(|error| SqlxError::from_upstream(&error))?;
                let seen = rows
                    .len()
                    .checked_add(1)
                    .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
                if seen == sentinel_limit {
                    return Err(SqlxError::RowLimitExceeded {
                        limit: self.max_rows,
                    });
                }
                rows.push(row);
            }
            Ok(rows)
        }
        .await;
        // 仅记录可安全观察的数量；SQL、参数、行内容和原始错误不进入事件。
        #[cfg(feature = "tracing")]
        let observed_rows = match &result {
            Ok(rows) => rows.len(),
            Err(SqlxError::RowLimitExceeded { limit }) => *limit,
            Err(_) => 0,
        };
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_all_as",
            self.driver,
            observed_rows,
            self.max_rows,
            &result,
            started,
        );
        result
    }

    /// 读取标量查询的第一列；无行时返回 [`SqlxError::RowNotFound`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// let value: i64 = client.fetch_scalar_async(client.query_scalar::<i64>("SELECT 1")).await?;
    /// # let _ = value;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_scalar_async<'q, T>(
        &self,
        query: QueryScalar<'q, Any, T, AnyArguments>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin,
        (T,): for<'r> FromRow<'r, SqlxRow>,
    {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 执行前只检查调用方上下文，底层错误统一转换为不含 SQL/参数的类别。
            ensure_runtime()?;
            query
                .fetch_one(&self.pool)
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event(
            "fetch_scalar",
            self.driver,
            usize::from(result.is_ok()),
            0,
            &result,
            started,
        );
        result
    }
}
