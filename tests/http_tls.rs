#![cfg(feature = "http")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use axutils::http::{
    HttpClient, HttpConfig, HttpError, HttpMethod, HttpRequest, HttpTransportErrorKind, RetryPolicy,
};
#[cfg(feature = "http-async")]
use reqwest::{Certificate as ReqwestCertificate, Client as ReqwestClient, StatusCode};
use rustls::crypto::ring as rustls_ring;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, PrivateSec1KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use ureq::{
    tls::{
        Certificate as UreqCertificate, RootCerts as UreqRootCerts, TlsConfig as UreqTlsConfig,
        TlsProvider as UreqTlsProvider,
    },
    Agent as UreqAgent, Error as UreqError,
};

const FIXTURE_CERT_PEM: &[u8] = include_bytes!("fixtures/http_tls/server.crt");
const FIXTURE_CA_PEM: &[u8] = include_bytes!("fixtures/http_tls/ca.crt");
const FIXTURE_KEY_PEM: &[u8] = include_bytes!("fixtures/http_tls/server.key");
const RESPONSE: &[u8] = b"http-tls-fixture";

fn spawn_tls_server() -> (String, String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind TLS fixture server");
    listener
        .set_nonblocking(true)
        .expect("set TLS fixture listener nonblocking");
    let port = listener
        .local_addr()
        .expect("TLS fixture server address")
        .port();
    let localhost_url = format!("https://localhost:{port}/fixture");
    let ip_url = format!("https://127.0.0.1:{port}/fixture");
    let config = Arc::new(server_config());

    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "TLS fixture server did not receive a client connection"
                    );
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept TLS fixture connection: {error}"),
            }
        };
        serve_tls(stream, config);
    });

    (localhost_url, ip_url, handle)
}

fn server_config() -> ServerConfig {
    let certificate = CertificateDer::from_pem_slice(FIXTURE_CERT_PEM)
        .expect("valid TLS fixture certificate")
        .into_owned();
    let key =
        PrivateSec1KeyDer::from_pem_slice(FIXTURE_KEY_PEM).expect("valid TLS fixture private key");
    let provider = Arc::new(rustls_ring::default_provider());

    ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("default TLS protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![certificate], PrivateKeyDer::Sec1(key))
        .expect("matching TLS fixture certificate and key")
}

fn serve_tls(stream: TcpStream, config: Arc<ServerConfig>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let connection = ServerConnection::new(config).expect("TLS fixture server connection");
    let mut stream = StreamOwned::new(connection, stream);
    let mut request = [0_u8; 4096];
    let Ok(read) = stream.read(&mut request) else {
        // Unknown CA and hostname-mismatch tests intentionally abort during the handshake.
        return;
    };
    if read == 0 {
        return;
    }

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        RESPONSE.len()
    );
    stream
        .write_all(response.as_bytes())
        .expect("write TLS fixture response headers");
    stream
        .write_all(RESPONSE)
        .expect("write TLS fixture response");
    stream.flush().expect("flush TLS fixture response");
}

fn fixture_ureq_agent() -> UreqAgent {
    // 仅用于 loopback fixture；生产 HttpClient 不暴露自定义根证书配置。
    let certificate =
        UreqCertificate::from_pem(FIXTURE_CA_PEM).expect("valid ureq TLS fixture certificate");
    let roots = UreqRootCerts::new_with_certs(std::slice::from_ref(&certificate));
    UreqAgent::config_builder()
        .proxy(None)
        .tls_config(
            UreqTlsConfig::builder()
                .provider(UreqTlsProvider::Rustls)
                .root_certs(roots)
                .build(),
        )
        .build()
        .new_agent()
}

#[cfg(feature = "http-async")]
fn fixture_reqwest_certificate() -> ReqwestCertificate {
    ReqwestCertificate::from_pem(FIXTURE_CA_PEM).expect("valid reqwest TLS fixture root")
}

#[cfg(feature = "http-async")]
fn fixture_reqwest_client() -> ReqwestClient {
    // 仅用于 loopback fixture；生产 HttpClient 不暴露自定义根证书配置。
    ReqwestClient::builder()
        .no_proxy()
        .tls_certs_only([fixture_reqwest_certificate()])
        .build()
        .expect("build reqwest test client with the fixture root")
}

