#[cfg(feature = "sync")]
fn main() {
    use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity, EmailUtils};

    let config = EmailConfig::new(
        "smtp.example.com",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "password",
        "sender@example.com",
    )
    .expect("the fixture configuration should be valid");
    let client = EmailClient::new(config).expect("the fixture client should be constructible");
    let message = EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
        .expect("the fixture message should be valid");

    let _send: fn(&EmailClient, EmailMessage) -> Result<(), axutils::EmailError> =
        EmailClient::send;
    let _init: fn(EmailConfig) -> Result<(), axutils::EmailError> = EmailUtils::init;
    let _singleton_send: fn(EmailMessage) -> Result<(), axutils::EmailError> = EmailUtils::send;
    let _ = (client, message);
}

#[cfg(feature = "all")]
#[allow(dead_code)]
async fn compile_async_api(client: &axutils::EmailClient, message: axutils::EmailMessage) {
    let _ = client.send_async(message).await;
    let _ = axutils::EmailUtils::send_async(
        axutils::EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
            .expect("the fixture message should be valid"),
    )
    .await;
}

#[cfg(feature = "all")]
fn main() {
    use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity, EmailUtils};

    let config = EmailConfig::new(
        "smtp.example.com",
        587,
        EmailSecurity::StartTls,
        "sender@example.com",
        "password",
        "sender@example.com",
    )
    .expect("the fixture configuration should be valid");
    let client = EmailClient::new(config).expect("the fixture client should be constructible");
    let message = EmailMessage::html(
        vec!["receiver@example.com".to_owned()],
        "subject",
        "<p>body</p>",
    )
    .expect("the fixture message should be valid");

    let _send_async = EmailClient::send_async;
    let _singleton_send_async = EmailUtils::send_async;
    let _ = (client, message, compile_async_api);
}

#[cfg(feature = "negative-email-module")]
fn main() {
    let _ = axutils::email::EmailClient::new;
}

#[cfg(feature = "negative-email-client")]
fn main() {
    let _ = axutils::EmailClient::new;
}

#[cfg(feature = "negative-email-utils")]
fn main() {
    let _ = axutils::EmailUtils::init;
}

#[cfg(feature = "negative-tokio-email-module")]
fn main() {
    let _ = axutils::email::EmailClient::new;
}

#[cfg(feature = "negative-tokio-email-client")]
fn main() {
    let _ = axutils::EmailClient::new;
}

#[cfg(feature = "negative-tokio-email-utils")]
fn main() {
    let _ = axutils::EmailUtils::init;
}

#[cfg(feature = "negative-async")]
fn main() {
    use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

    let client = EmailClient::new(
        EmailConfig::new(
            "smtp.example.com",
            465,
            EmailSecurity::ImplicitTls,
            "sender@example.com",
            "password",
            "sender@example.com",
        )
        .expect("the fixture configuration should be valid"),
    )
    .expect("the fixture client should be constructible");
    let message = EmailMessage::text(vec!["receiver@example.com".to_owned()], "subject", "body")
        .expect("the fixture message should be valid");
    let _ = client.send_async(message);
}

#[cfg(feature = "tokio-only")]
fn main() {}

#[cfg(not(any(
    feature = "sync",
    feature = "all",
    feature = "negative-email-module",
    feature = "negative-email-client",
    feature = "negative-email-utils",
    feature = "negative-tokio-email-module",
    feature = "negative-tokio-email-client",
    feature = "negative-tokio-email-utils",
    feature = "negative-async",
    feature = "tokio-only"
)))]
fn main() {}
