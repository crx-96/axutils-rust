use super::*;

pub(super) fn server() -> AxumServer {
    AxumApp::from_router(Router::new().route("/health", get(|| async { "ok" })))
        .into_server_builder()
        .build()
        .expect("build server")
}

pub(super) async fn http_request(addr: SocketAddr, path: &str, headers: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    let request =
        format!("GET {path} HTTP/1.1\r\nhost: localhost\r\n{headers}connection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.expect("write");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.expect("read");
    String::from_utf8(response).expect("utf8")
}

#[cfg(feature = "axum-tower-http")]
pub(super) async fn raw_http(addr: SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(request).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8(response).unwrap()
}
