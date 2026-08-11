# HTTP 使用文档

HTTP 能力属于独立的 `http` feature。只启用 `http` 时提供同步 `HttpClient`；同时启用
`tokio` 时才增加异步 `HttpClient::execute_async` 和 `HttpUtils::execute_async`。`tokio`
feature 不会反向启用 `http`，因此只启用 Tokio 不会导出 HTTP 类型。

Serde JSON/query/字节快捷方法需要显式同时启用 `http` 与 `serde`；异步快捷方法还需要
`tokio`。`http` 不会自动启用 `serde`。

```toml
[dependencies]
axutils = { version = "0.1", features = ["http"] }
```

需要三参数 JSON/字节快捷方法时：

```toml
[dependencies]
axutils = { version = "0.1", features = ["http", "serde"] }
```

异步调用方还需要直接依赖 Tokio 并提供 runtime：

```toml
[dependencies]
axutils = { version = "0.1", features = ["http", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

异步 JSON/字节快捷方法使用：

```toml
[dependencies]
axutils = { version = "0.1", features = ["http", "serde", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

客户端使用 Rustls；同步后端为 `ureq`，异步后端为 `reqwest`。当前 `http` feature 为保持
`http + tokio` 的组合式异步 API 契约，会同时编译两个可选后端；只调用同步 API 的项目也会承担
`reqwest` 及其传递依赖的编译成本。两者都关闭系统代理、自动重定向、自动压缩和后端隐式重试。
库只保证客户端侧约束，不承诺 SSRF 防护；调用方仍须自行限制目标主机、出口网络、DNS 重绑定
风险以及业务认证信息。

## 公共导出路径

HTTP 模块可从以下路径访问：

| 项目 | 推荐路径 | 兼容/次级路径 | feature |
| --- | --- | --- | --- |
| `HttpClient` | `axutils::http::HttpClient` | `axutils::HttpClient` | `http` |
| `HttpConfig` | `axutils::http::HttpConfig` | `axutils::HttpConfig` | `http` |
| `HttpConfigBuilder` | `axutils::http::HttpConfigBuilder` | `axutils::HttpConfigBuilder` | `http` |
| `HttpHeaders` | `axutils::http::HttpHeaders` | `axutils::HttpHeaders` | `http` |
| `HttpMethod` | `axutils::http::HttpMethod` | `axutils::HttpMethod` | `http` |
| `HttpRequest` | `axutils::http::HttpRequest` | `axutils::HttpRequest` | `http` |
| `HttpRequestBuilder` | `axutils::http::HttpRequestBuilder` | `axutils::HttpRequestBuilder` | `http` |
| `HttpRequestOptions` | `axutils::http::HttpRequestOptions` | `axutils::HttpRequestOptions` | `http` |
| `HttpResponse` | `axutils::http::HttpResponse` | `axutils::HttpResponse` | `http` |
| `HttpError` | `axutils::http::HttpError` | `axutils::HttpError` | `http` |
| `HttpTransportErrorKind` | `axutils::http::HttpTransportErrorKind` | `axutils::HttpTransportErrorKind` | `http` |
| `RetryPolicy` | `axutils::http::RetryPolicy` | `axutils::RetryPolicy` | `http` |
| `DeduplicationPolicy` | `axutils::http::DeduplicationPolicy` | `axutils::DeduplicationPolicy` | `http` |
| `DeduplicationMode` | `axutils::http::DeduplicationMode` | `axutils::DeduplicationMode` | `http` |
| 全局入口 | `axutils::HttpUtils` | `axutils::utils::HttpUtils`、`axutils::utils::http_utils::HttpUtils` | `http` |
| JSON/query/字节快捷方法 | `HttpClient::{get,post,delete,patch,put,options}` 等 | `HttpUtils` 同名静态方法 | `http` + `serde` |
| 异步方法 | `HttpClient::execute_async`、`HttpUtils::execute_async` 及 `_async` 快捷方法 | 无同步别名 | `http` + `tokio`；Serde 快捷方法还要求 `serde` |

`axutils::http::client`、`coalesce`、`config`、`error`、`headers`、`options`、`request`、
`response`、`retry` 和 `serde_api` 是实现文件，不是公开子模块。HTTP 不提供公开常量、trait、
类型别名或宏。

## 安全默认值和执行语义

- 只接受 `http` 和 `https`，拒绝 URL 用户信息、片段、控制字符和不安全的 scheme；不设置
  `base_url` 时仍可执行完整的绝对 URL，相对 URL 会返回 `HttpError::InvalidUrl`。如果同时
  设置了 `base_url`，请求自身的绝对 URL 仍优先于配置基地址。
- Header 名称必须是 HTTP token，值拒绝控制字符；Header 数量、单值和总大小均有限制。
  `Authorization`、`Cookie` 和 `Set-Cookie` 不允许通过公开 `append` 形成重复项，默认 Header
  与请求 Header 发生敏感冲突时返回错误。配置了 `base_url` 时，跨 origin 的绝对请求 URL
  不继承默认敏感 Header；请求对象上显式设置的敏感 Header 仍会发送，调用方必须自行确认目标。
- 默认请求总时间预算为 30 秒、连接预算为 10 秒、请求体和响应体上限均为 1 MiB。
  `max_retries` 沿用现有方法名，但表示包括首次请求在内的最大总网络尝试次数；默认值为 3，
  `1` 表示禁用自动重试。退避是有限的指数退避，不带随机抖动且受总时间预算约束。
- 默认只对无体 `GET`、`HEAD`、`OPTIONS` 做 single-flight。`POST`、`PUT`、`PATCH`、
  `DELETE` 和带体请求只有在请求级显式设置去重策略后才允许合并。leader 取消或异常退出时，
  follower 收到 `CoalescedRequestCancelled`；follower 自己超时不会取消 leader。
- 完成缓存必须通过 `DeduplicationPolicy::with_completed_ttl` 显式开启，只缓存满足安全条件的
  2xx 无体 `GET`/`HEAD`；请求带认证、Cookie、Range、条件 Header、`Pragma`，请求或响应的
  `Cache-Control` 含 `no-store`/`no-cache`，或响应带 `Set-Cookie`、`Vary: *` 时不缓存。缓存
  按客户端隔离、使用单调时钟并受条目数和响应体总大小上限约束。
- 4xx/5xx 是正常的 `HttpResponse`，不会转换为 `HttpError`；连接、TLS、协议、超时、大小
  和本地校验失败才返回 `HttpError`。`HttpError` 不保存 URL、Header 值、请求/响应体或第三方
  原始错误文本。

## `HttpError` 和传输错误分类

`HttpError` 是 `Clone + Debug + Eq + PartialEq` 的稳定错误枚举，实现 `Display` 和
`std::error::Error`，但不提供 `source()` 以避免把第三方错误文本或敏感 URL 带出边界。
公开变体包括：

- `InvalidConfig`、`InvalidRequest`、`InvalidUrl`：配置或请求结构不合法；字段名是固定的，
  不包含用户值。
- `InvalidHeaderName`、`InvalidHeaderValue`、`HeaderLimitExceeded`、`DuplicateSensitiveHeader`：
  Header 校验或安全合并失败。
- `RequestBodyTooLarge`、`ResponseTooLarge`、`InvalidUtf8`：大小或文本解码失败。
- `JsonSerialize`、`QuerySerialize`、`JsonDeserialize`：Serde JSON body、query 或响应解码失败；
  错误不会包含底层 Serde 文本。
- `Transport { kind, attempts, exhausted }`：网络传输失败；`attempts` 为已发起的网络尝试数。
- `RuntimeRequired`、`BlockingInAsyncRuntime`：异步 runtime 缺失或同步入口误入 Tokio runtime。
- `NotInitialized`、`AlreadyInitialized`：`HttpUtils` 全局入口状态错误。
- `CoalescedRequestCancelled`、`CoalescedWaitTimeout`：single-flight leader/follower 状态。
- `ClientBuild`：后端客户端构造失败。

`HttpTransportErrorKind` 有 `Connection`、`Timeout`、`Tls`、`Protocol` 和 `Other` 五个变体。
可以按分类匹配，但不要依赖底层库的错误文本。

## `HttpHeaders`

`HttpHeaders` 是保留重复项顺序的自有 Header 集合。名称按 ASCII 不区分大小写；值按字节保存，
因此不会被强制转换为 UTF-8。它实现 `Clone + Debug + Default + Eq + Hash + PartialEq`，`Debug`
只显示数量和总字节数。

### `HttpHeaders::new`

```rust
# #[cfg(feature = "http")]
# fn main() {
let headers = axutils::HttpHeaders::new();
assert!(headers.is_empty());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::with_capacity`

```rust
# #[cfg(feature = "http")]
# fn main() {
let headers = axutils::HttpHeaders::with_capacity(4);
assert_eq!(headers.len(), 0);
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::len`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
assert!(headers.is_empty());
headers.set("accept", "application/json")?;
assert_eq!(headers.len(), 1);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::is_empty`

```rust
# #[cfg(feature = "http")]
# fn main() {
let headers = axutils::HttpHeaders::new();
assert!(headers.is_empty());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::set`

`set` 替换全部同名条目；它适合设置唯一的认证、Cookie 或内容类型 Header。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.set("X-Trace", "one")?;
headers.set("x-trace", "two")?;
assert_eq!(headers.get("X-TRACE"), Some(&b"two"[..]));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::append`

`append` 保留同名普通 Header 的顺序，但拒绝重复敏感 Header。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.append("accept", "application/json")?;
headers.append("accept", "text/plain")?;
assert_eq!(headers.len(), 2);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::remove`

`remove` 会删除全部同名条目，并返回是否实际删除了内容；无效名称返回 `false`。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.set("x-cache", "no")?;
assert!(headers.remove("X-Cache"));
assert!(!headers.remove("X-Cache"));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::contains`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.set("accept", "application/json")?;
assert!(headers.contains("ACCEPT"));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::get`

`get` 返回第一个同名值的字节切片；重复普通 Header 可通过 `iter` 完整遍历。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.set("content-type", "application/json")?;
assert_eq!(headers.get("Content-Type"), Some(&b"application/json"[..]));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpHeaders::iter`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let mut headers = axutils::HttpHeaders::new();
headers.append("accept", "application/json")?;
headers.append("accept", "text/plain")?;
let values: Vec<&[u8]> = headers.iter().map(|(_, value)| value).collect();
assert_eq!(values, vec![&b"application/json"[..], &b"text/plain"[..]]);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `HttpMethod`

`HttpMethod` 有 `Get`、`Head`、`Post`、`Put`、`Patch`、`Delete`、`Options`、`Trace`、`Connect`
和 `Custom(String)` 变体。它实现 `Clone + Debug + Eq + Hash + PartialEq + Display`，并实现
`FromStr<Err = HttpError>`。默认重试安全集合只有 `GET`、`HEAD` 和 `OPTIONS`。

### `HttpMethod::custom`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let method = axutils::HttpMethod::custom("REPORT")?;
assert_eq!(method.as_str(), "REPORT");
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpMethod::as_str`

```rust
# #[cfg(feature = "http")]
# fn main() {
let method = axutils::HttpMethod::Get;
assert_eq!(method.as_str(), "GET");
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `Display` 和 `FromStr`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
use std::str::FromStr;
let method = axutils::HttpMethod::from_str("PATCH")?;
assert_eq!(method.to_string(), "PATCH");
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `RetryPolicy`

`RetryPolicy` 实现 `Clone + Debug + Eq + Hash + PartialEq`。默认值为最多 3 次总网络尝试、100 ms
初始退避、2 s 最大退避和状态码 `408/425/429/500/502/503/504`；延迟没有随机抖动，且最终受
请求总时间预算约束。`max_retries` 方法名沿用现有 API，但参数和返回值均表示总尝试次数，包括首次请求。
传入 `1` 可禁用自动重试。

### `RetryPolicy::new`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
assert_eq!(policy.max_retries(), 3);
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::with_max_retries`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let policy = axutils::RetryPolicy::new().with_max_retries(2)?;
assert_eq!(policy.max_retries(), 2);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::with_backoff`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let policy = axutils::RetryPolicy::new()
    .with_backoff(std::time::Duration::from_millis(10), std::time::Duration::from_secs(1))?;
assert_eq!(policy.base_delay(), std::time::Duration::from_millis(10));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::with_retry_status`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let policy = axutils::RetryPolicy::new().with_retry_status(500, false)?;
assert!(!policy.retry_statuses().any(|status| *status == 500));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::with_allow_non_idempotent`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new().with_allow_non_idempotent(true);
assert!(policy.allows_non_idempotent());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::max_retries`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
let _ = policy.max_retries();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::base_delay`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
let _ = policy.base_delay();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::max_delay`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
let _ = policy.max_delay();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::allows_non_idempotent`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
let _ = policy.allows_non_idempotent();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `RetryPolicy::retry_statuses`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::RetryPolicy::new();
assert!(policy.retry_statuses().any(|status| *status == 503));
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `DeduplicationPolicy` 和 `DeduplicationMode`

`DeduplicationMode` 的 `Disabled`、`InFlight`、`WithCompletedTtl` 分别表示关闭、只合并执行中
请求、合并执行中请求并缓存成功响应。`DeduplicationPolicy` 实现 `Clone + Debug + Eq + Hash +
PartialEq`；默认是 `InFlight`，默认完成缓存 TTL 为零。

### `DeduplicationPolicy::disabled`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::disabled();
assert!(!policy.is_enabled());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::in_flight`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let policy = axutils::DeduplicationPolicy::in_flight(128)?;
assert_eq!(policy.mode(), axutils::DeduplicationMode::InFlight);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::with_completed_ttl`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let policy = axutils::DeduplicationPolicy::with_completed_ttl(
    std::time::Duration::from_secs(5), 128, 64, 1024 * 1024,
)?;
assert!(policy.cache_enabled());
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::mode`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.mode();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::ttl`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.ttl();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::is_enabled`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.is_enabled();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::cache_enabled`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.cache_enabled();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::max_inflight_keys`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.max_inflight_keys();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::max_completed_entries`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.max_completed_entries();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `DeduplicationPolicy::max_cached_body_bytes`

```rust
# #[cfg(feature = "http")]
# fn main() {
let policy = axutils::DeduplicationPolicy::default();
let _ = policy.max_cached_body_bytes();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `HttpConfig` 和 `HttpConfigBuilder`

`HttpConfig` 实现 `Clone + Debug + Default + Eq + PartialEq`。默认配置没有基地址，因而只能执行绝对
URL 请求；对相对 URL 返回 `InvalidUrl`。如果配置了基地址，请求自身的绝对 URL 仍优先。配置错误
发生在构造阶段，不会写入全局 `OnceLock`。`Debug` 不显示基地址本身，
只显示是否配置；默认 Header 值也不显示。

### `HttpConfig::builder`

```rust
# #[cfg(feature = "http")]
# fn main() {
let builder = axutils::HttpConfig::builder();
let _ = builder;
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::base_url`

此方法是可选的。不调用时基地址为 `None`，绝对 HTTP/HTTPS URL 仍可直接执行，相对 URL
会在执行时返回 `HttpError::InvalidUrl`；即使调用了此方法，请求自身的绝对 URL 仍优先。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder().base_url("https://example.com/api/")?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::default_headers`

```rust
# #[cfg(feature = "http")]
# fn main() {
let headers = axutils::HttpHeaders::new();
let builder = axutils::HttpConfig::builder().default_headers(headers);
let _ = builder;
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::with_default_header`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder().with_default_header("accept", "application/json")?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::request_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder()
    .request_timeout(std::time::Duration::from_secs(10))?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::connect_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder()
    .connect_timeout(std::time::Duration::from_secs(2))?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::max_request_body_bytes`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder()
    .max_request_body_bytes(1024 * 1024)?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::max_response_body_bytes`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder().max_response_body_bytes(1024 * 1024)?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::max_idle_connections_per_host`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder().max_idle_connections_per_host(8)?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::idle_connection_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let builder = axutils::HttpConfig::builder()
    .idle_connection_timeout(std::time::Duration::from_secs(60))?;
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::retry_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let retry = axutils::RetryPolicy::new();
let builder = axutils::HttpConfig::builder().retry_policy(retry);
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::deduplication_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let dedup = axutils::DeduplicationPolicy::disabled();
let builder = axutils::HttpConfig::builder().deduplication_policy(dedup);
let _ = builder;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfigBuilder::build`

所有配置项都可以省略。空 builder 会填充有限默认值：请求总超时 30 秒、连接超时 10 秒、
请求/响应体上限 1 MiB、空闲连接超时 60 秒，以及包括首次请求在内的最多 3 次网络尝试。

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let config = axutils::HttpConfig::builder().build()?;
assert_eq!(config.max_response_body_bytes(), 1024 * 1024);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::base_url`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let config = axutils::HttpConfig::builder().base_url("https://example.com/")?.build()?;
let _ = config.base_url();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::default_headers`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.default_headers();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::request_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.request_timeout();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::connect_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.connect_timeout();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::max_request_body_bytes`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.max_request_body_bytes();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::max_response_body_bytes`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.max_response_body_bytes();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::max_idle_connections_per_host`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.max_idle_connections_per_host();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::idle_connection_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.idle_connection_timeout();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::retry_policy`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.retry_policy();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpConfig::deduplication_policy`

```rust
# #[cfg(feature = "http")]
# fn main() {
let config = axutils::HttpConfig::default();
let _ = config.deduplication_policy();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `HttpRequest` 和 `HttpRequestBuilder`

`HttpRequest` 和 `HttpRequestBuilder` 都会脱敏 `Debug` 输出：URL 只显示为占位符，Header 值和
请求体不会显示。`HttpRequest` 的 `with_body` 和 builder 的 `body` 在请求构造阶段限制为 16 MiB；
客户端还会使用 `HttpConfig` 的更小请求体上限做二次校验。

### `HttpRequest::new`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/health")?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::builder`

```rust
# #[cfg(feature = "http")]
# fn main() {
let builder = axutils::HttpRequest::builder();
let _ = builder;
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::with_header`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?
    .with_header("accept", "application/json")?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::append_header`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?
    .append_header("accept", "text/plain")?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::with_body`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Post, "https://example.com/")?
    .with_body(b"payload".to_vec())?;
assert_eq!(request.body(), Some(&b"payload"[..]));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::with_timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?
    .with_timeout(std::time::Duration::from_secs(5))?;
assert_eq!(request.timeout(), Some(std::time::Duration::from_secs(5)));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::with_retry_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?
    .with_retry_policy(axutils::RetryPolicy::new());
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::with_deduplication_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?
    .with_deduplication_policy(axutils::DeduplicationPolicy::disabled());
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::method`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.method();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::url`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.url();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::headers`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.headers();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::body`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.body();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.timeout();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::retry_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.retry_policy();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequest::deduplication_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let _ = request.deduplication_policy();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::method`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Post)
;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::url`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::header`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .header("accept", "application/json")?
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::append_header`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .append_header("accept", "text/plain")?
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::body`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Post)
    .url("https://example.com/")
    .body(b"payload".to_vec())?
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::timeout`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .timeout(std::time::Duration::from_secs(5))?
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::retry_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .retry_policy(axutils::RetryPolicy::new())
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::deduplication_policy`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .deduplication_policy(axutils::DeduplicationPolicy::disabled())
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpRequestBuilder::build`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::builder()
    .method(axutils::HttpMethod::Get)
    .url("https://example.com/")
    .build()?;
let _ = request;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `HttpResponse`

`HttpResponse` 实现 `Clone`；响应体使用共享不可变存储，single-flight follower 不会复制底层字节。
`Debug` 只显示状态码、Header 数量、响应体长度和网络尝试数。

下面的各个示例使用 `no_run`，只验证 API 类型，不会访问 `example.com`：

### `HttpClient::execute` 返回响应

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let response = client.execute(request)?;
let _ = response;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::status`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
assert!((100..=599).contains(&response.status()));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::is_success`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
let _successful = response.is_success();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::headers`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
let _ = response.headers();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::header`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
let _ = response.header("content-type");
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::body`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
let bytes: &[u8] = response.body();
let _ = bytes;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::into_body`

消费 `HttpResponse` 并返回拥有型 `Vec<u8>`。响应体没有被缓存或 single-flight 共享时直接取回
底层缓冲区，存在其他共享者时才复制；响应状态、Header 和尝试次数在消费后不再可访问。

~~~rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(
    axutils::HttpMethod::Get,
    "https://example.com/bytes",
)?)?;
let bytes = response.into_body();
let _ = bytes;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### `HttpResponse::json`

