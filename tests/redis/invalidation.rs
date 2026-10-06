use std::{
    collections::BTreeSet,
    sync::{atomic::AtomicUsize, atomic::Ordering, mpsc, Arc, Mutex},
    time::Duration,
};

use axutils::redis::{
    RedisClient, RedisError, RedisInvalidationConfig, RedisInvalidationEnqueue,
    RedisInvalidationQueue,
};
use tokio::{runtime::Builder, sync::mpsc as async_mpsc, task, time::timeout};

#[path = "../support/redis_server.rs"]
mod redis_server;
use redis_server::{test_config, RedisTestServer};

const TEST_TIMEOUT: Duration = Duration::from_secs(3);

fn queue_config() -> RedisInvalidationConfig {
    RedisInvalidationConfig {
        capacity: 16,
        batch_items: 8,
        batch_bytes: 128,
        io_timeout: Duration::from_secs(1),
        retry_delays: Vec::new(),
    }
}

fn test_client(server: &RedisTestServer) -> RedisClient {
    RedisClient::new(test_config(&format!("redis://{}/0", server.address))).unwrap()
}

fn assert_accepted(result: RedisInvalidationEnqueue, count: usize) {
    assert_eq!(result.accepted, count);
    assert_eq!(result.rejected, 0);
    assert_eq!(result.worker_error, None);
}

fn recording_server(
    reply: impl Fn(&[String]) -> Option<&'static [u8]> + Send + Sync + 'static,
) -> (RedisTestServer, async_mpsc::UnboundedReceiver<Vec<String>>) {
    let (sent, received) = async_mpsc::unbounded_channel();
    let server = RedisTestServer::start(move |command| {
        if command[0] == "DEL" {
            // 失败测试提前退出时关闭测试连接，使服务线程仍可正常清理。
            if sent.send(command[1..].to_vec()).is_err() {
                return None;
            }
            reply(&command[1..])
        } else if command[0] == "PING" {
            Some(b"+PONG\r\n")
        } else {
            Some(b"+OK\r\n")
        }
    });
    (server, received)
}

async fn next_batch(received: &mut async_mpsc::UnboundedReceiver<Vec<String>>) -> Vec<String> {
    timeout(TEST_TIMEOUT, received.recv())
        .await
        .expect("等待 DEL 超时")
        .expect("测试服务提前结束")
}

async fn assert_no_more_batches(received: &mut async_mpsc::UnboundedReceiver<Vec<String>>) {
    assert!(
        timeout(Duration::from_millis(150), received.recv())
            .await
            .is_err(),
        "不应继续发送 DEL"
    );
}

#[test]
fn public_queue_is_send_sync_and_missing_runtime_retains_accepted_keys() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RedisInvalidationQueue>();

    let (server, mut received) = recording_server(|_| Some(b":1\r\n"));
    let queue = RedisInvalidationQueue::new(test_client(&server), queue_config()).unwrap();
    let result = queue.enqueue(["outside-runtime".to_owned()]);
    assert_eq!(result.accepted, 1);
    assert_eq!(result.rejected, 0);
    assert_eq!(result.worker_error, Some(RedisError::RuntimeRequired));
    assert!(server.commands.lock().unwrap().is_empty());

    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        assert_accepted(queue.enqueue(std::iter::empty::<String>()), 0);
        assert_eq!(next_batch(&mut received).await, ["outside-runtime"]);
    });
    drop(queue);
    drop(server);
}

#[tokio::test]
async fn deduplicates_pending_keys_and_respects_count_and_byte_budgets() {
    for (batch_items, batch_bytes) in [(2, 128), (8, 8), (0, 0)] {
        let (server, mut received) = recording_server(|_| Some(b":1\r\n"));
        let queue = RedisInvalidationQueue::new(
            test_client(&server),
            RedisInvalidationConfig {
                batch_items,
                batch_bytes,
                ..queue_config()
            },
        )
        .unwrap();
        let keys = ["key0", "key1", "key2", "key3", "key4"];
        assert_accepted(
            queue.enqueue(keys.into_iter().chain(["key0", "key3"]).map(String::from)),
            7,
        );

        let mut actual = Vec::new();
        while actual.len() < keys.len() {
            let batch = next_batch(&mut received).await;
            assert!(!batch.is_empty());
            assert!(batch.len() <= batch_items.max(1));
            if batch_bytes == 0 {
                assert_eq!(batch.len(), 1);
            } else {
                assert!(batch.iter().map(String::len).sum::<usize>() <= batch_bytes);
            }
            actual.extend(batch);
        }
        actual.sort();
        assert_eq!(actual, keys);
        assert_no_more_batches(&mut received).await;
        drop(queue);
        drop(server);
    }
}

#[tokio::test]
async fn oversized_key_can_form_a_single_batch_without_blocking_other_keys() {
    let (server, mut received) = recording_server(|_| Some(b":1\r\n"));
    let queue = RedisInvalidationQueue::new(
        test_client(&server),
        RedisInvalidationConfig {
            batch_bytes: 4,
            ..queue_config()
        },
    )
    .unwrap();
    let keys = ["oversized-key", "a", "bb"];
    assert_accepted(queue.enqueue(keys.map(String::from)), 3);
    let mut actual = BTreeSet::new();
    while actual.len() < keys.len() {
        let batch = next_batch(&mut received).await;
        if batch.iter().map(String::len).sum::<usize>() > 4 {
            assert_eq!(batch, ["oversized-key"]);
        }
        for key in batch {
            assert!(actual.insert(key), "同一批入队的键只能删除一次");
        }
    }
    assert_eq!(
        actual,
        keys.map(String::from).into_iter().collect::<BTreeSet<_>>()
    );
    drop(queue);
    drop(server);
}

