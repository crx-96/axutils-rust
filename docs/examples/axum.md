# Axum 服务工具使用文档

## Feature、导出与非目标

服务 API 仅在 `axum + tokio` 下从 `axutils::*` 与 `axutils::axum::*` 导出；`AxumUtils` 还支持 `axutils::utils::*` 和 `axutils::utils::axum_utils::*`。Tower、Tower HTTP、tower_governor wrapper 分别要求同名 provider feature。签名中的 Axum Router/MethodRouter、`HeaderValue`、Tower Layer、Tokio TcpListener 需调用方直接依赖兼容版本。`tower_governor` 0.8 的 `axum` feature 会启用 Axum default，带入 form/json/query/original-uri/tower-log/tracing 并间接启用 Tokio macros；provider-only 不会拉入 Axum。这是上游固定 feature 扩张，不表示 axutils 自动安装相关行为。

每个示例使用声明的最小 axutils feature；宿主还必须直接声明示例实际引用的 provider/runtime 类型。
基础路由示例的最小依赖是：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["axum", "tokio"] }
axum = { version = "0.8.9", default-features = false, features = ["http1", "tokio"] }
tokio = { version = "1.53.1", default-features = false, features = ["net", "rt-multi-thread", "signal"] }
```

`AxumApp::route_service`/`with_layer` 的代码若直接写 `tower::...`，只需追加
`tower = { version = "0.5.3", default-features = false, features = ["util"] }`；这些 API 本身
仍只要求 axutils 的 `axum,tokio`，不要求另开本 crate 的 `tower` provider feature。
`AxumServerBuilder::with_concurrency_limit` 还要求 axutils 的 `tower` feature，并使用上述
直接依赖。`tower-http` 示例追加 `tower-http = { version = "0.7.0", default-features = false,
features = ["catch-panic", "cors", "limit", "request-id", "timeout", "trace"] }` 与 axutils 的
`tower-http` feature；`tower_governor` 示例追加 `tower_governor = { version = "0.8.0",
default-features = false, features = ["axum"] }` 与 axutils 的 `tower_governor` feature；trace 示例再追加
`tracing = { version = "0.1.44", default-features = false, features = ["std"] }` 与 `tracing`。
宿主应使用与本 crate 兼容的版本；Tokio runtime 由宿主创建，本 crate 不隐式创建 runtime。

首版只提供 HTTP/1 与协作式 graceful drain，不提供 TLS、HTTP/2、header/read timeout、强制 drain deadline、trusted-proxy CIDR 验证、鉴权或业务错误格式。

```rust,no_run
use axum::routing::get;
use axutils::AxumApp;
let server = AxumApp::new().route("/health", get(|| async { "ok" }))
    .into_server_builder().build()?;
