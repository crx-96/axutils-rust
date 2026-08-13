use std::time::Duration;

#[cfg(all(feature = "lettre", feature = "tokio"))]
use std::sync::OnceLock;

use lettre::{
    transport::smtp::{authentication::Credentials, PoolConfig},
    SmtpTransport, Transport,
};

use super::{config::EmailConfig, error::EmailError, message::EmailMessage};

#[cfg(all(feature = "lettre", feature = "tokio"))]
use super::error::EmailTransportErrorKind;

#[cfg(all(feature = "lettre", feature = "tokio"))]
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

const POOL_MAX_SIZE: u32 = 10;
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// 可独立配置和复用连接池的 SMTP 邮件客户端。
///
/// 每个实例消费一份 [`EmailConfig`] 并拥有独立的同步连接池；启用 `lettre` 与 `tokio` 时还
/// 保存独立的 Tokio 异步连接池配置，并在首次异步发送时于调用方的 runtime 中初始化连接池。
/// 构造实例不会建立网络连接或要求 Tokio runtime，实际连接只在发送时发生。每个池最多 10
/// 条连接、空闲 60 秒回收；同时启用同步和异步时单实例最多持有两组池，调用方应按账号
/// 实例数量和并发量控制总资源。异步 transport 的池清理任务依赖 Tokio runtime；同一个
/// `EmailClient` 应在同一个仍然存活的 Tokio runtime 中创建/首次使用并持续复用，不保证跨
/// 已结束 runtime 的迁移。
pub struct EmailClient {
    from: lettre::message::Mailbox,
    transport: SmtpTransport,
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    async_config: AsyncTransportConfig,
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    async_transport: OnceLock<Result<AsyncSmtpTransport<Tokio1Executor>, EmailError>>,
}

impl EmailClient {
    /// 消费已校验配置，创建一个不访问网络的 SMTP 客户端。
    ///
    /// 根据 [`crate::EmailSecurity`] 选择强制 SMTPS 或强制 STARTTLS，显式设置端口、凭据、
    /// 命令超时和同步连接池上限；同时启用异步 feature 时保存经过校验的异步 transport
    /// 配置，并由首次 `send_async` 在调用方 Tokio runtime 中完成异步连接池初始化。构造
    /// 失败时不会留下可发送的半初始化客户端。首次异步使用后应在同一个仍然存活的 Tokio
    /// runtime 中继续复用该实例；不要把它迁移到已经结束的 runtime。
    ///
    /// # Errors
    ///
    /// 返回配置或同步 transport builder 的脱敏错误；不会返回底层 SMTP 错误文本，也不会
    /// 因为构造客户端而建立网络连接或要求 Tokio runtime。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailClient, EmailConfig, EmailSecurity};
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let config = EmailConfig::new(
    ///     "smtp.example.com",
    ///     465,
    ///     EmailSecurity::ImplicitTls,
    ///     "sender@example.com",
    ///     "application-password",
    ///     "sender@example.com",
    /// )?;
    /// let _client = EmailClient::new(config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(config: EmailConfig) -> Result<Self, EmailError> {
        let from = config.mailbox();
        #[cfg(all(feature = "lettre", feature = "tokio"))]
        let async_config = AsyncTransportConfig::from_config(&config);
        let transport = build_sync_transport(&config)?;

        Ok(Self {
            from,
            transport,
            #[cfg(all(feature = "lettre", feature = "tokio"))]
            async_config,
            #[cfg(all(feature = "lettre", feature = "tokio"))]
            async_transport: OnceLock::new(),
        })
    }

    /// 同步发送一封邮件。
    ///
    /// 该方法消费 [`EmailMessage`]，在当前线程执行阻塞 SMTP I/O，并复用客户端连接池；不要
    /// 直接在 Tokio worker 线程调用它，异步服务应使用 `send_async`。发送失败只返回
    /// 脱敏后的稳定错误分类，不包含主题、正文、地址或 SMTP 服务端原始文本。
    ///
    /// # Errors
    ///
    /// 如果消息无法转换为 `lettre` 消息或 SMTP/TLS/网络传输失败，返回稳定的
    /// [`EmailError`](crate::EmailError) 分类；错误不会暴露原始服务端响应。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let client = EmailClient::new(EmailConfig::new(
    ///     "smtp.example.com",
    ///     465,
    ///     EmailSecurity::ImplicitTls,
    ///     "sender@example.com",
    ///     "application-password",
    ///     "sender@example.com",
    /// )?)?;
    /// let message = EmailMessage::text(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "subject",
    ///     "body",
    /// )?;
    /// // 发送会产生网络 I/O；这里只取得方法类型，避免 doctest 连接外部 relay。
    /// let _send: fn(&EmailClient, EmailMessage) -> Result<(), axutils::EmailError> =
    ///     EmailClient::send;
    /// let _ = (client, message);
    /// # Ok(())
    /// # }
    /// ```
    pub fn send(&self, message: EmailMessage) -> Result<(), EmailError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = message.into_lettre_from(&self.from).and_then(|message| {
            self.transport
                .send(&message)
                .map(|_| ())
                .map_err(|error| EmailError::from_smtp(&error))
        });
        #[cfg(feature = "tracing")]
        crate::tracing::email::record_send("sync", &result, started);
        result
    }

    /// 在调用方已有的 Tokio runtime 中异步发送一封邮件。
    ///
    /// 该方法仅在同时启用 `lettre` 与 `tokio` feature 时导出；它消费消息、复用独立异步
    /// 连接池且不会创建 runtime 或调用 `block_on`。如果调用方没有处于 Tokio runtime，返回
    /// `EmailTransportErrorKind::Client`，不会 panic。服务端不支持 STARTTLS 时发送失败，
    /// 不会回退到明文认证。
    ///
    /// # Errors
    ///
    /// 如果消息无法转换为 `lettre` 消息或异步 SMTP/TLS/网络传输失败，返回稳定的
    /// [`EmailError`](crate::EmailError) 分类；错误不会暴露原始服务端响应。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "lettre", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::EmailError> {
    /// use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};
    ///
    /// let client = EmailClient::new(EmailConfig::new(
    ///     "smtp.example.com",
    ///     587,
    ///     EmailSecurity::StartTls,
    ///     "sender@example.com",
    ///     "application-password",
    ///     "sender@example.com",
    /// )?)?;
    /// let message = EmailMessage::text(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "subject",
    ///     "body",
    /// )?;
    /// let _send_async = EmailClient::send_async;
    /// let _ = (client, message);
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    pub async fn send_async(&self, message: EmailMessage) -> Result<(), EmailError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = self.send_async_inner(message).await;
        #[cfg(feature = "tracing")]
        crate::tracing::email::record_send("async", &result, started);
        result
    }

    #[cfg(all(feature = "lettre", feature = "tokio"))]
    async fn send_async_inner(&self, message: EmailMessage) -> Result<(), EmailError> {
        let message = message.into_lettre_from(&self.from)?;
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(EmailError::Transport(EmailTransportErrorKind::Client));
        }

        let async_transport = self.async_transport.get_or_init(|| {
            let result = build_async_transport(&self.async_config);
            #[cfg(feature = "tracing")]
            crate::tracing::email::record_transport_init(result.as_ref().map(|_| ()));
            result
        });

        match async_transport {
            Ok(transport) => transport
                .send(message)
                .await
                .map(|_| ())
                .map_err(|error| EmailError::from_smtp(&error)),
            Err(error) => Err(*error),
        }
    }
}

