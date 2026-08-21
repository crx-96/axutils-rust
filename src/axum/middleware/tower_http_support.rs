use super::super::{AxumError, AxumServerBuilder};
use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::time::Duration;
#[cfg(feature = "tracing")]
use tower_http::request_id::RequestId;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestId, MakeRequestUuid},
    timeout::TimeoutLayer,
};

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
/// `tower-http` service timeout 的 HTTP 响应状态。
///
/// 需要同时启用 `axum`、`tokio` 与 `tower-http` feature，仅决定
/// [`AxumServerBuilder::with_timeout`] 超时响应的状态码，不改变时限、不产生 I/O，也不会自行
/// 报错。408 表示本服务处理超时；504 只应在调用方明确采用网关语义时使用。
///
/// # Examples
///
/// ```rust
/// assert_eq!(
///     axutils::AxumTimeoutStatus::default(),
///     axutils::AxumTimeoutStatus::RequestTimeout,
/// );
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AxumTimeoutStatus {
    /// 返回 HTTP 408 Request Timeout；这是默认值，表示本服务的 service future 超时。
    #[default]
    RequestTimeout,
    /// 返回 HTTP 504 Gateway Timeout；只适合当前服务确实承担网关角色的场景。
    GatewayTimeout,
}
/// `tower-http` CORS layer 的允许 origin 模式。
///
/// 需要同时启用 `axum`、`tokio` 与 `tower-http` feature。空列表与 `Disabled` 都不会安装
/// CORS layer，绝不会隐式扩大为 `Any`。该值只保存配置，不访问网络、修改全局状态或自行报错；
/// 具体校验由 [`AxumServerBuilder::with_cors`] 执行。
///
/// # Examples
///
/// ```rust
/// use axutils::AxumCorsOrigin;
///
/// let origins = AxumCorsOrigin::List(vec![
///     "https://example.com".parse().unwrap(),
/// ]);
/// assert!(matches!(origins, AxumCorsOrigin::List(_)));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AxumCorsOrigin {
    /// 禁用 CORS；builder 不安装 CORS layer。
    Disabled,
    /// 允许任意 origin；不能与凭据模式组合。
    Any,
    /// 允许给定的 HTTP `Origin` header 值；最多 64 项且每项最多 1,024 字节，空列表表示禁用。
    List(Vec<HeaderValue>),
}
/// 有界的 `tower-http` CORS 配置。
///
/// 需要同时启用 `axum`、`tokio` 与 `tower-http` feature。三个列表分别最多 64 项，origin
/// 列表还限制每项最多 1,024 字节，`max_age` 最多一天；`Any` 不能与 `allow_credentials`
/// 同时使用。默认值禁用 CORS，且所有列表为空。本类型只保存内存配置，不访问网络或修改
/// 全局状态；安装时的无效组合由 [`AxumServerBuilder::with_cors`] 以 `AxumError` 返回。
///
/// # Examples
///
/// ```rust
/// use axutils::{AxumCorsConfig, AxumCorsOrigin};
///
/// let config = AxumCorsConfig {
///     origins: AxumCorsOrigin::List(vec!["https://example.com".parse().unwrap()]),
///     methods: vec!["GET".parse().unwrap()],
///     ..AxumCorsConfig::default()
/// };
/// assert_eq!(config.methods.len(), 1);
/// ```
#[derive(Clone, Debug)]
pub struct AxumCorsConfig {
    /// 允许的 origin 模式；`Disabled` 或空 `List` 表示不安装 CORS layer。
    pub origins: AxumCorsOrigin,
    /// `Access-Control-Allow-Methods` 的方法列表；空列表不设置，最多 64 项。
    pub methods: Vec<Method>,
    /// `Access-Control-Allow-Headers` 的 header 名列表；空列表不设置，最多 64 项。
    pub headers: Vec<HeaderName>,
    /// `Access-Control-Expose-Headers` 的 header 名列表；空列表不设置，最多 64 项。
    pub expose_headers: Vec<HeaderName>,
    /// 是否允许凭据；为 `true` 时不能把 `origins` 设为 `Any`。
    pub allow_credentials: bool,
    /// 预检结果的 `Access-Control-Max-Age`；`None` 表示不设置，最大为 86,400 秒。
    pub max_age: Option<Duration>,
}
impl Default for AxumCorsConfig {
    fn default() -> Self {
        Self {
            origins: AxumCorsOrigin::Disabled,
            methods: Vec::new(),
            headers: Vec::new(),
            expose_headers: Vec::new(),
            allow_credentials: false,
            max_age: None,
        }
    }
}
impl AxumCorsConfig {
    fn layer(self) -> Result<Option<CorsLayer>, AxumError> {
        if self.methods.len() > 64 || self.headers.len() > 64 || self.expose_headers.len() > 64 {
            return Err(AxumError::InvalidConfig { field: "cors_list" });
        }
        if self.methods.iter().any(|value| value.as_str() == "*") {
            return Err(AxumError::InvalidConfig {
                field: "cors_methods",
            });
        }
        if self.headers.iter().any(|value| value.as_str() == "*") {
            return Err(AxumError::InvalidConfig {
                field: "cors_headers",
            });
        }
        if self
            .expose_headers
            .iter()
            .any(|value| value.as_str() == "*")
        {
            return Err(AxumError::InvalidConfig {
                field: "cors_expose_headers",
            });
        }
        if self
            .max_age
            .is_some_and(|v| v > Duration::from_secs(86_400))
        {
            return Err(AxumError::InvalidConfig {
                field: "cors_max_age",
            });
        }
        let mut layer = CorsLayer::new();
        match self.origins {
            AxumCorsOrigin::Disabled => return Ok(None),
            AxumCorsOrigin::Any => {
                if self.allow_credentials {
                    return Err(AxumError::InvalidConfig {
                        field: "cors_credentials",
                    });
                }
                layer = layer.allow_origin(Any)
            }
            AxumCorsOrigin::List(v) => {
                if v.is_empty() {
                    return Ok(None);
                }
                if v.len() > 64
                    || v.iter()
                        .any(|origin| origin.as_bytes() == b"*" || origin.as_bytes().len() > 1_024)
                {
                    return Err(AxumError::InvalidConfig {
                        field: "cors_origins",
                    });
                }
                layer = layer.allow_origin(v)
            }
        }
        if !self.methods.is_empty() {
            layer = layer.allow_methods(self.methods)
        }
        if !self.headers.is_empty() {
            layer = layer.allow_headers(self.headers)
        }
        if !self.expose_headers.is_empty() {
            layer = layer.expose_headers(self.expose_headers)
        }
        if self.allow_credentials {
            layer = layer.allow_credentials(true)
        }
        if let Some(v) = self.max_age {
            layer = layer.max_age(v)
        }
        Ok(Some(layer))
    }
}