fn http_config_without_retries() -> HttpConfig {
    HttpConfig::builder()
        .request_timeout(Duration::from_secs(5))
        .expect("valid fixture request timeout")
        .connect_timeout(Duration::from_secs(2))
        .expect("valid fixture connect timeout")
        .retry_policy(
            RetryPolicy::new()
                .with_max_retries(1)
                .expect("one fixture attempt"),
        )
        .build()
        .expect("valid fixture HTTP config")
}

#[test]
fn sync_http_client_uses_webpki_roots_and_redacts_untrusted_tls_errors() {
    let (url, _, server) = spawn_tls_server();
    let sensitive_url = format!("{url}/sensitive-token");
    let client = HttpClient::new(http_config_without_retries()).expect("build HTTP client");
    let request =
        HttpRequest::new(HttpMethod::Get, sensitive_url.clone()).expect("fixture request");

    let error = client
        .execute(request)
        .expect_err("self-signed fixture must not be trusted");
    assert!(matches!(
        error,
        HttpError::Transport {
            kind: HttpTransportErrorKind::Tls,
            attempts: 1,
            exhausted: true,
        }
    ));
    let display = error.to_string();
    assert!(!display.contains(&sensitive_url));
    assert!(!display.contains("sensitive-token"));
    server.join().expect("TLS fixture server thread");
}

#[test]
fn ureq_fixture_root_allows_verified_loopback_tls() {
    let (url, _, server) = spawn_tls_server();
    let response = fixture_ureq_agent()
        .get(&url)
        .call()
        .expect("explicit fixture root should be trusted by ureq");

    assert_eq!(response.status(), 200);
    server.join().expect("TLS fixture server thread");
}

#[test]
fn ureq_fixture_root_still_checks_hostname() {
    let (_, ip_url, server) = spawn_tls_server();
    let error = fixture_ureq_agent()
        .get(&ip_url)
        .call()
        .expect_err("fixture root must not bypass hostname verification");

    assert!(matches!(
        error,
        UreqError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData
    ));
    server.join().expect("TLS fixture server thread");
}

#[cfg(feature = "http-async")]
#[test]
fn reqwest_builder_does_not_panic_without_a_preinstalled_provider() {
    let result = std::panic::catch_unwind(|| ReqwestClient::builder().no_proxy().build());
    assert!(
        result.is_ok(),
        "reqwest TLS client construction must not panic"
    );
    assert!(
        result.expect("reqwest builder panic result").is_ok(),
        "reqwest TLS client construction must succeed"
    );
}

#[cfg(feature = "http-async")]
#[tokio::test(flavor = "current_thread")]
async fn async_http_client_uses_platform_verifier_and_redacts_untrusted_tls_errors() {
    let (url, _, server) = spawn_tls_server();
    let sensitive_url = format!("{url}/sensitive-token");
    let client = HttpClient::new(http_config_without_retries()).expect("build HTTP client");
    let request =
        HttpRequest::new(HttpMethod::Get, sensitive_url.clone()).expect("fixture request");

    let error = client
        .execute_async(request)
        .await
        .expect_err("self-signed fixture must not be trusted");
    assert!(matches!(error, HttpError::Transport { attempts: 1, .. }));
    let display = error.to_string();
    assert!(!display.contains(&sensitive_url));
    assert!(!display.contains("sensitive-token"));
    server.join().expect("TLS fixture server thread");
}

#[cfg(feature = "http-async")]
#[tokio::test(flavor = "current_thread")]
async fn reqwest_fixture_root_allows_verified_loopback_tls() {
    let (url, _, server) = spawn_tls_server();
    let response = fixture_reqwest_client()
        .get(&url)
        .send()
        .await
        .expect("explicit fixture root should be trusted by reqwest");

    assert_eq!(response.status(), StatusCode::OK);
    server.join().expect("TLS fixture server thread");
}

#[cfg(feature = "http-async")]
#[tokio::test(flavor = "current_thread")]
async fn reqwest_fixture_root_still_checks_hostname() {
    let (_, ip_url, server) = spawn_tls_server();
    let error = fixture_reqwest_client()
        .get(&ip_url)
        .send()
        .await
        .expect_err("fixture root must not bypass hostname verification");

    assert!(error.is_connect());
    server.join().expect("TLS fixture server thread");
}
