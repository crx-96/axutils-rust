# HTTP

HTTP 是显式启用的客户端领域；类型和错误都从 `axutils::http` 导入，全局生命周期入口仅从
`axutils::utils::HttpUtils` 导入。不要使用 crate 根路径或实现叶模块路径。

## 启用

| 需要的能力 | `axutils` feature | 说明 |
| --- | --- | --- |
| 同步 HTTP | `http` | 使用 `ureq + url`；不会编译 `reqwest`。 |
| 异步 HTTP | `http-async` | 包含 `http`、`reqwest` 和 HTTP 所需的最小 Tokio runtime 能力。 |
| JSON body/query/response | `http-json` | 包含 `http` 和 Serde JSON/query 支持。 |
| 异步 JSON | `http-async`, `http-json` | 两个能力组合；调用方仍应直接依赖 Tokio。 |

同步客户端：

```toml
[dependencies]
axutils = { version = "1.0", features = ["http"] }
```

异步 JSON 客户端：

```toml
[dependencies]
axutils = { version = "1.0", features = ["http-async", "http-json"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

单独启用 `tokio` 不会公开任何 HTTP API；单独启用 `http` 也不会编译异步 transport。
客户端使用 Rustls，关闭系统代理、自动重定向、自动压缩和 transport 的隐式重试；不提供跳过
证书或 hostname 校验的选项。调用方仍须自行限制允许的主机、出口网络和 DNS 重绑定风险，库不将
客户端 URL 校验等同于 SSRF 防护。

## 导入与同步实例

`HttpClient` 可以有多个、各自独立的连接池和去重缓存。未设 `base_url` 时仅接受绝对 HTTP/HTTPS
URL；设定基地址后，相对路径会在该基地址下解析，而请求自己的绝对 URL 优先。

```rust,no_run
use std::time::Duration;

use axutils::http::{
    DeduplicationPolicy, HttpClient, HttpConfig, HttpError, HttpMethod, HttpRequest, RetryPolicy,
};

fn main() -> Result<(), HttpError> {
    let retry = RetryPolicy::new()
        // 名称沿用历史 API，但 2 表示最多两次总尝试，包含首次请求。
        .with_max_retries(2)?
        .with_backoff(Duration::from_millis(50), Duration::from_millis(500))?;
    let deduplication = DeduplicationPolicy::with_completed_ttl(
        Duration::from_secs(10),
        64,
        32,
        256 * 1024,
    )?;
    let config = HttpConfig::builder()
        .base_url("https://api.example.invalid/")?
        .request_timeout(Duration::from_secs(5))?
        .connect_timeout(Duration::from_secs(2))?
        .retry_policy(retry)
        .deduplication_policy(deduplication)
        .with_default_header("accept", "application/json")?
        .build()?;
    let client = HttpClient::new(config)?;

    let request = HttpRequest::new(HttpMethod::Get, "/health")?;
    let response = client.execute(request)?;
    if !response.is_success() {
        return Ok(());
    }
    let _body = response.body();
    Ok(())
}
```

`HttpRequest` 可以按请求覆盖 timeout、重试策略和去重策略。`HttpHeaders::set` 替换同名项；普通
Header 可用 `append` 保留顺序，但 `Authorization`、`Cookie`、`Set-Cookie` 不能形成重复项。
不要把敏感 Header 放入默认 Header 后再无条件跨 origin 调用：跨 origin 的绝对 URL 会丢弃默认
敏感 Header；请求本身显式设置的敏感 Header 仍会发送。

```rust,no_run
use std::time::Duration;

use axutils::http::{HttpClient, HttpConfig, HttpError, HttpMethod, HttpRequest};

