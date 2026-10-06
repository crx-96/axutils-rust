use std::{sync::Barrier, thread};

use ::tokio::{runtime::Builder, task, time};

use super::*;
use crate::redis::RedisConfig;

fn config() -> RedisInvalidationConfig {
    RedisInvalidationConfig {
        capacity: 3,
        batch_items: 2,
        batch_bytes: 8,
        io_timeout: Duration::from_secs(1),
        retry_delays: vec![Duration::from_secs(2), Duration::from_secs(5)],
    }
}

fn queue(config: RedisInvalidationConfig) -> RedisInvalidationQueue {
    let client =
        RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap()).unwrap();
    RedisInvalidationQueue::new(client, config).unwrap()
}

fn keys(batch: &pending::Batch) -> Vec<&str> {
    batch.iter().map(|(key, _)| key.as_str()).collect()
}

#[test]
fn construction_is_lazy_zero_capacity_rejects_and_empty_enqueue_needs_no_runtime() {
    let queue = queue(RedisInvalidationConfig {
        capacity: 0,
        batch_items: 0,
        ..config()
    });
    assert!(queue.worker.lock().unwrap().is_none());
    assert_eq!(queue.config.batch_items, 1);
    assert_eq!(queue.enqueue([]), RedisInvalidationEnqueue::default());
    assert_eq!(
        queue.enqueue(["a".into(), "a".into()]),
        RedisInvalidationEnqueue {
            accepted: 0,
            rejected: 2,
            worker_error: None,
        }
    );
    assert!(queue.worker.lock().unwrap().is_none());
}

#[test]
fn rejects_unrepresentable_time_budgets_and_preserves_zero_delays() {
    for field in ["io_timeout", "retry_delays"] {
        let mut config = config();
        if field == "io_timeout" {
            config.io_timeout = Duration::MAX;
        } else {
            config.retry_delays = vec![Duration::MAX];
        }
        let client =
            RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap()).unwrap();
        assert_eq!(
            RedisInvalidationQueue::new(client, config).unwrap_err(),
            RedisError::InvalidConfig { field }
        );
    }
    let queue = queue(RedisInvalidationConfig {
        io_timeout: Duration::ZERO,
        retry_delays: vec![Duration::ZERO],
        ..config()
    });
    assert!(queue.worker.lock().unwrap().is_none());
}

#[test]
fn bounded_admission_reports_partial_success_and_refreshes_at_capacity() {
    let queue = queue(config());
    let report = queue.enqueue(["a", "b", "c", "a", "d"].map(String::from));
    assert_eq!(
        report,
        RedisInvalidationEnqueue {
            accepted: 4,
            rejected: 1,
            worker_error: Some(RedisError::RuntimeRequired),
        }
    );
    assert_eq!(
        keys(&queue.pending.take_batch(&queue.config, Instant::now()).0),
        ["a", "b"]
    );
    assert_eq!(
        keys(&queue.pending.take_batch(&queue.config, Instant::now()).0),
        ["c"]
    );
}

#[test]
fn concurrent_producers_share_one_capacity_and_deduplicate() {
    let queue = Arc::new(queue(config()));
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let queue = Arc::clone(&queue);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                queue.enqueue(["shared".into(), format!("unique-{i}")])
            })
        })
        .collect();
    let reports: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(
        reports.iter().map(|report| report.accepted).sum::<usize>(),
        10
    );
    assert_eq!(
        reports.iter().map(|report| report.rejected).sum::<usize>(),
        6
    );
    assert!(reports
        .iter()
        .all(|report| report.worker_error == Some(RedisError::RuntimeRequired)));
    let mut config = config();
    config.batch_items = 10;
    config.batch_bytes = 100;
    let (batch, next) = queue.pending.take_batch(&config, Instant::now());
    assert_eq!(batch.len(), 3);
    assert!(keys(&batch).contains(&"shared"));
    assert_eq!(next, None);
}

#[tokio::test(start_paused = true)]
async fn retry_delays_are_ordered_finite_and_expire_at_the_boundary() {
    let pending = Pending::default();
    let config = config();
    let _ = pending.enqueue(["a".into()], config.capacity);
    let (first, _) = pending.take_batch(&config, Instant::now());
    let started = Instant::now();
    pending.retry(first, &config, started);
    let (early, due) = pending.take_batch(&config, started);
    assert!(early.is_empty());
    assert_eq!(due, Some(started + Duration::from_secs(2)));
    time::advance(Duration::from_millis(1999)).await;
    assert!(pending.take_batch(&config, Instant::now()).0.is_empty());
    time::advance(Duration::from_millis(1)).await;
    let (second, _) = pending.take_batch(&config, Instant::now());
    assert_eq!(keys(&second), ["a"]);
    pending.retry(second, &config, Instant::now());
    assert_eq!(
        pending.take_batch(&config, Instant::now()).1,
        Some(started + Duration::from_secs(7))
    );
    time::advance(Duration::from_secs(5)).await;
    let (third, _) = pending.take_batch(&config, Instant::now());
    assert_eq!(keys(&third), ["a"]);
    pending.retry(third, &config, Instant::now());
    let (exhausted, due) = pending.take_batch(&config, Instant::now());
    assert!(exhausted.is_empty());
    assert_eq!(due, None);
}

