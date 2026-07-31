use std::{error::Error as StdError, fmt};

use lettre::transport::smtp::Error as SmtpError;

/// SMTP 传输失败的稳定分类。
///
/// 分类只保留适合记录到应用日志的元数据，不携带 SMTP 服务端原始响应、用户名、主机名
/// 或其他可能包含敏感信息的错误文本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EmailTransportErrorKind {
    /// 建立到 SMTP relay 的连接失败。
    Connection,
    /// TLS 握手或证书校验失败。
    Tls,
    /// SMTP relay 拒绝了认证信息。
    Authentication,
    /// 网络操作超过了配置的命令超时时间。
    Timeout,
    /// SMTP relay 返回了失败响应。
    SmtpResponse,
    /// 底层网络 I/O 失败，但无法进一步归类。
    Network,
    /// SMTP 客户端配置或协议状态发生内部错误。
    Client,
    /// 连接池已经关闭。
    Shutdown,
}

impl fmt::Display for EmailTransportErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Connection => "connection",
            Self::Tls => "tls",
            Self::Authentication => "authentication",
            Self::Timeout => "timeout",
            Self::SmtpResponse => "smtp response",
            Self::Network => "network",
            Self::Client => "client",
            Self::Shutdown => "connection pool shutdown",
        };
        formatter.write_str(label)
    }
}

/// 邮件配置、消息构建、传输或全局单例状态错误。
///
/// 错误只包含固定字段名、收件人索引和稳定的传输分类，不包含密码、正文、主题、用户名、
/// 完整 SMTP 主机名、发件人或收件人地址。SMTP/TLS 原始错误也不会作为 [`std::error::Error::source`]
/// 暴露，避免错误链绕过脱敏边界。
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum EmailError {
    /// 配置字段不符合本 crate 的格式或资源上限。
    InvalidConfig {
        /// 发生错误的固定字段类别。
        field: &'static str,
    },
    /// 消息字段不符合本 crate 的格式或资源上限。
    InvalidMessage {
        /// 发生错误的固定字段类别。
        field: &'static str,
    },
    /// 某个收件人地址解析失败或超出资源上限。
    InvalidRecipient {
        /// 收件人在输入 `Vec` 中的零基索引，不回显地址内容。
        index: usize,
    },
    /// `lettre` 拒绝了已经通过本 crate 校验的消息结构。
    MessageBuild,
    /// SMTP/TLS/网络传输失败。
    Transport(EmailTransportErrorKind),
    /// 全局 [`crate::EmailUtils`] 尚未初始化。
    NotInitialized,
    /// 全局 [`crate::EmailUtils`] 已经成功初始化，不能覆盖。
    AlreadyInitialized,
}

impl EmailError {
    pub(crate) fn invalid_config(field: &'static str) -> Self {
        Self::InvalidConfig { field }
    }

    pub(crate) fn invalid_message(field: &'static str) -> Self {
        Self::InvalidMessage { field }
    }

    pub(crate) fn from_smtp(error: &SmtpError) -> Self {
        let kind = if error.is_timeout() {
            EmailTransportErrorKind::Timeout
        } else if error.is_tls() {
            EmailTransportErrorKind::Tls
        } else if error.is_transport_shutdown() {
            EmailTransportErrorKind::Shutdown
        } else if error.is_response() {
            if matches!(
                error.status().map(u16::from),
                Some(432 | 454 | 530 | 534 | 535)
            ) {
                EmailTransportErrorKind::Authentication
            } else {
                EmailTransportErrorKind::SmtpResponse
            }
        } else if error.is_client() {
            EmailTransportErrorKind::Client
        } else if is_connection_error(error) {
            EmailTransportErrorKind::Connection
        } else {
            EmailTransportErrorKind::Network
        };

        Self::Transport(kind)
    }
}

fn is_connection_error(error: &SmtpError) -> bool {
    let mut source = StdError::source(error);
    while let Some(current) = source {
        if current
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| {
                matches!(
                    io_error.kind(),
                    std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                        | std::io::ErrorKind::AddrNotAvailable
                        | std::io::ErrorKind::AddrInUse
                )
            })
        {
            return true;
        }
        source = current.source();
    }
    false
}

impl fmt::Debug for EmailError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => formatter
                .debug_struct("EmailError::InvalidConfig")
                .field("field", field)
                .finish(),
            Self::InvalidMessage { field } => formatter
                .debug_struct("EmailError::InvalidMessage")
                .field("field", field)
                .finish(),
            Self::InvalidRecipient { index } => formatter
                .debug_struct("EmailError::InvalidRecipient")
                .field("index", index)
                .finish(),
            Self::MessageBuild => formatter.write_str("EmailError::MessageBuild"),
            Self::Transport(kind) => formatter
                .debug_tuple("EmailError::Transport")
                .field(kind)
                .finish(),
            Self::NotInitialized => formatter.write_str("EmailError::NotInitialized"),
            Self::AlreadyInitialized => formatter.write_str("EmailError::AlreadyInitialized"),
        }
    }
}

impl fmt::Display for EmailError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid email configuration field: {field}")
            }
            Self::InvalidMessage { field } => {
                write!(formatter, "invalid email message field: {field}")
            }
            Self::InvalidRecipient { index } => {
                write!(formatter, "invalid email recipient at index {index}")
            }
            Self::MessageBuild => formatter.write_str("email message could not be built"),
            Self::Transport(kind) => write!(formatter, "email transport failed: {kind}"),
            Self::NotInitialized => formatter.write_str("EmailUtils has not been initialized"),
            Self::AlreadyInitialized => formatter.write_str("EmailUtils is already initialized"),
        }
    }
}

impl std::error::Error for EmailError {}

#[cfg(test)]
mod tests {
    use super::{EmailError, EmailTransportErrorKind};

    #[test]
    fn error_output_does_not_include_sensitive_values() {
        let password = "password-that-must-not-appear";
        let subject = "subject-that-must-not-appear";
        let body = "body-that-must-not-appear";
        let username = "username-that-must-not-appear";
        let host = "smtp-that-must-not-appear.example";

        let error = EmailError::Transport(EmailTransportErrorKind::Authentication);
        let display = error.to_string();
        let debug = format!("{error:?}");

        for output in [display, debug] {
            assert!(!output.contains(password));
            assert!(!output.contains(subject));
            assert!(!output.contains(body));
            assert!(!output.contains(username));
            assert!(!output.contains(host));
        }
    }
}