fn main() -> Result<(), HttpError> {
    let token = "runtime-token";
    let client = HttpClient::new(
        HttpConfig::builder()
            .base_url("https://api.example.invalid/")?
            .max_request_body_bytes(128 * 1024)?
            .max_response_body_bytes(512 * 1024)?
            .build()?,
    )?;
    let request = HttpRequest::new(HttpMethod::Post, "/orders")?
        .with_header("authorization", format!("Bearer {token}"))?
        .with_header("content-type", "application/json")?
        .with_body(br#"{"item":"book"}"#.to_vec())?
        .with_timeout(Duration::from_secs(3))?;
    let response = client.execute(request)?;
    let _status = response.status(); // 4xx/5xx 仍是正常 HttpResponse。
    Ok(())
}
```

## JSON 与异步

`http-json` 为 `HttpClient` 提供 `get`、`post`、`put`、`patch`、`delete`、`options`、`head` 及
对应 `*_bytes` 方法；`HttpResponse::json` 也由该 feature 提供。`http-async` 只增加显式
`execute_async`；异步 JSON 快捷方法要求同时启用 `http-async + http-json`，并以 `_async` 结尾。
异步入口必须在调用方建立的 Tokio runtime 中运行，库不会创建 runtime 或调用 `block_on`。

```rust,no_run
use axutils::http::{HttpClient, HttpConfig, HttpError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Health {
    ok: bool,
}

#[derive(Serialize)]
struct CreateOrder<'a> {
    item: &'a str,
}

#[tokio::main]
async fn main() -> Result<(), HttpError> {
    let client = HttpClient::new(
        HttpConfig::builder()
            .base_url("https://api.example.invalid/")?
            .build()?,
    )?;
    let health: Health = client.get_async("/health", None::<()>, None).await?;
    let _created: Health = client
        .post_async("/orders", Some(CreateOrder { item: "book" }), None)
        .await?;
    assert!(health.ok);
    Ok(())
}
```

启用 `http-async` 后，在 Tokio runtime 内调用同步 `execute` 会返回
`HttpError::BlockingInAsyncRuntime`；异步场景应使用 `execute_async` 或 `_async` JSON 方法。
仅启用 `http` 时没有 runtime 检测，同步入口会直接阻塞当前线程，不能在异步服务中把它当作
非阻塞调用。

## 全局生命周期入口

只有需要一个进程级默认客户端时才使用 `HttpUtils`。它只负责 `init`、状态查询和实例访问；所有
业务请求都由 `HttpUtils::client()?` 返回的 `HttpClient` 完成。首次成功初始化后不可替换。

```rust,no_run
use axutils::{
    http::{HttpConfig, HttpError, HttpMethod, HttpRequest},
    utils::HttpUtils,
};

fn main() -> Result<(), HttpError> {
    HttpUtils::init(HttpConfig::builder().base_url("https://api.example.invalid/")?.build()?)?;
    assert!(HttpUtils::is_initialized());

    let response = HttpUtils::client()?.execute(HttpRequest::new(HttpMethod::Get, "/health")?)?;
    let _body = response.body();
    Ok(())
}
```

重复初始化返回 `HttpError::AlreadyInitialized`，尚未初始化时 `client()` 返回
`HttpError::NotInitialized`。初始化构造失败不会占用全局槽位。

## 边界、重试与缓存

- URL 仅接受 `http`/`https`，拒绝用户信息、片段、控制字符和不安全 scheme。Header 名和值、请求体、
  响应体均有本地上限；`HttpError` 不保留 URL、Header 值、body 或第三方错误文本。
- 默认总 timeout 为 30 秒、连接 timeout 为 10 秒。`RetryPolicy::max_retries` 是总网络尝试数，默认
  3，传入 1 禁用自动重试；只有安全方法默认参与重试，非幂等写入需显式允许并自行确认幂等语义。
- 默认仅合并无 body 的 `GET`/`HEAD`/`OPTIONS` in-flight 请求。带 body 或写请求须在请求级显式
  设置去重策略。follower 超时只返回 `CoalescedWaitTimeout`，不会取消 leader；leader 取消时 follower
  得到 `CoalescedRequestCancelled`。
- 完成缓存必须显式使用 `DeduplicationPolicy::with_completed_ttl` 开启，且只缓存满足安全条件的成功
  无 body `GET`/`HEAD`。带认证、Cookie、Range、条件请求或禁止缓存指令的请求不会进入缓存。
- 传输、TLS、超时、大小限制和本地校验才返回 `HttpError`；HTTP 4xx/5xx 通过 `HttpResponse` 返回。
  匹配 `HttpError` 或 `HttpTransportErrorKind` 时应保留 wildcard，因为它们是 `non_exhaustive`。
