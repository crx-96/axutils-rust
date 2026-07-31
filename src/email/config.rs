use std::{fmt, net::IpAddr, str::FromStr, time::Duration};

use lettre::{message::Mailbox, Address};

use super::error::EmailError;

const MAX_HOST_BYTES: usize = 253;
const MAX_CREDENTIAL_BYTES: usize = 4 * 1024;
const MAX_FROM_NAME_BYTES: usize = 512;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const MIN_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// SMTP 连接的强制 TLS 模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailSecurity {
    /// 使用 SMTPS 包装 TLS，通常对应 465 端口。
    ImplicitTls,
    /// 先建立 SMTP 连接，再强制升级到 STARTTLS，通常对应 587 端口。
    StartTls,
}

/// 已校验的 SMTP 账号和发件人配置。
///
/// 使用 [`EmailConfig::new`] 创建后，可选地通过 builder 方法设置显示名和命令超时。字段
/// 私有且不会实现 `Clone`；配置被 [`crate::EmailClient::new`] 消费后，调用方不能再读取密码。
/// `host` 仅接受 ASCII DNS 主机名，不接受 SMTP URL、端口、路径或 IP 字面量。
pub struct EmailConfig {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) security: EmailSecurity,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) from: Address,
    pub(crate) from_name: Option<String>,
    pub(crate) timeout: Duration,
}

impl EmailConfig {
    /// 创建并校验 SMTP 配置。
    ///
    /// `host` 必须是 ASCII DNS 主机名；`port` 不能为 `0`；用户名、密码和发件地址不能为空，
    /// 且账号相关字段最多 4 KiB。密码不会被 trim、转换或写入本 crate 的错误文本。默认
    /// 命令超时为 30 秒。该方法只做本地校验，不建立网络连接。
    ///
    /// # Errors
    ///
    /// 返回 [`EmailError::InvalidConfig`](crate::EmailError::InvalidConfig) 并指出固定字段名，
    /// 如果主机名、端口、用户名、密码或发件地址为空、超出上限或格式非法；主机名、用户名
    /// 和发件地址中的控制字符也会被拒绝。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailConfig, EmailSecurity};
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
    /// let _ = config;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        host: impl Into<String>,
        port: u16,
        security: EmailSecurity,
        username: impl Into<String>,
        password: impl Into<String>,
        from_email: impl Into<String>,
    ) -> Result<Self, EmailError> {
        let host = host.into();
        validate_host(&host)?;

        if port == 0 {
            return Err(EmailError::invalid_config("port"));
        }

        let username = username.into();
        validate_non_empty_bounded(&username, MAX_CREDENTIAL_BYTES, "username")?;
        if username.trim() != username || contains_control(&username) {
            return Err(EmailError::invalid_config("username"));
        }

        let password = password.into();
        validate_non_empty_bounded(&password, MAX_CREDENTIAL_BYTES, "password")?;

        let from_email = from_email.into();
        validate_non_empty_bounded(&from_email, MAX_CREDENTIAL_BYTES, "from_email")?;
        if from_email.trim() != from_email || contains_control(&from_email) {
            return Err(EmailError::invalid_config("from_email"));
        }
        let from =
            Address::from_str(&from_email).map_err(|_| EmailError::invalid_config("from_email"))?;

        Ok(Self {
            host,
            port,
            security,
            username,
            password,
            from,
            from_name: None,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    /// 设置发件人的显示名并返回更新后的配置。
    ///
    /// 显示名最多 512 字节，不允许空字符串、首尾空白、NUL、换行或其他控制字符。显示名
    /// 只用于邮件头，不会改变 SMTP 登录用户名或发件地址。
    ///
    /// # Errors
    ///
    /// 如果显示名为空、超出 512 字节、包含首尾空白或控制字符，返回字段为 `from_name` 的
    /// [`EmailError::InvalidConfig`](crate::EmailError::InvalidConfig)。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{EmailConfig, EmailSecurity};
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let config = EmailConfig::new(
    ///     "smtp.example.com",
    ///     465,
    ///     EmailSecurity::ImplicitTls,
    ///     "sender@example.com",
    ///     "application-password",
    ///     "sender@example.com",
    /// )?
    /// .with_from_name("Axutils")?;
    /// let _ = config;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_from_name(mut self, from_name: impl Into<String>) -> Result<Self, EmailError> {
        let from_name = from_name.into();
        if from_name.is_empty()
            || from_name.len() > MAX_FROM_NAME_BYTES
            || from_name.trim() != from_name
            || contains_control(&from_name)
        {
            return Err(EmailError::invalid_config("from_name"));
        }

        self.from_name = Some(from_name);
        Ok(self)
    }

    /// 设置 SMTP 命令超时并返回更新后的配置。
    ///
    /// 自定义值必须在 1 秒到 5 分钟（含边界）之间。超时只限制 SMTP 命令等待时间，不会
    /// 自动重试、创建后台任务或改变连接池的 60 秒空闲回收时间。
    ///
    /// # Errors
    ///
    /// 如果超时小于 1 秒或大于 5 分钟，返回字段为 `timeout` 的
    /// [`EmailError::InvalidConfig`](crate::EmailError::InvalidConfig)。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use axutils::{EmailConfig, EmailSecurity};
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let config = EmailConfig::new(
    ///     "smtp.example.com",
    ///     587,
    ///     EmailSecurity::StartTls,
    ///     "sender@example.com",
    ///     "application-password",
    ///     "sender@example.com",
    /// )?
    /// .with_timeout(Duration::from_secs(45))?;
    /// let _ = config;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, EmailError> {
        if !(MIN_TIMEOUT..=MAX_TIMEOUT).contains(&timeout) {
            return Err(EmailError::invalid_config("timeout"));
        }

        self.timeout = timeout;
        Ok(self)
    }

    pub(crate) fn mailbox(&self) -> Mailbox {
        Mailbox::new(self.from_name.clone(), self.from.clone())
    }
}

impl fmt::Debug for EmailConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EmailConfig")
            .field("host", &"[REDACTED]")
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &"[REDACTED]")
            .field("password", &"[REDACTED]")
            .field("from_email", &"[REDACTED]")
            .field("from_name", &"[REDACTED]")
            .field("timeout", &self.timeout)
            .finish()
    }
}