fn pool_config() -> PoolConfig {
    PoolConfig::new()
        .max_size(POOL_MAX_SIZE)
        .idle_timeout(POOL_IDLE_TIMEOUT)
}

fn build_sync_transport(config: &EmailConfig) -> Result<SmtpTransport, EmailError> {
    let builder = match config.security {
        super::EmailSecurity::ImplicitTls => SmtpTransport::relay(&config.host),
        super::EmailSecurity::StartTls => SmtpTransport::starttls_relay(&config.host),
    }
    .map_err(|error| EmailError::from_smtp(&error))?;

    Ok(builder
        .port(config.port)
        .credentials(Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ))
        .timeout(Some(config.timeout))
        .pool_config(pool_config())
        .build())
}

#[cfg(all(feature = "lettre", feature = "tokio"))]
struct AsyncTransportConfig {
    host: String,
    port: u16,
    security: super::EmailSecurity,
    username: String,
    password: String,
    timeout: Duration,
}

#[cfg(all(feature = "lettre", feature = "tokio"))]
impl AsyncTransportConfig {
    fn from_config(config: &EmailConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            security: config.security,
            username: config.username.clone(),
            password: config.password.clone(),
            timeout: config.timeout,
        }
    }
}

#[cfg(all(feature = "lettre", feature = "tokio"))]
fn build_async_transport(
    config: &AsyncTransportConfig,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, EmailError> {
    let builder = match config.security {
        super::EmailSecurity::ImplicitTls => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
        }
        super::EmailSecurity::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
        }
    }
    .map_err(|error| EmailError::from_smtp(&error))?;

    Ok(builder
        .port(config.port)
        .credentials(Credentials::new(
            config.username.clone(),
            config.password.clone(),
        ))
        .timeout(Some(config.timeout))
        .pool_config(pool_config())
        .build())
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    use std::{future::Future, sync::Arc, task::Wake};

    use super::EmailClient;
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    use super::{build_async_transport, AsyncTransportConfig};
    use crate::email::{EmailConfig, EmailSecurity};
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    use crate::email::{EmailError, EmailMessage, EmailTransportErrorKind};

    fn config(host: &str, port: u16, security: EmailSecurity) -> EmailConfig {
        EmailConfig::new(
            host,
            port,
            security,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
        .unwrap_or_else(|_| unreachable!())
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn client_is_send_sync_and_constructor_does_not_connect() {
        assert_send_sync::<EmailClient>();
        let implicit = EmailClient::new(config(
            "smtp-one.example.com",
            465,
            EmailSecurity::ImplicitTls,
        ));
        let starttls =
            EmailClient::new(config("smtp-two.example.com", 587, EmailSecurity::StartTls));
        assert!(implicit.is_ok());
        assert!(starttls.is_ok());
    }

    #[cfg(all(feature = "lettre", feature = "tokio"))]
    #[tokio::test(flavor = "current_thread")]
    async fn async_transport_is_built_inside_the_callers_runtime() {
        let config = config("smtp.example.com", 465, EmailSecurity::ImplicitTls);
        let transport = build_async_transport(&AsyncTransportConfig::from_config(&config));
        assert!(transport.is_ok());
    }

    #[cfg(all(feature = "lettre", feature = "tokio"))]
    #[test]
    fn async_send_without_runtime_returns_client_error() {
        struct NoopWaker;
        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }

        let client = EmailClient::new(config("smtp.example.com", 465, EmailSecurity::ImplicitTls))
            .unwrap_or_else(|_| unreachable!());
        let message =
            EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
                .unwrap_or_else(|_| unreachable!());
        let mut future = Box::pin(client.send_async(message));
        let waker = std::task::Waker::from(Arc::new(NoopWaker));
        let mut context = std::task::Context::from_waker(&waker);
        let result = future.as_mut().poll(&mut context);

        assert!(matches!(
            result,
            std::task::Poll::Ready(Err(EmailError::Transport(EmailTransportErrorKind::Client)))
        ));
    }
}