在 `http + serde` 下把已取得的响应 body 解码为实现 `DeserializeOwned` 的类型。
失败时返回稳定的 `HttpError::JsonDeserialize`，不会暴露 Serde 原始错误文本。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(
    axutils::HttpMethod::Get,
    "https://example.com/health",
)?)?;
let value: BTreeMap<String, bool> = response.json()?;
let _ = value;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpResponse::text`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
let _text = response.text()?;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpResponse::attempts`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?)?;
assert!(response.attempts() >= 1);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

## `HttpClient`

### `HttpClient::new`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
let _ = client;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpClient::config`

```rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
assert_eq!(client.config().max_request_body_bytes(), 1024 * 1024);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpClient::execute`

同步入口可执行绝对 URL；没有基地址时相对 URL 返回 `InvalidUrl`，配置了基地址时相对 URL
按基地址解析，而请求自身的绝对 URL 优先。如果启用了 `tokio` feature，当前线程位于 runtime
时同步入口返回 `BlockingInAsyncRuntime`。

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let config = axutils::HttpConfig::builder().base_url("https://example.com/")?.build()?;
let client = axutils::HttpClient::new(config)?;
let response = client.execute(axutils::HttpRequest::new(axutils::HttpMethod::Get, "/health")?)?;
let _ = response.status();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpClient::execute_async`