# Ok::<(), axutils::AxumError>(())
```

## AxumApp

### create_router / new / Default / from_router
创建保留 missing-state 泛型的原生空 Router、空 AxumApp 或包装调用方 Router；只组装内存状态，
不 bind。

### route / route_service / nest / merge / fallback
保持 Axum 0.8 的注册、嵌套、合并和 fallback 语义。单 route middleware 应先在 MethodRouter/子 Router 上使用原生 layer。

### with_layer
延迟登记全 Router layer，最终覆盖 routes、405 与 fallback。声明 a、b 时请求顺序为 a→b→handler，响应反向。

### with_matched_route_layer
延迟登记仅匹配 routes 的 layer，不覆盖 404 fallback。空 Router 使用此栈时 `build` 返回 `InvalidConfig`，不会触发上游 panic。

### with_state / into_server_builder
`with_state` 把 `Router<S>` 收敛为 `Router<()>`；无 state 应用使用 `into_server_builder`。两者统一应用延迟栈。

## AxumCorsOrigin

`AxumCorsOrigin` 只在 `axum + tokio + tower-http` 下导出，推荐路径是
`axutils::AxumCorsOrigin`，领域路径是 `axutils::axum::AxumCorsOrigin`；没有
`axutils::utils::AxumCorsOrigin` 兼容路径。它只保存内存配置，不访问网络、不安装 layer，也不
自行返回错误；`AxumServerBuilder::with_cors` 才执行组合校验。`List` 的元素是宿主直接依赖
Axum 0.8 提供的 `axum::http::HeaderValue`，不能把普通字符串直接放入 `Vec<HeaderValue>`。

### `AxumCorsOrigin::Disabled`

要求 `axum + tokio + tower-http` feature。禁用 CORS；`with_cors` 不安装 CORS layer。它是
`AxumCorsOrigin` 的默认配置语义，不能隐式
扩大为允许任意 origin。

```rust
# #[cfg(all(feature = "axum", feature = "tokio", feature = "tower-http"))]
let origins = axutils::AxumCorsOrigin::Disabled;
```

### `AxumCorsOrigin::Any`

要求 `axum + tokio + tower-http` feature。允许任意 origin；当 `AxumCorsConfig::allow_credentials` 为 `true` 时，`with_cors` 返回
`AxumError::InvalidConfig { field: "cors_credentials" }`，不会进入上游 wildcard + credentials
路径。该 variant 本身只保存配置，不产生副作用。

```rust
# #[cfg(all(feature = "axum", feature = "tokio", feature = "tower-http"))]
let origins = axutils::AxumCorsOrigin::Any;
```

### `AxumCorsOrigin::List(Vec<HeaderValue>)`

要求 `axum + tokio + tower-http` feature。允许有限 origin 列表。`HeaderValue` 应由宿主直接依赖的 Axum HTTP 类型构造：可信静态值可用
`HeaderValue::from_static`，运行时字符串应使用 `HeaderValue::from_str`/`parse` 并处理
`InvalidHeaderValue`，原始字节应使用 `HeaderValue::from_bytes` 并处理错误；本 crate 不替调用方
过滤控制字符或做 origin 语义解析。列表最多 64 项，每项 `as_bytes()` 最多 1,024 字节；值为
`*` 或超限列表在安装时返回 `InvalidConfig { field: "cors_origins" }`，空列表则表示不安装
CORS layer。`HeaderValue` 不能包含换行等非法字节，
因此构造失败不会产生部分配置。

```rust
# #[cfg(all(feature = "axum", feature = "tokio", feature = "tower-http"))]
{
    use axum::http::HeaderValue;

    let origin = HeaderValue::from_static("https://example.com");
    let origins = axutils::AxumCorsOrigin::List(vec![origin]);
    let _ = origins;
}
```

## AxumConfig

### new / Default
配置仅保存有限边界，不安装默认 middleware。

### with_service_timeout / service_timeout
范围 1 毫秒到 10 分钟；它是 service future 预算，不是连接/header read 或 drain deadline。

### with_max_body_bytes / max_body_bytes
范围 1 字节到 64 MiB；provider body-limit 仍需显式安装。

### with_max_concurrency / max_concurrency
范围 1..=65,536；provider concurrency 仍需显式安装。

## AxumServerBuilder

内建 request-ID、trace、timeout、catch-panic 在 `build` 时按固定安全顺序收敛；request-ID finalizer 和 trace 位于错误响应生成层之外，因此 408/500 仍携带内部 ID 并产生完成事件。重复安装 request-ID 幂等。用户提供的普通 Router layer 仍遵循 Axum 声明顺序。

### config / build
替换配置并构造共享单次状态机，不 bind。配置或空 matched stack 错误在 build 返回。

### with_concurrency_limit（tower）
安装 HandleError→LoadShed→ConcurrencyLimit，满载立即返回脱敏 503，不排队。

### with_request_id（tower-http）
删除全部入站 x-request-id，生成内部 UUID，并最终覆盖 handler 冲突响应值。

### with_timeout（tower-http）
安装 1 毫秒..=10 分钟的 408 或显式 504 service timeout。

### with_body_limit / with_catch_panic（tower-http）
body 上限为 1..=64 MiB；catch-panic 返回脱敏 500，不能捕获 abort。layer 自身不记录 payload，但 Rust 进程 panic hook 会在捕获前运行；宿主必须安装脱敏 hook，且不得把 secret 放入 panic payload。

### with_cors（tower-http）
`AxumCorsConfig` 区分 Disabled、Any 与有限 origin list；credentials 与 wildcard 组合被拒绝。放在 matched route 时可能漏掉 preflight/404，因此此 builder 方法应用于全 Router。

### with_http_trace（tower-http + tracing）
只记录 method、matched route 模板、status、latency 和内部 request ID；404 使用固定占位，不记录原始 URI/query/header/body/panic payload。库不安装 subscriber。

### with_governor_peer / with_governor_forwarded_headers_unchecked（tower_governor）
前者只使用真实 peer SocketAddr；后者明确无条件信任 Forwarded/X-Forwarded-For/X-Real-IP，入口可信代理必须覆盖客户端 header。配额参数为每次补 token 的 Duration 和非零 burst；429/500 使用固定脱敏 body。每个 server 共享 limiter；serve 通过独立于 Tokio time driver 的 `futures-timer` 每 60 秒清理 stale key，正常 shutdown 会取消并等待清理任务，Abandoned 路径尽力取消。高基数来源仍需由部署边界限制。

## AxumServer

### config / shutdown_handle
读取 immutable 配置或获得共享句柄；server clone 共享身份、状态与 limiter。

### serve_addr / serve / serve_with_shutdown
`serve_addr` 执行 bind，失败回滚 Ready；`serve` 接受已 bind TcpListener；自定义 future 必须返回 `AxumShutdownReason`。同一实例只允许一次 active serve；正常 drain 后 Stopped，future 被 drop/panic 后 Abandoned，终态不可重启。返回 `AxumServeOutcome` 的实际地址与首个原因。卡住请求可能让 drain 无限等待。

## AxumShutdownHandle / AxumUtils

### AxumShutdownHandle::shutdown
签名：`pub fn shutdown(&self, reason: AxumShutdownReason) -> Result<AxumShutdownReason, AxumError>`；
要求 `axum + tokio` feature。输入关闭原因只改变共享内存状态并唤醒正在 drain 的 server；Running
首次调用返回该原因，Draining 重复调用幂等返回首原因，Ready/Starting 返回 `NotRunning`，Stopped
或 Abandoned 返回对应终态错误；不 bind、不访问网络、不创建 runtime。

### AxumUtils::init / is_initialized
全局 OnceLock 只成功初始化一次，不提供 reset/replace；停止后仍保持 initialized。

### AxumUtils::shutdown_handle / shutdown
转发到全局 server；未初始化返回 `NotInitialized`。

### AxumUtils::serve_addr / serve / serve_with_shutdown
转发全局单次服务入口，runtime、listener、shutdown 和错误语义与实例方法一致。

## 结果与错误

`AxumServeOutcome::local_addr`/`reason` 读取实际地址和原因。`AxumShutdownReason` 与 `AxumError` 是
`#[non_exhaustive]` 枚举；在 crate 外部匹配时必须保留 wildcard 分支（例如 `_ => ...`），不能假定当前
变体集合永久不变。`AxumTimeoutStatus` 当前不是 `#[non_exhaustive]`，但它仍只表示 timeout response
status，不表示连接或 drain 状态。错误 Display/Debug 不包含请求、Header、body 或 provider 原始消息；
governor cleanup task 异常退出时 serve 返回脱敏 `AxumError::BackgroundTask`，不会静默吞掉 JoinError。

