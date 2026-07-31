#![cfg(feature = "lettre")]

use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

struct LiveConfig {
    smtp_host: String,
    smtp_port: u16,
    smtp_security: EmailSecurity,
    smtp_username: String,
    smtp_password: String,
    from_email: String,
    from_name: Option<String>,
    to_email: String,
}

#[test]
#[ignore = "requires config/email-test.toml and explicit AXUTILS_EMAIL_LIVE_TEST=1"]
fn sends_email_with_sync_client_live() {
    let config = match load_live_config() {
        Ok(Some(config)) => config,
        Ok(None) => return,
        Err(field) => panic!("SMTP live configuration is invalid: {field}"),
    };
    let message = live_message(&config.to_email)
        .unwrap_or_else(|| panic!("SMTP live message configuration is invalid: to_email"));
    let client =
        build_client(config).unwrap_or_else(|| panic!("SMTP live client configuration is invalid"));

    if let Err(error) = client.send(message) {
        panic!("SMTP live test failed: {error}");
    }
}

#[cfg(all(feature = "lettre", feature = "tokio"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires config/email-test.toml and explicit AXUTILS_EMAIL_LIVE_TEST=1"]
async fn sends_email_with_async_client_live() {
    let config = match load_live_config() {
        Ok(Some(config)) => config,
        Ok(None) => return,
        Err(field) => panic!("SMTP live configuration is invalid: {field}"),
    };
    let message = live_message(&config.to_email)
        .unwrap_or_else(|| panic!("SMTP live message configuration is invalid: to_email"));
    let client =
        build_client(config).unwrap_or_else(|| panic!("SMTP live client configuration is invalid"));

    if let Err(error) = client.send_async(message).await {
        panic!("SMTP live test failed: {error}");
    }
}

fn build_client(config: LiveConfig) -> Option<EmailClient> {
    let mut email_config = EmailConfig::new(
        config.smtp_host,
        config.smtp_port,
        config.smtp_security,
        config.smtp_username,
        config.smtp_password,
        config.from_email,
    )
    .ok()?;
    if let Some(from_name) = config.from_name {
        email_config = email_config.with_from_name(from_name).ok()?;
    }
    EmailClient::new(email_config).ok()
}

fn live_message(to_email: &str) -> Option<EmailMessage> {
    let identifier = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    EmailMessage::text(
        vec![to_email.to_owned()],
        format!("axutils SMTP live test {identifier}"),
        format!("This is a local axutils SMTP integration test ({identifier})."),
    )
    .ok()
}

fn load_live_config() -> Result<Option<LiveConfig>, &'static str> {
    if env::var("AXUTILS_EMAIL_LIVE_TEST").ok().as_deref() != Some("1") {
        return Ok(None);
    }

    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("email-test.toml");
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    let values = content
        .lines()
        .filter_map(parse_line)
        .collect::<std::collections::HashMap<_, _>>();

    let required = |field: &'static str| values.get(field).cloned().ok_or(field);
    let smtp_security = parse_security(required("smtp_security")?.as_str())?;

    Ok(Some(LiveConfig {
        smtp_host: required("smtp_host")?,
        smtp_port: required("smtp_port")?.parse().map_err(|_| "smtp_port")?,
        smtp_security,
        smtp_username: required("smtp_username")?,
        smtp_password: required("smtp_password")?,
        from_email: required("from_email")?,
        from_name: values.get("from_name").cloned(),
        to_email: required("to_email")?,
    }))
}

fn parse_security(value: &str) -> Result<EmailSecurity, &'static str> {
    match value {
        "tls" | "implicit_tls" => Ok(EmailSecurity::ImplicitTls),
        "starttls" => Ok(EmailSecurity::StartTls),
        _ => Err("smtp_security"),
    }
}

fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let value = value.trim();
    let value = if let Some(value) = value.strip_prefix('"') {
        let end = value.find('"')?;
        &value[..end]
    } else {
        value.split('#').next()?.trim()
    };
    Some((key.trim().to_owned(), value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{parse_line, parse_security};
    use axutils::EmailSecurity;

    #[test]
    fn accepts_documented_security_values_and_inline_comments() {
        assert!(matches!(
            parse_security("tls"),
            Ok(EmailSecurity::ImplicitTls)
        ));
        assert!(matches!(
            parse_security("implicit_tls"),
            Ok(EmailSecurity::ImplicitTls)
        ));
        assert!(matches!(
            parse_security("starttls"),
            Ok(EmailSecurity::StartTls)
        ));
        assert_eq!(
            parse_line(r#"smtp_security = "tls" # inline comment"#),
            Some(("smtp_security".to_owned(), "tls".to_owned()))
        );
    }
}
