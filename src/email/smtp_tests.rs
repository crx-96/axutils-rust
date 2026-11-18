use std::{
    fs,
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use lettre::transport::smtp::client::{Certificate, TlsParameters};
use rustls::{
    crypto::ring,
    pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, PrivateSec1KeyDer},
    ServerConfig, ServerConnection, StreamOwned,
};

use super::{
    EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity, EmailTransportErrorKind,
};

type Session = BufReader<StreamOwned<ServerConnection, TcpStream>>;
const WAIT: Duration = Duration::from_secs(6);

fn config(port: u16, security: EmailSecurity) -> EmailConfig {
    EmailConfig::new(
        "localhost",
        port,
        security,
        "fixture",
        "fixture",
        "sender@example.invalid",
    )
    .unwrap()
}

fn message() -> EmailMessage {
    EmailMessage::text(vec!["recipient@example.invalid".into()], "fixture", "body").unwrap()
}

#[test]
fn client_is_send_sync_and_constructor_does_not_connect() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EmailClient>();
    for security in [EmailSecurity::ImplicitTls, EmailSecurity::StartTls] {
        assert!(EmailClient::new(config(1, security)).is_ok());
    }
}

fn command(reader: &mut impl BufRead, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.starts_with(prefix), "unexpected SMTP command");
}

fn reply<S: Read + Write>(reader: &mut BufReader<S>, line: &[u8]) {
    reader.get_mut().write_all(line).unwrap();
    reader.get_mut().flush().unwrap();
}

fn fixture(
    security: EmailSecurity,
    timeout: Duration,
    serve: impl FnOnce(&mut Session) + Send + 'static,
) -> (EmailClient, thread::JoinHandle<()>) {
    // TLS 用例运行时读取仓库的本地 fixture，发布包的编译不依赖 tests/ 目录。
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/http_tls");
    let ca = fs::read(fixtures.join("ca.crt")).unwrap();
    let cert = CertificateDer::from_pem_file(fixtures.join("server.crt")).unwrap();
    let key = PrivateSec1KeyDer::from_pem_file(fixtures.join("server.key")).unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut config = config(listener.local_addr().unwrap().port(), security)
        .with_timeout(timeout)
        .unwrap();
    config.host = Ipv4Addr::LOCALHOST.to_string();
    let tls = TlsParameters::builder("localhost".into())
        .add_root_certificate(Certificate::from_pem(&ca).unwrap())
        .build()
        .unwrap();
    let client = EmailClient::with_tls(config, tls);
    let server = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert], PrivateKeyDer::Sec1(key))
        .unwrap();
    listener.set_nonblocking(true).unwrap();
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + WAIT;
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(2))
                }
                _ => panic!("loopback SMTP accept failed"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        stream.set_write_timeout(Some(WAIT)).unwrap();
        let mut plain = BufReader::new(stream);
        if security == EmailSecurity::StartTls {
            reply(&mut plain, b"220 localhost ready\r\n");
            command(&mut plain, "EHLO ");
            reply(&mut plain, b"250-localhost\r\n250 STARTTLS\r\n");
            command(&mut plain, "STARTTLS\r\n");
            reply(&mut plain, b"220 go ahead\r\n");
        }
        let conn = ServerConnection::new(Arc::new(server)).unwrap();
        let mut session = BufReader::new(StreamOwned::new(conn, plain.into_inner()));
        if security == EmailSecurity::ImplicitTls {
            reply(&mut session, b"220 localhost ready\r\n");
        }
        command(&mut session, "EHLO ");
        reply(&mut session, b"250-localhost\r\n250 AUTH PLAIN\r\n");
        command(&mut session, "AUTH PLAIN ");
        reply(&mut session, b"235 authenticated\r\n");
        serve(&mut session);
    });
    (client, handle)
}

fn receive_data(session: &mut Session) {
    command(session, "MAIL FROM:");
    reply(session, b"250 ok\r\n");
    command(session, "RCPT TO:");
    reply(session, b"250 ok\r\n");
    command(session, "DATA\r\n");
    reply(session, b"354 continue\r\n");
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(session.read_line(&mut line).unwrap(), 0);
        if line == ".\r\n" {
            return;
        }
    }
}

#[test]
fn sync_checks_final_data_reply() {
    for (response, success) in [
        (b"250 accepted\r\n".as_slice(), true),
        (b"354 continue\r\n", false),
    ] {
        let (client, server) = fixture(EmailSecurity::ImplicitTls, WAIT, move |s| {
            receive_data(s);
            reply(s, response);
        });
        let result = client.send(message());
        if success {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(EmailError::Transport(EmailTransportErrorKind::SmtpResponse))
            ));
        }
        drop(client);
        server.join().unwrap();
    }
}

#[cfg(feature = "email-async")]
mod asynchronous {
    use super::*;
    use std::{
        future::Future,
        sync::mpsc,
        task::{Context, Poll, Waker},
    };
    use tokio::{
        runtime::{Builder, Runtime},
        task::spawn_blocking,
        time,
    };

    fn runtime() -> Runtime {
        Builder::new_current_thread().enable_all().build().unwrap()
    }