### `AxumShutdownReason`

要求 `axum + tokio` feature；推荐从 `axutils::AxumShutdownReason` 导入，也可从
`axutils::axum::AxumShutdownReason` 导入。该 `#[non_exhaustive]` 枚举表示一次服务首次进入
graceful shutdown 的原因；`AxumServeOutcome::reason` 和 `AxumShutdownHandle::shutdown` 使用它。
外部 crate 匹配时必须写 `_` wildcard，以兼容未来新增原因。

| 变体 | 语义与调用方责任 |
| --- | --- |
| `Programmatic` | 由 `AxumShutdownHandle::shutdown` 或其他程序化入口触发；不表示 OS 信号。 |
| `CtrlC` | 跨平台 Ctrl+C 信号触发；服务只把它作为关闭原因，不替调用方安装 signal handler。 |
| `Sigterm` | Unix `SIGTERM` 触发；在非 Unix 平台不会由库的 OS signal 分支产生，但宿主可在自定义 future 中选择其他原因。 |
| `Custom(String)` | 宿主或测试提供的非敏感标签；`Display` 输出 `custom:<标签>`，因此字符串不得包含 secret、token、请求内容或其他敏感信息。 |

该类型只保存内存状态，不执行 I/O；`Custom` 的字符串会随 outcome/共享状态 clone。若只关心已知原因，
应保留 `_` 分支，例如 `match outcome.reason() { AxumShutdownReason::CtrlC => ..., _ => ... }`。

### `AxumError`

要求 `axum + tokio` feature；推荐从 `axutils::AxumError` 导入，也可从
`axutils::axum::AxumError` 导入。该 `#[non_exhaustive]` 枚举覆盖配置、服务状态、listener 和
signal 错误；外部 crate 匹配时必须保留 `_` wildcard。`Display` 与 `Debug` 均经过脱敏，不能用来
恢复 provider 原始文本；`Io`/`Signal` 的 `io::Error` 仍可通过标准 `Error::source()` 供程序分类，
但调用方不得把其文本直接写入面向用户的响应或日志。

| 变体 | 字段与语义 |
| --- | --- |
| `InvalidConfig { field: &'static str }` | 配置字段越界或组合无效；`field` 是稳定字段名（如 `service_timeout`、`max_body_bytes`、`cors_credentials`），不含调用方输入值。 |
| `AlreadyRunning` | 服务正在启动、运行或 draining；同一 `AxumServer` 不能并行进行第二次 active serve。 |
| `AlreadyStopped` | 单次服务已经正常停止；终态不可重新启动。 |
| `Abandoned` | serve future 被 drop 或 panic 异常离开，状态不可复用；不会暴露 panic payload。 |
| `NotRunning` | 当前实例处于 Ready/Starting，不能触发 shutdown；不是全局 `AxumUtils` 未初始化错误。 |
| `NotInitialized` | `AxumUtils` 的全局 `OnceLock` 尚未初始化；先调用 `AxumUtils::init`。 |
| `AlreadyInitialized` | `AxumUtils::init` 已经成功过；全局入口不可 reset/replace。 |
| `Io(io::Error)` | listener bind 或 local address 查询失败；错误文本在 `Display`/`Debug` 中脱敏。 |
| `Signal(io::Error)` | 注册 OS shutdown signal 失败；错误文本同样脱敏。 |
| `BackgroundTask` | 内部生命周期/限流清理任务异常退出；只返回稳定脱敏分类，不暴露 JoinError 或 panic payload。 |

这些错误只在对应 API 调用或服务生命周期阶段产生；构造 `AxumApp`、配置和 builder 不 bind、不访问
网络。对 `InvalidConfig` 应按 `field` 做程序化处理，对其余变体使用 wildcard 兜底，避免把错误文本当作
稳定协议。

### `AxumTimeoutStatus`

