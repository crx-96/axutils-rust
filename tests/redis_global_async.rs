#![cfg(feature = "redis-async")]

use axutils::redis::{RedisError, RedisTransportErrorKind};
use axutils::utils::RedisUtils;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{
    sync::{oneshot, Barrier},
    time as tokio_time,
};

#[path = "support/redis_server.rs"]
mod redis_server;
use redis_server::{test_config, RedisTestServer};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_init_checks_before_installing_and_can_retry_after_failure_or_cancellation() {
    let unused = RedisTestServer::start(|_| panic!("缺少 runtime 不应访问网络"));
    let url = format!("redis://{}/0", unused.address);
    std::thread::spawn(move || {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};
        let mut future = Box::pin(RedisUtils::init_async(test_config(&url)));
        assert_eq!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(Err(RedisError::RuntimeRequired))
        );
    })
    .join()
    .unwrap();
    assert!(!RedisUtils::is_initialized());
    assert!(unused.commands.lock().unwrap().is_empty());
    drop(unused);

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
            RedisUtils::init_async(test_config(&url)).await,
            Err(RedisError::Transport(expected))
        );
        assert!(!RedisUtils::is_initialized());
    }

    let rejected = RedisTestServer::start(|command| {
        Some(if command[0] == "AUTH" {
            b"-WRONGPASS test password rejected\r\n"
        } else {
            b"+OK\r\n"
        })
    });
    let url = format!("redis://:test-password@{}/0", rejected.address);
    assert_eq!(
        RedisUtils::init_async(test_config(&url)).await,
        Err(RedisError::Transport(
            RedisTransportErrorKind::Authentication
        ))
    );
    assert!(!RedisUtils::is_initialized());
    drop(rejected);

    let (sent, received) = oneshot::channel();
    let sent = Mutex::new(Some(sent));
    let waiting = RedisTestServer::start(move |command| {
        if command[0] == "PING" {
            if let Some(sent) = sent.lock().unwrap().take() {
                sent.send(()).unwrap();
            }
            Some(b"")
        } else {
            Some(b"+OK\r\n")
        }
    });
    let config = test_config(&format!("redis://{}/0", waiting.address));
    let attempt = tokio::spawn(RedisUtils::init_async(config));
    tokio_time::timeout(Duration::from_secs(2), received)
        .await
        .unwrap()
        .unwrap();
    assert!(
        !RedisUtils::is_initialized(),
        "收到 PONG 前不得保存全局状态"
    );
    attempt.abort();
    assert!(attempt.await.unwrap_err().is_cancelled());
    assert!(!RedisUtils::is_initialized());
    drop(waiting);

    let server = RedisTestServer::start(|command| {
        Some(if command[0] == "PING" {
            b"+PONG\r\n"
        } else {
            b"+OK\r\n"
        })
    });
    let url = format!("redis://{}/0", server.address);
    let barrier = Arc::new(Barrier::new(8));
    let attempts = (0..8)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let config = test_config(&url);
            tokio::spawn(async move {
                barrier.wait().await;
                RedisUtils::init_async(config).await
            })
        })
        .collect::<Vec<_>>();
    let mut successes = 0;
    for attempt in attempts {
        match attempt.await.unwrap() {
            Ok(()) => successes += 1,
            Err(error) => assert_eq!(error, RedisError::AlreadyInitialized),
        }
    }
    assert_eq!(successes, 1);
    assert!(RedisUtils::is_initialized());
    assert_eq!(
        RedisUtils::client().unwrap().ping_async().await.unwrap(),
        "PONG"
    );
    let unused = RedisTestServer::start(|_| panic!("重复初始化不应访问新目标"));
    let url = format!("redis://{}/0", unused.address);
    assert_eq!(
        RedisUtils::init(test_config(&url)),
        Err(RedisError::AlreadyInitialized)
    );
    assert_eq!(
        RedisUtils::init_async(test_config(&url)).await,
        Err(RedisError::AlreadyInitialized)
    );
    assert!(unused.commands.lock().unwrap().is_empty());
}
