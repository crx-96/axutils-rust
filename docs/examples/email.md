# email 模块与 EmailUtils 使用文档

> 需要 `lettre` feature；异步发送还需要同时启用 `tokio`。本模块只提供受限的 SMTP 配置、
> 消息构造、连接池客户端和一次初始化的静态入口，示例使用保留域名和占位凭据，不连接真实
> relay。

## 导出内容

公开模块路径：

- `axutils::email`：邮件领域类型的直接模块路径；
- `axutils::utils::email_utils`：`EmailUtils` 的公开子模块路径；
- `axutils::utils` 公开，但邮件类型的实际可访问路径见下表。

`EmailClient`、`EmailConfig`、`EmailSecurity`、`EmailMessage`、`EmailBody`、`EmailError` 和
`EmailTransportErrorKind` 均需要 `lettre` feature，并同时支持 crate 根和领域模块路径：

- 推荐：`axutils::EmailClient`、`axutils::EmailConfig`、`axutils::EmailSecurity`、
  `axutils::EmailMessage`、`axutils::EmailBody`、`axutils::EmailError`、
  `axutils::EmailTransportErrorKind`；
- 次级：`axutils::email::EmailClient`、`axutils::email::EmailConfig`、
  `axutils::email::EmailSecurity`、`axutils::email::EmailMessage`、`axutils::email::EmailBody`、
  `axutils::email::EmailError`、`axutils::email::EmailTransportErrorKind`。

`EmailUtils` 也需要 `lettre`，支持：

- 推荐：`axutils::EmailUtils`；
- `axutils::utils::EmailUtils`；
- `axutils::utils::email_utils::EmailUtils`。

`EmailSecurity` 的变体为 `ImplicitTls` 和 `StartTls`，实现 `Debug`、`Clone`、`Copy`、
`PartialEq`、`Eq`，没有 `#[non_exhaustive]`。`EmailBody` 的变体为 `Text(String)` 和
`Html(String)`，没有 `#[non_exhaustive]`；自定义 `Debug` 只显示 `kind` (`"text"` 或
`"html"`)，不显示正文。

`EmailConfig` 字段私有，无 `Clone`，实现脱敏 `Debug`；密码没有 getter。构造配置后通过
`EmailClient::new` 消费它。`EmailMessage` 字段私有，无 `Clone` 或 `Debug`，发送时会被消费。

`EmailError` 有 7 个当前变体，标记 `#[non_exhaustive]`，实现 `Clone`、`Copy`、自定义
`Debug`、`Display` 和 `std::error::Error`：

- `InvalidConfig { field: &'static str }`：配置字段类别；当前为 `host`、`port`、`username`、
  `password`、`from_email`、`from_name` 或 `timeout`；
- `InvalidMessage { field: &'static str }`：消息字段类别；当前为 `recipients`、`subject` 或
  `body`；
- `InvalidRecipient { index: usize }`：输入收件人 `Vec` 中的零基索引；
- `MessageBuild`：`lettre` 无法构建已经通过本 crate 校验的消息；
- `Transport(EmailTransportErrorKind)`：脱敏的传输分类；
- `NotInitialized`：`EmailUtils` 尚未初始化；
- `AlreadyInitialized`：`EmailUtils` 已成功初始化，不能覆盖。

`EmailTransportErrorKind` 也标记 `#[non_exhaustive]`，有 `Connection`、`Tls`、
`Authentication`、`Timeout`、`SmtpResponse`、`Network`、`Client`、`Shutdown` 八个当前变体，
实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq` 和 `Display`，但不实现 `Error`。匹配两个
错误枚举都必须保留 `_` 通配分支。

`EmailClient` 字段私有，不实现 `Clone` 或 `Debug`；当前测试证明它满足 `Send + Sync`，调用方
可以自行选择并发共享容器，但文档不承诺更强的线程或生命周期语义。模块没有公共自由函数、
trait、类型别名、常量、静态项或宏。

## 安装与启用

同步 SMTP：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre"] }
```

异步 SMTP：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

本 crate 的 `lettre` 配置使用 Rustls、`ring` 和 `webpki-roots`，不启用 native-tls/OpenSSL；
当前不支持通过关闭证书校验绕过私有 CA relay。应用侧负责选择受信任的公共证书链或另行设计
私有 CA 支持。

## 配置与消息方法

### `EmailConfig::new(host: impl Into<String>, port: u16, security: EmailSecurity, username: impl Into<String>, password: impl Into<String>, from_email: impl Into<String>) -> Result<EmailConfig, EmailError>`

