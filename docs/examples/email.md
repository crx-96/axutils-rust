# 邮件

邮件能力按传输方式分为两个 feature：同步 SMTP 使用 `email`，在已有 Tokio runtime
中异步发送时使用 `email-async`。后者包含前者；`tokio` feature 本身不启用邮件能力。

```toml
[dependencies]
axutils = { version = "1.0", features = ["email"] }
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

## 异步发送

异步发送要求启用 `email-async`，并由调用方提供仍然存活的 Tokio runtime。client 可以在 runtime
外构造；首次 `send_async` 会在当前 runtime 中建立异步 transport，后续异步发送应在同一个仍存活的
runtime 中持续复用，不能假设可以跨已结束的 runtime 迁移。

```toml
[dependencies]
axutils = { version = "1.0", features = ["email-async"] }
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
