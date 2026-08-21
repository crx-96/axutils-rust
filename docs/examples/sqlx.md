# SQLx 使用文档

本文档对应 `sqlx + tokio` feature 组合，覆盖 `SqlxConfig`、`SqlxClient`、`SqlxUtils` 及其
原生 SQLx 类型别名。实现基于 SQLx `0.9.0` 的 `AnyPool`，在连接时按 URL 选择 PostgreSQL、
MySQL/MariaDB 或 SQLite driver。

## 安装和 feature 前提

`sqlx` 和 `tokio` 都必须显式启用。调用方还应直接依赖匹配的 SQLx 0.9.x 版本，因为本 crate
的 query 构造函数返回 SQLx 原生 `Query`/`QueryAs`/`QueryScalar`，事务也返回原生
`Transaction`。SQLx 0.9 只把静态字符串直接视为 `SqlSafeStr`；动态 SQL 必须由调用方审计后
包装 `sqlx::AssertSqlSafe`，用户数据应优先绑定参数：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["sqlx", "tokio"] }
sqlx = { version = "0.9.0", default-features = false, features = ["any", "postgres", "mysql", "sqlite-bundled", "runtime-tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

只启用 `sqlx` 会编译可选依赖，但不会导出 `axutils::sqlx`、根类型或 `SqlxUtils`；只启用
`tokio` 也不会引入 SQLx。首版不启用 SQLx facade 的 `macros`、`migrate`、`json` 或任何 TLS
feature。SQLx 0.9.0 的驱动依赖会在内部依赖树中带出 `sqlx-core` 的 `migrate` 支持（不再带出旧版
的 `json` core feature），这是上游实现依赖，不代表本 crate 提供 JSON、宏或 migration API。

所有连接、执行、读取、事务、初始化和关闭操作都要求调用方已经在 Tokio runtime 中运行。
crate 不创建 runtime、不调用 `block_on`，也不把 runtime 的所有权藏在 client 中。

本 crate 的普通 SQLx 运行测试以离线的 `sqlite::memory:` 为唯一成功数据库场景（其他后端只做
配置解析或失败路径检查）；它们不能证明 PostgreSQL 或
MySQL/MariaDB 的 placeholder 规则、行解码差异或连接错误映射。无网络 fixture 只证明对象构造、
公共 API 和 feature/依赖边界。本轮不运行远端数据库；若要验证 PG/MySQL 的运行时语义，必须另行
获得授权，在受控服务和显式 ignored live test 环境中执行，不能把本文件的 SQLite 结果表述为跨后端
运行时保证。

## URL、连接池和本地配置

`SqlxConfig` 是可 clone 的本地配置。以下每个 builder 都要求 `sqlx + tokio` feature；它们只做
本地校验，不连接数据库、不安装 Any driver、不创建 pool。支持的 scheme 是：

- PostgreSQL：`postgres://`、`postgresql://`；
- MySQL/MariaDB：`mysql://`、`mariadb://`；
- SQLite：`sqlite:`、`sqlite://`，包括 `sqlite::memory:`、`sqlite://:memory:` 和
  `?mode=memory` 形式。

首版不配置 TLS。URL 中可本地识别的 `sslmode=require`、证书路径、TLS mode 等显式 TLS 要求
会返回 `SqlxError::InvalidConfig { field: "tls" }`；没有显式 TLS 参数但远端 driver 后续要求
TLS 时，连接错误仍会被映射为稳定的脱敏错误，不能把这类 URL 宣称为已支持 TLS。

### `SqlxConfig::new`

签名：`pub fn new(url: impl AsRef<str>) -> Result<SqlxConfig, SqlxError>`；要求 `sqlx + tokio` feature。
解析 PostgreSQL、MySQL/MariaDB 或 SQLite URL，拒绝未知 scheme 和本地可识别的显式 TLS 要求，
分别返回 `InvalidConfig { field: "url_scheme" }`、`InvalidConfig { field: "tls" }` 或 URL
解析错误。普通 URL 的 `max_connections` 默认为 `10`，SQLite memory URL 默认为 `1`；其他默认值
是 `min_connections = 0`、获取连接超时 30 秒、`max_rows = 1_024`。不访问网络、不创建 pool，
`Debug` 不打印 URL 或凭据。

```rust
fn example() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")
}
```

### `SqlxConfig::with_max_connections`

签名：`pub fn with_max_connections(self, max_connections: u32) -> Result<Self, SqlxError>`；要求 `sqlx + tokio` feature。
允许 `1..=100`；SQLite memory URL 只能为 `1`，且不能小于已设置的 `min_connections`。无效值
返回 `InvalidConfig`，不访问网络或文件。

```rust
fn example() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")?.with_max_connections(1)
}
```

### `SqlxConfig::with_min_connections`

签名：`pub fn with_min_connections(self, min_connections: u32) -> Result<Self, SqlxError>`；要求 `sqlx + tokio` feature。
允许 `0..=max_connections`；大于 0 的值可能在后续 `connect` 阶段预先建立多个连接并产生网络、
认证或数据库资源副作用，但 builder 本身不连接。超过上限返回 `InvalidConfig`。

```rust
fn example() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")?.with_min_connections(0)
}
```

### `SqlxConfig::with_acquire_timeout`

签名：`pub fn with_acquire_timeout(self, acquire_timeout: Duration) -> Result<Self, SqlxError>`；要求 `sqlx + tokio` feature。
允许 `1ms..=5min`，拒绝零值、超长值和无限等待，错误为 `InvalidConfig`。它只限制 pool 获取
连接的等待预算，不替代数据库 server 的 statement timeout，也不在 builder 阶段等待。

```rust
fn example() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")?
        .with_acquire_timeout(std::time::Duration::from_secs(5))
}
```

### `SqlxConfig::with_max_rows`

签名：`pub fn with_max_rows(self, max_rows: usize) -> Result<Self, SqlxError>`；要求 `sqlx + tokio` feature。
允许 `1..=100_000`，错误为 `InvalidConfig`。该上限只约束 `fetch_all_async` 和
`fetch_all_as_async` 的结果行数，不限制单行字段大小，也不改变单行、可选行或标量入口；设置
只发生在内存中。

```rust
fn example() -> Result<axutils::SqlxConfig, axutils::SqlxError> {
    axutils::SqlxConfig::new("sqlite::memory:")?.with_max_rows(512)
}
```

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

签名：`pub async fn connect(config: SqlxConfig) -> Result<SqlxClient, SqlxError>`；要求
`sqlx + tokio` 和调用方 Tokio runtime。
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

### `SqlxClient::query`

签名：`pub fn query<'q>(&self, sql: impl sqlx::SqlSafeStr) -> sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments>`；
要求 `sqlx + tokio` feature。只构造固定 `Any` 后端的 query，不访问 pool、不执行 SQL，输出保留
`.bind(...)`、`.persistent(...)` 等 SQLx 链式 API。SQLx 0.9 默认只接受静态 SQL；动态片段必须
由调用方审计后包装 `sqlx::AssertSqlSafe`，不能把不可信标识符拼接进 SQL。

```rust
fn example(client: &axutils::SqlxClient) {
    let _query = client.query("SELECT id FROM users WHERE id = ?").bind(1_i64);
}
```

### `SqlxClient::query_as`

签名：`pub fn query_as<'q, T>(&self, sql: impl sqlx::SqlSafeStr) -> sqlx::query::QueryAs<'q, sqlx::Any, T, sqlx::any::AnyArguments>`，
其中 `T: for<'r> sqlx::FromRow<'r, axutils::SqlxRow>`；要求 `sqlx + tokio` feature。只构造 query，不访问
pool，类型解码错误延迟到执行方法并映射为 `SqlxError`；值仍应使用 `.bind(...)`。

```rust
fn example(client: &axutils::SqlxClient) {
    let _mapped = client.query_as::<(i64,)>("SELECT id FROM users");
}
```

### `SqlxClient::query_scalar`

签名：`pub fn query_scalar<'q, T>(&self, sql: impl sqlx::SqlSafeStr) -> sqlx::query::QueryScalar<'q, sqlx::Any, T, sqlx::any::AnyArguments>`，
其中 `(T,): for<'r> sqlx::FromRow<'r, axutils::SqlxRow>`；要求 `sqlx + tokio` feature。只构造读取第一列
的 query，不执行 SQL；`Decode`/`Type` 兼容性和无行错误由执行方法报告。

```rust
fn example(client: &axutils::SqlxClient) {
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

### `SqlxClient::execute_async`

签名：`pub async fn execute_async<'q>(&self, query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<SqlxQueryResult, SqlxError>`；
要求 `sqlx + tokio` feature 和调用方 Tokio runtime。
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

### `SqlxClient::fetch_one_async`

签名：`pub async fn fetch_one_async<'q>(&self, query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<SqlxRow, SqlxError>`；
要求 `sqlx + tokio` feature 和调用方 Tokio runtime。执行一个 query 并返回原生 `SqlxRow`；没有行返回
`RowNotFound`，连接、解码或 server 失败返回脱敏的稳定 `SqlxError` 分类。一次只保留一行结果，
不改变 pool 的配置行上限。

```rust,no_run
async fn example(client: &axutils::SqlxClient) -> Result<axutils::SqlxRow, axutils::SqlxError> {
    client.fetch_one_async(client.query("SELECT id FROM items WHERE id = ?").bind(1_i64)).await
}
```

### `SqlxClient::fetch_one_as_async`

签名：`pub async fn fetch_one_as_async<'q, T>(&self, query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<T, SqlxError>`；
`T` 需要 `FromRow + Send + Unpin`。要求 `sqlx + tokio` feature 和 runtime；没有行返回 `RowNotFound`，
类型解码、连接和 server 失败映射为脱敏 `SqlxError`。结果只有一个 `T`，不受多行上限影响。

```rust,no_run
async fn example(
    client: &axutils::SqlxClient,
) -> Result<(i64, String), axutils::SqlxError> {
    client
        .fetch_one_as_async(
            client
                .query_as::<(i64, String)>("SELECT id, name FROM items WHERE id = ?")
                .bind(1_i64),
        )
        .await
}
```

### `SqlxClient::fetch_optional_async`

签名：`pub async fn fetch_optional_async<'q>(&self, query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<Option<SqlxRow>, SqlxError>`；
要求 `sqlx + tokio` feature 和 runtime。没有行返回 `Ok(None)`，不会转换成 `RowNotFound`；SQLx 解码、
连接和 server 错误仍按稳定分类返回。最多保留一行结果。

```rust,no_run
async fn example(
    client: &axutils::SqlxClient,
) -> Result<Option<axutils::SqlxRow>, axutils::SqlxError> {
    client
        .fetch_optional_async(client.query("SELECT id FROM items WHERE id = ?").bind(999_i64))
        .await
}
```

### `SqlxClient::fetch_optional_as_async`

签名：`pub async fn fetch_optional_as_async<'q, T>(&self, query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<Option<T>, SqlxError>`；
`T` 需要 `FromRow + Send + Unpin`。要求 `sqlx + tokio` feature 和 runtime；无行是 `Ok(None)`，类型、连接
和 server 错误映射为脱敏 `SqlxError`。最多保留一行结果。

```rust,no_run
async fn example(
    client: &axutils::SqlxClient,
) -> Result<Option<(i64,)>, axutils::SqlxError> {
    client
        .fetch_optional_as_async(
            client.query_as::<(i64,)>("SELECT id FROM items WHERE id = ?").bind(999_i64),
        )
        .await
}
```

### `SqlxClient::fetch_all_async`

签名：`pub async fn fetch_all_async<'q>(&self, query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<Vec<SqlxRow>, SqlxError>`；
要求 `sqlx + tokio` feature 和 runtime。逐行消费 query，最多读取 `max_rows + 1` 行；0 行或刚好达到
`max_rows` 成功，读到第 `max_rows + 1` 行立即返回 `RowLimitExceeded { limit: max_rows }`，并停止
stream 使连接回 pool。stream 先发生的 SQLx 错误按脱敏稳定分类返回。上限只约束行数，不约束单行
字段大小，调用方仍需限制 BLOB/TEXT 和分页。

```rust,no_run
async fn example(
    client: &axutils::SqlxClient,
) -> Result<Vec<axutils::SqlxRow>, axutils::SqlxError> {
    client
        .fetch_all_async(client.query("SELECT id FROM items ORDER BY id"))
        .await
}
```

### `SqlxClient::fetch_all_as_async`

签名：`pub async fn fetch_all_as_async<'q, T>(&self, query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<Vec<T>, SqlxError>`；
`T` 需要 `FromRow + Send + Unpin`。要求 `sqlx + tokio` feature 和 runtime；使用同样的逐行消费和
`max_rows + 1` 上限，超限返回 `RowLimitExceeded`，解码/连接/server 失败映射为脱敏
`SqlxError`。上限不约束单个字段大小。

```rust,no_run
async fn example(
    client: &axutils::SqlxClient,
) -> Result<Vec<(i64, String)>, axutils::SqlxError> {
    client
        .fetch_all_as_async(
            client.query_as::<(i64, String)>("SELECT id, name FROM items ORDER BY id"),
        )
        .await
}
```

### `SqlxClient::fetch_scalar_async`

签名：`pub async fn fetch_scalar_async<'q, T>(&self, query: QueryScalar<'q, Any, T, sqlx::any::AnyArguments>) -> Result<T, SqlxError>`；
`T` 需要 `Send + Unpin` 且 `(T,): FromRow`，要求 `sqlx + tokio` feature 和 runtime。
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

### `SqlxClient::begin_async`

签名：`pub async fn begin_async(&self) -> Result<SqlxTransaction<'static>, SqlxError>`；要求
`sqlx + tokio` 和调用方 Tokio runtime。开始事务可能访问数据库并占用 pool 连接，失败返回脱敏
`SqlxError`。
`begin_async().await` 返回 `SqlxTransaction<'static>`，也就是 SQLx 原生
`Transaction<'static, sqlx::Any>`。调用方必须显式 `commit` 或 `rollback`；事务 drop 只作为
回滚兜底。

SQLx 0.9.0 没有为 `Transaction` 直接实现 `Executor`。事务内应使用 `&mut *tx`，并直接依赖
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

### `SqlxClient::close_async`

签名：`pub async fn close_async(&self) -> Result<(), SqlxError>`；要求 `sqlx + tokio` feature 和调用方
Tokio runtime。等待共享 pool 优雅关闭，失败只返回脱敏 `SqlxError`；关闭后所有 clone 都进入
closed 状态，后续执行返回 `PoolClosed`，不会重新打开或创建 runtime。等待时间受 pool/底层连接
关闭语义约束，本方法不提供额外的超时预算。

```rust,no_run
async fn close(client: &axutils::SqlxClient) -> Result<(), axutils::SqlxError> {
    client.close_async().await?;
    assert!(client.is_closed());
    Ok(())
}
```

### `SqlxClient::is_closed`

签名：`pub fn is_closed(&self) -> bool`；要求 `sqlx + tokio` feature，同步读取本地 pool 状态，不访问
网络、不检查远端健康，也不返回错误。共享 pool 被 `close_async` 关闭后返回 `true`，没有资源
副作用。

```rust
fn example(client: &axutils::SqlxClient) -> bool {
    client.is_closed()
}
```

## `SqlxUtils`

`SqlxUtils` 内部只维护 `OnceLock<SqlxClient>`，不复制实例 client 的查询逻辑。它适合进程中
只有一个默认数据库入口的场景；需要多个数据库、多组配置或独立生命周期时，应直接持有
`SqlxClient`。

### `SqlxUtils::init`

签名：`pub async fn init(config: SqlxConfig) -> Result<(), SqlxError>`；要求 `sqlx + tokio` feature 和
调用方 Tokio runtime。首次成功调用可能访问数据库或 SQLite 文件。
`init(config).await` 在首次调用时连接并在成功后写入全局 slot；失败不会消耗初始化机会。已
初始化时会在连接前快速返回 `AlreadyInitialized`，不会再次连接传入的 URL。并发初始化中输掉
`OnceLock` 竞争的 client 会先执行 `close_async`；即使清理本身失败，公开结果仍稳定返回
`AlreadyInitialized`，启用 `tracing` 时只记录脱敏的清理错误类别。

```rust,no_run
async fn init_global() -> Result<(), axutils::SqlxError> {
    axutils::SqlxUtils::init(axutils::SqlxConfig::new("sqlite::memory:")?).await
}
```

### `SqlxUtils::is_initialized`

签名：`pub fn is_initialized() -> bool`；要求 `sqlx + tokio` feature，同步读取进程内 `OnceLock`，不访问
网络、不返回错误，也不产生 I/O 副作用。
`is_initialized()` 只表示全局 slot 曾经成功写入，不检查远端健康状态，也不因 pool 关闭而回到
`false`。它是同步方法，不访问网络。

```rust
fn state() -> bool {
    axutils::SqlxUtils::is_initialized()
}
```

### `SqlxUtils::query`

签名：`pub fn query<'q>(sql: impl sqlx::SqlSafeStr) -> Query<'q, Any, sqlx::any::AnyArguments>`；要求 `sqlx + tokio` feature。
只构造 SQLx Any query，不检查全局初始化、不访问网络；执行时若尚未初始化才返回
`NotInitialized`。动态 SQL 仍必须由调用方审计后使用 `sqlx::AssertSqlSafe`。

```rust
fn example() {
    let _query = axutils::SqlxUtils::query("SELECT ?").bind(1_i64);
}
```

### `SqlxUtils::query_as`

签名：`pub fn query_as<'q, T>(sql: impl sqlx::SqlSafeStr) -> QueryAs<'q, Any, T, sqlx::any::AnyArguments>`，其中
`T: FromRow`；要求 `sqlx + tokio` feature。只构造映射 query，不检查全局状态或执行 SQL；类型错误延迟
到执行入口。

```rust
fn example() {
    let _query = axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1");
}
```

### `SqlxUtils::query_scalar`

签名：`pub fn query_scalar<'q, T>(sql: impl sqlx::SqlSafeStr) -> QueryScalar<'q, Any, T, sqlx::any::AnyArguments>`，其中
`(T,): FromRow`；要求 `sqlx + tokio` feature。只构造读取第一列的 query，不访问 pool 或网络。

```rust
fn example() {
    let _query = axutils::SqlxUtils::query_scalar::<i64>("SELECT 1");
}
```

每个静态执行/读取入口都要求 `sqlx + tokio` feature 和调用方 Tokio runtime，只获取全局 client 后转发；
未初始化时返回 `NotInitialized`。下面各节保留对应实例入口的独立错误和资源语义。

### `SqlxUtils::execute_async`

签名：`pub async fn execute_async<'q>(query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<SqlxQueryResult, SqlxError>`。
执行 query 并返回影响行数结果；未初始化返回 `NotInitialized`，runtime、连接或 SQLx 失败返回
脱敏 `SqlxError`。执行可能产生数据库写入或 DDL 副作用，不提供额外事务/超时上限。

```rust,no_run
async fn example() -> Result<axutils::SqlxQueryResult, axutils::SqlxError> {
    axutils::SqlxUtils::execute_async(axutils::SqlxUtils::query("SELECT 1")).await
}
```

### `SqlxUtils::fetch_one_async`

签名：`pub async fn fetch_one_async<'q>(query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<SqlxRow, SqlxError>`。
未初始化返回 `NotInitialized`，无行返回 `RowNotFound`，其他 SQLx 错误按稳定脱敏分类返回；
最多保留一个原生 row。

```rust,no_run
async fn example() -> Result<axutils::SqlxRow, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_one_async(axutils::SqlxUtils::query("SELECT 1")).await
}
```

### `SqlxUtils::fetch_one_as_async`

签名：`pub async fn fetch_one_as_async<'q, T>(query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<T, SqlxError>`，
`T: FromRow + Send + Unpin`。未初始化、无行、解码和连接错误分别保持 `NotInitialized`、
`RowNotFound` 或稳定脱敏分类；结果只有一个映射值。

```rust,no_run
async fn example() -> Result<(i64,), axutils::SqlxError> {
    axutils::SqlxUtils::fetch_one_as_async(
        axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1"),
    )
    .await
}
```

### `SqlxUtils::fetch_optional_async`

签名：`pub async fn fetch_optional_async<'q>(query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<Option<SqlxRow>, SqlxError>`。
未初始化返回 `NotInitialized`；无行是 `Ok(None)`，最多读取一行；解码、连接和 server 错误按
稳定脱敏分类返回。

```rust,no_run
async fn example() -> Result<Option<axutils::SqlxRow>, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_optional_async(
        axutils::SqlxUtils::query("SELECT 1 WHERE 0"),
    )
    .await
}
```

### `SqlxUtils::fetch_optional_as_async`

签名：`pub async fn fetch_optional_as_async<'q, T>(query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<Option<T>, SqlxError>`，
`T: FromRow + Send + Unpin`。无行是 `Ok(None)`，未初始化、解码和连接错误保持稳定分类；不受
多行上限影响，因为最多返回一个值。

```rust,no_run
async fn example() -> Result<Option<(i64,)>, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_optional_as_async(
        axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1 WHERE 0"),
    )
    .await
}
```

### `SqlxUtils::fetch_all_async`

签名：`pub async fn fetch_all_async<'q>(query: Query<'q, Any, sqlx::any::AnyArguments>) -> Result<Vec<SqlxRow>, SqlxError>`。
未初始化返回 `NotInitialized`；按全局 client 的 `max_rows` 逐行消费，读到第 `max_rows + 1` 行
返回 `RowLimitExceeded`，不限制单行字段大小；其他错误脱敏返回。

```rust,no_run
async fn example() -> Result<Vec<axutils::SqlxRow>, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_all_async(axutils::SqlxUtils::query("SELECT 1")).await
}
```

### `SqlxUtils::fetch_all_as_async`

签名：`pub async fn fetch_all_as_async<'q, T>(query: QueryAs<'q, Any, T, sqlx::any::AnyArguments>) -> Result<Vec<T>, SqlxError>`，
`T: FromRow + Send + Unpin`。使用全局 client 的 `max_rows` 逐行消费，超限返回 `RowLimitExceeded`，
解码、连接和未初始化错误保持对应稳定分类；不限制单个字段大小。

```rust,no_run
async fn example() -> Result<Vec<(i64,)>, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_all_as_async(
        axutils::SqlxUtils::query_as::<(i64,)>("SELECT 1"),
    )
    .await
}
```

### `SqlxUtils::fetch_scalar_async`

签名：`pub async fn fetch_scalar_async<'q, T>(query: QueryScalar<'q, Any, T, sqlx::any::AnyArguments>) -> Result<T, SqlxError>`，
`(T,): FromRow + Send + Unpin`。未初始化返回 `NotInitialized`，无行返回 `RowNotFound`，第一列
解码、连接和 server 错误按稳定脱敏分类返回。

```rust,no_run
async fn example() -> Result<i64, axutils::SqlxError> {
    axutils::SqlxUtils::fetch_scalar_async(
        axutils::SqlxUtils::query_scalar::<i64>("SELECT COUNT(*) FROM items"),
    )
    .await
}
```

### `SqlxUtils::begin_async`

签名：`pub async fn begin_async() -> Result<SqlxTransaction<'static>, SqlxError>`；要求
`sqlx + tokio` 和 runtime。未初始化返回 `NotInitialized`，成功后占用全局 pool 连接；事务错误
保持原生 SQLx 语义。
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

签名：`pub async fn close_async() -> Result<(), SqlxError>`；要求 `sqlx + tokio` feature 和 runtime。
未初始化返回 `NotInitialized`；已初始化时等待共享 pool 关闭并返回脱敏错误。
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