要求 `axum + tokio + tower-http` feature；推荐从 `axutils::AxumTimeoutStatus` 导入，也可从
`axutils::axum::AxumTimeoutStatus` 导入。它只作为 `AxumServerBuilder::with_timeout` 的 response
status 参数，不改变 timeout 时限、不创建 runtime、不访问网络，也不表示连接读取或 graceful drain
deadline。当前源码未标记 `#[non_exhaustive]`，因此现有两个变体可穷尽匹配；调用方若希望为未来扩展
保留兼容分支，仍可使用 `_` wildcard。

| 变体 | HTTP 状态与语义 |
| --- | --- |
| `RequestTimeout` | 返回 HTTP 408 `Request Timeout`；默认值，表示本服务的 service future 超时。 |
| `GatewayTimeout` | 返回 HTTP 504 `Gateway Timeout`；仅在当前服务确实承担网关语义时使用，不能据此声称库提供代理或 upstream timeout。 |

`with_timeout` 仍要求时限在 1 毫秒到 10 分钟之间；这里只选择超时响应码，超时不会终止连接读写，
也不会给 shutdown drain 增加硬 deadline。

## 逐项 API 签名与示例

以下小节补齐上文概览中合并展示的方法；均要求 `axum + tokio`，额外 feature 单独标明。

### `AxumApp::create_router`
该方法定义在 `AxumApp<S>` 上，签名为 `pub fn create_router() -> axum::Router<S>`，其中
`S: Clone + Send + Sync + 'static`；要求 `axum + tokio` feature。无输入，返回等价于
`axum::Router::<S>::new()` 的原生空 Router，并保留 `S` 表达的 missing-state 类型；不返回错误、
不 bind、不创建 runtime、不访问网络。路由注册、冲突、layer、state 和资源边界仍由调用方与
Axum 0.8 负责。返回类型来自调用方需要直接依赖的兼容 Axum 版本。
```rust
let router: axum::Router<String> = axutils::AxumApp::<String>::create_router();
let _builder = axutils::AxumApp::from_router(router).with_state("axutils".to_owned());
```

### `AxumApp::new`
签名：`pub fn new() -> AxumApp<()>`；要求 `axum + tokio` feature。无输入，返回空 Router
构成的 app，不返回错误，不 bind、不创建 runtime、不访问网络；路由数量和后续 layer 资源由调用方
控制。
```rust
let app = axutils::AxumApp::new(); let _ = app.into_server_builder();
```

### `AxumApp::default`
签名：`impl Default for AxumApp<()>`；要求 `axum + tokio` feature。输入为空，输出与 `new`
相同；不返回错误、不产生 I/O 或其他外部副作用。
```rust
let app: axutils::AxumApp = Default::default(); let _ = app.into_server_builder();
```

### `AxumApp::from_router`
签名：`pub fn from_router<S>(router: axum::Router<S>) -> AxumApp<S>`（`S` 需满足 Axum 的 clone、
并发和 `'static` 约束）；要求 `axum + tokio` feature。保留输入 Router 和 missing-state 类型，
不返回错误、不执行 handler、不 bind 或访问网络；路径冲突等 panic 语义沿用 Axum。
```rust
let app = axutils::AxumApp::from_router(axum::Router::new()); let _ = app.into_server_builder();
```

### `AxumApp::route`
签名：`pub fn route(self, path: &str, route: axum::routing::MethodRouter<S>) -> Self`；要求
`axum + tokio` feature。输入 path 与 MethodRouter，输出保留延迟 layer 栈的 app；不返回错误，
非法 path/冲突沿用 Axum panic；只修改内存路由，不执行 handler、不 bind、不访问网络。
```rust
use axum::routing::get; let app=axutils::AxumApp::new().route("/health",get(||async{"ok"})); let _=app.into_server_builder();
```

### `AxumApp::route_service`
签名：`pub fn route_service<T>(self, path: &str, service: T) -> Self`，`T` 必须是响应可转换且
错误为 `Infallible` 的可 clone/send Tower service；要求 `axum + tokio` feature，代码若直接引用
`tower::service_fn` 还需宿主直接依赖 Tower。输出为更新后的 app；不返回错误、不调用 service，
路径冲突仍可能按 Axum 约束 panic，注册阶段无 I/O。
```rust
use tower::service_fn; use axum::{body::Body,http::{Request,Response}}; use std::convert::Infallible;
let service=service_fn(|_:Request<Body>|async{Ok::<_,Infallible>(Response::new(Body::empty()))});let _=axutils::AxumApp::new().route_service("/svc",service);
```

### `AxumApp::nest`
签名：`pub fn nest(self, path: &str, router: axum::Router<S>) -> Self`；要求 `axum + tokio`
feature。输入同 state Router，输出更新后的 app；不返回错误、不执行 handler、不 bind 或访问网络，
非法前缀/冲突及根路径约束沿用 Axum panic。
```rust
let _=axutils::AxumApp::new().nest("/api",axum::Router::new());
```

### `AxumApp::merge`
签名：`pub fn merge<T>(self, router: T) -> Self`，其中 `T: Into<axum::Router<S>>`；要求
`axum + tokio` feature。输出合并后的 app，不返回错误、不访问网络；路由或 fallback 冲突沿用
Axum panic 语义，路由规模和内存占用由调用方负责。
```rust
let _=axutils::AxumApp::new().merge(axum::Router::new());
```

