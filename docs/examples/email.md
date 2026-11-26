# 邮件

邮件能力按传输方式分为两个 feature：同步 SMTP 使用 `email`，在已有 Tokio runtime
中异步发送时使用 `email-async`。后者包含前者；`tokio` feature 本身不启用邮件能力。

```toml
[dependencies]
axutils = { version = "3.0", features = ["email"] }
```

## 实例 API

配置在本地校验，`EmailClient::new` 不访问网络。以下示例使用占位地址；实际密码应从受控配置或
密钥管理系统获取，绝不能写入源码、日志或错误消息。

```rust,no_run
use axutils::email::{EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity};

fn build_client() -> Result<EmailClient, EmailError> {
    let config = EmailConfig::new(
        "smtp.example.invalid",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.invalid",
        "configured-outside-source",
        "sender@example.invalid",
    )?;
    EmailClient::new(config)
}

fn message() -> Result<EmailMessage, EmailError> {
    EmailMessage::text(
        vec!["recipient@example.invalid".to_owned()],
        "Status update",
        "The job completed.",
    )
}

fn send() -> Result<(), EmailError> {
    build_client()?.send(message()?)
}
```

`send` 会连接 SMTP 服务并执行 I/O，因此应由应用在适当的重试、超时和审计边界内调用。输入、收件人
和服务端错误均映射为 `axutils::email::EmailError`；不要将原始凭据或完整邮件正文拼接到错误输出。

每个同步或异步连接池最多保留 10 条空闲连接，空闲达到 60 秒后由池清理。这不限制正在发送的
连接数，应用应自行限制发送并发。同步和异步发送都以 DATA 正文结束后的最终 2xx 回复作为
SMTP 服务端已接受邮件的依据。最终 3xx 仅表示需要继续交互，返回 `SmtpResponse`。
SMTP 认证拒绝归为 `Authentication`，其他已解析的 4xx/5xx 拒绝归为 `SmtpResponse`，错误中不保留服务端原始文本。

## 异步发送

异步发送要求启用 `email-async`，并由调用方提供仍然存活的 Tokio runtime。client 可以在 runtime
外构造和析构。首次 `send_async` 按配置建立隐式 TLS 连接或强制升级 STARTTLS，随后认证。
后续异步发送应在同一个仍存活的 runtime 中复用连接。该 runtime 需要启用 I/O 和 time driver。

`EmailConfig::with_timeout` 默认 30 秒，允许 1 秒至 5 分钟。同步 `send` 为底层连接和命令 I/O
分别设置时限，一封邮件的多个操作可能累计超过该值。`send_async` 在本地消息构造后给整次传输
一个共同的预算，包括获取连接、TLS、认证、提交正文和等待最终响应。预算耗尽返回
`EmailError::Transport(EmailTransportErrorKind::Timeout)`。每次发送独占一条连接。收到最终 2xx 后，
连接可进入空闲池，下次发送先用 NOOP 探活。最终回复异常的连接会丢弃。对于 4xx/5xx，Lettre
可能先尝试 QUIT，这次交互也受同一预算限制。超时或取消会直接释放在途连接，即使在 runtime 外
丢弃正在等待回复的 future，也无需等待 QUIT。空闲连接到期由定时器直接释放。若在提交正文后失去
回复，服务端可能已经接受邮件，客户端不会自动重发。

```toml
[dependencies]
axutils = { version = "3.0", features = ["email-async"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust,no_run
use axutils::email::{EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity};

async fn send_async() -> Result<(), EmailError> {
    let config = EmailConfig::new(
        "smtp.example.invalid",
        587,
        EmailSecurity::StartTls,
        "sender@example.invalid",
        "configured-outside-source",
        "sender@example.invalid",
    )?;
    let client = EmailClient::new(config)?;
    let message = EmailMessage::text(
        vec!["recipient@example.invalid".to_owned()],
        "Status update",
        "The job completed.",
    )?;
    client.send_async(message).await
}
```

## 进程级入口

`EmailUtils` 仅管理一次初始化和实例访问；业务发送必须在取得的 `EmailClient` 上执行。成功初始化后
不能 reset 或 replace；初始化失败不占用初始化机会。

```rust,no_run
use axutils::{
    email::{EmailConfig, EmailError, EmailSecurity},
    utils::EmailUtils,
};

fn initialize() -> Result<(), EmailError> {
    let config = EmailConfig::new(
        "smtp.example.invalid",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.invalid",
        "configured-outside-source",
        "sender@example.invalid",
    )?;
    EmailUtils::init(config)?;
    let _client = EmailUtils::client()?;
    Ok(())
}
```

在需要多个 SMTP 配置、隔离租户或可控生命周期时，直接持有多个 `EmailClient`，不要使用全局入口。
