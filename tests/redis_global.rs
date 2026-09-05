#![cfg(feature = "redis")]

use axutils::redis::{RedisConfig, RedisError, RedisTransportErrorKind};
use axutils::utils::RedisUtils;

#[path = "support/redis_server.rs"]
mod redis_server;
use redis_server::{test_config, RedisTestServer};

#[test]
fn global_entry_reports_state_without_exposing_a_client() {
    assert!(!RedisUtils::is_initialized());
    assert!(matches!(
        RedisUtils::client().and_then(|client| client.get::<_, u8>("missing")),
        Err(RedisError::NotInitialized)
    ));
    assert!(matches!(
        RedisConfig::single("rediss://127.0.0.1:6379/0"),
        Err(RedisError::InvalidConfig { field: "scheme" })
    ));

    // 每个失败分支都不得占用全局状态；服务只响应受控测试请求。
    for (reply, expected) in [
        (None, RedisTransportErrorKind::Network),
        (
            Some(&b"-ERR test unavailable\r\n"[..]),
            RedisTransportErrorKind::Server,
        ),
        (
            Some(&b"+NOT_PONG\r\n"[..]),
            RedisTransportErrorKind::Protocol,
        ),
        (Some(&b""[..]), RedisTransportErrorKind::Timeout),
    ] {
        let server = RedisTestServer::start(move |command| {
            if command[0] == "PING" {
                reply
            } else {
                Some(b"+OK\r\n")
            }
        });
        let url = format!("redis://{}/0", server.address);
        assert_eq!(
            RedisUtils::init(test_config(&url)),
            Err(RedisError::Transport(expected))
        );
        assert!(!RedisUtils::is_initialized());
        assert!(server
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|name| name == "PING"));
    }

    let rejected = RedisTestServer::start(|command| {
        if command[0] == "AUTH" {
            Some(b"-WRONGPASS test password rejected\r\n")
        } else {
            Some(b"+OK\r\n")
        }
    });
    let url = format!("redis://:test-password@{}/0", rejected.address);
    assert_eq!(
        RedisUtils::init(test_config(&url)),
        Err(RedisError::Transport(
            RedisTransportErrorKind::Authentication
        ))
    );
    assert!(!RedisUtils::is_initialized());
    drop(rejected);

    let server = RedisTestServer::start(|command| {
        Some(if command[0] == "PING" {
            b"+PONG\r\n"
        } else {
            b"+OK\r\n"
        })
    });
    let url = format!("redis://{}/0", server.address);
    let barrier = std::sync::Barrier::new(8);
    let results = std::thread::scope(|scope| {
        let handles = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    RedisUtils::init(test_config(&url))
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("initialization thread should not panic")
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(RedisError::AlreadyInitialized)))
            .count(),
        7
    );
    assert!(RedisUtils::is_initialized());
    assert!(server
        .commands
        .lock()
        .unwrap()
        .iter()
        .any(|name| name == "PING"));
    let unused = RedisTestServer::start(|_| panic!("重复初始化不应连接新目标"));
    assert!(matches!(
        RedisUtils::init(test_config(&format!("redis://{}/0", unused.address))),
        Err(RedisError::AlreadyInitialized)
    ));
    assert!(unused.commands.lock().unwrap().is_empty());
    let client = RedisUtils::client().unwrap();
    assert_eq!(client.ping().unwrap(), "PONG");
    assert_eq!(client.get_bytes(""), Err(RedisError::InvalidKey));
}