此方法只在 `http + tokio` 存在，并且不会创建 runtime。示例使用保留域名和 `no_run`，不执行
外部网络副作用。

```rust,no_run
# #[cfg(all(feature = "http", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
    let response = client.execute_async(request).await?;
    let _ = response.status();
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "tokio")))]
# fn main() {}
```

## `HttpUtils`

`HttpUtils` 是 `OnceLock<HttpClient>` 的一次初始化全局入口。它不会在配置无效时占用初始化
机会，也不能 reset 或替换；多账号、不同基地址或不同生命周期场景应直接使用 `HttpClient`。

### `HttpUtils::init`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
axutils::HttpUtils::init(axutils::HttpConfig::builder().build()?)?;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

在真实应用中只应在进程启动阶段调用一次；上例使用 `no_run`，不会改变 doctest 进程的全局状态。

### `HttpUtils::is_initialized`

```rust
# #[cfg(feature = "http")]
# fn main() {
let _ = axutils::HttpUtils::is_initialized();
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

### `HttpUtils::execute`

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
let response = axutils::HttpUtils::execute(request)?;
let _ = response.status();
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

未初始化时返回 `HttpError::NotInitialized`；同步入口同样不能在 Tokio runtime 中调用。

### `HttpUtils::execute_async`

```rust,no_run
# #[cfg(all(feature = "http", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let request = axutils::HttpRequest::new(axutils::HttpMethod::Get, "https://example.com/")?;
    let response = axutils::HttpUtils::execute_async(request).await?;
    let _ = response.status();
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "tokio")))]
# fn main() {}
```

未初始化时返回 `HttpError::NotInitialized`，没有 runtime 时返回 `HttpError::RuntimeRequired`。

## Serde JSON、query 和字节快捷方法

本节的方法只在 `http + serde` 下导出。每个方法固定三个参数：第一个是 URL，第二个是
`Option<Q>` query 或 `Option<B>` JSON body，第三个是 `Option<HttpRequestOptions>`。
`Q` 和 `B` 只需实现 `serde::Serialize`，JSON 返回类型 `T` 需要实现
`serde::de::DeserializeOwned`。传入 `None` 时不发送 query/body；如果类型无法从
`None` 推断，使用 `None::<()>`。JSON 方法默认添加 `Accept: application/json`，
有 body 的方法默认添加 `Content-Type: application/json`；第三个参数中的普通同名 Header
可以覆盖客户端默认值和上述 JSON 默认值。`Authorization`、`Cookie`、`Set-Cookie` 等敏感
Header 与客户端默认值冲突时返回 `HttpError::DuplicateSensitiveHeader`，不会静默覆盖。JSON
序列化、query 编码和响应解码失败分别返回
`HttpError::JsonSerialize`、`QuerySerialize` 和 `JsonDeserialize`。

### HttpClient::get

发送 GET 请求，把第二个参数编码为 query 并追加到 URL，返回反序列化后的 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut query = BTreeMap::new();
query.insert("page", "1");
let reply: BTreeMap<String, bool> =
    client.get("https://example.com/health", Some(query), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::post

发送 POST 请求，把第二个参数序列化为 JSON body，返回反序列化后的 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "demo");
let reply: BTreeMap<String, bool> =
    client.post("https://example.com/items", Some(body), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::delete

发送 DELETE 请求，把第二个参数编码为 query 并返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut query = BTreeMap::new();
query.insert("id", "42");
let reply: BTreeMap<String, bool> =
    client.delete("https://example.com/items", Some(query), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::patch

发送 PATCH 请求，把第二个参数序列化为 JSON body 并返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "updated");
let reply: BTreeMap<String, bool> =
    client.patch("https://example.com/items/42", Some(body), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::put

发送 PUT 请求，把第二个参数序列化为 JSON body 并返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "replacement");
let reply: BTreeMap<String, bool> =
    client.put("https://example.com/items/42", Some(body), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::options

发送 OPTIONS 请求，把第二个参数编码为 query 并返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut query = BTreeMap::new();
query.insert("resource", "items");
let reply: BTreeMap<String, bool> =
    client.options("https://example.com/items", Some(query), None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::head

发送 HEAD 请求，把第二个参数编码为 query 并尝试按 JSON 解码响应体。符合 HTTP 语义的 HEAD
响应通常没有 body；此时应使用 `head_bytes` 读取空字节，只有服务端确实返回可解析 body
时才使用本方法。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let reply: std::collections::BTreeMap<String, bool> =
    client.head("https://example.com/health", None::<()>, None)?;
let _ = reply;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::get_bytes

发送 GET 请求并返回原始响应体字节，不进行 JSON 反序列化；第二个参数是 query。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let bytes = client.get_bytes("https://example.com/image", None::<()>, None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::post_bytes

发送 POST 请求，把第二个参数序列化为 JSON body，并返回原始响应体字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "demo");
let bytes = client.post_bytes("https://example.com/items", Some(body), None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::delete_bytes

发送 DELETE 请求，把第二个参数编码为 query，并返回原始响应体字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let bytes = client.delete_bytes("https://example.com/items/42", None::<()>, None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::patch_bytes

发送 PATCH 请求，把第二个参数序列化为 JSON body，并返回原始响应体字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "updated");
let bytes = client.patch_bytes("https://example.com/items/42", Some(body), None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::put_bytes

发送 PUT 请求，把第二个参数序列化为 JSON body，并返回原始响应体字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use std::collections::BTreeMap;
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let mut body = BTreeMap::new();
body.insert("name", "replacement");
let bytes = client.put_bytes("https://example.com/items/42", Some(body), None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::options_bytes

发送 OPTIONS 请求，把第二个参数编码为 query，并返回原始响应体字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let bytes = client.options_bytes("https://example.com/items", None::<()>, None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### HttpClient::head_bytes

发送 HEAD 请求，把第二个参数编码为 query，并返回原始响应体字节；符合 HTTP 语义的空 body
会得到空的 `Vec<u8>`。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};

let client = HttpClient::new(HttpConfig::default())?;
let bytes = client.head_bytes("https://example.com/health", None::<()>, None)?;
let _ = bytes;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

## HttpClient 异步快捷方法

异步快捷方法只在 `http + serde + tokio` 下导出，方法名在同步版本后追加 `_async`；
它们仍使用相同的三个参数，并要求调用方已经提供 Tokio runtime。

### `HttpClient::get_async`

把 query 编码到 URL 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.get_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::post_async`