impl AxumServerBuilder {
    /// 安装内部 request ID；移除全部入站同名 header 并覆盖 handler 冲突响应值。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_request_id(); }
    /// ```
    pub fn with_request_id(mut self) -> Self {
        self.request_id_installed = true;
        self
    }
    /// 安装 1 毫秒..=10 分钟 service timeout；不是连接/header/drain timeout。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_timeout(std::time::Duration::from_secs(1),axutils::AxumTimeoutStatus::RequestTimeout).unwrap(); }
    /// ```
    pub fn with_timeout(
        mut self,
        duration: Duration,
        status: AxumTimeoutStatus,
    ) -> Result<Self, AxumError> {
        if !(Duration::from_millis(1)..=Duration::from_secs(600)).contains(&duration) {
            return Err(AxumError::InvalidConfig {
                field: "service_timeout",
            });
        }
        self.timeout_layer = Some((duration, status));
        Ok(self)
    }
    /// 安装 1 字节..=64 MiB 请求体上限；约束 Content-Length 和流式累计 body。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { assert!(axutils::AxumApp::new().into_server_builder().with_body_limit(0).is_err()); }
    /// ```
    pub fn with_body_limit(mut self, max_bytes: usize) -> Result<Self, AxumError> {
        if !(1..=64 * 1024 * 1024).contains(&max_bytes) {
            return Err(AxumError::InvalidConfig {
                field: "max_body_bytes",
            });
        }
        self.router = self.router.layer(RequestBodyLimitLayer::new(max_bytes));
        Ok(self)
    }
    /// 捕获内层 unwind 并返回脱敏 500；不能捕获 abort。该 layer 自身不记录 payload，
    /// 但进程 panic hook 会在捕获前运行，宿主必须安装脱敏 hook，不能把 secret 放入 panic payload。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_catch_panic(); }
    /// ```
    pub fn with_catch_panic(mut self) -> Self {
        self.catch_panic_installed = true;
        self
    }
    /// 安装全 Router CORS；Disabled/空列表不安装，wildcard + credentials 返回错误。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http"))] { let _=axutils::AxumApp::new().into_server_builder().with_cors(axutils::AxumCorsConfig::default()).unwrap(); }
    /// ```
    pub fn with_cors(mut self, config: AxumCorsConfig) -> Result<Self, AxumError> {
        if let Some(layer) = config.layer()? {
            self.router = self.router.layer(layer)
        }
        Ok(self)
    }
    pub(crate) fn finalize_tower_http(mut self) -> Self {
        if self.catch_panic_installed {
            self.router = self.router.layer(CatchPanicLayer::new());
        }
        if let Some((duration, status)) = self.timeout_layer {
            let code = match status {
                AxumTimeoutStatus::RequestTimeout => StatusCode::REQUEST_TIMEOUT,
                AxumTimeoutStatus::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
            };
            self.router = self
                .router
                .layer(TimeoutLayer::with_status_code(code, duration));
        }
        #[cfg(feature = "tracing")]
        if self.http_trace_installed {
            self.router = self.router.layer(axum::middleware::from_fn(trace_request));
        }
        if self.request_id_installed {
            self.router = self
                .router
                .layer(axum::middleware::from_fn(force_request_id));
        }
        self
    }
}
async fn force_request_id(mut request: Request, next: Next) -> Response {
    request.headers_mut().remove(&REQUEST_ID);
    let mut maker = MakeRequestUuid;
    let id = maker
        .make_request_id(&request)
        .expect("MakeRequestUuid always produces a valid header");
    request
        .headers_mut()
        .insert(REQUEST_ID, id.header_value().clone());
    request.extensions_mut().insert(id.clone());
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(REQUEST_ID, id.into_header_value());
    response
}

#[cfg(feature = "tracing")]
impl AxumServerBuilder {
    /// 安装脱敏 trace 并确保内部 request ID；只记录 method/matched route/status/latency/ID。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio",feature="tower-http",feature="tracing"))] { let _=axutils::AxumApp::new().into_server_builder().with_http_trace(); }
    /// ```
    pub fn with_http_trace(mut self) -> Self {
        self.request_id_installed = true;
        self.http_trace_installed = true;
        self
    }
}
#[cfg(feature = "tracing")]
async fn trace_request(request: Request, next: Next) -> Response {
    use std::time::Instant;
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|v| v.as_str().to_owned())
        .unwrap_or_else(|| "<unmatched>".into());
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .unwrap_or("<missing>")
        .to_owned();
    let start = Instant::now();
    let response = next.run(request).await;
    tracing::info!(target:"axutils::axum",method=%method,matched_route=%route,request_id,status=response.status().as_u16(),latency_micros=start.elapsed().as_micros(),"http request completed");
    response
}
