//! SQLx 客户端状态及连接池生命周期；查询执行位于私有 query 模块。

use std::fmt;

use sqlx::{any::AnyPoolOptions, AnyPool};
use tokio::runtime::Handle;

use super::{driver, SqlxConfig, SqlxError, SqlxTransaction};
#[cfg(feature = "tracing")]
use crate::telemetry::sqlx as sqlx_trace;

mod query;

/// 可克隆的 SQLx Any 连接池客户端。
///
/// 客户端只在 [`SqlxClient::connect`] 时访问数据库，构造查询对象不会访问连接池。所有异步
/// 方法都要求调用方已经运行在 Tokio runtime 中；本 crate 不创建 runtime，也不调用 `block_on`。
/// 客户端 clone 共享 SQLx pool 的引用计数，`close_async` 会关闭共享 pool，且不会重新打开它。
#[derive(Clone)]
pub struct SqlxClient {
    /// 实例及其克隆共享的连接池；关闭任一实例会关闭全部共享入口。
    pool: AnyPool,
    /// 多行结果允许的最大行数；单行字节数仍由数据库和调用方控制。
    max_rows: usize,
    /// telemetry 使用的固定驱动名称，不保存 URL 或认证信息。
    #[cfg(feature = "tracing")]
    driver: &'static str,
}

impl SqlxClient {
    /// 按本地配置建立 SQLx Any 连接池。
    ///
    /// 该方法会检查当前 Tokio runtime、校验配置、安装一次 SQLx 默认 Any drivers，并建立连接
    /// 池，因此可能产生网络、认证和 SQLite 文件 I/O。连接失败不会改变 `SqlxUtils` 的全局初始化
    /// 状态。若调用方已在本进程通过 SQLx 自定义注册器安装 Any drivers，默认安装函数可能 panic；
    /// 本 crate 必须是进程中唯一的 Any driver 注册方；不捕获该 panic，也不提供 reset。
    /// 内存 SQLite 保持唯一连接，不自动按空闲时间或寿命回收，也不在 checkout 前执行 ping；
    /// 这避免上述自动回收或 checkout 前 ping 的取消丢失唯一连接；其他连接故障、连接丢失
    /// 或显式关闭仍可能释放内存数据，不提供持久化保证。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example() -> Result<(), SqlxError> {
    /// let client = SqlxClient::connect(SqlxConfig::new("sqlite::memory:")?).await?;
    /// assert!(!client.is_closed());
    /// client.close_async().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(config: SqlxConfig) -> Result<Self, SqlxError> {
        // 事件仅记录固定操作元数据和耗时，不格式化配置或底层错误。
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        #[cfg(feature = "tracing")]
        let metadata = sqlx_trace::ConnectMetadata {
            driver: config.driver_name(),
            sqlite_memory: config.sqlite_memory,
            max_connections: config.max_connections,
            min_connections: config.min_connections,
            acquire_timeout: config.acquire_timeout,
            max_rows: config.max_rows,
        };
        let result = Self::connect_inner(config).await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_connect(metadata, &result, started);
        result
    }

    /// 在调用方 runtime 中校验配置并创建连接池，失败时只保留脱敏错误。
    async fn connect_inner(config: SqlxConfig) -> Result<Self, SqlxError> {
        // 初始化依赖和访问数据库之前先检查上下文及配置，避免发布半初始化实例。
        ensure_runtime()?;
        config.validate()?;
        driver::install_default_drivers();

        // 普通数据库沿用 SQLx 的空闲回收、连接寿命及健康检查策略。
        let options = AnyPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(config.acquire_timeout);
        // 内存 SQLite 的唯一连接拥有整个数据库，不能被定期淘汰；取消 checkout
        // 期间的异步 ping 也可能丢弃该连接，因此取消这一额外 await 边界。
        let options = if config.sqlite_memory {
            options
                .idle_timeout(None)
                .max_lifetime(None)
                .test_before_acquire(false)
        } else {
            options
        };
        let pool = options
            .connect_with(config.connect_options.clone())
            .await
            .map_err(|error| SqlxError::from_upstream(&error))?;

        // 仅在连接成功后交付共享所有者；原始配置不保留在公开 Debug 状态中。
        Ok(Self {
            pool,
            max_rows: config.max_rows,
            #[cfg(feature = "tracing")]
            driver: config.driver_name(),
        })
    }

    /// 开启原生 SQLx Any 事务。
    ///
    /// 调用方必须显式 `commit` 或 `rollback`；drop 只作为回滚兜底。SQLx 0.9.0 没有为
    /// `Transaction` 直接实现 `Executor`，事务内执行应使用 `&mut *tx`。该返回值暴露 SQLx
    /// 原生事务和原生错误语义，因此调用方需要直接依赖匹配的 SQLx 版本并导入所需 trait。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut tx = client.begin_async().await?;
    /// sqlx::query::<sqlx::Any>("SELECT 1").execute(&mut *tx).await?;
    /// tx.commit().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn begin_async(&self) -> Result<SqlxTransaction<'static>, SqlxError> {
        // 事件仅记录固定操作元数据和耗时，不格式化配置或底层错误。
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 异步资源操作依赖调用方的 runtime；后端错误只转换为本库固定分类。
            ensure_runtime()?;
            self.pool
                .begin()
                .await
                .map_err(|error| SqlxError::from_upstream(&error))
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event("begin", self.driver, 0, 0, &result, started);
        result
    }

    /// 优雅地关闭共享连接池并等待关闭完成。
    ///
    /// 关闭后 `is_closed` 返回 `true`，后续执行会返回 [`SqlxError::PoolClosed`]。该方法不会
    /// 创建新 runtime，也不会重新打开 pool；多个 client clone 共享同一个关闭状态。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axutils::sqlx::{SqlxClient, SqlxError};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # async fn example(client: &SqlxClient) -> Result<(), SqlxError> {
    /// client.close_async().await?;
    /// assert!(client.is_closed());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close_async(&self) -> Result<(), SqlxError> {
        // 事件仅记录固定操作元数据和耗时，不格式化配置或底层错误。
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = async {
            // 异步资源操作依赖调用方的 runtime；后端错误只转换为本库固定分类。
            ensure_runtime()?;
            self.pool.close().await;
            Ok(())
        }
        .await;
        #[cfg(feature = "tracing")]
        sqlx_trace::record_event("close", self.driver, 0, 0, &result, started);
        result
    }

    /// 返回连接池是否已经进入关闭状态。
    ///
    /// 该方法不执行异步操作，也不检查远端数据库健康状态。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::sqlx::{SqlxClient};
    /// # #[cfg(any(feature = "sqlx", feature = "sqlx-postgres", feature = "sqlx-mysql", feature = "sqlx-sqlite"))]
    /// # {
    /// let _is_closed = SqlxClient::is_closed;
    /// # }
    /// ```
    pub fn is_closed(&self) -> bool {
        self.pool.is_closed()
    }
}

