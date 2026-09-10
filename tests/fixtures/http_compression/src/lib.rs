//! 下游同时依赖 HTTP provider 时的真实 loopback 响应契约。

#[cfg(test)]
mod tests {
    use axutils::http::{HttpClient, HttpConfig, HttpError, HttpMethod, HttpRequest, RetryPolicy};
    #[cfg(feature = "compression")]
    use reqwest::Client as ProviderClient;
    use std::{
        io::{ErrorKind, Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };
    use tokio::runtime::Builder as RuntimeBuilder;

    const PLAIN: &[u8] = b"payload";
    // 固定有效压缩帧；测试不依赖额外编码库生成输入。
    const ENCODED: &[(&str, &[u8])] = &[
        (
            "gzip",
            &[
                31, 139, 8, 0, 0, 0, 0, 0, 2, 10, 43, 72, 172, 204, 201, 79, 76, 1, 0, 21, 106, 44,
                66, 7, 0, 0, 0,
            ],
        ),
        (
            "br",
            &[
                0x0b, 0x03, 0x80, 0x70, 0x61, 0x79, 0x6c, 0x6f, 0x61, 0x64, 0x03,
            ],
        ),
        (
            "deflate",
            &[
                120, 156, 43, 72, 172, 204, 201, 79, 76, 1, 0, 11, 221, 2, 235,
            ],
        ),
        ("zstd", b"\x28\xb5\x2f\xfd\x20\x07\x39\x00\x00payload"),
    ];

    fn server(encoding: &'static str, body: &'static [u8]) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "loopback accept timed out");
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("loopback accept failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 8192, "request header limit exceeded");
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            // 一次写入完整响应，避免客户端拒绝超大 Content-Length 时与分段写入竞争。
            let mut response = format!("HTTP/1.1 200 OK\r\nContent-Encoding: {encoding}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
            response.extend_from_slice(body);
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
            String::from_utf8(request).unwrap()
        });
        (url, server)
    }

    fn client() -> HttpClient {
        client_with_limit(1024)
    }

    fn client_with_limit(limit: usize) -> HttpClient {
        HttpClient::new(
            HttpConfig::builder()
                .request_timeout(Duration::from_secs(2))
                .unwrap()
                .max_response_body_bytes(limit)
                .unwrap()
                .retry_policy(RetryPolicy::new().with_max_retries(1).unwrap())
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn sync_matches_declared_ureq_feature_boundary() {
        let client = client();
        for &(encoding, encoded) in ENCODED {
            let (url, server) = server(encoding, encoded);
            let response = client
                .execute(HttpRequest::new(HttpMethod::Get, url).unwrap())
                .unwrap();
            let decoded = cfg!(feature = "compression") && matches!(encoding, "gzip" | "br");
            assert_eq!(
                response.body(),
                if decoded { PLAIN } else { encoded },
                "{encoding}"
            );
            assert_eq!(
                response.headers().get("content-encoding"),
                if decoded {
                    None
                } else {
                    Some(encoding.as_bytes())
                }
            );
            let length = encoded.len().to_string();
            assert_eq!(
                response.headers().get("content-length"),
                if decoded {
                    None
                } else {
                    Some(length.as_bytes())
                }
            );
            assert!(!server
                .join()
                .unwrap()
                .to_ascii_lowercase()
                .contains("accept-encoding:"));
        }
    }

    #[test]
    fn async_keeps_encoded_bytes_and_headers_with_any_provider_features() {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let client = client();
            for &(encoding, encoded) in ENCODED {
                let (url, server) = server(encoding, encoded);
                let response = client
                    .execute_async(HttpRequest::new(HttpMethod::Get, url).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.body(), encoded, "{encoding}");
                assert_eq!(
                    response.headers().get("content-encoding"),
                    Some(encoding.as_bytes())
                );
                assert_eq!(
                    response.headers().get("content-length"),
                    Some(encoded.len().to_string().as_bytes())
                );
                assert!(!server
                    .join()
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("accept-encoding:"));
            }
        });
    }

    #[cfg(feature = "compression")]
    #[test]
    fn provider_decodes_every_fixture_when_decompression_is_enabled() {
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let client = ProviderClient::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            for &(encoding, encoded) in ENCODED {
                let (url, server) = server(encoding, encoded);
                assert_eq!(
                    client
                        .get(url)
                        .send()
                        .await
                        .unwrap()
                        .bytes()
                        .await
                        .unwrap()
                        .as_ref(),
                    PLAIN,
                    "{encoding}"
                );
                server.join().unwrap();
            }
        });
    }

    #[test]
    fn response_limit_applies_to_the_returned_bytes() {
        let client = client_with_limit(PLAIN.len());
        let runtime = RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for &(encoding, encoded) in ENCODED {
            let (url, server) = server(encoding, encoded);
            let response = client.execute(HttpRequest::new(HttpMethod::Get, url).unwrap());
            if cfg!(feature = "compression") && matches!(encoding, "gzip" | "br") {
                assert_eq!(response.unwrap().body(), PLAIN);
            } else {
                assert!(
                    matches!(response, Err(HttpError::ResponseTooLarge { limit }) if limit == PLAIN.len())
                );
            }
            server.join().unwrap();

            let (url, server) = self::server(encoding, encoded);
            let response = runtime
                .block_on(client.execute_async(HttpRequest::new(HttpMethod::Get, url).unwrap()));
            assert!(
                matches!(response, Err(HttpError::ResponseTooLarge { limit }) if limit == PLAIN.len())
            );
            server.join().unwrap();
        }
    }
}
