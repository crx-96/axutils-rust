use std::fmt;

use futures_util::StreamExt;

use super::{SqlxConfig, SqlxError, SqlxRow, SqlxTransaction};

/// 可克隆的 SQLx Any 连接池客户端。
///
/// 客户端只在 [`SqlxClient::connect`] 时访问数据库，构造查询对象不会访问连接池。所有异步
/// 方法都要求调用方已经运行在 Tokio runtime 中；本 crate 不创建 runtime，也不调用 `block_on`。
/// 客户端 clone 共享 SQLx pool 的引用计数，`close_async` 会关闭共享 pool，且不会重新打开它。
#[derive(Clone)]
pub struct SqlxClient {
    pub(crate) pool: sqlx::AnyPool,
    pub(crate) max_rows: usize,
}

impl SqlxClient {
    /// 按本地配置建立 SQLx Any 连接池。
    ///
    /// 该方法会检查当前 Tokio runtime、校验配置、安装一次 SQLx 默认 Any drivers，并建立连接
    /// 池，因此可能产生网络、认证和 SQLite 文件 I/O。连接失败不会改变 `SqlxUtils` 的全局初始化
    /// 状态。若调用方已在本进程通过 SQLx 自定义注册器安装 Any drivers，默认安装函数可能 panic；
    /// 首版要求本 crate 是进程中唯一的 Any driver 注册方，不捕获该 panic，也不提供 reset。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::SqlxError> {
    /// use axutils::{SqlxClient, SqlxConfig};
    /// let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:")?).await?;
    /// assert!(!client.is_closed());
    /// client.close_async().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(config: SqlxConfig) -> Result<Self, SqlxError> {
        ensure_runtime()?;
        config.validate()?;
        super::driver::install_default_drivers();

        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.acquire_timeout)
            .connect_with(config.connect_options.clone())
            .await
            .map_err(|error| SqlxError::from_upstream(&error))?;

        Ok(Self {
            pool,
            max_rows: config.max_rows,
        })
    }

    /// 创建固定为 SQLx `Any` 后端的参数化查询对象。
    ///
    /// 该方法只调用 SQLx 原生构造函数，不访问数据库。调用方继续使用 SQLx 的 `.bind(...)`、
    /// `.persistent(...)` 等链式 API；未受信任的 SQL 片段不能通过拼接传入。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # fn main() {
    /// use axutils::SqlxClient;
    /// let _ = SqlxClient::query;
    /// # }
    /// ```
    pub fn query<'q>(
        &self,
        sql: &'q str,
    ) -> sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>> {
        sqlx::query::<sqlx::Any>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、映射到 `T` 的查询对象。
    ///
    /// 该方法不执行 SQL；`T` 的 `FromRow`、类型兼容性和参数绑定仍由 SQLx 负责。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// use axutils::SqlxClient;
    /// let _method = SqlxClient::query_as::<(i64, String)>;
    /// # }
    /// ```
    pub fn query_as<'q, T>(
        &self,
        sql: &'q str,
    ) -> sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>
    where
        T: for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        sqlx::query_as::<sqlx::Any, T>(sql)
    }

    /// 创建固定为 SQLx `Any` 后端、读取第一列为 `T` 的查询对象。
    ///
    /// 该方法不执行 SQL；标量的 `Decode`/`Type` 兼容性仍由 SQLx 负责。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// use axutils::SqlxClient;
    /// let _method = SqlxClient::query_scalar::<i64>;
    /// # }
    /// ```
    pub fn query_scalar<'q, T>(
        &self,
        sql: &'q str,
    ) -> sqlx::query::QueryScalar<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>
    where
        (T,): for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        sqlx::query_scalar::<sqlx::Any, T>(sql)
    }

    /// 执行一个 SQLx `Query` 并返回受 SQLx 定义的影响行数结果。
    ///
    /// SQL 文本和参数仍由 SQLx 处理；底层错误会映射为不含原始 SQL/URL/数据库消息的
    /// [`SqlxError`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// client.execute_async(client.query("CREATE TABLE items (id INTEGER)")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_async<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>,
    ) -> Result<sqlx::any::AnyQueryResult, SqlxError> {
        ensure_runtime()?;
        query
            .execute(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 读取一个原生 SQLx row；没有结果时返回 [`SqlxError::RowNotFound`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let row = client.fetch_one_async(client.query("SELECT 1")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_async<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>,
    ) -> Result<SqlxRow, SqlxError> {
        ensure_runtime()?;
        query
            .fetch_one(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 读取一个映射为 `T` 的 row；没有结果时返回 [`SqlxError::RowNotFound`]。
    ///
    /// `T` 必须实现 `FromRow`，并满足 SQLx 异步查询所需的 `Send + Unpin`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let row: (i64,) = client.fetch_one_as_async(client.query_as::<(i64,)>("SELECT 1")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_one_as_async<'q, T>(
        &self,
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        ensure_runtime()?;
        query
            .fetch_one(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 最多读取一个原生 row；没有结果时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let row = client.fetch_optional_async(client.query("SELECT 1 WHERE 0")).await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_async<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>,
    ) -> Result<Option<SqlxRow>, SqlxError> {
        ensure_runtime()?;
        query
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 最多读取一个映射为 `T` 的 row；没有结果时返回 `None`。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let row: Option<(i64,)> = client
    ///     .fetch_optional_as_async(client.query_as::<(i64,)>("SELECT 1 WHERE 0"))
    ///     .await?;
    /// # let _ = row;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_optional_as_async<'q, T>(
        &self,
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>,
    ) -> Result<Option<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        ensure_runtime()?;
        query
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 逐行收集原生 row，并在消费第 `max_rows + 1` 行时返回 [`SqlxError::RowLimitExceeded`]。
    ///
    /// 不会调用无界的 SQLx `fetch_all`；刚好达到上限仍成功，超限后立即停止 stream 并释放连接。
    /// 上限只限制返回行数，不限制单行字段大小。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let rows = client.fetch_all_async(client.query("SELECT 1")).await?;
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_async<'q>(
        &self,
        query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>,
    ) -> Result<Vec<SqlxRow>, SqlxError> {
        ensure_runtime()?;
        let sentinel_limit = self
            .max_rows
            .checked_add(1)
            .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
        let mut stream = query.fetch(&self.pool);
        let mut rows = Vec::new();

        while let Some(result) = stream.next().await {
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

    /// 逐行收集映射为 `T` 的结果，并在消费第 `max_rows + 1` 行时返回限制错误。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let rows: Vec<(i64,)> = client
    ///     .fetch_all_as_async(client.query_as::<(i64,)>("SELECT 1"))
    ///     .await?;
    /// # let _ = rows;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_all_as_async<'q, T>(
        &self,
        query: sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>,
    ) -> Result<Vec<T>, SqlxError>
    where
        T: Send + Unpin + for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        ensure_runtime()?;
        let sentinel_limit = self
            .max_rows
            .checked_add(1)
            .ok_or(SqlxError::InvalidConfig { field: "max_rows" })?;
        let mut stream = query.fetch(&self.pool);
        let mut rows = Vec::new();

        while let Some(result) = stream.next().await {
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

    /// 读取标量查询的第一列；无行时返回 [`SqlxError::RowNotFound`]。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// let value: i64 = client.fetch_scalar_async(client.query_scalar::<i64>("SELECT 1")).await?;
    /// # let _ = value;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn fetch_scalar_async<'q, T>(
        &self,
        query: sqlx::query::QueryScalar<'q, sqlx::Any, T, sqlx::any::AnyArguments<'q>>,
    ) -> Result<T, SqlxError>
    where
        T: Send + Unpin,
        (T,): for<'r> sqlx::FromRow<'r, SqlxRow>,
    {
        ensure_runtime()?;
        query
            .fetch_one(&self.pool)
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 开启原生 SQLx Any 事务。
    ///
    /// 调用方必须显式 `commit` 或 `rollback`；drop 只作为回滚兜底。SQLx 0.8.6 没有为
    /// `Transaction` 直接实现 `Executor`，事务内执行应使用 `&mut *tx`。该返回值暴露 SQLx
    /// 原生事务和原生错误语义，因此调用方需要直接依赖匹配的 SQLx 版本并导入所需 trait。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut tx = client.begin_async().await?;
    /// sqlx::query::<sqlx::Any>("SELECT 1").execute(&mut *tx).await?;
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn begin_async(&self) -> Result<SqlxTransaction<'static>, SqlxError> {
        ensure_runtime()?;
        self.pool
            .begin()
            .await
            .map_err(|error| SqlxError::from_upstream(&error))
    }

    /// 优雅地关闭共享连接池并等待关闭完成。
    ///
    /// 关闭后 `is_closed` 返回 `true`，后续执行会返回 [`SqlxError::PoolClosed`]。该方法不会
    /// 创建新 runtime，也不会重新打开 pool；多个 client clone 共享同一个关闭状态。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # async fn example(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    /// client.close_async().await?;
    /// assert!(client.is_closed());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close_async(&self) -> Result<(), SqlxError> {
        ensure_runtime()?;
        self.pool.close().await;
        Ok(())
    }

    /// 返回连接池是否已经进入关闭状态。
    ///
    /// 该方法不执行异步操作，也不检查远端数据库健康状态。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "sqlx", feature = "tokio"))]
    /// # {
    /// let _is_closed = axutils::SqlxClient::is_closed;
    /// # }
    /// ```
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }
}

impl fmt::Debug for SqlxClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlxClient")
            .field("max_rows", &self.max_rows)
            .field("is_closed", &self.is_closed())
            .finish()
    }
}

fn ensure_runtime() -> Result<(), SqlxError> {
    tokio::runtime::Handle::try_current()
        .map(|_| ())
        .map_err(|_| SqlxError::RuntimeRequired)
}
