//! SMTP 客户端构造、同步发送与在调用方 runtime 中执行的异步发送。

use std::time::Duration;

use lettre::{
    message::Mailbox,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
        response::{Response, Severity},
        PoolConfig,
    },
    SmtpTransport, Transport,
};

#[cfg(feature = "email-async")]
use super::async_smtp::AsyncSmtp;
use super::{
    config::{EmailConfig, EmailSecurity},
    error::{EmailError, EmailTransportErrorKind},
    message::EmailMessage,
};
#[cfg(feature = "tracing")]
use crate::telemetry::email as email_trace;

/// 每个传输池最多保留的空闲连接数，发送并发由调用方控制。
pub(super) const POOL_MAX_IDLE_CONNECTIONS: u32 = 10;
/// 空闲连接的回收期限，由同步池或调用方 runtime 中的异步池执行。
pub(super) const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// 可独立配置和复用连接池的 SMTP 邮件客户端。
///
/// 构造实例只准备本地配置。同步池和异步池分别最多保留 10 条空闲连接，空闲期限为 60 秒。
/// 启用 `email-async` 时，异步发送应在同一个存活的 Tokio runtime 中复用。每次异步发送
/// 独占连接，只有收到 DATA 的最终 2xx 回复才归还连接。客户端也可以在 runtime 外析构。
pub struct EmailClient {
    /// 已验证的固定发件身份，每次消息转换时借用。
    from: Mailbox,
    /// 独立同步 transport，内部共享其连接池。
    transport: SmtpTransport,
    /// 按需建连、认证并缓存已完成事务的异步 SMTP 连接。
    #[cfg(feature = "email-async")]
    async_smtp: AsyncSmtp,
}

impl EmailClient {
    /// 消费已校验配置，创建一个不访问网络的 SMTP 客户端。
    ///
    /// 根据 [`crate::email::EmailSecurity`] 选择强制 SMTPS 或强制 STARTTLS，显式设置端口、凭据、
    /// 超时和连接池策略。异步发送在调用方已有的 Tokio runtime 中按需建连。
    ///
    /// # Errors
    ///
    /// 如果无法初始化 TLS 参数，返回脱敏的 [`EmailError`]。
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
        // 同步与异步路径共用同一主机名校验和受信任根。
        let tls = TlsParameters::new(config.host.clone())
            .map_err(|error| EmailError::from_smtp(&error))?;
        Ok(Self::with_tls(config, tls))
    }

    /// 用同一组 TLS 参数和凭据初始化两个传输入口，保留各自独立的连接池。
    fn with_tls(config: EmailConfig, tls: TlsParameters) -> Self {
        let from = config.mailbox();
        let credentials = Credentials::new(config.username, config.password);
        // 密码只能通过隐式 TLS 或已完成强制 STARTTLS 的连接发送。
        let sync_tls = match config.security {
            EmailSecurity::ImplicitTls => Tls::Wrapper(tls.clone()),
            EmailSecurity::StartTls => Tls::Required(tls.clone()),
        };
        let transport = SmtpTransport::builder_dangerous(&config.host)
            .tls(sync_tls)
            .port(config.port)
            .credentials(credentials.clone())
            .timeout(Some(config.timeout))
            .pool_config(
                PoolConfig::new()
                    .max_size(POOL_MAX_IDLE_CONNECTIONS)
                    .idle_timeout(POOL_IDLE_TIMEOUT),
            )
            .build();

        // 构造时只保存异步连接参数，首次发送才需要 Tokio 的 I/O 与 time driver。
        Self {
            from,
            transport,
            #[cfg(feature = "email-async")]
            async_smtp: AsyncSmtp::new(
                config.host,
                config.port,
                config.security,
                tls,
                credentials,
                config.timeout,
            ),
        }
    }

    /// 同步发送一封邮件。
    ///
    /// 该方法消费 [`EmailMessage`]，在当前线程执行阻塞 SMTP I/O，并复用客户端连接池；不要
    /// 直接在 Tokio worker 线程调用它，异步服务应使用 `send_async`。发送失败只返回
    /// 脱敏后的稳定错误分类，不包含主题、正文、地址或 SMTP 服务端原始文本。
    ///
    /// # Errors
    ///
    /// 如果消息无法构造、传输失败或 DATA 最终回复不是 2xx，返回脱敏的 [`EmailError`]。
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
            let reply = self
                .transport
                .send(&message)
                .map_err(|error| EmailError::from_sync_smtp(&error))?;
            require_completion(&reply)
        });
        // 观测只记录稳定分类及耗时，不改变发送结果。
        #[cfg(feature = "tracing")]
        email_trace::record_send("sync", &result, started);
        result
    }

    /// 在调用方已有的 Tokio runtime 中异步发送一封邮件。
    ///
    /// 该方法仅在启用 `email-async` feature 时导出。runtime 需启用 I/O 与 time driver。
    /// 本地消息构造后，从取连接到最终 2xx 回复共享一个配置时限。超时或取消直接释放在途连接，
    /// 此时服务端可能已经接收邮件。再次发送由调用方决定。
    ///
    /// # Errors
    ///
    /// 如果消息无法构造、传输失败或 DATA 最终回复不是 2xx，返回脱敏的 [`EmailError`]。
    /// 缺少 runtime 时返回 `EmailTransportErrorKind::Client`，超时返回 `EmailTransportErrorKind::Timeout`。
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
        let result = match message.into_lettre_from(&self.from) {
            Ok(message) => self.async_smtp.send(message).await,
            Err(error) => Err(error),
        };
        #[cfg(feature = "tracing")]
        email_trace::record_send("async", &result, started);
        result
    }
}

/// Lettre 将 2xx 和 3xx 都视为正向回复；只有完成回复才能确认最终命令已经执行。
pub(super) fn require_completion(response: &Response) -> Result<(), EmailError> {
    if response.code().severity == Severity::PositiveCompletion {
        Ok(())
    } else {
        Err(EmailError::Transport(EmailTransportErrorKind::SmtpResponse))
    }
}

#[cfg(test)]
#[path = "smtp_tests.rs"]
mod tests;
