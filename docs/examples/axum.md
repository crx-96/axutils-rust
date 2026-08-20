# Axum 服务工具使用文档

## Feature、导出与非目标

服务 API 仅在 `axum + tokio` 下从 `axutils::*` 与 `axutils::axum::*` 导出；`AxumUtils` 还支持 `axutils::utils::*` 和 `axutils::utils::axum_utils::*`。Tower、Tower HTTP、tower_governor wrapper 分别要求同名 provider feature。签名中的 Axum Router/MethodRouter、Tower Layer、Tokio TcpListener 需调用方直接依赖兼容版本。`tower_governor` 0.8 的 `axum` feature 会启用 Axum default，带入 form/json/query/original-uri/tower-log/tracing 并间接启用 Tokio macros；provider-only 不会拉入 Axum。这是上游固定 feature 扩张，不表示 axutils 自动安装相关行为。

首版只提供 HTTP/1 与协作式 graceful drain，不提供 TLS、HTTP/2、header/read timeout、强制 drain deadline、trusted-proxy CIDR 验证、鉴权或业务错误格式。

```rust,no_run
use axum::routing::get;
use axutils::AxumApp;
let server = AxumApp::new().route("/health", get(|| async { "ok" }))
    .into_server_builder().build()?;
# Ok::<(), axutils::AxumError>(())
```

## AxumApp

### new / Default / from_router
创建空 Router 或包装调用方 Router；只组装内存状态，不 bind。

### route / route_service / nest / merge / fallback
保持 Axum 0.8 的注册、嵌套、合并和 fallback 语义。单 route middleware 应先在 MethodRouter/子 Router 上使用原生 layer。

### with_layer
延迟登记全 Router layer，最终覆盖 routes、405 与 fallback。声明 a、b 时请求顺序为 a→b→handler，响应反向。

### with_matched_route_layer
延迟登记仅匹配 routes 的 layer，不覆盖 404 fallback。空 Router 使用此栈时 `build` 返回 `InvalidConfig`，不会触发上游 panic。

### with_state / into_server_builder
`with_state` 把 `Router<S>` 收敛为 `Router<()>`；无 state 应用使用 `into_server_builder`。两者统一应用延迟栈。

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
仅 Running 可首次触发，Draining 重复调用幂等返回首原因；其他状态返回明确错误。

### AxumUtils::init / is_initialized
全局 OnceLock 只成功初始化一次，不提供 reset/replace；停止后仍保持 initialized。

### AxumUtils::shutdown_handle / shutdown
转发到全局 server；未初始化返回 `NotInitialized`。

### AxumUtils::serve_addr / serve / serve_with_shutdown
转发全局单次服务入口，runtime、listener、shutdown 和错误语义与实例方法一致。

## 结果与错误

`AxumServeOutcome::local_addr`/`reason` 读取实际地址和原因。`AxumShutdownReason` 与 `AxumError` 均为可扩展枚举；错误 Display/Debug 不包含请求、Header、body 或 provider 原始消息；governor cleanup task 异常退出时 serve 返回脱敏 `AxumError::BackgroundTask`，不会静默吞掉 JoinError。

## 逐项 API 签名与示例

以下小节补齐上文概览中合并展示的方法；均要求 `axum + tokio`，额外 feature 单独标明。

### `AxumApp::new`
签名 `pub fn new() -> AxumApp<()>`；创建空 Router，不访问网络。
```rust
let app = axutils::AxumApp::new(); let _ = app.into_server_builder();
```

### `AxumApp::default`
`Default` 与 `new` 等价。
```rust
let app: axutils::AxumApp = Default::default(); let _ = app.into_server_builder();
```

### `AxumApp::from_router`
签名 `pub fn from_router(router: axum::Router<S>) -> Self`；保留 missing-state 类型。
```rust
let app = axutils::AxumApp::from_router(axum::Router::new()); let _ = app.into_server_builder();
```

### `AxumApp::route`
接受 path 与 `MethodRouter<S>`，返回更新后的 builder；非法 path 沿用 Axum panic 契约。
```rust
use axum::routing::get; let app=axutils::AxumApp::new().route("/health",get(||async{"ok"})); let _=app.into_server_builder();
```

### `AxumApp::route_service`
接受 `Service<Request, Error=Infallible>`；Router 应改用 `nest`。
```rust
use tower::service_fn; use axum::{body::Body,http::{Request,Response}}; use std::convert::Infallible;
let service=service_fn(|_:Request<Body>|async{Ok::<_,Infallible>(Response::new(Body::empty()))});let _=axutils::AxumApp::new().route_service("/svc",service);
```

### `AxumApp::nest`
嵌套同 state Router；根路径嵌套的 panic 语义沿用 Axum。
```rust
let _=axutils::AxumApp::new().nest("/api",axum::Router::new());
```