- **feature**：`lettre`。
- **参数**：`host` 是 ASCII DNS 主机名；`port` 不能为 `0`；`security` 选择强制 SMTPS
  (`ImplicitTls`) 或强制 STARTTLS (`StartTls`)；`username`、`password` 和 `from_email`
  是账号与发件人信息。
- **返回值**：成功返回已校验的配置，默认 SMTP 命令超时为 30 秒；失败只返回固定字段类别。
- **示例**：构造阶段只做本地校验，不连接网络。

```rust
use axutils::{EmailConfig, EmailSecurity};

let config = EmailConfig::new(
    "smtp.example.com",
    465,
    EmailSecurity::ImplicitTls,
    "sender@example.com",
    "application-password",
    "sender@example.com",
).unwrap();
let debug = format!("{config:?}");
assert!(debug.contains("[REDACTED]"));
assert!(!debug.contains("application-password"));
```

`host` 必须非空、无首尾空白和控制字符，总长不超过 253 字节；每个 label 非空且不超过
63 字节，只含 ASCII 字母、数字和连字符，首尾不能是连字符；允许单 label 主机名，但拒绝
IP 字面量、端口、URL、路径和非 ASCII 主机名。用户名非空、无首尾空白/控制字符、最多 4 KiB；
密码非空、最多 4 KiB，不额外禁止空白或控制字符；`from_email` 非空、最多 4 KiB、无首尾
空白/控制字符且必须能被 `lettre` 解析。错误为 `InvalidConfig { field }`。

```rust
use axutils::{EmailConfig, EmailError, EmailSecurity};

assert!(matches!(
    EmailConfig::new(
        "smtp.example.com",
        0,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "placeholder-password",
        "sender@example.com",
    ),
    Err(EmailError::InvalidConfig { field: "port" })
));
```

### `EmailConfig::with_from_name(from_name: impl Into<String>) -> Result<EmailConfig, EmailError>`

- **参数**：显示名，最多 512 字节，不能空、不能有首尾空白或控制字符。
- **返回值**：消费当前配置并返回带显示名的新配置；错误为
  `InvalidConfig { field: "from_name" }`。
- **示例**：

```rust
use axutils::{EmailConfig, EmailSecurity};

let config = EmailConfig::new(
    "smtp.example.com",
    465,
    EmailSecurity::ImplicitTls,
    "sender@example.com",
    "placeholder-password",
    "sender@example.com",
)
.unwrap()
.with_from_name("Axutils")
.unwrap();
let _ = config;
```

### `EmailConfig::with_timeout(timeout: Duration) -> Result<EmailConfig, EmailError>`

- **参数**：SMTP 命令等待时间，允许 1 秒到 5 分钟（含边界）。
- **返回值**：消费当前配置并返回新配置；超出范围返回
  `InvalidConfig { field: "timeout" }`。
- **示例**：

```rust
use std::time::Duration;
use axutils::{EmailConfig, EmailSecurity};

let config = EmailConfig::new(
    "smtp.example.com",
    587,
    EmailSecurity::StartTls,
    "sender@example.com",
    "placeholder-password",
    "sender@example.com",
)
.unwrap()
.with_timeout(Duration::from_secs(45))
.unwrap();
let _ = config;
```

超时只限制 SMTP 命令等待，不会自动重试，也不改变连接池 60 秒空闲回收时间。

### `EmailMessage::text(to: Vec<String>, subject: impl Into<String>, body: impl Into<String>) -> Result<EmailMessage, EmailError>`

- **参数**：`to` 为 1–100 个收件人字符串，可是裸地址或带显示名的 mailbox；`subject` 为
  邮件主题；`body` 为纯文本正文。
- **返回值**：成功返回 `text/plain` 消息；收件人、主题或正文不合法时返回脱敏错误。
- **示例**：

```rust
use axutils::EmailMessage;

let message = EmailMessage::text(
    vec!["Receiver <receiver@example.com>".to_owned()],
    "A test message",
    "Hello from axutils.\n",
).unwrap();
let _ = message;
```

收件人逐项解析，单个最多 4 KiB，不能空、含控制字符或首尾空白；收件人失败返回零基
`InvalidRecipient { index }`。主题最多 16 KiB，拒绝控制字符以避免 header injection，但允许
空值和首尾空白；正文最多 10 MiB，允许普通换行。所有上限按 UTF-8 字节计。

```rust
use axutils::{EmailError, EmailMessage};

assert!(matches!(
    EmailMessage::text(vec!["invalid-address".to_owned()], "subject", "body"),
    Err(EmailError::InvalidRecipient { index: 0 })
));
assert!(matches!(
    EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "subject\nBcc: injected@example.com",
        "body",
    ),
    Err(EmailError::InvalidMessage { field: "subject" })
));
```

