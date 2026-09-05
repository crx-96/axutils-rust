# Axum

基础 server 使用 `axum`。附加 middleware 能力按需单独开启：`axum-tower` 提供 Tower
limit/load-shed，`axum-tower-http` 提供 CORS、request-id、timeout、body-limit 与 panic；
HTTP trace 还需组合 `tracing`。`axum-governor` 提供 Governor 限流。这些扩展均包含基础 `axum`，
但彼此不互相启用。

```toml
[dependencies]
axutils = { version = "1.0", features = ["axum", "axum-tower-http"] }
```

路由与 layer 的公共签名保留 Axum/Tower 原生类型。应用若要命名 `Router`、注册 route 或直接组合
layer，应直接依赖兼容的 Axum 0.8.x；直接使用 Tower 类型时还应依赖兼容的 Tower 0.5.x，不能依赖
传递依赖恰好可见。执行 `serve*` 还要求调用方提供 Tokio runtime；应用应直接依赖兼容的 Tokio
1.x 并启用所需 runtime、network 与 signal 能力。

## 创建应用和 server

`AxumApp` 是路由和 layer 的构建入口，`AxumConfig` 是 server 配置。构建 server 不绑定端口；
实际运行会监听网络并等待 shutdown，因此示例不执行。

```rust,no_run
use axutils::axum::{AxumApp, AxumConfig, AxumError, AxumServer};

fn build_server() -> Result<AxumServer, AxumError> {
    AxumApp::new()
        .into_server_builder()
        .config(AxumConfig::new())
        .build()
}

async fn run() -> Result<(), AxumError> {
    let address = "127.0.0.1:3000".parse().expect("static socket address");
    let _outcome = build_server()?.serve_addr(address).await?;
    Ok(())
}
```

`AxumConfig` 当前只校验并记录 `service_timeout`、`max_body_bytes` 和 `max_concurrency` 声明值，
`build()` 不会自动把它们安装成 middleware。要真正执行这些预算，调用方必须分别通过
`axum-tower-http` 的 `with_timeout`/`with_body_limit`、`axum-tower` 的
`with_concurrency_limit`，或应用自有 layer 显式安装，并自行保证 layer 参数与配置声明一致。
读取 `server.config()` 不能证明这些限制已经生效。

有状态路由可先通过 `AxumApp::<State>::create_router()` 创建原生 router，再使用
`AxumApp::from_router(router)` 继续构建。应用负责路由 handler 的认证、输入限制与业务错误映射；
启用的 middleware 不是这些安全策略的替代品。

## 进程级入口

`AxumUtils` 只管理已经构建的 server 的一次初始化、状态和实例访问，不提供路由或应用构建转发。
成功初始化后不可 reset 或 replace。`AxumServer` 是可 clone 但共享状态的单次运行状态机：同一实例
并发 `serve*` 返回 `AlreadyRunning`，进入 stopped 或 abandoned 后不能再次启动；每个监听器或独立
生命周期都必须单独构建一个 server。

```rust,no_run
use axutils::{
    axum::{AxumApp, AxumConfig, AxumError},
    utils::AxumUtils,
};

fn initialize() -> Result<(), AxumError> {
    let server = AxumApp::new()
        .into_server_builder()
        .config(AxumConfig::new())
        .build()?;
    AxumUtils::init(server)?;
    let _server = AxumUtils::server()?;
    Ok(())
}
```

调用 `server()` 不会启动监听；启动、shutdown、端口与信号策略仍由应用显式控制。

基础 server 只支持 HTTP/1，不内建 TLS、HTTP/2 或强制 drain deadline。Governor 的 forwarded-header
模式不会验证可信代理 CIDR，只有受信入口代理会覆盖/清理客户端转发 Header 时才可使用；其他部署
应使用真实 peer IP 模式或在应用边界完成可信代理校验。
