use std::{fmt, str::FromStr};

use lettre::{message::header::ContentType, message::Mailbox, Message as LettreMessage};

use super::error::EmailError;

const MAX_RECIPIENTS: usize = 100;
const MAX_RECIPIENT_BYTES: usize = 4 * 1024;
const MAX_SUBJECT_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// 邮件正文的 MIME 类型。
///
/// HTML 正文不会自动生成纯文本 fallback，也不会被当作模板执行或净化；不可信 HTML 的
/// 转义和清理由调用方负责。
pub enum EmailBody {
    /// UTF-8 `text/plain` 正文。
    Text(String),
    /// UTF-8 `text/html` 正文。
    Html(String),
}

/// 已校验的收件人、主题和正文。
///
/// 构造函数会逐项解析收件人并检查输入规模：每封邮件至少一个、最多 100 个收件人，主题
/// 最多 16 KiB，正文最多 10 MiB。空主题和空正文是允许的。类型不实现 `Clone` 或 `Debug`，
/// 避免调用方无意中复制或展示大正文；发送时会消费消息值。
pub struct EmailMessage {
    recipients: Vec<Mailbox>,
    subject: String,
    body: EmailBody,
}

impl EmailMessage {
    /// 创建纯文本邮件并立即校验所有收件人、主题和正文。
    ///
    /// 收件人可以是普通地址或带显示名的 mailbox 字符串，但不能包含控制字符、首尾空白或
    /// 无效地址。主题拒绝 CR、LF、NUL 和其他控制字符，以防止邮件头注入；正文允许普通换行。
    ///
    /// # Errors
    ///
    /// 如果收件人数量、单个收件人长度、主题或正文超出上限，或收件人/主题格式非法，返回
    /// [`EmailError`](crate::EmailError)；收件人错误只包含其零基索引，不回显地址内容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::EmailMessage;
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let message = EmailMessage::text(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "A test message",
    ///     "Hello from axutils.",
    /// )?;
    /// let _ = message;
    /// # Ok(())
    /// # }
    /// ```
    pub fn text(
        to: Vec<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<Self, EmailError> {
        Self::new(to, subject.into(), EmailBody::Text(body.into()))
    }

    /// 创建 HTML 邮件并立即校验所有收件人、主题和正文。
    ///
    /// 正文按 UTF-8 `text/html` 构建，不附带纯文本版本、不执行模板，也不自动清理 HTML。
    /// 空正文允许发送；不可信内容必须由调用方自行转义或清理。
    ///
    /// # Errors
    ///
    /// 如果收件人数量、单个收件人长度、主题或正文超出上限，或收件人/主题格式非法，返回
    /// [`EmailError`](crate::EmailError)；收件人错误只包含其零基索引，不回显地址内容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::EmailMessage;
    ///
    /// # fn main() -> Result<(), axutils::EmailError> {
    /// let message = EmailMessage::html(
    ///     vec!["receiver@example.com".to_owned()],
    ///     "An HTML message",
    ///     "<p>Hello from <strong>axutils</strong>.</p>",
    /// )?;
    /// let _ = message;
    /// # Ok(())
    /// # }
    /// ```
    pub fn html(
        to: Vec<String>,
        subject: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<Self, EmailError> {
        Self::new(to, subject.into(), EmailBody::Html(body.into()))
    }

    fn new(to: Vec<String>, subject: String, body: EmailBody) -> Result<Self, EmailError> {
        if to.is_empty() || to.len() > MAX_RECIPIENTS {
            return Err(EmailError::invalid_message("recipients"));
        }
        if subject.len() > MAX_SUBJECT_BYTES || contains_control(&subject) {
            return Err(EmailError::invalid_message("subject"));
        }

        let body_len = match &body {
            EmailBody::Text(value) | EmailBody::Html(value) => value.len(),
        };
        if body_len > MAX_BODY_BYTES {
            return Err(EmailError::invalid_message("body"));
        }

        let mut recipients = Vec::with_capacity(to.len());
        for (index, value) in to.into_iter().enumerate() {
            if value.is_empty()
                || value.len() > MAX_RECIPIENT_BYTES
                || value.trim() != value
                || contains_control(&value)
            {
                return Err(EmailError::InvalidRecipient { index });
            }

            let mailbox =
                Mailbox::from_str(&value).map_err(|_| EmailError::InvalidRecipient { index })?;
            recipients.push(mailbox);
        }

        Ok(Self {
            recipients,
            subject,
            body,
        })
    }

    pub(crate) fn into_lettre_from(self, from: &Mailbox) -> Result<LettreMessage, EmailError> {
        let mut builder = LettreMessage::builder()
            .from(from.clone())
            .subject(self.subject);

        for recipient in self.recipients {
            builder = builder.to(recipient);
        }

        match self.body {
            EmailBody::Text(body) => builder
                .header(ContentType::TEXT_PLAIN)
                .body(body)
                .map_err(|_| EmailError::MessageBuild),
            EmailBody::Html(body) => builder
                .header(ContentType::TEXT_HTML)
                .body(body)
                .map_err(|_| EmailError::MessageBuild),
        }
    }
}

impl fmt::Debug for EmailBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Text(_) => "text",
            Self::Html(_) => "html",
        };
        formatter
            .debug_struct("EmailBody")
            .field("kind", &kind)
            .finish()
    }
}

