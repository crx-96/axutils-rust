#![cfg(all(feature = "http", feature = "serde"))]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use axutils::{HttpClient, HttpConfig, HttpRequestOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq)]
struct Reply {
    ok: bool,
}

#[derive(Serialize)]
struct Query {
    page: u32,
    keyword: String,
}

#[derive(Serialize)]
struct Payload {
    value: String,
}

type CapturedRequests = Arc<Mutex<Vec<Vec<u8>>>>;

fn spawn_server(expected_requests: usize) -> (String, CapturedRequests, thread::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test server");
    let address = format!("http://{}", listener.local_addr().expect("server address"));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_request(&mut stream);
            let is_bytes = request
                .split(|byte| *byte == b'\n')
                .next()
                .is_some_and(|line| line.windows(6).any(|window| window == b"/bytes"));
            let is_invalid_json = request
                .split(|byte| *byte == b'\n')
                .next()
                .is_some_and(|line| line.windows(8).any(|window| window == b"/invalid"));
            captured
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(request);
            let body: &[u8] = if is_bytes {
                b"\x00\xffraw"
            } else if is_invalid_json {
                b"not-json"
            } else {
                br#"{"ok":true}"#
            };
            let content_type = if is_bytes {
                "application/octet-stream"
            } else {
                "application/json"
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream
                .write_all(header.as_bytes())
                .expect("write response headers");
            stream.write_all(body).expect("write response body");
            stream.flush().expect("flush response");
        }
    });
    (address, requests, handle)
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set read timeout");
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    let header_end = loop {
        let read = stream.read(&mut byte).expect("read request");
        if read == 0 {
            panic!("request ended before headers");
        }
        request.push(byte[0]);
        if request.len() >= 4 && request.ends_with(b"\r\n\r\n") {
            break request.len();
        }
        assert!(request.len() <= 64 * 1024, "request headers are too large");
    };
    let header_text = String::from_utf8_lossy(&request);
    let content_length = header_text
        .lines()
        .find_map(|line| {
            line.strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
        })
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while request.len() < header_end.saturating_add(content_length) {
        let read = stream.read(&mut byte).expect("read request body");
        if read == 0 {
            panic!("request ended before body");
        }
        request.push(byte[0]);
    }
    request
}

fn request_line(request: &[u8]) -> &str {
    let end = request
        .windows(2)
        .position(|window| window == b"\r\n")
        .expect("request line");
    std::str::from_utf8(&request[..end]).expect("UTF-8 request line")
}

fn request_headers(request: &[u8]) -> &str {
    let end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request headers");
    std::str::from_utf8(&request[..end]).expect("UTF-8 request headers")
}

fn request_body(request: &[u8]) -> &[u8] {
    let start = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request body")
        + 4;
    &request[start..]
}

fn client(address: &str) -> HttpClient {
    let config = HttpConfig::builder()
        .base_url(address)
        .expect("base URL")
        .request_timeout(Duration::from_secs(2))
        .expect("request timeout")
        .connect_timeout(Duration::from_millis(500))
        .expect("connect timeout")
        .build()
        .expect("config");
    HttpClient::new(config).expect("client")
}

#[test]
fn sync_serde_methods_encode_queries_bodies_and_decode_json_or_bytes() {
    let (address, requests, server) = spawn_server(8);
    let client = client(&address);
    let query = || Query {
        page: 2,
        keyword: "rust lang".to_owned(),
    };
    let body = || Payload {
        value: "payload".to_owned(),
    };
    let options = || {
        Some(
            HttpRequestOptions::new()
                .with_header("x-test", "sync")
                .expect("header")
                .with_header("accept", "application/custom")
                .expect("accept header")
                .with_timeout(Duration::from_secs(1))
                .expect("timeout")
                .with_max_retries(0)
                .expect("retry count"),
        )
    };

    let existing_query_url = format!("{address}/get?fixed=1");
    let reply: Reply = client
        .get(existing_query_url, Some(query()), options())
        .expect("GET");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client.post("/post", Some(body()), options()).expect("POST");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .delete("/delete", Some(query()), None)
        .expect("DELETE");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client.patch("/patch", Some(body()), None).expect("PATCH");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client.put("/put", Some(body()), None).expect("PUT");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .options("/options", Some(query()), None)
        .expect("OPTIONS");
    assert_eq!(reply, Reply { ok: true });

    assert_eq!(
        client
            .get_bytes("/bytes-get", Some(query()), None)
            .expect("GET bytes"),
        b"\x00\xffraw"
    );
    assert_eq!(
        client
            .post_bytes("/bytes-post", Some(body()), None)
            .expect("POST bytes"),
        b"\x00\xffraw"
    );

    server.join().expect("server thread");
    let requests = requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(requests.len(), 8);
    assert!(request_line(&requests[0])
        .starts_with("GET /get?fixed=1&page=2&keyword=rust+lang HTTP/1.1"));
    assert!(request_line(&requests[1]).starts_with("POST /post HTTP/1.1"));
    assert!(
        request_line(&requests[2]).starts_with("DELETE /delete?page=2&keyword=rust+lang HTTP/1.1")
    );
    assert!(request_line(&requests[3]).starts_with("PATCH /patch HTTP/1.1"));
    assert!(request_line(&requests[4]).starts_with("PUT /put HTTP/1.1"));
    assert!(request_line(&requests[5])
        .starts_with("OPTIONS /options?page=2&keyword=rust+lang HTTP/1.1"));
    assert!(
        request_line(&requests[6]).starts_with("GET /bytes-get?page=2&keyword=rust+lang HTTP/1.1")
    );
    assert!(request_line(&requests[7]).starts_with("POST /bytes-post HTTP/1.1"));
    assert!(request_headers(&requests[0]).contains("x-test: sync"));
    assert!(request_headers(&requests[0]).contains("accept: application/custom"));
    assert!(!request_headers(&requests[0]).contains("accept: application/json"));
    assert!(request_headers(&requests[2]).contains("accept: application/json"));
    assert!(request_headers(&requests[1]).contains("content-type: application/json"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(request_body(&requests[1])).expect("POST JSON"),
        serde_json::json!({"value": "payload"})
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(request_body(&requests[3]))
            .expect("PATCH JSON"),
        serde_json::json!({"value": "payload"})
    );
}

#[test]
fn serde_shortcuts_return_stable_serialization_errors() {
    use serde::ser::{Error as SerdeError, Serializer};

    struct FailingSerialize;

    impl Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("secret serializer detail"))
        }
    }

    let client = client("http://127.0.0.1:1");
    assert_eq!(
        client
            .post::<Reply, _>("http://127.0.0.1:1", Some(FailingSerialize), None)
            .expect_err("body serialization should fail"),
        axutils::HttpError::JsonSerialize
    );
    assert_eq!(
        client
            .get::<Reply, _>("http://127.0.0.1:1", Some(FailingSerialize), None)
            .expect_err("query serialization should fail"),
        axutils::HttpError::QuerySerialize
    );
}

