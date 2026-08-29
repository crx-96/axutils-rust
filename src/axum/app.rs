use std::convert::Infallible;

use axum::{
    extract::Request,
    handler::Handler,
    response::IntoResponse,
    routing::{MethodRouter, Route},
    Router,
};
use tower::{Layer, Service};

use super::{AxumConfig, AxumError, AxumServerBuilder};

type IdentityLayerStack = fn(Router) -> Router;
fn identity(router: Router) -> Router {
    router
}

/// 保留 Axum missing-state 类型并延迟组装 layer 栈的应用构建器。
///
/// `S` 是尚未注入的 Axum state；`G` 与 `R` 分别记录全局和仅匹配路由的延迟 layer 栈，
/// 调用方通常无需显式写出后两项。类型仅在同时启用 crate 的 `axum` 与 `tokio` feature 时公开，
/// 推荐通过 `axutils::AxumApp` 使用，也可通过 `axutils::axum::AxumApp` 使用。
///
/// 路由注册沿用 Axum 0.8 的路径、冲突和 panic 约束。本类型不限制路由数量，也不提供 TLS、
/// HTTP/2、鉴权或业务错误格式。组装过程只修改内存中的路由和 layer 闭包，不会 bind、启动
/// runtime 或访问网络；真正的网络副作用发生在后续 server 的 `serve*` 方法。
///
/// # Examples
///
/// ```rust,no_run
/// let app = axutils::AxumApp::new().route(
///     "/health",
///     axum::routing::get(|| async { "ok" }),
/// );
/// let _builder = app.into_server_builder();
/// ```
pub struct AxumApp<S = (), G = IdentityLayerStack, R = IdentityLayerStack> {
    router: Router<S>,
    global: G,
    matched: R,
    has_matched_layer: bool,
}

impl<S> AxumApp<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// 创建一个不含路由、fallback 或已注入 state 的原生 `axum::Router<S>`。
    ///
    /// 返回值等价于 `axum::Router::<S>::new()`，并保留由 `S` 表达的 missing-state 类型，便于
    /// 调用方从 axutils 的 Axum 工厂入口开始组装原生 Router。该方法不会失败、bind、创建
    /// runtime 或访问网络；路由数量、注册冲突和后续 layer 资源仍由调用方与 Axum 0.8 负责。
    /// 仅在 crate 同时启用 `axum` 与 `tokio` feature 时可用；调用方若在签名中使用返回类型，
    /// 仍需直接依赖兼容版本的 Axum。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let router: axum::Router<String> = axutils::AxumApp::<String>::create_router();
    /// let _builder = axutils::AxumApp::from_router(router).with_state("axutils".to_owned());
    /// ```
    pub fn create_router() -> Router<S> {
        Router::new()
    }

    /// 从原生 `axum::Router<S>` 创建应用构建器。
    ///
    /// 输入 router 的路由、fallback 和 missing state 类型会原样保留；返回值使用空的延迟 layer
    /// 栈。该方法不返回错误，已有 router 的路径与冲突约束沿用 Axum 0.8；它不执行 handler、
    /// 不 bind 且不访问网络。仅要求 crate 的 `axum + tokio` feature，无需额外 provider feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let router = axum::Router::<()>::new().route(
    ///     "/health",
    ///     axum::routing::get(|| async { "ok" }),
    /// );
    /// let _app = axutils::AxumApp::from_router(router);
    /// ```
    pub fn from_router(router: Router<S>) -> Self {
        Self {
            router,
            global: identity,
            matched: identity,
            has_matched_layer: false,
        }
    }
}

