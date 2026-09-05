# SQLx

SQLx 能力按实际数据库 driver 选择。只启用一个 driver 可减少依赖与编译成本；`sqlx` 是三个 driver
的聚合入口。

```toml
[dependencies]
# 三选一：sqlx-postgres、sqlx-mysql 或 sqlx-sqlite
axutils = { version = "1.0", features = ["sqlx-postgres"] }
```

使用全部 driver 时：

```toml
[dependencies]
axutils = { version = "1.0", features = ["sqlx"] }
```

`SqlxConfig` 可在本地解析 PostgreSQL、MySQL/MariaDB 与 SQLite URL scheme；这不代表每种连接都
可用。实际连接只能使用已编译的 driver，缺少 driver 的连接会返回 `axutils::sqlx::SqlxError`。
首版不配置 TLS，URL 中可识别的显式 TLS 要求会在 `SqlxConfig::new` 阶段被拒绝。

## 实例 API

`SqlxConfig::new` 和 `SqlxClient::query`/`query_as`/`query_scalar` 只在本地构造配置或查询对象，
不需要 runtime。`connect`、execute/fetch、begin 和 close 等异步执行 API 必须在调用方 Tokio
runtime 中运行；本库不会创建 runtime 或调用 `block_on`。应用应直接依赖兼容的 Tokio 1.x 并启用
所需 runtime feature，不能依赖 SQLx feature 的传递依赖恰好可见。连接可能访问网络或 SQLite
文件，故示例标为 `no_run`。

```rust,no_run
use axutils::sqlx::{SqlxClient, SqlxConfig, SqlxError};

async fn connect() -> Result<SqlxClient, SqlxError> {
    let config = SqlxConfig::new("postgres://app:configured-outside-source@db.example.invalid/app")?;
    SqlxClient::connect(config).await
}
```

`SqlxClient` clone 共享连接池；`close_async` 会关闭共享 pool，且不会重新打开它。查询要使用
`SqlxClient` 创建的参数化 query，应用应将 URL、凭据、SQL 参数和数据库错误视为敏感边界。
首次 `connect` 会安装进程级 SQLx Any 默认 drivers；如果应用已经用 SQLx 自定义注册器安装过
Any drivers，当前实现可能 panic，因此本版本要求 axutils 是进程中唯一的 Any driver 注册方。
查询、bind、`FromRow`、row/result 和 transaction 的公共签名保留 SQLx 原生类型，调用方需要直接
依赖兼容的 SQLx 0.9.x 才能使用这些原生扩展点。

## 进程级入口

`SqlxUtils` 只负责一次异步初始化、状态与实例访问。成功后不可 reset 或 replace；重复初始化返回
`AlreadyInitialized`，失败或并发竞争中未获胜的连接不会占用初始化机会。

```rust,no_run
use axutils::{
    sqlx::{SqlxConfig, SqlxError},
    utils::SqlxUtils,
};

async fn initialize() -> Result<(), SqlxError> {
    let config = SqlxConfig::new("sqlite::memory:")?;
    SqlxUtils::init_async(config).await?;
    let _client = SqlxUtils::client()?;
    Ok(())
}
```

全局 client 关闭后仍可取得；查询对象仍可本地构造，但后续 execute/fetch 和事务等连接池操作保留
pool-closed 错误语义。多数据库、测试隔离或可控
生命周期应直接持有 `SqlxClient`。