#[test]
fn serde_shortcuts_reject_sensitive_header_override() {
    let config = HttpConfig::builder()
        .with_default_header("authorization", "Bearer default")
        .expect("default authorization header")
        .build()
        .expect("config");
    let client = HttpClient::new(config).expect("client");
    let options = HttpRequestOptions::new()
        .with_header("authorization", "Bearer override")
        .expect("request authorization header");

    let error = client
        .get::<Reply, _>("http://127.0.0.1:1", None::<()>, Some(options))
        .expect_err("sensitive default header must not be overridden");
    assert_eq!(error, axutils::HttpError::DuplicateSensitiveHeader);
}

#[test]
fn serde_shortcuts_hide_json_decode_details() {
    let (address, _requests, server) = spawn_server(1);
    let client = client(&address);
    let error = client
        .get::<Reply, _>("/invalid", None::<()>, None)
        .expect_err("invalid JSON should fail");
    assert_eq!(error, axutils::HttpError::JsonDeserialize);
    assert!(!error.to_string().contains("not-json"));
    server.join().expect("server thread");
}

#[cfg(feature = "tokio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_serde_methods_cover_common_verbs_and_bytes() {
    let (address, requests, server) = spawn_server(8);
    let client = client(&address);
    let query = || Query {
        page: 3,
        keyword: "async".to_owned(),
    };
    let body = || Payload {
        value: "async-body".to_owned(),
    };

    let reply: Reply = client
        .get_async("/get", Some(query()), None)
        .await
        .expect("GET");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .post_async("/post", Some(body()), None)
        .await
        .expect("POST");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .delete_async("/delete", Some(query()), None)
        .await
        .expect("DELETE");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .patch_async("/patch", Some(body()), None)
        .await
        .expect("PATCH");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .put_async("/put", Some(body()), None)
        .await
        .expect("PUT");
    assert_eq!(reply, Reply { ok: true });
    let reply: Reply = client
        .options_async("/options", Some(query()), None)
        .await
        .expect("OPTIONS");
    assert_eq!(reply, Reply { ok: true });

    assert_eq!(
        client
            .get_bytes_async("/bytes-get", Some(query()), None)
            .await
            .expect("GET bytes"),
        b"\x00\xffraw"
    );
    assert_eq!(
        client
            .post_bytes_async("/bytes-post", Some(body()), None)
            .await
            .expect("POST bytes"),
        b"\x00\xffraw"
    );

    server.join().expect("server thread");
    let requests = requests
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(requests.len(), 8);
    assert!(request_line(&requests[0]).starts_with("GET /get?page=3&keyword=async HTTP/1.1"));
    assert!(request_line(&requests[1]).starts_with("POST /post HTTP/1.1"));
    assert!(request_line(&requests[2]).starts_with("DELETE /delete?page=3&keyword=async HTTP/1.1"));
    assert!(
        request_line(&requests[5]).starts_with("OPTIONS /options?page=3&keyword=async HTTP/1.1")
    );
    assert!(request_headers(&requests[1]).contains("content-type: application/json"));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(request_body(&requests[1]))
            .expect("async POST JSON"),
        serde_json::json!({"value": "async-body"})
    );
}
