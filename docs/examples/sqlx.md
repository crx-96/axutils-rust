# SQLx 使用文档

本文档对应 `sqlx + tokio` feature 组合，覆盖 `SqlxConfig`、`SqlxClient`、`SqlxUtils` 及其
原生 SQLx 类型别名。实现基于 SQLx `0.8.6` 的 `AnyPool`，在连接时按 URL 选择 PostgreSQL、
MySQL/MariaDB 或 SQLite driver。

## 安装和 feature 前提

`sqlx` 和 `tokio` 都必须显式启用。调用方还应直接依赖匹配的 SQLx 0.8.x 版本，因为本 crate
的 query 构造函数返回 SQLx 原生 `Query`/`QueryAs`/`QueryScalar`，事务也返回原生
`Transaction`：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["sqlx", "tokio"] }
sqlx = { version = "0.8.6", default-features = false, features = ["any", "postgres", "mysql", "sqlite", "runtime-tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

只启用 `sqlx` 会编译可选依赖，但不会导出 `axutils::sqlx`、根类型或 `SqlxUtils`；只启用
`tokio` 也不会引入 SQLx。首版不启用 SQLx facade 的 `macros`、`migrate`、`json` 或任何 TLS
feature。SQLx 0.8.6 的驱动依赖会在内部依赖树中带出 `sqlx-core` 的 `json`/`migrate` 支持，
这是上游实现依赖，不代表本 crate 提供 JSON、宏或 migration API。

所有连接、执行、读取、事务、初始化和关闭操作都要求调用方已经在 Tokio runtime 中运行。
crate 不创建 runtime、不调用 `block_on`，也不把 runtime 的所有权藏在 client 中。

## URL、连接池和本地配置

### `SqlxConfig`

`SqlxConfig` 是可 clone 的本地配置。`new` 只解析 URL、检查 scheme、识别显式 TLS 要求并设置
默认边界，不做网络连接、不安装 Any driver、不创建 pool。支持的 scheme 是：

- PostgreSQL：`postgres://`、`postgresql://`；
- MySQL/MariaDB：`mysql://`、`mariadb://`；
- SQLite：`sqlite:`、`sqlite://`，包括 `sqlite::memory:`、`sqlite://:memory:` 和
  `?mode=memory` 形式。

首版不配置 TLS。URL 中可本地识别的 `sslmode=require`、证书路径、TLS mode 等显式 TLS 要求
会返回 `SqlxError::InvalidConfig { field: "tls" }`；没有显式 TLS 参数但远端 driver 后续要求
TLS 时，连接错误仍会被映射为稳定的脱敏错误，不能把这类 URL 宣称为已支持 TLS。

```rust
fn build_config() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")?
        .with_max_connections(1)?
        .with_min_connections(0)?
        .with_acquire_timeout(std::time::Duration::from_secs(5))?
        .with_max_rows(512)
}
```

`SqlxConfig::new(url)`

- 返回 `Result<SqlxConfig, SqlxError>`；URL 语法、scheme 或本地可识别的 TLS 要求无效时返回
  `InvalidConfig`；
- 普通数据库 URL 的 `max_connections` 默认是 `10`，SQLite 内存 URL 默认是 `1`；
- `min_connections` 默认是 `0`；
- `acquire_timeout` 默认是 30 秒；
- `max_rows` 默认是 `1_024`；
- 不提供 URL getter；`Debug` 不打印 URL、用户名、密码、查询参数或其他凭据。

`SqlxConfig::with_max_connections(max_connections)`

允许 `1..=100`。SQLite 内存 URL 只能使用 `1`，因为每条独立连接可能拥有不同的内存数据库。
`0`、超过 100、超过内存 SQLite 的 1，或小于当前 `min_connections` 的值都会返回
`InvalidConfig`。普通 SQLite 文件 URL 可以使用多连接，但文件创建/修改和并发语义由 SQLite
及调用方负责。

`SqlxConfig::with_min_connections(min_connections)`

允许 `0..=max_connections`。设置大于 0 的值可能在连接阶段预先建立多个连接，产生网络、认证
和数据库资源副作用；配置 builder 本身仍不连接数据库。

`SqlxConfig::with_acquire_timeout(acquire_timeout)`

允许 `1ms..=5min`，拒绝零值、超长值和无限等待。该值限制 pool 获取连接的等待预算，不替代
数据库 server 的 statement timeout。

`SqlxConfig::with_max_rows(max_rows)`

允许 `1..=100_000`，只限制 `fetch_all_async`/`fetch_all_as_async` 的结果行数，不限制单行
字段大小，也不改变单行/可选行/标量入口。每个 builder 都返回 `Result<Self, SqlxError>`，
因此单字段和跨字段约束会在 builder 阶段失败，而不是延迟到连接之后。

## `SqlxError` 与稳定错误分类

`SqlxError` 是本 crate 的脱敏错误。它不保存 SQLx 原始错误对象，因此 `source()` 不会返回
底层错误链；`Debug`/`Display` 也不包含 SQL、完整 URL、密码、数据库原始消息或列名。事务
返回的原生 SQLx 错误不经过这个类型包装，调用方应按 SQLx 版本处理。

公开变体包括：

- `InvalidConfig { field }`：本地配置字段无效；`field` 是固定字段名；
- `RuntimeRequired`：异步入口不在 Tokio runtime 中；
- `NotInitialized` / `AlreadyInitialized`：`SqlxUtils` 的一次初始化状态；
- `RowNotFound`：单行或标量查询没有返回行；
- `RowLimitExceeded { limit }`：`fetch_all*` 读取到第 `limit + 1` 行；
- `PoolAcquireTimeout` / `PoolClosed`：pool 获取连接超时或 pool 已关闭；
- `TransactionFailed`：事务开始或事务状态失败；
- `Transport(SqlxTransportErrorKind)`：SQLx 底层连接、协议、服务端、编解码或其他失败的
  稳定分类。

`SqlxTransportErrorKind` 的分类是 `Connection`、`Timeout`、`Protocol`、`Server`、`Network`、
`Decode`、`Encode`、`Tls` 和 `Other`。分类不保证保留数据库的认证错误细分；需要重试或告警
时应按稳定分类和业务幂等性决策，不解析 `Display` 文本。

## `SqlxClient`

### `SqlxClient::connect`

`connect(config).await` 的顺序是：检查当前 Tokio runtime、再次校验本地配置、通过 SQLx 一次
性默认注册器安装已编译的 Any drivers、建立 `AnyPool` 并返回 client。它可能访问 PostgreSQL/
MySQL/MariaDB 网络，或创建/修改 SQLite 文件；配置构造阶段没有这些副作用。

Any driver 注册是进程级状态。本 crate 假定自己是进程中唯一的默认 Any 注册方；如果调用方先
通过 SQLx 自定义注册器安装 driver，SQLx 默认安装函数可能 panic。本 crate 不捕获该 panic、
不提供 reset，也不承诺与自定义注册器混用。SQLx 自己的注册函数保证默认路径只安装一次。

```rust,no_run
async fn connect() -> Result<(), axutils::SqlxError> {
    let config = axutils::SqlxConfig::new("sqlite::memory:")?;
    let client = axutils::SqlxClient::connect(config).await?;
    assert!(!client.is_closed());
    client.close_async().await?;
    Ok(())
}
```

### 查询构造：`query`、`query_as`、`query_scalar`

`SqlxClient::query(sql)`、`SqlxClient::query_as::<T>(sql)` 和
`SqlxClient::query_scalar::<T>(sql)` 只调用 SQLx 对应构造函数并固定 `Any` 后端，不访问 pool、
不执行 SQL。返回对象保留 SQLx 的 `.bind(...)`、`.persistent(...)` 和其他链式 API。

`query_as` 的 `T` 需要 `FromRow`；`query_scalar` 读取每行第一列为 `T`。参数必须通过
`.bind(...)` 传入；SQL 片段和标识符不能把不可信输入直接拼接进 SQL。PostgreSQL/MySQL/
SQLite 的占位符仍按实际 driver 和 SQLx 规则书写。

```rust
fn make_queries(client: &axutils::SqlxClient) {
    let _query = client.query("SELECT id FROM users WHERE id = ?").bind(1_i64);
    let _mapped = client.query_as::<(i64,)>("SELECT id FROM users");
    let _scalar = client.query_scalar::<i64>("SELECT COUNT(*) FROM users");
}
```

本 crate 不提供 `query_builder` 包装。动态 SQL 片段应直接使用 SQLx 原生
`QueryBuilder::<sqlx::Any>`，只对白名单化的 SQL 片段调用 `push`，对值使用 `push_bind`：

```rust
fn build_dynamic_query() {
    let mut builder = sqlx::QueryBuilder::<sqlx::Any>::new("SELECT ?");
    builder.push_bind(1_i64);
    let _query = builder.build();
}
```

### `execute_async`

`execute_async(query).await` 接受固定为 `Any` 的 SQLx `Query`，包括
`QueryBuilder::build()` 的结果，返回 `SqlxQueryResult`。它要求 runtime，并把 SQLx 错误映射为
脱敏的 `SqlxError`；不会把 SQL 文本或数据库响应放进错误。

```rust,no_run
async fn create_table(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    client
        .execute_async(client.query("CREATE TABLE items (id INTEGER NOT NULL)"))
        .await?;
    Ok(())
}
```

### 单行读取：`fetch_one_async`、`fetch_one_as_async`

`fetch_one_async(query).await` 返回原生 `SqlxRow`；`fetch_one_as_async::<T>(query).await` 返回
映射后的 `T`。两者在没有行时都返回 `SqlxError::RowNotFound`，映射类型需要满足 SQLx 的
`FromRow`、`Send` 和 `Unpin` 约束。

```rust,no_run
async fn read_one(client: &axutils::SqlxClient) -> Result<(i64, String), axutils::SqlxError> {
    client
        .fetch_one_as_async(
            client
                .query_as::<(i64, String)>("SELECT id, name FROM items WHERE id = ?")
                .bind(1_i64),
        )
        .await
}
```

### 可选单行：`fetch_optional_async`、`fetch_optional_as_async`

这两个入口分别返回 `Option<SqlxRow>` 和 `Option<T>`；没有行是 `Ok(None)`，不会转换成
`RowNotFound`。SQLx 解码、类型不兼容和 server 错误仍按 `SqlxError` 的稳定分类返回。

```rust,no_run
async fn find_optional(
    client: &axutils::SqlxClient,
) -> Result<Option<(i64,)>, axutils::SqlxError> {
    client
        .fetch_optional_as_async(
            client.query_as::<(i64,)>("SELECT id FROM items WHERE id = ?").bind(999_i64),
        )
        .await
}
```

### 有界多行：`fetch_all_async`、`fetch_all_as_async`

`fetch_all_async(query).await` 返回 `Vec<SqlxRow>`；`fetch_all_as_async::<T>(query).await` 返回
`Vec<T>`。两者都不调用 SQLx 无界的 `fetch_all`，而是逐行消费 stream：

1. 最多读取 `max_rows + 1` 行；
2. 0 行和刚好 `max_rows` 行返回成功；
3. 读取到第 `max_rows + 1` 行时立即返回 `RowLimitExceeded { limit: max_rows }`；
4. 超限后停止 stream，使连接回到 pool；stream 中先发生 SQLx 错误时返回对应脱敏错误。

实现对 `max_rows + 1` 使用 checked addition；当前配置范围使其始终有界。该上限只限制行数，
不限制单行字段大小，因此调用方仍应对 BLOB/TEXT 等字段设置业务级约束或分页。

```rust,no_run
async fn list_items(
    client: &axutils::SqlxClient,
) -> Result<Vec<(i64, String)>, axutils::SqlxError> {
    client
        .fetch_all_as_async(
            client.query_as::<(i64, String)>("SELECT id, name FROM items ORDER BY id"),
        )
        .await
}
```

### `fetch_scalar_async`

`fetch_scalar_async::<T>(query).await` 读取第一行的第一列并返回 `T`。没有行返回
`RowNotFound`；`Decode`/`Type` 不兼容和其他 SQLx 错误保持稳定分类。需要区分“没有行”和
“有一行但列值为 NULL”时，应按 SQLx 类型使用 `Option<T>`，并选择合适的查询形态。

```rust,no_run
async fn count_items(client: &axutils::SqlxClient) -> Result<i64, axutils::SqlxError> {
    client
        .fetch_scalar_async(client.query_scalar::<i64>("SELECT COUNT(*) FROM items"))
        .await
}
```

### `begin_async`

`begin_async().await` 返回 `SqlxTransaction<'static>`，也就是 SQLx 原生
`Transaction<'static, sqlx::Any>`。调用方必须显式 `commit` 或 `rollback`；事务 drop 只作为
回滚兜底。

SQLx 0.8.6 没有为 `Transaction` 直接实现 `Executor`。事务内应使用 `&mut *tx`，并直接依赖
匹配的 SQLx 版本以导入 SQLx 所需 trait：

```rust,no_run
async fn insert_in_transaction(
    client: &axutils::SqlxClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = client.begin_async().await?;
    sqlx::query::<sqlx::Any>("INSERT INTO items (id, name) VALUES (?, ?)")
        .bind(1_i64)
        .bind("one")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
```

本 crate 不提供 callback transaction wrapper，也不复制一套事务专用 query 方法；事务中的
原生 SQLx 错误不伪装成 `SqlxError`。

### `close_async` 和 `is_closed`

`close_async().await` 会等待共享 pool 优雅关闭；它要求 runtime，返回 `Result<(), SqlxError>`。
关闭后 `is_closed()` 返回 `true`，后续执行按 SQLx pool closed 语义返回 `PoolClosed`，不会重新
打开 pool。client clone 共享同一 pool 和关闭状态。

```rust,no_run
async fn close(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    client.close_async().await?;
    assert!(client.is_closed());
    Ok(())
}
```

## `SqlxUtils`

`SqlxUtils` 内部只维护 `OnceLock<SqlxClient>`，不复制实例 client 的查询逻辑。它适合进程中
只有一个默认数据库入口的场景；需要多个数据库、多组配置或独立生命周期时，应直接持有
`SqlxClient`。

### `SqlxUtils::init`

`init(config).await` 在首次调用时连接并在成功后写入全局 slot；失败不会消耗初始化机会。已
初始化时会在连接前快速返回 `AlreadyInitialized`，不会再次连接传入的 URL。并发初始化中输掉
`OnceLock` 竞争的 client 会先执行 `close_async`，不会泄漏已建立的 pool。

```rust,no_run
async fn init_global() -> Result<(), axutils::SqlxError> {
    axutils::SqlxUtils::init(axutils::SqlxConfig::new("sqlite::memory:")?).await
}
```

### `SqlxUtils::is_initialized`

`is_initialized()` 只表示全局 slot 曾经成功写入，不检查远端健康状态，也不因 pool 关闭而回到
`false`。它是同步方法，不访问网络。

```rust
fn state() -> bool {
    axutils::SqlxUtils::is_initialized()
}
```

### 静态查询构造：`query`、`query_as`、`query_scalar`

三个静态构造入口与 `SqlxClient` 对应方法完全同义，只做 SQLx Any query 构造，不检查
`SqlxUtils` 初始化状态；因此可以先构造 query，执行时再得到 `NotInitialized`。

```rust
fn make_global_queries() {
    let _query = axutils::SqlxUtils::query("SELECT ?").bind(1_i64);
    let _mapped = axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1");
    let _scalar = axutils::SqlxUtils::query_scalar::<i64>("SELECT 1");
}
```

### 静态执行和读取转发

`SqlxUtils::execute_async`、`fetch_one_async`、`fetch_one_as_async`、`fetch_optional_async`、
`fetch_optional_as_async`、`fetch_all_async`、`fetch_all_as_async` 和 `fetch_scalar_async` 只获取
全局 client 后转发。因此它们与实例入口共享完全相同的 runtime、错误脱敏和行数上限语义；
未初始化时返回 `NotInitialized`。

```rust,no_run
async fn use_global() -> Result<(), axutils::SqlxError> {
    axutils::SqlxUtils::execute_async(axutils::SqlxUtils::query("CREATE TABLE items (id INTEGER)"))
        .await?;
    let _row: Option<(i64,)> = axutils::SqlxUtils::fetch_optional_as_async(
        axutils::SqlxUtils::query_as::<(i64,)>("SELECT id FROM items WHERE id = ?").bind(1_i64),
    )
    .await?;
    let _rows = axutils::SqlxUtils::fetch_all_async(axutils::SqlxUtils::query("SELECT id FROM items"))
        .await?;
    let _value = axutils::SqlxUtils::fetch_scalar_async(
        axutils::SqlxUtils::query_scalar::<i64>("SELECT COUNT(*) FROM items"),
    )
    .await?;
    Ok(())
}
```

### `SqlxUtils::begin_async`

该方法转发全局 client 的原生事务入口，返回 `SqlxTransaction<'static>`；事务内仍使用
`&mut *tx`，并由调用方显式 commit/rollback：

```rust,no_run
async fn global_transaction() -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = axutils::SqlxUtils::begin_async().await?;
    sqlx::query::<sqlx::Any>("SELECT 1")
        .execute(&mut *tx)
        .await?;
    tx.rollback().await?;
    Ok(())
}
```

### `SqlxUtils::close_async`

`close_async().await` 优雅关闭全局 pool，但不清除 `OnceLock`。关闭后
`SqlxUtils::is_initialized()` 仍是 `true`，后续执行返回 `PoolClosed`，再次 `init` 返回
`AlreadyInitialized`。这使全局入口不可 reset；需要重新连接时必须在新进程或直接使用新的
`SqlxClient` 设计生命周期。

```rust,no_run
async fn close_global() -> Result<(), axutils::SqlxError> {
    axutils::SqlxUtils::close_async().await?;
    assert!(axutils::SqlxUtils::is_initialized());
    Ok(())
}
```

## 公共类型别名和导出路径

推荐从 `axutils::sqlx` 导入领域类型；为了兼容既有工具类风格，实例类型和全局入口也从 crate
根导出：

| 类型 | 领域/根路径 | 说明 |
| --- | --- | --- |
| `SqlxConfig` | `axutils::sqlx::SqlxConfig`、`axutils::SqlxConfig` | URL 和本地池边界配置 |
| `SqlxClient` | `axutils::sqlx::SqlxClient`、`axutils::SqlxClient` | 可 clone 的 Any pool client |
| `SqlxError` | `axutils::sqlx::SqlxError`、`axutils::SqlxError` | 脱敏稳定错误 |
| `SqlxTransportErrorKind` | `axutils::sqlx::SqlxTransportErrorKind`、`axutils::SqlxTransportErrorKind` | 底层错误分类 |
| `SqlxRow` | `axutils::sqlx::SqlxRow`、`axutils::SqlxRow` | SQLx `AnyRow` 别名 |
| `SqlxQueryResult` | `axutils::sqlx::SqlxQueryResult`、`axutils::SqlxQueryResult` | SQLx `AnyQueryResult` 别名 |
| `SqlxTransaction<'a>` | `axutils::sqlx::SqlxTransaction<'a>`、`axutils::SqlxTransaction<'a>` | SQLx `Transaction<'a, Any>` 别名 |
| `SqlxUtils` | `axutils::SqlxUtils`、`axutils::utils::SqlxUtils`、`axutils::utils::sqlx_utils::SqlxUtils` | OnceLock 全局入口 |

`client`、`config`、`error`、`driver` 是实现模块，不是稳定公共导入路径。SQLite bundled
driver 会增加编译时间和 native 构建成本；SQLite 文件 URL 可能创建或修改本地文件，测试和
示例默认使用 `sqlite::memory:`，并将连接数固定为 1。
