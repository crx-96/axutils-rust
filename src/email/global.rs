use std::sync::OnceLock;

use super::{EmailClient, EmailConfig, EmailError};
#[cfg(feature = "tracing")]
use crate::telemetry::email as email_trace;

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
        email_trace::record_client_init(&result, started);
        result
    }

    /// 返回全局邮件客户端是否已经成功初始化。
    pub fn is_initialized() -> bool {
        EMAIL_CLIENT.get().is_some()
    }

    /// 返回已初始化的全局邮件客户端。
    ///
    /// 未初始化时返回 [`EmailError::NotInitialized`]。调用方可通过返回的实例选择同步或异步
    /// 发送；异步发送仍要求调用方提供 Tokio runtime。
    pub fn client() -> Result<&'static EmailClient, EmailError> {
        EMAIL_CLIENT.get().ok_or(EmailError::NotInitialized)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::EmailUtils;
    use crate::email::{EmailConfig, EmailError, EmailSecurity};

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
        assert!(matches!(
            EmailUtils::client(),
            Err(EmailError::NotInitialized)
        ));

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