### `AxumApp::merge`
合并同 state Router；冲突 fallback 的 panic 语义沿用 Axum。
```rust
let _=axutils::AxumApp::new().merge(axum::Router::new());
```

### `AxumApp::fallback`
设置 fallback handler，仍受全 Router layer 覆盖。
```rust
let _=axutils::AxumApp::new().fallback(||async{"missing"});
```

### `AxumApp::with_state`
签名 `with_state(self, state: S) -> AxumServerBuilder`，把 `Router<S>` 收敛为 `Router<()>`。
```rust
#[derive(Clone)] struct State; let app=axutils::AxumApp::from_router(axum::Router::<State>::new());let _=app.with_state(State);
```

### `AxumApp::into_server_builder`
仅适用于无 missing state 的 app；统一应用延迟 layer 栈。
```rust
let builder=axutils::AxumApp::new().into_server_builder();let _=builder.build().unwrap();
```

### `AxumConfig::new`
返回有限默认边界，不安装 middleware。
```rust
assert_eq!(axutils::AxumConfig::new().max_concurrency(),1024);
```

### `AxumConfig::default`
与 `new` 等价。
```rust
let _:axutils::AxumConfig=Default::default();
```

### `AxumConfig::with_service_timeout`
范围 1ms..=600s；越界返回 `InvalidConfig`。
```rust
assert!(axutils::AxumConfig::new().with_service_timeout(std::time::Duration::ZERO).is_err());
```

### `AxumConfig::service_timeout`
返回配置值，不探测已安装 layer。
```rust
assert_eq!(axutils::AxumConfig::new().service_timeout(),std::time::Duration::from_secs(30));
```

### `AxumConfig::with_max_body_bytes`
范围 1..=64 MiB；仅保存边界。
```rust
assert!(axutils::AxumConfig::new().with_max_body_bytes(0).is_err());
```

### `AxumConfig::max_body_bytes`
返回配置值。
```rust
assert_eq!(axutils::AxumConfig::new().max_body_bytes(),1024*1024);
```

### `AxumConfig::with_max_concurrency`
范围 1..=65,536。
```rust
assert!(axutils::AxumConfig::new().with_max_concurrency(65_537).is_err());
```

### `AxumConfig::max_concurrency`
返回配置值。
```rust
assert_eq!(axutils::AxumConfig::new().max_concurrency(),1024);
```

### `AxumServerBuilder::config`
替换 immutable 配置，不自动安装 provider。
```rust
let _=axutils::AxumApp::new().into_server_builder().config(axutils::AxumConfig::new());
```

### `AxumServerBuilder::build`
构造共享单次状态机，不 bind；配置错误以 `AxumError` 返回。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=server.clone();
```

### `AxumServerBuilder::with_body_limit`
要求 `tower-http`；范围 1..=64 MiB。
```rust
# #[cfg(feature="tower-http")] { assert!(axutils::AxumApp::new().into_server_builder().with_body_limit(0).is_err()); }
```

### `AxumServerBuilder::with_catch_panic`
要求 `tower-http`；只捕获 unwind，返回脱敏 500。它不抑制进程 panic hook；宿主负责配置脱敏 hook。
```rust
# #[cfg(feature="tower-http")] { let _=axutils::AxumApp::new().into_server_builder().with_catch_panic(); }
```

### `AxumServerBuilder::with_governor_peer`
要求 `tower_governor`；只使用真实 peer，周期 1ms..=1h，burst 1..=65,536。
```rust
# #[cfg(feature="tower_governor")] { let _=axutils::AxumApp::new().into_server_builder().with_governor_peer(std::time::Duration::from_secs(1),std::num::NonZeroU32::new(10).unwrap()).unwrap(); }
```

### `AxumServerBuilder::with_governor_forwarded_headers_unchecked`
要求 `tower_governor`；无条件信任转发 Header，必须由可信入口代理覆盖客户端值。
```rust
# #[cfg(feature="tower_governor")] { let _=axutils::AxumApp::new().into_server_builder().with_governor_forwarded_headers_unchecked(std::time::Duration::from_secs(1),std::num::NonZeroU32::new(10).unwrap()).unwrap(); }
```

### `AxumServer::config`
返回构建时配置的借用。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=server.config();
```

### `AxumServer::shutdown_handle`
返回共享句柄；Ready 状态 shutdown 返回 `NotRunning`。
```rust
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();assert!(server.shutdown_handle().shutdown(axutils::AxumShutdownReason::Programmatic).is_err());
```

### `AxumServer::serve_addr`
执行 bind 并默认监听 OS/程序化关闭；bind 失败回滚 Ready。
```rust,no_run
# async fn demo(server:axutils::AxumServer)->Result<(),axutils::AxumError>{let _=server.serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
```