#[tokio::test(start_paused = true)]
async fn resubmitting_during_backoff_resets_deadline_and_retry_budget() {
    let pending = Pending::default();
    let config = config();
    let _ = pending.enqueue(["a".into()], 1);
    let (first, _) = pending.take_batch(&config, Instant::now());
    pending.retry(first, &config, Instant::now());
    time::advance(Duration::from_secs(1)).await;
    let (report, _) = pending.enqueue(["a".into(), "b".into()], 1);
    assert_eq!(report.accepted, 1);
    assert_eq!(report.rejected, 1);
    let (refreshed, _) = pending.take_batch(&config, Instant::now());
    assert_eq!(keys(&refreshed), ["a"]);
    pending.retry(refreshed, &config, Instant::now());
    assert_eq!(
        pending.take_batch(&config, Instant::now()).1,
        Some(Instant::now() + Duration::from_secs(2))
    );
}

#[tokio::test(start_paused = true)]
async fn in_flight_batch_frees_capacity_and_failed_old_work_cannot_replace_new_work() {
    let pending = Pending::default();
    let config = RedisInvalidationConfig {
        capacity: 2,
        ..config()
    };
    let _ = pending.enqueue(["a".into(), "b".into()], config.capacity);
    let (in_flight, _) = pending.take_batch(&config, Instant::now());
    assert_eq!(in_flight.len(), 2);
    let (report, _) = pending.enqueue(["b".into(), "c".into()], config.capacity);
    assert_eq!(report.accepted, 2);
    pending.retry(in_flight, &config, Instant::now());
    let (new_work, due) = pending.take_batch(&config, Instant::now());
    assert_eq!(keys(&new_work), ["b", "c"]);
    assert_eq!(due, None);
    time::advance(Duration::from_secs(20)).await;
    assert!(pending.take_batch(&config, Instant::now()).0.is_empty());
}

#[tokio::test(start_paused = true)]
async fn bytes_count_utf8_and_oversized_first_key_makes_progress() {
    let pending = Pending::default();
    let config = RedisInvalidationConfig {
        batch_items: 10,
        batch_bytes: 4,
        ..config()
    };
    let _ = pending.enqueue(["一", "二", "超长字符串"].map(String::from), 3);
    for expected in ["一", "二", "超长字符串"] {
        let (batch, _) = pending.take_batch(&config, Instant::now());
        assert_eq!(keys(&batch), [expected]);
    }
    assert!(pending.take_batch(&config, Instant::now()).0.is_empty());
}

#[tokio::test(start_paused = true)]
async fn notification_between_empty_check_and_wait_is_not_lost() {
    let pending = Pending::default();
    assert!(pending.take_batch(&config(), Instant::now()).0.is_empty());
    let _ = pending.enqueue(["a".into()], 1);
    pending.ready.notify_one();
    assert!(time::timeout(Duration::ZERO, pending.ready.notified())
        .await
        .is_ok());
    assert_eq!(
        keys(&pending.take_batch(&config(), Instant::now()).0),
        ["a"]
    );
}

#[tokio::test(start_paused = true)]
async fn last_owner_drop_requests_abort_and_runtime_releases_shared_state() {
    let queue = Arc::new(queue(config()));
    let retained_owner = Arc::clone(&queue);
    let weak_pending = Arc::downgrade(&queue.pending);
    assert_eq!(queue.enqueue([String::new()]).worker_error, None);
    task::yield_now().await;
    drop(queue);
    assert!(weak_pending.upgrade().is_some());
    {
        let worker = retained_owner.worker.lock().unwrap();
        assert!(!worker.as_ref().unwrap().is_finished());
    }
    drop(retained_owner);
    // Drop 不负责推进 runtime；实际共享资源在取消被调度之后释放。
    for _ in 0..10 {
        task::yield_now().await;
        if weak_pending.upgrade().is_none() {
            return;
        }
    }
    panic!("worker abort 后未释放 pending");
}