### `AxumApp::fallback`
签名：`pub fn fallback<H, T>(self, handler: H) -> Self`，其中 `H: Handler<T, S>`；要求
`axum + tokio` feature。输出设置 fallback 的 app，不返回错误、不执行 handler、不 bind；handler
错误/响应由 Axum 处理，仍受全 Router layer 覆盖，资源开销由调用方负责。
```rust
let _=axutils::AxumApp::new().fallback(||async{"missing"});
```

### `AxumApp::with_state`
签名：`pub fn with_state(self, state: S) -> AxumServerBuilder`；要求 `axum + tokio` feature。
输入 state 会按 Axum 语义注入并收敛为 `Router<()>`，输出 builder；本方法不直接返回错误、不 bind、
不创建 runtime 或访问网络，空 Router 的 matched-layer 错误延迟到 `build`。
```rust
#[derive(Clone)] struct State; let app=axutils::AxumApp::from_router(axum::Router::<State>::new());let _=app.with_state(State);
```

### `AxumApp::into_server_builder`
签名：`pub fn into_server_builder(self) -> AxumServerBuilder`（仅适用于 `AxumApp<()>`）；要求
`axum + tokio` feature。输出应用统一延迟 layer 栈的 builder，不直接返回错误、不 bind、不创建
runtime 或访问网络；空 Router 的 matched-layer 错误在后续 `build` 返回。
```rust
let builder=axutils::AxumApp::new().into_server_builder();let _=builder.build().unwrap();
```

### `AxumConfig::new`
签名：`pub fn new() -> AxumConfig`；要求 `axum + tokio` feature。返回默认超时、body 和并发边界，
不返回错误、不安装 middleware、不访问网络；各上限只在对应 provider 显式安装时生效。
```rust
assert_eq!(axutils::AxumConfig::new().max_concurrency(),1024);
```

### `AxumConfig::default`
签名：`impl Default for AxumConfig`；要求 `axum + tokio` feature。无输入，输出与 `new` 等价；
只构造内存配置，不返回错误、不产生阻塞或 I/O。
```rust
let _:axutils::AxumConfig=Default::default();
```

### `AxumConfig::with_service_timeout`
签名：`pub fn with_service_timeout(self, value: Duration) -> Result<Self, AxumError>`；要求
`axum + tokio` feature。输入必须为 1ms..=600s，输出更新配置；越界返回
`InvalidConfig { field: "service_timeout" }`，成功路径只修改内存，不安装 timeout layer 或访问网络。
```rust
assert!(axutils::AxumConfig::new().with_service_timeout(std::time::Duration::ZERO).is_err());
```

### `AxumConfig::service_timeout`
签名：`pub fn service_timeout(&self) -> Duration`；要求 `axum + tokio` feature。返回已保存的
Duration，不探测 provider layer、不返回错误、不阻塞或访问网络。
```rust
assert_eq!(axutils::AxumConfig::new().service_timeout(),std::time::Duration::from_secs(30));
```

### `AxumConfig::with_max_body_bytes`
签名：`pub fn with_max_body_bytes(self, value: usize) -> Result<Self, AxumError>`；要求
`axum + tokio` feature。输入范围为 1..=64 MiB，越界返回 `InvalidConfig { field: "max_body_bytes" }`；
成功只保存边界，不安装 body-limit layer、不访问网络。
```rust
assert!(axutils::AxumConfig::new().with_max_body_bytes(0).is_err());
```

### `AxumConfig::max_body_bytes`
签名：`pub fn max_body_bytes(&self) -> usize`；要求 `axum + tokio` feature。返回已保存上限，
不返回错误、不探测 layer、不阻塞或产生 I/O。
```rust
assert_eq!(axutils::AxumConfig::new().max_body_bytes(),1024*1024);
```

### `AxumConfig::with_max_concurrency`
签名：`pub fn with_max_concurrency(self, value: usize) -> Result<Self, AxumError>`；要求
`axum + tokio` feature。输入范围为 1..=65,536，越界返回 `InvalidConfig { field: "max_concurrency" }`；
成功只保存边界，不安装 Tower layer、不访问网络。
```rust
assert!(axutils::AxumConfig::new().with_max_concurrency(65_537).is_err());
```

### `AxumConfig::max_concurrency`
签名：`pub fn max_concurrency(&self) -> usize`；要求 `axum + tokio` feature。返回已保存并发上限，
不返回错误、不探测 layer、不阻塞或产生 I/O。
```rust
assert_eq!(axutils::AxumConfig::new().max_concurrency(),1024);
```

### `AxumServerBuilder::config`
签名：`pub fn config(self, config: AxumConfig) -> Self`；要求 `axum + tokio` feature。输入配置
替换 builder 的 immutable 边界，输出 builder；不返回错误、不自动安装 provider、不 bind、不创建
runtime 或访问网络，内存占用随配置和已登记 layer 由调用方控制。
```rust
let _=axutils::AxumApp::new().into_server_builder().config(axutils::AxumConfig::new());
```