impl fmt::Debug for SqlxClient {
    /// 输出预算和关闭状态，不暴露连接选项、URL 或认证信息。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlxClient")
            .field("max_rows", &self.max_rows)
            .field("is_closed", &self.is_closed())
            .finish()
    }
}

/// 只检查现有 Tokio 上下文，不创建 runtime，也不承诺其 I/O/time driver 已启用。
fn ensure_runtime() -> Result<(), SqlxError> {
    Handle::try_current()
        .map(|_| ())
        .map_err(|_| SqlxError::RuntimeRequired)
}

#[cfg(all(test, feature = "sqlx-sqlite"))]
mod tests {
    use super::{SqlxClient, SqlxConfig};

    #[tokio::test]
    async fn memory_pool_preserves_its_only_connection() {
        for url in [
            "sqlite::memory:",
            "sqlite://axutils-pool-contract?mode=memory",
        ] {
            let client = SqlxClient::connect(SqlxConfig::new(url).unwrap())
                .await
                .unwrap();
            let options = client.pool.options();
            assert_eq!(options.get_idle_timeout(), None);
            assert_eq!(options.get_max_lifetime(), None);
            assert!(!options.get_test_before_acquire());
            client.close_async().await.unwrap();
        }
    }

    #[tokio::test]
    async fn non_memory_pool_keeps_standard_recycling_policy() {
        let client = SqlxClient::connect(SqlxConfig::new("sqlite:").unwrap())
            .await
            .unwrap();
        let options = client.pool.options();
        assert!(options.get_idle_timeout().is_some());
        assert!(options.get_max_lifetime().is_some());
        assert!(options.get_test_before_acquire());
        client.close_async().await.unwrap();
    }
}