fn validate_host(host: &str) -> Result<(), EmailError> {
    if host.is_empty()
        || host.len() > MAX_HOST_BYTES
        || host.trim() != host
        || contains_control(host)
        || host.parse::<IpAddr>().is_ok()
    {
        return Err(EmailError::invalid_config("host"));
    }

    let valid = host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });

    if valid {
        Ok(())
    } else {
        Err(EmailError::invalid_config("host"))
    }
}

fn validate_non_empty_bounded(
    value: &str,
    max_bytes: usize,
    field: &'static str,
) -> Result<(), EmailError> {
    if value.is_empty() {
        return Err(EmailError::invalid_config(field));
    }
    if value.len() > max_bytes {
        return Err(EmailError::invalid_config(field));
    }
    Ok(())
}

fn contains_control(value: &str) -> bool {
    value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, time::Duration};

    use super::{EmailConfig, EmailSecurity, MAX_CREDENTIAL_BYTES, MAX_FROM_NAME_BYTES};
    use crate::email::EmailError;

    fn config() -> Result<EmailConfig, EmailError> {
        EmailConfig::new(
            "smtp.example.com",
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
    }

    #[test]
    fn accepts_valid_security_modes_and_defaults() {
        assert!(config().is_ok());
        assert!(EmailConfig::new(
            "smtp.example.com",
            587,
            EmailSecurity::StartTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
        .is_ok());

        let label = "a".repeat(63);
        let valid_max_host = format!("{label}.{label}.{label}.{}", "a".repeat(61));
        assert_eq!(valid_max_host.len(), 253);
        assert!(EmailConfig::new(
            valid_max_host,
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
        .is_ok());
    }

    #[test]
    fn rejects_empty_or_invalid_hosts() {
        let ipv4 = Ipv4Addr::new(192, 0, 2, 1).to_string();
        for host in [
            "",
            " smtp.example.com",
            "smtp.example.com ",
            "smtp.example.com:465",
            "smtp://smtp.example.com",
            "smtp.example.com/path",
            "smtp..example.com",
            "-smtp.example.com",
            "smtp-.example.com",
            ipv4.as_str(),
            "[2001:db8::1]",
        ] {
            let result = EmailConfig::new(
                host,
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "sender@example.com",
            );
            assert!(matches!(
                result,
                Err(EmailError::InvalidConfig { field: "host" })
            ));
        }

        let label = "a".repeat(63);
        let over_host = format!("{label}.{label}.{label}.{}", "a".repeat(62));
        assert_eq!(over_host.len(), 254);
        assert!(matches!(
            EmailConfig::new(
                over_host,
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "sender@example.com",
            ),
            Err(EmailError::InvalidConfig { field: "host" })
        ));

        let overlong_label = format!("{}.example.com", "a".repeat(64));
        assert!(matches!(
            EmailConfig::new(
                overlong_label,
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "sender@example.com",
            ),
            Err(EmailError::InvalidConfig { field: "host" })
        ));
    }

    #[test]
    fn rejects_invalid_scalar_fields_and_limits() {
        assert!(matches!(
            EmailConfig::new(
                "smtp.example.com",
                0,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "sender@example.com",
            ),
            Err(EmailError::InvalidConfig { field: "port" })
        ));

        for username in [
            "",
            " sender@example.com",
            "sender@example.com\n",
            "sender\u{0085}@example.com",
        ] {
            let result = EmailConfig::new(
                "smtp.example.com",
                465,
                EmailSecurity::ImplicitTls,
                username,
                "secret-password",
                "sender@example.com",
            );
            assert!(matches!(
                result,
                Err(EmailError::InvalidConfig { field: "username" })
            ));
        }

        for (password, from_email, field) in [
            ("", "sender@example.com", "password"),
            ("secret-password", "", "from_email"),
        ] {
            let result = EmailConfig::new(
                "smtp.example.com",
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                password,
                from_email,
            );
            assert!(
                matches!(result, Err(EmailError::InvalidConfig { field: actual }) if actual == field)
            );
        }

        let long_value = "x".repeat(MAX_CREDENTIAL_BYTES + 1);
        for (username, password, from_email, field) in [
            (
                long_value.as_str(),
                "secret-password",
                "sender@example.com",
                "username",
            ),
            (
                "sender@example.com",
                long_value.as_str(),
                "sender@example.com",
                "password",
            ),
            (
                "sender@example.com",
                "secret-password",
                long_value.as_str(),
                "from_email",
            ),
        ] {
            let result = EmailConfig::new(
                "smtp.example.com",
                465,
                EmailSecurity::ImplicitTls,
                username,
                password,
                from_email,
            );
            assert!(
                matches!(result, Err(EmailError::InvalidConfig { field: actual }) if actual == field)
            );
        }

        let max_username = "u".repeat(MAX_CREDENTIAL_BYTES);
        let max_password = "p".repeat(MAX_CREDENTIAL_BYTES);
        assert!(EmailConfig::new(
            "smtp.example.com",
            465,
            EmailSecurity::ImplicitTls,
            max_username,
            max_password,
            "sender@example.com",
        )
        .is_ok());

        assert!(matches!(
            EmailConfig::new(
                "smtp.example.com",
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "sender\n@example.com",
            ),
            Err(EmailError::InvalidConfig {
                field: "from_email"
            })
        ));
        assert!(matches!(
            EmailConfig::new(
                "smtp.example.com",
                465,
                EmailSecurity::ImplicitTls,
                "sender@example.com",
                "secret-password",
                "not-an-email",
            ),
            Err(EmailError::InvalidConfig {
                field: "from_email"
            })
        ));
    }

    #[test]
    fn validates_display_name_and_timeout_boundaries() {
        assert!(config()
            .unwrap_or_else(|_| unreachable!())
            .with_from_name("Axutils")
            .is_ok());
        for name in [
            "",
            " Axutils",
            "Axutils ",
            "Ax\nutils",
            "Ax\0utils",
            "Ax\u{0085}utils",
        ] {
            assert!(matches!(
                config()
                    .unwrap_or_else(|_| unreachable!())
                    .with_from_name(name),
                Err(EmailError::InvalidConfig { field: "from_name" })
            ));
        }

        let long_name = "x".repeat(MAX_FROM_NAME_BYTES + 1);
        assert!(matches!(
            config()
                .unwrap_or_else(|_| unreachable!())
                .with_from_name(long_name),
            Err(EmailError::InvalidConfig { field: "from_name" })
        ));
        assert!(config()
            .unwrap_or_else(|_| unreachable!())
            .with_from_name("x".repeat(MAX_FROM_NAME_BYTES))
            .is_ok());

        assert!(config()
            .unwrap_or_else(|_| unreachable!())
            .with_timeout(Duration::from_secs(1))
            .is_ok());
        assert!(config()
            .unwrap_or_else(|_| unreachable!())
            .with_timeout(Duration::from_secs(300))
            .is_ok());
        assert!(matches!(
            config()
                .unwrap_or_else(|_| unreachable!())
                .with_timeout(Duration::from_millis(999)),
            Err(EmailError::InvalidConfig { field: "timeout" })
        ));
        assert!(matches!(
            config()
                .unwrap_or_else(|_| unreachable!())
                .with_timeout(Duration::from_secs(301)),
            Err(EmailError::InvalidConfig { field: "timeout" })
        ));
    }

    #[test]
    fn debug_redacts_all_sensitive_configuration_fields() {
        let config = config()
            .unwrap_or_else(|_| unreachable!())
            .with_from_name("Sensitive Display Name")
            .unwrap_or_else(|_| unreachable!());
        let debug = format!("{config:?}");
        for value in [
            "smtp.example.com",
            "sender@example.com",
            "secret-password",
            "Sensitive Display Name",
        ] {
            assert!(!debug.contains(value));
        }
        assert!(debug.contains("[REDACTED]"));
    }
}
