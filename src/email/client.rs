//! SMTP 客户端构造、同步发送与在调用方 runtime 中延迟构造的异步发送。

use std::time::Duration;

#[cfg(feature = "email-async")]
use std::sync::OnceLock;

use lettre::{
    message::Mailbox,
    transport::smtp::{authentication::Credentials, PoolConfig},
    SmtpTransport, Transport,
};

use super::{
    config::{EmailConfig, EmailSecurity},
    error::EmailError,
    message::EmailMessage,
};
#[cfg(feature = "tracing")]
use crate::telemetry::email as email_trace;

#[cfg(feature = "email-async")]
use super::error::EmailTransportErrorKind;

#[cfg(feature = "email-async")]
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
#[cfg(feature = "email-async")]
use tokio::runtime::Handle as RuntimeHandle;

/// 每个传输池最多保留的空闲连接数，不是发送并发上限。
const POOL_MAX_IDLE_CONNECTIONS: u32 = 10;
/// 空闲连接达到此时长后，可由 Lettre 的池清理逻辑回收。
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// 可独立配置和复用连接池的 SMTP 邮件客户端。
///
/// 每个实例消费一份 [`EmailConfig`] 并拥有独立的同步连接池；启用 `email-async` 时还
/// 保存独立的 Tokio 异步连接池配置，并在首次异步发送时于调用方的 runtime 中初始化连接池。
/// 构造实例不会建立网络连接或要求 Tokio runtime，实际连接只在发送时发生。每个池最多保留
/// 10 条空闲连接，空闲达到 60 秒后由池清理；该上限不限制正在发送的连接数，发送并发由调用方
/// 控制。同时启用同步和异步时单实例持有两组池，调用方应按账号数量与并发量控制总资源。
/// 异步 transport 的池清理任务依赖 Tokio runtime；client 可在
/// runtime 外构造，但首次异步发送及后续异步复用应位于同一个仍然存活的 runtime，不保证跨
/// 已结束 runtime 的迁移。
pub struct EmailClient {
    /// 已验证的固定发件身份，每次消息转换时借用。
    from: Mailbox,
    /// 独立同步 transport，内部共享其连接池。
    transport: SmtpTransport,
    /// 首次异步发送前保留的连接配置；不持有 runtime 或网络连接。
    #[cfg(feature = "email-async")]
    async_config: AsyncTransportConfig,
    /// 在首次异步发送的 runtime 中初始化一次；成功或构造错误均被缓存。
    #[cfg(feature = "email-async")]
    async_transport: OnceLock<Result<AsyncSmtpTransport<Tokio1Executor>, EmailError>>,
}

impl EmailClient {
    /// 消费已校验配置，创建一个不访问网络的 SMTP 客户端。
    ///
    /// 根据 [`crate::email::EmailSecurity`] 选择强制 SMTPS 或强制 STARTTLS，显式设置端口、凭据、
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
    /// use axutils::email::{EmailClient, EmailConfig, EmailError, EmailSecurity};
    ///
    /// # fn main() -> Result<(), EmailError> {
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
        // 从已校验配置提取发件身份；异步部分仅复制构造参数，避免在这里要求 runtime。
        let from = config.mailbox();
        #[cfg(feature = "email-async")]
        let async_config = AsyncTransportConfig::from_config(&config);
        let transport = build_sync_transport(&config)?;