    fn expect_closed(s: &mut Session) {
        let closed = match s.read(&mut [0_u8]) {
            Ok(n) => n == 0,
            Err(e) => matches!(
                e.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
            ),
        };
        assert!(
            closed,
            "expected disconnect without a QUIT command or socket timeout"
        );
    }

    #[test]
    fn async_send_without_runtime_returns_client_error() {
        let client = EmailClient::new(config(1, EmailSecurity::ImplicitTls)).unwrap();
        let mut future = Box::pin(client.send_async(message()));
        let result = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        assert!(matches!(
            result,
            Poll::Ready(Err(EmailError::Transport(EmailTransportErrorKind::Client)))
        ));
    }

    #[test]
    fn timeout_after_data_disconnects_while_client_remains_alive() {
        let (client, server) = fixture(EmailSecurity::StartTls, Duration::from_secs(1), |s| {
            receive_data(s);
            expect_closed(s);
        });
        let rt = runtime();
        assert!(matches!(
            rt.block_on(client.send_async(message())),
            Err(EmailError::Transport(EmailTransportErrorKind::Timeout))
        ));
        server.join().unwrap();
    }

    #[test]
    fn pending_send_dropped_outside_runtime_disconnects_after_data() {
        let (sent, received) = mpsc::channel();
        let (client, server) = fixture(EmailSecurity::ImplicitTls, WAIT, move |s| {
            receive_data(s);
            sent.send(()).unwrap();
            expect_closed(s);
        });
        let rt = runtime();
        let mut send = Box::pin(client.send_async(message()));
        rt.block_on(async {
            tokio::select! {
                _ = &mut send => panic!("send completed without final DATA reply"),
                ready = spawn_blocking(move || received.recv_timeout(WAIT)) => ready.unwrap().unwrap(),
            }
        });
        drop(send);
        server.join().unwrap(); // current-thread runtime is not being driven
    }

    #[tokio::test]
    async fn intermediate_final_reply_discards_connection() {
        let (client, server) = fixture(EmailSecurity::ImplicitTls, WAIT, |s| {
            receive_data(s);
            reply(s, b"354 continue\r\n");
            expect_closed(s);
        });
        assert!(matches!(
            client.send_async(message()).await,
            Err(EmailError::Transport(EmailTransportErrorKind::SmtpResponse))
        ));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn negative_data_reply_with_unanswered_quit_uses_send_deadline() {
        let (client, server) = fixture(EmailSecurity::ImplicitTls, Duration::from_secs(1), |s| {
            receive_data(s);
            reply(s, b"550 rejected\r\n");
            command(s, "QUIT\r\n");
            expect_closed(s);
        });
        assert!(matches!(
            client.send_async(message()).await,
            Err(EmailError::Transport(EmailTransportErrorKind::Timeout))
        ));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn peer_disconnecting_after_data_is_an_error() {
        let (client, server) = fixture(EmailSecurity::ImplicitTls, WAIT, receive_data);
        assert!(matches!(
            client.send_async(message()).await,
            Err(EmailError::Transport(_))
        ));
        server.join().unwrap();
    }

    #[test]
    fn completed_sessions_reuse_connection_and_drop_after_runtime_shutdown() {
        let (client, server) = fixture(EmailSecurity::StartTls, WAIT, |s| {
            receive_data(s);
            reply(s, b"250 accepted\r\n");
            command(s, "NOOP\r\n");
            reply(s, b"250 ok\r\n");
            receive_data(s);
            reply(s, b"250 accepted\r\n");
            expect_closed(s);
        });
        let rt = runtime();
        rt.block_on(client.send_async(message())).unwrap();
        rt.block_on(client.send_async(message())).unwrap();
        drop(rt);
        drop(client);
        server.join().unwrap();
    }

    #[tokio::test]
    async fn idle_deadline_closes_connection_without_another_send() {
        let (client, server) = fixture(EmailSecurity::ImplicitTls, WAIT, |s| {
            receive_data(s);
            reply(s, b"250 accepted\r\n");
            expect_closed(s);
        });
        client.send_async(message()).await.unwrap();
        time::pause();
        time::advance(Duration::from_secs(61)).await;
        // 保持客户端存活并持续驱动定时任务，等待服务端观察到断开。
        spawn_blocking(move || server.join())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn async_send_times_out_waiting_for_smtp_command_response() {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::TcpListener,
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut config = config(
            listener.local_addr().unwrap().port(),
            EmailSecurity::StartTls,
        )
        .with_timeout(Duration::from_secs(1))
        .unwrap();
        config.host = Ipv4Addr::LOCALHOST.to_string();
        let client = EmailClient::new(config).unwrap();
        let send = tokio::spawn(async move { client.send_async(message()).await });
        time::timeout(WAIT, async {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"220 localhost ready\r\n").await.unwrap();
            let mut line = String::new();
            BufReader::new(&mut stream)
                .read_line(&mut line)
                .await
                .unwrap();
            assert!(line.starts_with("EHLO "));
            assert!(matches!(
                send.await.unwrap(),
                Err(EmailError::Transport(EmailTransportErrorKind::Timeout))
            ));
        })
        .await
        .unwrap();
    }
}