### `AxumServerBuilder::build`
签名：`pub fn build(self) -> Result<AxumServer, AxumError>`；要求 `axum + tokio` feature。
收敛 provider layer 并创建共享单次状态机，但不 bind、不访问网络、不创建 runtime；空 matched
layer 或其他保存的配置错误返回 `InvalidConfig`，成功输出可 clone 的 server，服务状态资源在后续
`serve*` 期间使用。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=server.clone();
```

### `AxumServerBuilder::with_body_limit`
要求 axutils 的 `axum + tokio + tower-http` feature；范围 1..=64 MiB。宿主若只调用该方法无需
直接引用 tower-http 类型，但应保留与本 crate 兼容的 provider 依赖闭包。签名：
`pub fn with_body_limit(self, max_bytes: usize) -> Result<Self, AxumError>`；越界返回
`InvalidConfig { field: "max_body_bytes" }`，成功只登记内存 layer，不 bind 或访问网络；请求处理时
body 缓冲受该上限约束。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { assert!(axutils::AxumApp::new().into_server_builder().with_body_limit(0).is_err()); }
```

### `AxumServerBuilder::with_catch_panic`
要求 axutils 的 `axum + tokio + tower-http` feature；只捕获 unwind，返回脱敏 500。它不抑制
进程 panic hook；宿主负责配置脱敏 hook。签名：`pub fn with_catch_panic(self) -> Self`；无输入
错误，成功只登记 layer，不 bind 或访问网络；请求阶段不能捕获 abort，panic hook 仍可能先观察
panic payload。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_catch_panic(); }
```

### `AxumServerBuilder::with_governor_peer`
要求 axutils 的 `axum + tokio + tower_governor` feature；只使用真实 peer，周期 1ms..=1h，
burst 1..=65,536。宿主需直接依赖兼容的 `tower_governor` provider。签名：
`pub fn with_governor_peer(self, replenish_interval: Duration, burst: NonZeroU32) -> Result<Self, AxumError>`；
越界或 provider 构造失败返回 `InvalidConfig`，成功仅登记内存 limiter，不 bind；serve 时会按
peer 维护有界参数之外仍可能增长的 key 集合并周期清理，部署方须限制高基数来源。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower_governor"))] { let _=axutils::AxumApp::new().into_server_builder().with_governor_peer(std::time::Duration::from_secs(1),std::num::NonZeroU32::new(10).unwrap()).unwrap(); }
```

### `AxumServerBuilder::with_governor_forwarded_headers_unchecked`
要求 axutils 的 `axum + tokio + tower_governor` feature；无条件信任转发 Header，必须由可信
入口代理覆盖客户端值。宿主需直接依赖兼容的 `tower_governor` provider。签名：
`pub fn with_governor_forwarded_headers_unchecked(self, replenish_interval: Duration, burst: NonZeroU32) -> Result<Self, AxumError>`；
周期和 burst 越界返回 `InvalidConfig`，成功不 bind、不访问网络；请求阶段按未验证 header 维护
limiter 状态，必须由可信代理承担 header 覆盖和高基数资源边界。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower_governor"))] { let _=axutils::AxumApp::new().into_server_builder().with_governor_forwarded_headers_unchecked(std::time::Duration::from_secs(1),std::num::NonZeroU32::new(10).unwrap()).unwrap(); }
```

### `AxumServer::config`
签名：`pub fn config(&self) -> &AxumConfig`；要求 `axum + tokio` feature。返回构建时配置借用，
不返回错误、不探测运行状态、不阻塞或访问网络；引用受 server 生命周期约束。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=server.config();
```

### `AxumServer::shutdown_handle`
签名：`pub fn shutdown_handle(&self) -> AxumShutdownHandle`；要求 `axum + tokio` feature。返回
共享状态句柄，Ready 状态调用其 `shutdown` 返回 `NotRunning`；本方法只 clone 内存状态，不返回
错误、不 bind 或访问网络。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();assert!(server.shutdown_handle().shutdown(axutils::AxumShutdownReason::Programmatic).is_err());
```

### `AxumServer::serve_addr`
签名：`pub async fn serve_addr(&self, addr: SocketAddr) -> Result<AxumServeOutcome, AxumError>`；
要求 `axum + tokio` feature、调用方 Tokio runtime。输入地址会触发 TCP bind 和 HTTP/1 服务，输出
实际地址/首个关闭原因；bind 或 signal 失败返回脱敏错误并回滚 Ready，运行期间只允许单次 active
serve，drain 可能等待卡住请求，不提供强制 deadline。
```rust,no_run
# async fn demo(server:axutils::AxumServer)->Result<(),axutils::AxumError>{let _=server.serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
```

### `AxumServer::serve`
签名：`pub async fn serve(&self, listener: tokio::net::TcpListener) -> Result<AxumServeOutcome, AxumError>`；
要求 `axum + tokio` feature、调用方 Tokio runtime。输入已 bind listener，输出实际地址/关闭原因；
不会再次 bind，但会产生网络 I/O，重复运行或终态返回状态错误，取消/异常离开会将 server 标记为
Abandoned，graceful drain 没有强制时间上限。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=server.serve(listener).await?;Ok(())}
```