#[tokio::test]
async fn new_submission_after_success_wakes_the_queue() {
    let (server, mut received) = recording_server(|_| Some(b":1\r\n"));
    let client = test_client(&server);
    let queue = RedisInvalidationQueue::new(client.clone(), queue_config()).unwrap();
    assert_accepted(queue.enqueue(["first".to_owned()]), 1);
    assert_eq!(next_batch(&mut received).await, ["first"]);
    // 同一客户端往返一次，再给 worker 处理成功结果的机会。
    assert_eq!(client.ping_async().await.unwrap(), "PONG");
    task::yield_now().await;

    assert_accepted(queue.enqueue(["second".to_owned()]), 1);
    assert_eq!(next_batch(&mut received).await, ["second"]);
    drop(queue);
    drop(server);
}

#[tokio::test]
async fn old_success_or_failure_preserves_a_new_submission_of_the_in_flight_key() {
    for first_reply in [&b":1\r\n"[..], &b"-ERR test failure\r\n"[..]] {
        let (allow_reply, wait_for_reply) = mpsc::channel();
        let wait_for_reply = Mutex::new(wait_for_reply);
        let attempts = AtomicUsize::new(0);
        let (server, mut received) = recording_server(move |_| {
            if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                // 测试提前结束时关闭连接，避免服务线程在清理期间继续阻塞。
                wait_for_reply
                    .lock()
                    .unwrap()
                    .recv_timeout(TEST_TIMEOUT)
                    .ok()?;
                Some(first_reply)
            } else {
                Some(b":1\r\n")
            }
        });
        let queue = RedisInvalidationQueue::new(
            test_client(&server),
            RedisInvalidationConfig {
                capacity: 1,
                retry_delays: vec![Duration::from_secs(10)],
                ..queue_config()
            },
        )
        .unwrap();
        assert_accepted(queue.enqueue(["shared-key".to_owned()]), 1);
        assert_eq!(next_batch(&mut received).await, ["shared-key"]);
        assert_accepted(queue.enqueue(["shared-key".to_owned()]), 1);
        allow_reply.send(()).unwrap();

        // 旧成功不能清除新请求；旧失败不能把新请求延迟到十秒后的重试时间。
        assert_eq!(next_batch(&mut received).await, ["shared-key"]);
        assert_no_more_batches(&mut received).await;
        drop(queue);
        drop(server);
    }
}

#[tokio::test]
async fn server_errors_exhaust_the_configured_retry_budget() {
    let (server, mut received) = recording_server(|_| Some(b"-ERR test failure\r\n"));
    let queue = RedisInvalidationQueue::new(
        test_client(&server),
        RedisInvalidationConfig {
            retry_delays: vec![Duration::from_millis(5), Duration::from_millis(10)],
            ..queue_config()
        },
    )
    .unwrap();
    assert_accepted(queue.enqueue(["retry-key".to_owned()]), 1);
    for _ in 0..3 {
        assert_eq!(next_batch(&mut received).await, ["retry-key"]);
    }
    assert_no_more_batches(&mut received).await;
    drop(queue);
    drop(server);
}

#[tokio::test]
async fn timeouts_are_retried_only_within_the_configured_budget() {
    let (server, mut received) = recording_server(|_| Some(b""));
    let queue = RedisInvalidationQueue::new(
        test_client(&server),
        RedisInvalidationConfig {
            io_timeout: Duration::from_millis(100),
            retry_delays: vec![Duration::from_millis(5)],
            ..queue_config()
        },
    )
    .unwrap();
    assert_accepted(queue.enqueue(["timeout-key".to_owned()]), 1);
    for _ in 0..2 {
        assert_eq!(next_batch(&mut received).await, ["timeout-key"]);
    }
    assert_no_more_batches(&mut received).await;
    drop(queue);
    drop(server);
}

#[tokio::test]
async fn dropping_the_last_owner_cancels_pending_batches_and_retry_work() {
    let (allow_reply, wait_for_reply) = mpsc::channel();
    let wait_for_reply = Mutex::new(wait_for_reply);
    let attempts = AtomicUsize::new(0);
    let (server, mut received) = recording_server(move |_| {
        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            wait_for_reply
                .lock()
                .unwrap()
                .recv_timeout(TEST_TIMEOUT)
                .ok()?;
        }
        Some(b"-ERR test failure\r\n")
    });
    let queue = Arc::new(
        RedisInvalidationQueue::new(
            test_client(&server),
            RedisInvalidationConfig {
                batch_items: 1,
                retry_delays: vec![Duration::from_millis(5)],
                ..queue_config()
            },
        )
        .unwrap(),
    );
    let last_owner = Arc::clone(&queue);
    assert_accepted(queue.enqueue(["in-flight".to_owned()]), 1);
    assert_eq!(next_batch(&mut received).await, ["in-flight"]);
    drop(queue);
    assert_accepted(last_owner.enqueue(["still-pending".to_owned()]), 1);
    drop(last_owner);
    allow_reply.send(()).unwrap();
    assert_no_more_batches(&mut received).await;
    drop(server);
}