#[test]
fn worker_exit_keeps_pending_work_and_a_later_enqueue_restarts_it() {
    let queue = queue(config());
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    // 只进入 context 并登记任务而不 poll，关闭后所有键仍在 pending。
    {
        let _context = runtime.enter();
        assert_eq!(queue.enqueue([String::new()]).worker_error, None);
    }
    drop(runtime);
    assert!(queue.worker.lock().unwrap().as_ref().unwrap().is_finished());
    let report = queue.enqueue([]);
    assert_eq!(report.worker_error, Some(RedisError::RuntimeRequired));
    let next_runtime = Builder::new_current_thread().enable_all().build().unwrap();
    next_runtime.block_on(async {
        assert_eq!(queue.enqueue([]).worker_error, None);
        task::yield_now().await;
        assert!(!queue.worker.lock().unwrap().as_ref().unwrap().is_finished());
        let (batch, due) = queue.pending.take_batch(&config(), Instant::now());
        assert!(batch.is_empty());
        assert!(due.is_some(), "本地 InvalidKey 应进入有限退避重试");
    });
}

#[tokio::test(start_paused = true)]
async fn new_submission_wakes_a_backing_off_worker_and_resets_its_retry_budget() {
    let queue = queue(config());
    assert_eq!(queue.enqueue([String::new()]).accepted, 1);
    task::yield_now().await;
    assert_eq!(
        queue.pending.take_batch(&config(), Instant::now()).1,
        Some(Instant::now() + Duration::from_secs(2))
    );

    time::advance(Duration::from_secs(1)).await;
    assert_eq!(queue.enqueue([String::new()]).accepted, 1);
    task::yield_now().await;
    // InvalidKey 不访问网络：立即失败后应重新使用第一档退避，而非保留旧 due 或等待它。
    assert_eq!(
        queue.pending.take_batch(&config(), Instant::now()).1,
        Some(Instant::now() + Duration::from_secs(2))
    );
    time::advance(Duration::from_secs(2)).await;
    task::yield_now().await;
    assert_eq!(
        queue.pending.take_batch(&config(), Instant::now()).1,
        Some(Instant::now() + Duration::from_secs(5))
    );
    time::advance(Duration::from_secs(5)).await;
    task::yield_now().await;
    assert_eq!(queue.pending.take_batch(&config(), Instant::now()).1, None);
}

#[test]
fn missing_time_driver_finishes_worker_and_loses_only_taken_batch() {
    let queue = queue(RedisInvalidationConfig {
        batch_items: 1,
        ..config()
    });
    let runtime = Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        assert_eq!(queue.enqueue(["a".into(), "b".into()]).worker_error, None);
        task::yield_now().await;
        assert!(queue.worker.lock().unwrap().as_ref().unwrap().is_finished());
        assert_eq!(
            keys(&queue.pending.take_batch(&config(), Instant::now()).0),
            ["b"]
        );
    });
}

#[tokio::test]
async fn immediate_failures_yield_before_exhausting_retry_budget() {
    use std::{future::poll_fn, future::Future, task::Poll};

    let queue = queue(RedisInvalidationConfig {
        retry_delays: vec![Duration::ZERO; 1024],
        ..config()
    });
    let _ = queue.pending.enqueue([String::new()], 1);
    let mut worker = Box::pin(RedisInvalidationQueue::run(
        Arc::clone(&queue.pending),
        queue.client.clone(),
        Arc::clone(&queue.config),
    ));

    // 直接驱动一次 poll：本地 InvalidKey 无 I/O 等待，仍须留下供其他任务运行和取消的边界。
    poll_fn(|context| {
        assert!(worker.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(
        keys(&queue.pending.take_batch(&queue.config, Instant::now()).0),
        [""],
        "worker 不应在一次 poll 中耗尽所有立即失败的重试"
    );
}

#[test]
fn debug_and_enqueue_errors_do_not_contain_keys() {
    let queue = queue(config());
    let report = queue.enqueue(["sensitive-cache-key".into()]);
    for text in [
        format!("{queue:?}"),
        format!("{report:?}"),
        report.worker_error.unwrap().to_string(),
    ] {
        assert!(!text.contains("sensitive-cache-key"));
        assert!(!text.contains("127.0.0.1"));
    }
}

#[test]
fn iterator_panic_preserves_already_admitted_keys_and_later_enqueue_recovers() {
    use std::panic::{self, AssertUnwindSafe};
    let queue = queue(config());
    let mut count = 0;
    let input = std::iter::from_fn(|| {
        count += 1;
        if count == 1 {
            Some("a".to_owned())
        } else {
            panic!("test iterator panic")
        }
    });
    assert!(panic::catch_unwind(AssertUnwindSafe(|| queue.enqueue(input))).is_err());
    assert_eq!(queue.enqueue(["b".into()]).accepted, 1);
    assert_eq!(
        keys(&queue.pending.take_batch(&config(), Instant::now()).0),
        ["a", "b"]
    );
}