### `AxumServer::serve_with_shutdown`
签名：`pub async fn serve_with_shutdown<F>(&self, listener: tokio::net::TcpListener, shutdown: F) -> Result<AxumServeOutcome, AxumError>`，
其中 `F: Future<Output = AxumShutdownReason> + Send + 'static`；要求 `axum + tokio` feature 和
Tokio runtime。输入 listener 与宿主关闭 future，输出实际地址/首个原因；监听期间产生网络 I/O，
重复运行/终态/后台清理失败返回稳定 `AxumError`，取消可能进入 Abandoned，drain 不设硬 deadline。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;Ok(())}
```

### `AxumServeOutcome::local_addr`
签名：`pub fn local_addr(&self) -> SocketAddr`；要求 `axum + tokio` feature。返回 serve 完成时的
实际监听地址（含端口 0 分配后的端口），不访问网络、不返回错误、不阻塞。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let out=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;let _=out.local_addr();Ok(())}
```

### `AxumServeOutcome::reason`
签名：`pub fn reason(&self) -> &AxumShutdownReason`；要求 `axum + tokio` feature。返回首个关闭
原因的借用，生命周期受 outcome 约束；不访问网络、不返回错误、不产生副作用。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let out=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;let _=out.reason();Ok(())}
```

### `AxumUtils::create_router`
签名：`pub fn create_router<S>() -> axum::Router<S>`，其中
`S: Clone + Send + Sync + 'static`；要求 `axum + tokio` feature。无输入，复用
`AxumApp::<S>::create_router` 返回保留 missing-state 类型的原生空 Router；不读取或修改全局
server，不返回错误、不 bind、不创建 runtime、不访问网络。调用方负责后续路由、layer、state 和
资源边界，并直接依赖兼容的 Axum。
```rust
let router: axum::Router<String> = axutils::AxumUtils::create_router();
let _builder = axutils::AxumApp::from_router(router).with_state("axutils".to_owned());
```

### `AxumUtils::create_app`
签名：`pub fn create_app() -> AxumApp<()>`；要求 `axum + tokio` feature。无输入，复用
`AxumApp::new` 返回独立的空应用构建器；不读取或修改全局 server，不返回错误、不 bind、不创建
runtime、不访问网络。返回值可继续注册路由并转换为 `AxumServerBuilder`。
```rust
let app = axutils::AxumUtils::create_app();
let _builder = app.into_server_builder();
```

### `AxumUtils::init`
签名：`pub fn init(server: AxumServer) -> Result<(), AxumError>`；要求 `axum + tokio` feature。
输入 server 写入进程级 OnceLock，首次成功返回 `Ok(())`，重复初始化返回
`AlreadyInitialized`；不 bind、不启动 runtime、不访问网络，OnceLock 不可 reset/replace，server
生命周期和资源由该全局入口持有。
```rust,no_run
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=axutils::AxumUtils::init(server);
```

### `AxumUtils::is_initialized`
签名：`pub fn is_initialized() -> bool`；要求 `axum + tokio` feature。同步读取 OnceLock，服务
停止或 abandoned 后仍返回 `true`；不返回错误、不访问网络、不探测 listener 健康，也不产生 I/O。
```rust
let _=axutils::AxumUtils::is_initialized();
```

### `AxumUtils::shutdown_handle`
签名：`pub fn shutdown_handle() -> Result<AxumShutdownHandle, AxumError>`；要求 `axum + tokio`
feature。成功返回全局 server 的共享句柄，未初始化返回 `NotInitialized`；只读取 OnceLock，不
bind、不阻塞或访问网络。
```rust,no_run
let _=axutils::AxumUtils::shutdown_handle();
```

### `AxumUtils::shutdown`
签名：`pub fn shutdown(reason: AxumShutdownReason) -> Result<AxumShutdownReason, AxumError>`；
要求 `axum + tokio` feature。输入关闭原因转发给全局句柄并返回首原因；未初始化返回
`NotInitialized`，状态不匹配保留 `NotRunning`/终态错误；只改变内存状态并唤醒服务，不自行 bind
或创建 runtime。
```rust,no_run
let _=axutils::AxumUtils::shutdown(axutils::AxumShutdownReason::Programmatic);
```

### `AxumUtils::serve_addr`
签名：`pub async fn serve_addr(addr: SocketAddr) -> Result<AxumServeOutcome, AxumError>`；要求
`axum + tokio` feature 和调用方 Tokio runtime。转发全局 server 的 bind/serve，未初始化返回
`NotInitialized`，其余 bind、signal、单次状态和 drain 语义与 `AxumServer::serve_addr` 相同；会产生
网络 I/O，不创建 runtime。
```rust,no_run
# async fn demo()->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
```

### `AxumUtils::serve`
签名：`pub async fn serve(listener: tokio::net::TcpListener) -> Result<AxumServeOutcome, AxumError>`；
要求 `axum + tokio` feature 和 Tokio runtime。输入已 bind listener，未初始化返回
`NotInitialized`，其余状态、取消和 graceful drain 语义转发给全局 server；会产生网络 I/O，不重复
bind 或隐式创建 runtime。
```rust,no_run
# async fn demo(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve(listener).await?;Ok(())}
```