### `AxumServer::serve`
接受已 bind `tokio::net::TcpListener`，默认监听 OS/程序化关闭。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=server.serve(listener).await?;Ok(())}
```

### `AxumServer::serve_with_shutdown`
接受 `Future<Output=AxumShutdownReason> + Send + 'static`；用于宿主协调和测试。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;Ok(())}
```

### `AxumServeOutcome::local_addr`
返回端口 0 bind 后的实际地址。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let out=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;let _=out.local_addr();Ok(())}
```

### `AxumServeOutcome::reason`
返回首个关闭原因的借用。
```rust,no_run
# async fn demo(server:axutils::AxumServer,listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let out=server.serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;let _=out.reason();Ok(())}
```

### `AxumUtils::init`
全局 OnceLock 只成功一次，不提供 reset/replace。
```rust,no_run
let server=axutils::AxumApp::new().into_server_builder().build().unwrap();let _=axutils::AxumUtils::init(server);
```

### `AxumUtils::is_initialized`
读取 OnceLock 状态；服务停止后仍为 true。
```rust
let _=axutils::AxumUtils::is_initialized();
```

### `AxumUtils::shutdown_handle`
未初始化返回 `NotInitialized`。
```rust,no_run
let _=axutils::AxumUtils::shutdown_handle();
```

### `AxumUtils::shutdown`
转发全局共享句柄并保留首原因。
```rust,no_run
let _=axutils::AxumUtils::shutdown(axutils::AxumShutdownReason::Programmatic);
```

### `AxumUtils::serve_addr`
转发全局地址入口。
```rust,no_run
# async fn demo()->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_addr("127.0.0.1:0".parse().unwrap()).await?;Ok(())}
```

### `AxumUtils::serve`
转发全局 listener 入口。
```rust,no_run
# async fn demo(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve(listener).await?;Ok(())}
```

### `AxumUtils::serve_with_shutdown`
转发全局自定义关闭入口。
```rust,no_run
# async fn demo(listener:tokio::net::TcpListener)->Result<(),axutils::AxumError>{let _=axutils::AxumUtils::serve_with_shutdown(listener,async{axutils::AxumShutdownReason::Programmatic}).await?;Ok(())}
```


### \`AxumApp::with_layer\`
签名 \`pub fn with_layer<L>(self, layer: L) -> AxumApp<...>\`；延迟登记全 Router layer，覆盖 route、405 与 fallback，声明顺序即请求顺序。
```rust
let app=axutils::AxumApp::new().with_layer(tower::layer::util::Identity::new());let _=app.into_server_builder();
```

### \`AxumApp::with_matched_route_layer\`
签名 \`pub fn with_matched_route_layer<L>(self, layer: L) -> AxumApp<...>\`；只覆盖匹配 route，空 Router 在 build 返回 \`InvalidConfig\`。
```rust
let result=axutils::AxumApp::new().with_matched_route_layer(tower::layer::util::Identity::new()).into_server_builder().build();assert!(result.is_err());
```

### \`AxumServerBuilder::with_concurrency_limit\`
要求 \`tower\`；范围 1..=65,536，满载立即返回脱敏 503，不等待 permit。
```rust
# #[cfg(feature="tower")] { assert!(axutils::AxumApp::new().into_server_builder().with_concurrency_limit(0).is_err()); }
```

### \`AxumServerBuilder::with_request_id\`
要求 \`tower-http\`；移除入站 spoofed ID，生成内部 UUID，并最终覆盖 handler 冲突值；重复调用幂等。
```rust
# #[cfg(feature="tower-http")] { let _=axutils::AxumApp::new().into_server_builder().with_request_id().with_request_id(); }
```

### \`AxumServerBuilder::with_timeout\`
要求 \`tower-http\`；service future 预算为 1ms..=600s，可选择 408 或 504；不是连接或 drain timeout。
```rust
# #[cfg(feature="tower-http")] { let _=axutils::AxumApp::new().into_server_builder().with_timeout(std::time::Duration::from_secs(1),axutils::AxumTimeoutStatus::GatewayTimeout).unwrap(); }
```

### \`AxumServerBuilder::with_cors\`
要求 \`tower-http\`；安装全 Router CORS。列表最大 64 项，wildcard 列表值及 wildcard + credentials 不进入上游 panic 路径而返回 \`InvalidConfig\`。
```rust
# #[cfg(feature="tower-http")] { let _=axutils::AxumApp::new().into_server_builder().with_cors(axutils::AxumCorsConfig::default()).unwrap(); }
```

### \`AxumServerBuilder::with_http_trace\`
要求 \`tower-http + tracing\`；自动安装内部 request ID，只记录 method、matched route 模板、status、latency 与该 ID；不安装 subscriber。
```rust
# #[cfg(all(feature="tower-http",feature="tracing"))] { let _=axutils::AxumApp::new().into_server_builder().with_http_trace(); }
```