        Ok(Self {
            from,
            transport,
            #[cfg(feature = "email-async")]
            async_config,
            #[cfg(feature = "email-async")]
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
    /// [`EmailError`](crate::email::EmailError) 分类；错误不会暴露原始服务端响应。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::email::{EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity};
    ///
    /// # fn main() -> Result<(), EmailError> {
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
    /// let _send: fn(&EmailClient, EmailMessage) -> Result<(), EmailError> =
    ///     EmailClient::send;
    /// let _ = (client, message);
    /// # Ok(())
    /// # }
    /// ```
    pub fn send(&self, message: EmailMessage) -> Result<(), EmailError> {
        // 消费内容完成 MIME 构造后执行阻塞 I/O，统一丢弃 provider 错误中的敏感信息。
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = message.into_lettre_from(&self.from).and_then(|message| {
            self.transport
                .send(&message)
                .map(|_| ())
                .map_err(|error| EmailError::from_smtp(&error))
        });
        // 观测只记录稳定分类及耗时，不改变发送结果。
        #[cfg(feature = "tracing")]
        email_trace::record_send("sync", &result, started);
        result
    }

    /// 在调用方已有的 Tokio runtime 中异步发送一封邮件。
    ///
    /// 该方法仅在启用 `email-async` feature 时导出；它消费消息、复用独立异步
    /// 连接池且不会创建 runtime 或调用 `block_on`。如果调用方没有处于 Tokio runtime，返回
    /// `EmailTransportErrorKind::Client`，不会 panic。服务端不支持 STARTTLS 时发送失败，
    /// 不会回退到明文认证。
    ///
    /// # Errors
    ///
    /// 如果消息无法转换为 `lettre` 消息或异步 SMTP/TLS/网络传输失败，返回稳定的
    /// [`EmailError`](crate::email::EmailError) 分类；错误不会暴露原始服务端响应。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::email::{EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity};
    /// # #[cfg(feature = "email-async")]
    /// # async fn example() -> Result<(), EmailError> {
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
    #[cfg(feature = "email-async")]
    pub async fn send_async(&self, message: EmailMessage) -> Result<(), EmailError> {
        // 观测封装不持有消息副本；取消 future 时不生成虚假的完成事件。
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = self.send_async_inner(message).await;
        #[cfg(feature = "tracing")]
        email_trace::record_send("async", &result, started);
        result
    }

    #[cfg(feature = "email-async")]
    /// 构建消息、确认 runtime 并复用首次初始化的异步 transport。
    async fn send_async_inner(&self, message: EmailMessage) -> Result<(), EmailError> {
        // 在创建依赖 Tokio 的池清理任务前拒绝缺失 runtime 的调用。
        let message = message.into_lettre_from(&self.from)?;
        if RuntimeHandle::try_current().is_err() {
            return Err(EmailError::Transport(EmailTransportErrorKind::Client));
        }

        // OnceLock 防止并发首次发送创建多组池；构造本身不产生网络连接。
        let async_transport = self.async_transport.get_or_init(|| {
            let result = build_async_transport(&self.async_config);
            #[cfg(feature = "tracing")]
            email_trace::record_transport_init(result.as_ref().map(|_| ()));
            result
        });

        // 使用初始化结果执行 I/O；失败只暴露稳定分类，缓存的构造错误直接复用。
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

/// 为同步和异步 transport 使用相同的空闲连接回收策略。
fn pool_config() -> PoolConfig {
    PoolConfig::new()
        .max_size(POOL_MAX_IDLE_CONNECTIONS)
        .idle_timeout(POOL_IDLE_TIMEOUT)
}

/// 从已校验配置构造同步 transport，只建立客户端状态和池，不访问 SMTP 服务。
fn build_sync_transport(config: &EmailConfig) -> Result<SmtpTransport, EmailError> {
    // 两种入口都要求 TLS，STARTTLS 不可用时不会退回明文认证。
    let builder = match config.security {
        EmailSecurity::ImplicitTls => SmtpTransport::relay(&config.host),
        EmailSecurity::StartTls => SmtpTransport::starttls_relay(&config.host),
    }
    .map_err(|error| EmailError::from_smtp(&error))?;

    // 显式覆盖端口、凭据、命令预算和空闲池策略，避免依赖 provider 的隐式默认值。
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

#[cfg(feature = "email-async")]
/// 在首次异步使用前保留的拥有型连接参数，不对外提供 Debug 或凭据访问。
struct AsyncTransportConfig {
    /// 已验证的 DNS 主机名，用于连接及 TLS 主机校验。
    host: String,
    /// 非零 TCP 端口。
    port: u16,
    /// 强制 TLS 模式。
    security: EmailSecurity,
    /// 认证用户名；只交给 transport。
    username: String,
    /// 原样保留的密码；不会写入本库错误或日志。
    password: String,
    /// 每次 SMTP 命令的等待预算。
    timeout: Duration,
}

#[cfg(feature = "email-async")]
impl AsyncTransportConfig {
    /// 复制异步 transport 后续需要的最小配置，不复制发件身份或建立网络连接。
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

#[cfg(feature = "email-async")]
/// 在调用方存活的 Tokio runtime 中构造带独立空闲池的异步 transport。
fn build_async_transport(
    config: &AsyncTransportConfig,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, EmailError> {
    // 与同步路径保持同一 TLS 策略；不在升级失败时放宽安全模式。
    let builder = match config.security {
        EmailSecurity::ImplicitTls => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host),
        EmailSecurity::StartTls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
        }
    }
    .map_err(|error| EmailError::from_smtp(&error))?;

    // 构造参数与同步池一致，但池任务和实际发送由异步 transport 独立管理。
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
    #[cfg(feature = "email-async")]
    use std::{future::Future, sync::Arc, task::Wake};

    use super::EmailClient;
    #[cfg(feature = "email-async")]
    use super::{build_async_transport, AsyncTransportConfig};
    use crate::email::{EmailConfig, EmailSecurity};
    #[cfg(feature = "email-async")]
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

    #[cfg(feature = "email-async")]
    #[tokio::test(flavor = "current_thread")]
    async fn async_transport_is_built_inside_the_callers_runtime() {
        let config = config("smtp.example.com", 465, EmailSecurity::ImplicitTls);
        let transport = build_async_transport(&AsyncTransportConfig::from_config(&config));
        assert!(transport.is_ok());
    }

    #[cfg(feature = "email-async")]
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