### `AxumUtils::serve_with_shutdown`
签名：`pub async fn serve_with_shutdown<F>(listener: tokio::net::TcpListener, shutdown: F) -> Result<AxumServeOutcome, AxumError>`，
其中 `F: Future<Output = AxumShutdownReason> + Send + 'static`；要求 `axum + tokio` feature 和
Tokio runtime。未初始化返回 `NotInitialized`，其余监听、关闭、后台任务和终态语义与实例方法
一致；会产生网络 I/O，取消可能使全局 server 进入 `Abandoned`。
```rust,no_run
# async fn demo(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;Ok(())}
```


### `AxumApp::with_layer`
要求 axutils 的 `axum + tokio` feature；签名 `pub fn with_layer<L>(self, layer: L) -> AxumApp<...>`，
宿主若直接使用 `tower::Layer` 类型需直接依赖兼容 Tower，但不需 axutils 的 `tower` provider feature。
输入 layer 只登记到内存闭包，输出保留该栈的 app；不返回错误、不执行 layer、不 bind 或访问网络，
覆盖 route、405 与 fallback，声明顺序即请求顺序，layer 资源开销由调用方负责。
```rust
let app=axutils::AxumApp::new().with_layer(tower::layer::util::Identity::new());let _=app.into_server_builder();
```

### `AxumApp::with_matched_route_layer`
要求 axutils 的 `axum + tokio` feature；签名 `pub fn with_matched_route_layer<L>(self, layer: L) -> AxumApp<...>`，
宿主若直接使用 `tower::Layer` 类型需直接依赖兼容 Tower，但不需 axutils 的 `tower` provider feature。
输入 layer 只登记到内存闭包，输出更新后的 app；不返回即时错误、不执行 layer、不 bind 或访问网络，
只覆盖匹配 route，不覆盖 fallback；空 Router 在后续 `build` 返回
`InvalidConfig { field: "matched_route_layer" }`，资源开销由调用方负责。
```rust
let result=axutils::AxumApp::new().with_matched_route_layer(tower::layer::util::Identity::new()).into_server_builder().build();assert!(result.is_err());
```

### `AxumServerBuilder::with_concurrency_limit`
要求 axutils 的 `axum + tokio + tower` feature；范围 1..=65,536，满载立即返回脱敏 503，不
等待 permit。宿主需直接依赖兼容 Tower。签名：`pub fn with_concurrency_limit(self, max: usize) -> Result<Self, AxumError>`；
越界返回 `InvalidConfig { field: "max_concurrency" }`，成功只登记内存 layer、不 bind 或访问网络；
请求阶段的并发数受该上限约束，等待/队列资源由 provider 语义决定。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower"))] { assert!(axutils::AxumApp::new().into_server_builder().with_concurrency_limit(0).is_err()); }
```

### `AxumServerBuilder::with_request_id`
要求 axutils 的 `axum + tokio + tower-http` feature；移除入站 spoofed ID，生成内部 UUID，并
最终覆盖 handler 冲突值；重复调用幂等。签名：`pub fn with_request_id(self) -> Self`；无输入
错误，只登记内存 layer、不 bind 或访问网络；请求处理会写入受控 `x-request-id` header 和内部
extension，响应仍受调用方路由资源约束。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_request_id().with_request_id(); }
```

### `AxumServerBuilder::with_timeout`
要求 axutils 的 `axum + tokio + tower-http` feature；service future 预算为 1ms..=600s，可选择
408 或 504；不是连接或 drain timeout。签名：`pub fn with_timeout(self, duration: Duration, status: AxumTimeoutStatus) -> Result<Self, AxumError>`；
越界返回 `InvalidConfig { field: "service_timeout" }`，成功只登记 layer、不 bind 或访问网络；
请求 service future 超时返回指定状态，不能终止连接读写或强制 drain。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_timeout(std::time::Duration::from_secs(1),axutils::AxumTimeoutStatus::GatewayTimeout).unwrap(); }
```

### `AxumServerBuilder::with_cors`
要求 axutils 的 `axum + tokio + tower-http` feature；安装全 Router CORS。列表最大 64 项，
wildcard 列表值及 wildcard + credentials 不进入上游 panic 路径而返回 `InvalidConfig`。签名：
`pub fn with_cors(self, config: AxumCorsConfig) -> Result<Self, AxumError>`；输入配置只在安装时
校验列表/`max_age`/credentials 组合，成功登记全 Router layer，不 bind 或访问网络；请求阶段
会产生 CORS 响应 header，`Disabled`/空列表不安装 layer。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_cors(axutils::AxumCorsConfig::default()).unwrap(); }
```

### `AxumServerBuilder::with_http_trace`
要求 axutils 的 `axum + tokio + tower-http + tracing` feature；自动安装内部 request ID，只记录
method、matched route 模板、status、latency 与该 ID；不安装 subscriber。宿主需直接依赖
`tracing` 并自行初始化 subscriber。签名：`pub fn with_http_trace(self) -> Self`；无输入错误，
只登记内存 middleware、不 bind 或访问网络；请求阶段会产生 tracing 事件并维护 request ID，事件
不包含原始 URI/query/header/body，调用方负责 subscriber 与日志资源边界。
```rust
# #[cfg(all(feature="axum",feature="tokio",feature="tower-http",feature="tracing"))] { let _=axutils::AxumApp::new().into_server_builder().with_http_trace(); }
```