fn contains_control(value: &str) -> bool {
    value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::{
        EmailBody, EmailMessage, MAX_BODY_BYTES, MAX_RECIPIENTS, MAX_RECIPIENT_BYTES,
        MAX_SUBJECT_BYTES,
    };
    use crate::email::{EmailConfig, EmailError, EmailSecurity};

    fn config() -> EmailConfig {
        EmailConfig::new(
            "smtp.example.com",
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "secret-password",
            "sender@example.com",
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn accepts_text_html_empty_values_and_display_name_recipients() {
        assert!(EmailMessage::text(vec!["receiver@example.com".to_owned()], "", "").is_ok());
        assert!(EmailMessage::html(
            vec!["Receiver <receiver@example.com>".to_owned()],
            "subject",
            "<p>body</p>",
        )
        .is_ok());
    }

    #[test]
    fn rejects_recipient_count_address_and_header_injection() {
        assert!(matches!(
            EmailMessage::text(Vec::new(), "subject", "body"),
            Err(EmailError::InvalidMessage {
                field: "recipients"
            })
        ));

        let too_many = (0..=MAX_RECIPIENTS)
            .map(|index| format!("receiver{index}@example.com"))
            .collect();
        assert!(matches!(
            EmailMessage::text(too_many, "subject", "body"),
            Err(EmailError::InvalidMessage {
                field: "recipients"
            })
        ));

        for recipient in [
            "",
            " receiver@example.com",
            "receiver@example.com ",
            "receiver@example.com\nBcc: injected@example.com",
            "invalid-address",
        ]
        .into_iter()
        {
            assert!(matches!(
                EmailMessage::text(vec![recipient.to_owned()], "subject", "body"),
                Err(EmailError::InvalidRecipient { index: 0 })
            ));
        }

        for subject in [
            "subject\nBcc: injected@example.com",
            "subject\r",
            "subject\0",
            "subject\u{0085}",
        ] {
            assert!(matches!(
                EmailMessage::text(vec!["receiver@example.com".to_owned()], subject, "body"),
                Err(EmailError::InvalidMessage { field: "subject" })
            ));
        }
    }

    #[test]
    fn enforces_subject_body_and_recipient_limits() {
        let max_subject = "s".repeat(MAX_SUBJECT_BYTES);
        assert!(
            EmailMessage::text(vec!["receiver@example.com".to_owned()], max_subject, "body",)
                .is_ok()
        );

        let over_subject = "s".repeat(MAX_SUBJECT_BYTES + 1);
        assert!(matches!(
            EmailMessage::text(
                vec!["receiver@example.com".to_owned()],
                over_subject,
                "body"
            ),
            Err(EmailError::InvalidMessage { field: "subject" })
        ));

        let max_body = "b".repeat(MAX_BODY_BYTES);
        assert!(
            EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", max_body,)
                .is_ok()
        );

        let over_body = "b".repeat(MAX_BODY_BYTES + 1);
        assert!(matches!(
            EmailMessage::html(
                vec!["receiver@example.com".to_owned()],
                "subject",
                over_body
            ),
            Err(EmailError::InvalidMessage { field: "body" })
        ));

        let recipient_suffix = "\" <receiver@example.com>";
        let max_recipient = format!(
            "\"{}{}",
            "r".repeat(MAX_RECIPIENT_BYTES - 1 - recipient_suffix.len()),
            recipient_suffix
        );
        assert_eq!(max_recipient.len(), MAX_RECIPIENT_BYTES);
        assert!(EmailMessage::text(vec![max_recipient], "subject", "body").is_ok());

        let over_recipient = format!(
            "\"{}{}",
            "r".repeat(MAX_RECIPIENT_BYTES - recipient_suffix.len()),
            recipient_suffix
        );
        assert_eq!(over_recipient.len(), MAX_RECIPIENT_BYTES + 1);
        assert!(matches!(
            EmailMessage::text(vec![over_recipient], "subject", "body"),
            Err(EmailError::InvalidRecipient { index: 0 })
        ));
    }

    #[test]
    fn builds_text_and_html_with_shared_message_path() {
        let text = EmailMessage::text(
            vec!["receiver@example.com".to_owned()],
            "text subject",
            "text body",
        )
        .unwrap_or_else(|_| unreachable!());
        let html = EmailMessage::html(
            vec!["receiver@example.com".to_owned()],
            "html subject",
            "<p>html body</p>",
        )
        .unwrap_or_else(|_| unreachable!());

        let text = text
            .into_lettre_from(&config().mailbox())
            .unwrap_or_else(|_| unreachable!());
        let html = html
            .into_lettre_from(&config().mailbox())
            .unwrap_or_else(|_| unreachable!());
        let text = String::from_utf8(text.formatted()).unwrap_or_else(|_| unreachable!());
        let html = String::from_utf8(html.formatted()).unwrap_or_else(|_| unreachable!());

        assert!(text.contains("Content-Type: text/plain; charset=utf-8"));
        assert!(text.contains("text body"));
        assert!(html.contains("Content-Type: text/html; charset=utf-8"));
        assert!(html.contains("<p>html body</p>"));

        let body = EmailBody::Html("body-that-is-not-debugged".to_owned());
        assert_eq!(format!("{body:?}"), "EmailBody { kind: \"html\" }");
    }
}
