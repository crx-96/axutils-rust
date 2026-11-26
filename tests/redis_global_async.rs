#![cfg(feature = "redis-async")]

use axutils::redis::{RedisClient, RedisError, RedisTransportErrorKind};
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

#[tokio::test]
async fn concurrent_first_commands_share_failure_and_later_commands_can_retry() {
    use std::future::Future;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Waker};

    let accepting = Arc::new(AtomicBool::new(false));
    let server_accepting = Arc::clone(&accepting);
    let server = RedisTestServer::start(move |command| {
        Some(match command[0].as_str() {
            "AUTH" if !server_accepting.load(Ordering::Acquire) => b"-WRONGPASS rejected\r\n",
            "PING" => b"+PONG\r\n",
            _ => b"+OK\r\n",
        })
    });
    let client =
        RedisClient::new(test_config(&format!("redis://:test@{}/0", server.address))).unwrap();
    assert!(server.commands.lock().unwrap().is_empty());
    let mut cancelled = Box::pin(client.ping_async());
    let mut first = Box::pin(client.ping_async());
    let mut late_waiter = Box::pin(client.ping_async());
    let mut cx = Context::from_waker(Waker::noop());
    for future in [&mut cancelled, &mut first, &mut late_waiter] {
        assert!(future.as_mut().poll(&mut cx).is_pending());
    }
    // 取消初始化者后，已登记的调用仍能取得本轮认证失败。
    drop(cancelled);
    let rejected = Err(RedisError::Transport(
        RedisTransportErrorKind::Authentication,
    ));
    assert_eq!(first.await, rejected);
    assert!(!server
        .commands
        .lock()
        .unwrap()
        .iter()
        .any(|cmd| cmd == "PING"));

    accepting.store(true, Ordering::Release);
    // 故意延迟旧等待者，使新一轮在它取走失败之前就完成。
    assert_eq!(
        tokio_time::timeout(Duration::from_secs(3), client.ping_async())
            .await
            .expect("旧等待者不应阻塞新一轮建连")
            .unwrap(),
        "PONG"
    );
    let auth_count = || {
        server
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|cmd| *cmd == "AUTH")
            .count()
    };
    let after_retry = auth_count();
    assert_eq!(late_waiter.await, rejected);
    assert_eq!(client.ping_async().await.unwrap(), "PONG");
    assert_eq!(auth_count(), after_retry, "旧失败不得清掉已成功的新连接");
}

#[tokio::test]
async fn cancelling_the_only_waiter_closes_the_pending_handshake() {
    use tokio::{io::AsyncReadExt, net::TcpListener};

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("redis://:test@{}/0", listener.local_addr().unwrap());
    let handshake_timeout = Duration::from_secs(15);
    let config = test_config(&url)
        .with_connection_timeout(handshake_timeout)
        .unwrap()
        .with_response_timeout(handshake_timeout)
        .unwrap();
    let client = RedisClient::new(config).unwrap();
    let caller = client.clone();
    tokio_time::timeout(Duration::from_secs(3), async {
        let attempt = tokio::spawn(async move { caller.ping_async().await });
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.read_u8().await.expect("已开始握手");
        attempt.abort();
        assert!(attempt.await.unwrap_err().is_cancelled());
        // 保持原客户端存活，确认取消会关闭在途连接，无需等握手超时或销毁客户端。
        stream
            .read_to_end(&mut Vec::new())
            .await
            .expect("握手连接应关闭");
    })
    .await
    .expect("取消后应在握手超时之前关闭连接");
    drop(client);
}

#[tokio::test]
async fn transactions_classify_execabort_by_queued_error_codes() {
    use axutils::redis::RedisTransaction;

    let queue = |tx: &mut RedisTransaction| {
        tx.delete("first")?;
        tx.delete("second")
    };
    for (second_error, expected) in [
        (&b"-cRoSsSlOt rejected\r\n"[..], RedisError::CrossSlot),
        (
            &b"-ERR mentions CROSSSLOT\r\n"[..],
            RedisError::TransactionFailed,
        ),
    ] {
        let server = RedisTestServer::start(move |cmd| {
            Some(match cmd[0].as_str() {
                "DEL" if cmd[1] == "first" => b"-ERR unrelated\r\n",
                "DEL" => second_error,
                "EXEC" => b"-EXECABORT mentions CROSSSLOT\r\n",
                _ => b"+OK\r\n",
            })
        });
        let client =
            RedisClient::new(test_config(&format!("redis://{}/0", server.address))).unwrap();
        assert_eq!(client.transaction(queue), Err(expected));
        assert_eq!(client.transaction_async(queue).await, Err(expected));
    }
}

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