impl<S, G, R> AxumApp<S, G, R>
where
    S: Clone + Send + Sync + 'static,
{
    /// 注册一个路径及其 `axum::routing::MethodRouter<S>`。
    ///
    /// `path` 与 `route` 直接交给 Axum 0.8，返回保留既有延迟 layer 栈的应用构建器。该方法不以
    /// `Result` 返回错误；非法路径或路由冲突遵循上游的 panic 约束。它不执行 handler 或访问
    /// 网络，也不限制路径长度和路由数量。仅要求 crate 的 `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let _app = axutils::AxumApp::new().route(
    ///     "/health",
    ///     axum::routing::get(|| async { "ok" }),
    /// );
    /// ```
    pub fn route(self, path: &str, route: MethodRouter<S>) -> Self {
        Self {
            router: self.router.route(path, route),
            ..self
        }
    }
    /// 在指定路径注册一个 Tower service。
    ///
    /// `path` 和满足签名约束的 `service` 直接交给 Axum 0.8；返回值保留既有延迟 layer 栈。
    /// service 的错误必须是 `Infallible`，响应必须实现 `IntoResponse`。该方法不以 `Result` 返回
    /// 错误；非法路径或路由冲突遵循上游的 panic 约束。注册时不调用 service、不 bind 且不访问
    /// 网络。API 本身只要求 crate 的 `axum + tokio` feature；签名中的 Tower trait 由 `axum`
    /// feature 的内部依赖提供，不要求另开本 crate 的 `tower` provider feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let service = tower::service_fn(|_: axum::extract::Request| async {
    ///     Ok::<_, std::convert::Infallible>("ok")
    /// });
    /// let _app = axutils::AxumApp::new().route_service("/health", service);
    /// ```
    pub fn route_service<T>(self, path: &str, service: T) -> Self
    where
        T: Service<Request, Error = Infallible> + Clone + Send + Sync + 'static,
        T::Response: IntoResponse,
        T::Future: Send + 'static,
    {
        Self {
            router: self.router.route_service(path, service),
            ..self
        }
    }
    /// 把一个具有相同 missing state 的 `axum::Router<S>` 嵌套到路径前缀下。
    ///
    /// `path` 与 `router` 直接交给 Axum 0.8；返回值保留既有延迟 layer 栈。该方法不以 `Result`
    /// 返回错误，非法前缀或路由冲突遵循上游的 panic 约束。嵌套不会执行 handler、bind 或访问
    /// 网络，且本 crate 不额外限制嵌套深度和路由数量。仅要求 crate 的 `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let api = axum::Router::new().route(
    ///     "/users",
    ///     axum::routing::get(|| async { "users" }),
    /// );
    /// let _app = axutils::AxumApp::new().nest("/api", api);
    /// ```
    pub fn nest(self, path: &str, router: Router<S>) -> Self {
        Self {
            router: self.router.nest(path, router),
            ..self
        }
    }
    /// 合并另一个可转换为 `axum::Router<S>` 的路由器。
    ///
    /// 输入直接交给 Axum 0.8，返回值保留既有延迟 layer 栈。该方法不以 `Result` 返回错误；重叠
    /// 路由或不兼容 fallback 等结构冲突遵循上游的 panic 约束。合并只修改内存中的路由结构，
    /// 不执行 handler、不 bind 且不访问网络；路由规模不受本 crate 额外限制。仅要求 crate 的
    /// `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let extra = axum::Router::new().route(
    ///     "/ready",
    ///     axum::routing::get(|| async { "ready" }),
    /// );
    /// let _app = axutils::AxumApp::new().merge(extra);
    /// ```
    pub fn merge<T>(self, router: T) -> Self
    where
        T: Into<Router<S>>,
    {
        Self {
            router: self.router.merge(router),
            ..self
        }
    }
    /// 设置没有匹配路由时使用的 fallback handler。
    ///
    /// `handler` 必须满足 Axum 的 `Handler<T, S>` 约束，返回值保留既有延迟 layer 栈。该方法不
    /// 返回错误，也不执行 handler；handler 的提取、响应和业务错误语义仍由调用方与 Axum 负责。
    /// 它不 bind 或访问网络，且只要求 crate 的 `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let _app = axutils::AxumApp::new()
    ///     .fallback(|| async { (axum::http::StatusCode::NOT_FOUND, "not found") });
    /// ```
    pub fn fallback<H, T>(self, handler: H) -> Self
    where
        H: Handler<T, S>,
        T: 'static,
    {
        Self {
            router: self.router.fallback(handler),
            ..self
        }
    }

    /// 把 layer 延迟加入全 `Router` 栈。
    ///
    /// 输入必须能包裹 Axum `Route`，且 service 错误可转换为 `Infallible`。返回的新
    /// `AxumApp` 保存闭包，直到 `with_state` 或 `into_server_builder` 才应用 layer。按 a、b 的
    /// 调用顺序登记时，请求顺序为 a → b → handler，响应反向；全局 layer 也覆盖 fallback。
    /// 本方法不返回错误、不执行 layer、不 bind 且不访问网络。layer 数量与其资源开销由调用方
    /// 控制。API 只要求 crate 的 `axum + tokio` feature，不要求本 crate 的 `tower` provider
    /// feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let layer = axum::middleware::from_fn(
    ///     |request: axum::extract::Request, next: axum::middleware::Next| async move {
    ///         next.run(request).await
    ///     },
    /// );
    /// let _app = axutils::AxumApp::new().with_layer(layer);
    /// ```
    pub fn with_layer<L>(self, layer: L) -> AxumApp<S, impl FnOnce(Router) -> Router, R>
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        G: FnOnce(Router) -> Router,
    {
        let previous = self.global;
        AxumApp {
            router: self.router,
            global: move |router: Router| previous(router.layer(layer)),
            matched: self.matched,
            has_matched_layer: self.has_matched_layer,
        }
    }

    /// 把 layer 延迟加入已匹配 route 的栈，不包裹 404 fallback。
    ///
    /// 输入约束与 `with_layer` 相同；返回的新 `AxumApp` 保存闭包，并在收敛 state 或转为 server
    /// builder 时应用。调用本方法本身不返回错误；若最终 router 没有路由，错误会被延迟记录，
    /// 随后由 `AxumServerBuilder::build` 返回
    /// `AxumError::InvalidConfig { field: "matched_route_layer" }`，而不是触发 Axum panic。
    /// layer 不会在登记时执行，也不会 bind 或访问网络；其资源消耗由调用方控制。API 只要求
    /// crate 的 `axum + tokio` feature，不要求本 crate 的 `tower` provider feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let layer = axum::middleware::from_fn(
    ///     |request: axum::extract::Request, next: axum::middleware::Next| async move {
    ///         next.run(request).await
    ///     },
    /// );
    /// let _app = axutils::AxumApp::new()
    ///     .route("/health", axum::routing::get(|| async { "ok" }))
    ///     .with_matched_route_layer(layer);
    /// ```
    pub fn with_matched_route_layer<L>(
        self,
        layer: L,
    ) -> AxumApp<S, G, impl FnOnce(Router) -> Router>
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
        R: FnOnce(Router) -> Router,
    {
        let previous = self.matched;
        AxumApp {
            router: self.router,
            global: self.global,
            matched: move |router: Router| previous(router.route_layer(layer)),
            has_matched_layer: true,
        }
    }
}