把 body 序列化为 JSON 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.post_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::delete_async`

把 query 编码到 URL 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.delete_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::patch_async`

把 body 序列化为 JSON 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.patch_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::put_async`

把 body 序列化为 JSON 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.put_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::options_async`

把 query 编码到 URL 并异步返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.options_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::head_async`

异步发送 HEAD 并按 JSON 解码；合规的空 body 应使用 `head_bytes_async`。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _: std::collections::BTreeMap<String, bool> =
        client.head_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::get_bytes_async`

异步发送 GET 并返回原始字节，不进行 JSON 解码。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.get_bytes_async("https://example.com/image", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::post_bytes_async`

异步发送 POST，把 body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.post_bytes_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::delete_bytes_async`

异步发送 DELETE，把 query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.delete_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::patch_bytes_async`

异步发送 PATCH，把 body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.patch_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::put_bytes_async`

异步发送 PUT，把 body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.put_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::options_bytes_async`

异步发送 OPTIONS，把 query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.options_bytes_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpClient::head_bytes_async`

异步发送 HEAD，把 query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let client = axutils::HttpClient::new(axutils::HttpConfig::default())?;
    let _ = client.head_bytes_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

## HttpUtils 的 Serde 快捷方法

`HttpUtils` 的快捷方法复用一次初始化的全局 `HttpClient`，签名、参数、默认 Header、
错误和返回值与实例方法一致。调用前应在进程启动阶段调用 `HttpUtils::init`；下面示例使用
`no_run`，只验证类型，不执行网络或全局初始化。

### `HttpUtils::get`

同步发送 GET，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::get("https://example.com/health", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::post`

同步发送 POST，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::post("https://example.com/items", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::delete`

同步发送 DELETE，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::delete("https://example.com/items", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::patch`

同步发送 PATCH，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::patch("https://example.com/items/42", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::put`

同步发送 PUT，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::put("https://example.com/items/42", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::options`

同步发送 OPTIONS，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::options("https://example.com/items", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::head`

同步发送 HEAD，第二个参数为 query，并尝试返回 JSON；合规空 body 应使用 `head_bytes`。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _: std::collections::BTreeMap<String, bool> =
    axutils::HttpUtils::head("https://example.com/health", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::get_bytes`

同步发送 GET 并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::get_bytes("https://example.com/image", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::post_bytes`

同步发送 POST，body 按 JSON 序列化，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::post_bytes("https://example.com/items", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::delete_bytes`

同步发送 DELETE，query 编码到 URL，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::delete_bytes("https://example.com/items/42", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::patch_bytes`

同步发送 PATCH，body 按 JSON 序列化，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::patch_bytes("https://example.com/items/42", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::put_bytes`

同步发送 PUT，body 按 JSON 序列化，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::put_bytes("https://example.com/items/42", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::options_bytes`

同步发送 OPTIONS，query 编码到 URL，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::options_bytes("https://example.com/items", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

### `HttpUtils::head_bytes`

同步发送 HEAD，query 编码到 URL，并返回原始响应字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
let _ = axutils::HttpUtils::head_bytes("https://example.com/health", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

## HttpUtils 异步 Serde 快捷方法

异步全局快捷方法只在 `http + serde + tokio` 下导出，使用 `_async` 和
`_bytes_async` 后缀，并要求调用方已有 Tokio runtime。

### `HttpUtils::get_async`

异步 GET，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::get_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::post_async`

异步 POST，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::post_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::delete_async`

异步 DELETE，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::delete_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::patch_async`

异步 PATCH，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::patch_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::put_async`

异步 PUT，第二个参数为 JSON body，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::put_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::options_async`

异步 OPTIONS，第二个参数为 query，返回 JSON 类型。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::options_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::head_async`

异步 HEAD，第二个参数为 query，并尝试返回 JSON；合规空 body 应使用 `head_bytes_async`。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _: std::collections::BTreeMap<String, bool> =
        axutils::HttpUtils::head_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::get_bytes_async`

异步 GET 并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::get_bytes_async("https://example.com/image", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::post_bytes_async`

异步 POST，body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::post_bytes_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::delete_bytes_async`

异步 DELETE，query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::delete_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::patch_bytes_async`

异步 PATCH，body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::patch_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::put_bytes_async`

异步 PUT，body 按 JSON 序列化，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::put_bytes_async("https://example.com/items/42", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::options_bytes_async`

异步 OPTIONS，query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::options_bytes_async("https://example.com/items", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

### `HttpUtils::head_bytes_async`

异步 HEAD，query 编码到 URL，并返回原始字节。

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), axutils::HttpError> {
    let _ = axutils::HttpUtils::head_bytes_async("https://example.com/health", None::<()>, None).await?;
    Ok(())
}
# #[cfg(not(all(feature = "http", feature = "serde", feature = "tokio")))]
# fn main() {}
~~~

## HttpRequestOptions

HttpRequestOptions 是三参数 JSON/字节快捷方法的第三个参数。它只覆盖当前调用的 Header、
timeout、RetryPolicy 和 DeduplicationPolicy；普通 Header 未提供时继续使用客户端配置，敏感
Header 与客户端默认值冲突时返回 `DuplicateSensitiveHeader`。其中 `with_max_retries` 的值是
包括首次请求在内的最大总尝试次数，`1` 表示不重试。它实现
Clone + Debug + Default + Eq + PartialEq，Debug 不显示 Header 值。

### HttpRequestOptions::new

~~~rust
# #[cfg(feature = "http")]
# fn main() {
let options = axutils::HttpRequestOptions::new();
assert!(options.headers().is_empty());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::with_header

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new()
    .with_header("x-request-id", "demo")?;
assert_eq!(options.headers().get("x-request-id"), Some(&b"demo"[..]));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::append_header

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new()
    .append_header("accept", "application/json")?;
assert!(options.headers().contains("accept"));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::with_timeout

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new()
    .with_timeout(std::time::Duration::from_secs(5))?;
assert_eq!(options.timeout(), Some(std::time::Duration::from_secs(5)));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::with_retry_policy

~~~rust
# #[cfg(feature = "http")]
# fn main() {
let options = axutils::HttpRequestOptions::new()
    .with_retry_policy(axutils::RetryPolicy::new());
assert!(options.retry_policy().is_some());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::with_max_retries

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new().with_max_retries(2)?;
assert_eq!(options.retry_policy().unwrap().max_retries(), 2);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::with_deduplication_policy

~~~rust
# #[cfg(feature = "http")]
# fn main() {
let options = axutils::HttpRequestOptions::new()
    .with_deduplication_policy(axutils::DeduplicationPolicy::disabled());
assert!(!options.deduplication_policy().unwrap().is_enabled());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::headers

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new().with_header("accept", "application/json")?;
assert!(options.headers().contains("accept"));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::timeout

~~~rust
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
let options = axutils::HttpRequestOptions::new()
    .with_timeout(std::time::Duration::from_secs(1))?;
assert_eq!(options.timeout(), Some(std::time::Duration::from_secs(1)));
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::retry_policy

~~~rust
# #[cfg(feature = "http")]
# fn main() {
let options = axutils::HttpRequestOptions::new()
    .with_retry_policy(axutils::RetryPolicy::new());
assert_eq!(options.retry_policy().unwrap().max_retries(), 3);
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

### HttpRequestOptions::deduplication_policy

~~~rust
# #[cfg(feature = "http")]
# fn main() {
let options = axutils::HttpRequestOptions::new()
    .with_deduplication_policy(axutils::DeduplicationPolicy::disabled());
assert!(!options.deduplication_policy().unwrap().is_enabled());
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
~~~

## 测试和发布边界

仓库中的 `tests/http.rs` 只使用 loopback TCP fixture，覆盖同步/异步执行、重试、状态码、响应
上限、Header 校验、single-flight 和 TTL 缓存，不会访问外部 relay。feature fixture 验证无 feature、
`tokio` only、`http`、`http + tokio` 以及负向公共 API 矩阵，并检查 Rustls 依赖边界。

`docs/examples/http.md` 随 crate 发布；发布前应使用 scratch crate 通过 `#![doc = include_str!(...)]`
和 `cargo test --doc` 验证文档中的 Rust 代码块，并运行 `cargo package --list` 确认文档进入发布包。