### `EmailMessage::html(to: Vec<String>, subject: impl Into<String>, body: impl Into<String>) -> Result<EmailMessage, EmailError>`

- **参数**：与 `text` 相同，但 `body` 按 `text/html` MIME 类型发送。
- **返回值**：成功返回 HTML 消息；校验失败返回与 `text` 相同的错误类别。
- **示例**：

```rust
use axutils::EmailMessage;

let message = EmailMessage::html(
    vec!["receiver@example.com".to_owned()],
    "An HTML message",
    "<p>Hello from <strong>axutils</strong>.</p>",
).unwrap();
let _ = message;
```

```rust
use axutils::{EmailError, EmailMessage};

let too_large = "x".repeat(10 * 1024 * 1024 + 1);
assert!(matches!(
    EmailMessage::html(vec!["receiver@example.com".to_owned()], "subject", too_large),
    Err(EmailError::InvalidMessage { field: "body" })
));
```

HTML 不会自动清理、转义、执行模板或生成纯文本 fallback；不可信 HTML 的处理由调用方负责。

## `EmailClient` 方法

每个 `EmailClient` 实例独立拥有同步连接池；启用 `tokio` 时还保存独立异步池配置。每个池
最多 10 条连接，空闲 60 秒回收；同步和异步同时使用时单实例最多持有两组池。构造客户端不
连接网络。

### `EmailClient::new(config: EmailConfig) -> Result<EmailClient, EmailError>`

- **参数**：消费一个已经通过 `EmailConfig::new` 校验的配置。
- **返回值**：成功返回独立客户端；失败返回脱敏配置或 transport builder 错误。
- **示例**：

```rust
use axutils::{EmailClient, EmailConfig, EmailSecurity};

let client = EmailClient::new(EmailConfig::new(
    "smtp.example.com",
    465,
    EmailSecurity::ImplicitTls,
    "sender@example.com",
    "placeholder-password",
    "sender@example.com",
).unwrap()).unwrap();
let _ = client;
```

### `EmailClient::send(&self, message: EmailMessage) -> Result<(), EmailError>`

- **参数**：消费一封已校验的 `EmailMessage`。
- **返回值**：发送成功返回 `Ok(())`；消息构建、SMTP/TLS/网络失败返回稳定的
  `EmailError` 分类。
- **示例**：发送会产生阻塞网络 I/O，文档只构造消息并取得方法指针，避免连接外部 relay。

```rust,no_run
use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

let client = EmailClient::new(EmailConfig::new(
    "smtp.example.com",
    465,
    EmailSecurity::ImplicitTls,
    "sender@example.com",
    "placeholder-password",
    "sender@example.com",
).unwrap()).unwrap();
let message = EmailMessage::text(
    vec!["receiver@example.com".to_owned()],
    "subject",
    "body",
).unwrap();
let _send: fn(&EmailClient, EmailMessage) -> Result<(), axutils::EmailError> = EmailClient::send;
let _ = (client, message);
```

不要在 Tokio worker 线程直接调用同步方法；异步服务应使用 `send_async`。

### `async EmailClient::send_async(&self, message: EmailMessage) -> Result<(), EmailError>`

- **feature**：`lettre` + `tokio`。
- **参数**：消费一封已校验的 `EmailMessage`。
- **返回值**：在调用方已有 Tokio runtime 时异步发送；没有 runtime 时返回
  `Transport(EmailTransportErrorKind::Client)`，不会 panic。
- **示例**：`no_run` 只用于编译 API；实际发送仍会连接网络。

```rust,no_run
#![cfg_attr(not(feature = "tokio"), allow(dead_code))]
# #[cfg(feature = "tokio")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), axutils::EmailError> {
    use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

    let client = EmailClient::new(EmailConfig::new(
        "smtp.example.com",
        587,
        EmailSecurity::StartTls,
        "sender@example.com",
        "placeholder-password",
        "sender@example.com",
    )?)?;
    let message = EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "subject",
        "body",
    )?;
    client.send_async(message).await
}
# #[cfg(not(feature = "tokio"))]
# fn main() {}
```

该方法不创建 runtime、不调用 `block_on`；服务端不支持 STARTTLS 时失败，不回退到明文认证。
`EmailClient` 的异步 transport 首次发送时才初始化，依赖 Tokio 的池清理任务；应在同一个
仍然存活的 runtime 中创建/首次使用并持续复用实例，不保证跨已结束 runtime 迁移。