impl<S, G, R> AxumApp<S, G, R>
where
    S: Clone + Send + Sync + 'static,
    G: FnOnce(Router) -> Router,
    R: FnOnce(Router) -> Router,
{
    /// 注入 missing state，并收敛为 `AxumServerBuilder` 所需的 `Router<()>`。
    ///
    /// `state` 的类型为 `S`；本实现先克隆当前 router，再按 Axum 语义注入 state，并统一应用延迟
    /// layer 栈。返回值不 bind、不创建 runtime，也不访问网络。该方法不直接返回错误；空 router
    /// 上存在 matched-route layer 时，会在 builder 中记录配置错误，后续 `build` 返回
    /// `AxumError::InvalidConfig { field: "matched_route_layer" }`。state 的克隆、并发安全与每次
    /// 请求的资源生命周期受 `S: Clone + Send + Sync + 'static` 及 Axum 0.8 语义约束。本 API 仅
    /// 要求 crate 的 `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// async fn handler(axum::extract::State(name): axum::extract::State<String>) -> String {
    ///     name
    /// }
    /// let router = axum::Router::<String>::new()
    ///     .route("/name", axum::routing::get(handler));
    /// let _builder = axutils::AxumApp::from_router(router).with_state("axutils".to_owned());
    /// ```
    pub fn with_state(self, state: S) -> AxumServerBuilder {
        let router = self.router.clone().with_state(state);
        self.finish(router)
    }
    fn finish(self, router: Router) -> AxumServerBuilder {
        let invalid = self.has_matched_layer && !router.has_routes();
        let router = if invalid {
            (self.global)(router)
        } else {
            (self.global)((self.matched)(router))
        };
        AxumServerBuilder::new_with_error(
            router,
            AxumConfig::default(),
            invalid.then_some(AxumError::InvalidConfig {
                field: "matched_route_layer",
            }),
        )
    }
}

impl AxumApp<()> {
    /// 创建不含路由、fallback 或延迟 layer 的无 state 应用构建器。
    ///
    /// 返回 `AxumApp<()>`。该方法不会失败、bind、创建 runtime 或访问网络；路由数量等限制由
    /// 后续注册操作和 Axum 0.8 决定。仅在 crate 同时启用 `axum` 与 `tokio` feature 时可用。
    /// `Default::default()` 与本方法等价。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let app = axutils::AxumApp::new();
    /// let _builder = app.into_server_builder();
    /// ```
    pub fn new() -> Self {
        Self::from_router(Self::create_router())
    }
}
impl<G, R> AxumApp<(), G, R>
where
    G: FnOnce(Router) -> Router,
    R: FnOnce(Router) -> Router,
{
    /// 把无 missing state 的应用收敛为 `AxumServerBuilder`。
    ///
    /// 本实现克隆当前 router，并统一应用延迟 layer 栈。返回的 builder 尚未 bind、创建 runtime 或
    /// 访问网络。该方法不直接返回错误；空 router 上存在 matched-route layer 时，会记录
    /// `AxumError::InvalidConfig { field: "matched_route_layer" }`，后续 `build` 才返回该错误。
    /// 真正启动服务前仍须调用 `build` 和 server 的 `serve*` 方法；本 API 仅要求 crate 的
    /// `axum + tokio` feature。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// let builder = axutils::AxumApp::new()
    ///     .route("/health", axum::routing::get(|| async { "ok" }))
    ///     .into_server_builder();
    /// let _server = builder.build()?;
    /// # Ok::<(), axutils::AxumError>(())
    /// ```
    pub fn into_server_builder(self) -> AxumServerBuilder {
        let router = self.router.clone();
        self.finish(router)
    }
}
impl Default for AxumApp<()> {
    fn default() -> Self {
        Self::new()
    }
}
