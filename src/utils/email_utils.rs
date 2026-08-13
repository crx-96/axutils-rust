use std::sync::OnceLock;

use crate::email::{EmailClient, EmailConfig, EmailError, EmailMessage};

static EMAIL_CLIENT: OnceLock<EmailClient> = OnceLock::new();

/// 单默认账号的进程级 SMTP 邮件便捷入口。
///
/// 必须先成功调用 [`Self::init`]；初始化成功后只能保留第一个客户端，不能 reset、replace
/// 或在运行时切换账号。需要多个账号或可控生命周期时，请直接持有多个 [`EmailClient`]。
pub struct EmailUtils;

impl EmailUtils {
    /// 初始化全局邮件客户端。
    ///
    /// 配置会先完成本地校验和同步 transport 构造；同时启用异步 feature 时只保存经过校验
    /// 的异步 transport 配置，并在首次 `send_async` 时于调用方 runtime 中初始化连接池。
    /// 构造阶段不会访问网络；只有完整成功后才会占用全局单例。成功初始化后再次调用返回
    /// [`EmailError::AlreadyInitialized`]，不会覆盖第一个账号。该方法不提供读取密码或替换
    /// 配置的能力。
    ///
    /// # Errors
    ///
    /// 如果配置或同步 transport builder 无效，返回相应的脱敏错误且不会占用单例；如果另
    /// 一个线程先完成初始化，返回 [`EmailError::AlreadyInitialized`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailConfig, EmailSecurity, EmailUtils};
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
    /// let _ = EmailUtils::init(config);
    /// // 该示例只构造连接池，不发送邮件；进程级单例不可重置。
    /// # Ok(())
    /// # }
    /// ```
    pub fn init(config: EmailConfig) -> Result<(), EmailError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let result = if EMAIL_CLIENT.get().is_some() {
            Err(EmailError::AlreadyInitialized)
        } else {
            match EmailClient::new(config) {
                Ok(client) => EMAIL_CLIENT
                    .set(client)
                    .map_err(|_| EmailError::AlreadyInitialized),
                Err(error) => Err(error),
            }
        };
        #[cfg(feature = "tracing")]
        crate::tracing::email::record_client_init(&result, started);
        result
    }

    /// 返回全局邮件客户端是否已经成功初始化。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::EmailUtils;
    ///
    /// let _initialized = EmailUtils::is_initialized();
    /// ```
    pub fn is_initialized() -> bool {
        EMAIL_CLIENT.get().is_some()
    }

    /// 使用全局客户端同步发送一封邮件。
    ///
    /// 未初始化时返回 [`EmailError::NotInitialized`]，不会 panic。该方法在当前线程执行阻塞
    /// SMTP I/O；Tokio 服务应使用 `send_async`。
    ///
    /// # Errors
    ///
    /// 如果全局客户端尚未初始化，返回 [`EmailError::NotInitialized`]；否则转发实例发送的
    /// 消息构建和脱敏传输错误。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailMessage, EmailUtils};
    ///
    /// let message = EmailMessage::text(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "subject",
    ///     "body",
    /// );
    /// let _send: fn(EmailMessage) -> Result<(), axutils::EmailError> = EmailUtils::send;
    /// let _ = message;
    /// ```
    pub fn send(message: EmailMessage) -> Result<(), EmailError> {
        EMAIL_CLIENT
            .get()
            .ok_or(EmailError::NotInitialized)?
            .send(message)
    }

    /// 在调用方已有的 Tokio runtime 中使用全局客户端异步发送一封邮件。
    ///
    /// 仅在同时启用 `lettre` 与 `tokio` feature 时导出；不会创建 runtime、调用 `block_on` 或
    /// 替换全局账号。未初始化时返回 [`EmailError::NotInitialized`]；调用方不在 Tokio runtime
    /// 中时返回异步 transport 的客户端错误，不会 panic。
    ///
    /// # Errors
    ///
    /// 如果全局客户端尚未初始化，返回 [`EmailError::NotInitialized`]；否则转发异步实例发送
    /// 的消息构建和脱敏传输错误。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(all(feature = "lettre", feature = "tokio"))]
    /// # async fn example() -> Result<(), axutils::EmailError> {
    /// use axutils::{EmailMessage, EmailUtils};
    ///
    /// let message = EmailMessage::text(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "subject",
    ///     "body",
    /// )?;
    /// let _send_async = EmailUtils::send_async;
    /// let _ = message;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    #[cfg(all(feature = "lettre", feature = "tokio"))]
    pub async fn send_async(message: EmailMessage) -> Result<(), EmailError> {
        EMAIL_CLIENT
            .get()
            .ok_or(EmailError::NotInitialized)?
            .send_async(message)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::EmailUtils;
    use crate::email::{EmailConfig, EmailError, EmailMessage, EmailSecurity};

    fn config(host: &str) -> Result<EmailConfig, EmailError> {
        EmailConfig::new(
            host,
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
    }

    #[test]
    fn initializes_once_and_wins_exactly_one_concurrent_attempt() {
        let invalid = EmailConfig::new(
            "",
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        );
        assert!(matches!(
            invalid,
            Err(EmailError::InvalidConfig { field: "host" })
        ));
        assert!(!EmailUtils::is_initialized());

        let message =
            EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
                .unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            EmailUtils::send(message),
            Err(EmailError::NotInitialized)
        ));

        #[cfg(all(feature = "lettre", feature = "tokio"))]
        {
            let message =
                EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
                    .unwrap_or_else(|_| unreachable!());
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap_or_else(|_| unreachable!());
            assert!(matches!(
                runtime.block_on(EmailUtils::send_async(message)),
                Err(EmailError::NotInitialized)
            ));
        }

        let barrier = Arc::new(Barrier::new(2));
        let first_barrier = Arc::clone(&barrier);
        let first = std::thread::spawn(move || {
            let config = config("smtp-first.example.com").unwrap_or_else(|_| unreachable!());
            first_barrier.wait();
            EmailUtils::init(config)
        });

        let second_barrier = Arc::clone(&barrier);
        let second = std::thread::spawn(move || {
            let config = config("smtp-second.example.com").unwrap_or_else(|_| unreachable!());
            second_barrier.wait();
            EmailUtils::init(config)
        });

        let results = [
            first.join().unwrap_or_else(|_| unreachable!()),
            second.join().unwrap_or_else(|_| unreachable!()),
        ];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(EmailError::AlreadyInitialized)))
                .count(),
            1
        );
        assert!(EmailUtils::is_initialized());

        let duplicate = config("smtp-duplicate.example.com").unwrap_or_else(|_| unreachable!());
        assert!(matches!(
            EmailUtils::init(duplicate),
            Err(EmailError::AlreadyInitialized)
        ));
    }
}
