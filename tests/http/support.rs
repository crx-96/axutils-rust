use super::*;

pub(super) struct TestResponse {
    pub(super) status: u16,
    pub(super) body: &'static [u8],
    pub(super) delay: Duration,
}

pub(super) fn spawn_server(responses: Vec<TestResponse>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
    listener
        .set_nonblocking(true)
        .expect("set test listener nonblocking");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let handle = thread::spawn(move || {
        let expected = responses.len();
        for (received, response) in responses.into_iter().enumerate() {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "test server received {received} of {expected} expected requests"
                        );
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("accept test request: {error}"),
                }
            };
            // Windows may inherit the listener's nonblocking mode; read timeouts do not
            // turn an accepted nonblocking stream into a blocking one.
            stream.set_nonblocking(false).unwrap_or_else(|error| {
                panic!("set test request #{received} stream blocking: {error}")
            });
            let peer = stream
                .peer_addr()
                .unwrap_or_else(|error| panic!("inspect test request #{received} peer: {error}"));
            read_request(&mut stream).unwrap_or_else(|error| {
                panic!("read test request #{received} from {peer}: {error}")
            });
            if !response.delay.is_zero() {
                thread::sleep(response.delay);
            }
            let header = format!(
                "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.status,
                response.body.len()
            );
            if let Err(error) = stream.write_all(header.as_bytes()) {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} headers not delivered: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("write test response #{received} to {peer} headers: {error}");
            }
            if let Err(error) = stream.write_all(response.body) {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} body not delivered: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("write test response #{received} to {peer} body: {error}");
            }
            if let Err(error) = stream.flush() {
                if is_client_disconnect(&error) {
                    eprintln!(
                        "test response #{received} to {peer} flush incomplete: client disconnected: {error}"
                    );
                    continue;
                }
                panic!("flush test response #{received} to {peer}: {error}");
            }
        }
    });
    (address, handle)
}

pub(super) fn is_client_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::NotConnected
    ) || matches!(error.raw_os_error(), Some(10053 | 10054))
}

pub(super) fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while request.len() < 64 * 1024 {
        let read = stream.read(&mut byte)?;
        if read == 0 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                "request ended before headers",
            ));
        }
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header_text = String::from_utf8_lossy(&request);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
        })
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = request.len();
    let mut remaining = content_length.saturating_sub(request.len().saturating_sub(body_start));
    while remaining > 0 {
        let read = stream.read(&mut byte)?;
        if read == 0 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                format!("request body ended with {remaining} bytes remaining"),
            ));
        }
        remaining -= read;
    }
    Ok(request)
}

pub(super) fn spawn_observing_server(
    response: TestResponse,
) -> (String, Arc<Mutex<Vec<u8>>>, thread::JoinHandle<()>) {
    // Bind the wildcard loopback-compatible IPv4 socket so the test can use two different
    // loopback host strings without depending on platform-specific `localhost` resolution.
    let listener = TcpListener::bind(("0.0.0.0", 0)).expect("bind observing server");
    listener
        .set_nonblocking(true)
        .expect("set observing listener nonblocking");
    let port = listener.local_addr().expect("server address").port();
    let address = format!("http://127.0.0.1:{port}");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_for_thread = Arc::clone(&observed);
    let handle = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "observing server timed out");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept observing request: {error}"),
            }
        };
        // Keep the observing fixture's accepted stream compatible with its read timeout too.
        stream
            .set_nonblocking(false)
            .expect("set observing request stream blocking");
        let peer = stream.peer_addr().expect("inspect observing request peer");
        let request = read_request(&mut stream)
            .unwrap_or_else(|error| panic!("read observing request: {error}"));
        *observed_for_thread.lock().expect("observed request lock") = request;
        let header = format!(
            "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.status,
            response.body.len()
        );
        stream
            .write_all(header.as_bytes())
            .unwrap_or_else(|error| panic!("write observing response to {peer} headers: {error}"));
        stream
            .write_all(response.body)
            .unwrap_or_else(|error| panic!("write observing response to {peer} body: {error}"));
        stream
            .flush()
            .unwrap_or_else(|error| panic!("flush observing response to {peer}: {error}"));
    });
    (address, observed, handle)
}

pub(super) fn client(base_url: &str) -> HttpClient {
    let config = HttpConfig::builder()
        .base_url(base_url)
        .expect("valid base URL")
        .request_timeout(Duration::from_secs(2))
        .expect("valid timeout")
        .connect_timeout(Duration::from_millis(500))
        .expect("valid connect timeout")
        .build()
        .expect("valid HTTP config");
    HttpClient::new(config).expect("build HTTP client")
}