## `EmailUtils` 方法

`EmailUtils` 是进程级一次初始化入口，成功初始化只能保留第一个客户端，不能 reset、replace
或切换账号。需要多个账号或可控生命周期时，直接持有多个 `EmailClient`。以下 `init` 和
发送示例使用 `no_run`，避免在文档测试中占用全局状态或访问网络。

### `EmailUtils::init(config: EmailConfig) -> Result<(), EmailError>`

- **参数**：消费一份邮件配置。
- **返回值**：首次完整成功初始化返回 `Ok(())`；已初始化返回
  `EmailError::AlreadyInitialized`；失败配置不会占用单例。
- **示例**：

```rust,no_run
use axutils::{EmailConfig, EmailSecurity, EmailUtils};

fn initialize_once() -> Result<(), axutils::EmailError> {
    let config = EmailConfig::new(
        "smtp.example.com",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "placeholder-password",
        "sender@example.com",
    )?;
    EmailUtils::init(config)
}
```

### `EmailUtils::is_initialized() -> bool`

- **参数**：无。
- **返回值**：是否已经成功设置进程级客户端。
- **示例**：

```rust
use axutils::EmailUtils;

let initialized = EmailUtils::is_initialized();
let _ = initialized;
```

### `EmailUtils::send(message: EmailMessage) -> Result<(), EmailError>`

- **参数**：消费邮件消息。
- **返回值**：未初始化时返回 `NotInitialized`；已初始化时转发到同步客户端，可能发生
  阻塞网络 I/O。
- **示例**：以下代码只编译调用关系，不连接 relay。

```rust,no_run
use axutils::{EmailMessage, EmailUtils};

fn send_once() -> Result<(), axutils::EmailError> {
    let message = EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "subject",
        "body",
    )?;
    EmailUtils::send(message)
}
```

### `async EmailUtils::send_async(message: EmailMessage) -> Result<(), EmailError>`

- **feature**：`lettre` + `tokio`。
- **参数**：消费邮件消息。
- **返回值**：在调用方已有 Tokio runtime 中异步转发；未初始化返回 `NotInitialized`，无
  runtime 时返回传输客户端错误。
- **示例**：

```rust,no_run
# #[cfg(feature = "tokio")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), axutils::EmailError> {
    use axutils::{EmailMessage, EmailUtils};

    let message = EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "subject",
        "body",
    )?;
    EmailUtils::send_async(message).await
}
# #[cfg(not(feature = "tokio"))]
# fn main() {}
```

## 错误分类和安全边界

`EmailTransportErrorKind` 的分类来源如下：`Connection` 表示连接建立失败，`Tls` 表示 TLS
握手或证书校验失败，`Authentication` 表示常见认证失败响应，`Timeout` 表示超过命令超时，
`SmtpResponse` 表示其他失败响应，`Network` 表示无法进一步分类的网络 I/O，`Client` 表示
客户端配置/协议状态错误，`Shutdown` 表示连接池已关闭。分类不携带 SMTP 原始响应、用户名、
主机名或凭据。

```rust
use axutils::{EmailError, EmailTransportErrorKind};

let error = EmailError::Transport(EmailTransportErrorKind::Tls);
match error {
    EmailError::Transport(kind) => assert_eq!(kind.to_string(), "tls"),
    _ => unreachable!(),
}
```

因为两个错误枚举都为 `#[non_exhaustive]`，跨版本匹配必须保留通配分支：

```rust
use axutils::EmailTransportErrorKind;

let kind = EmailTransportErrorKind::Network;
let label = match kind {
    EmailTransportErrorKind::Connection => "connection",
    EmailTransportErrorKind::Tls => "tls",
    EmailTransportErrorKind::Authentication => "authentication",
    EmailTransportErrorKind::Timeout => "timeout",
    EmailTransportErrorKind::SmtpResponse => "smtp response",
    EmailTransportErrorKind::Network => "network",
    EmailTransportErrorKind::Client => "client",
    EmailTransportErrorKind::Shutdown => "shutdown",
    _ => "other",
};
assert_eq!(label, "network");
```

本模块不支持附件、抄送、密送、模板、DKIM、OAuth2、自动重试或队列；HTML 不会自动清理，也
不会生成纯文本 fallback。错误和 `Debug` 输出不应回显密码、地址、主题、正文、SMTP/TLS
原始错误或完整主机名；应用日志仍需自行避免记录敏感输入。真实 SMTP 测试固定为 ignored，
没有用户明确授权时不要执行。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
