#![cfg(all(
    feature = "chrono",
    feature = "chrono_tz",
    feature = "tokio",
    feature = "croner"
))]

use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskId, TaskSchedule};

fn assert_send_sync<T: Send + Sync>() {}
fn assert_copy_hash<T: Copy + std::hash::Hash>() {}

#[test]
fn public_types_and_error_contract_are_stable() {
    assert_send_sync::<Scheduler>();
    assert_copy_hash::<TaskId>();
    assert_eq!(SchedulerConfig::default().max_tasks, 256);
    assert_eq!(SchedulerConfig::new(1).unwrap().max_tasks, 1);
    assert_eq!(SchedulerConfig::new(4096).unwrap().max_tasks, 4096);
    assert_eq!(
        SchedulerConfig::new(0),
        Err(SchedulerError::InvalidConfig { field: "max_tasks" })
    );
    assert_eq!(
        Scheduler::new(SchedulerConfig { max_tasks: 4097 })
            .err()
            .unwrap(),
        SchedulerError::InvalidConfig { field: "max_tasks" }
    );

    let errors = [
        (
            SchedulerError::InvalidConfig { field: "max_tasks" },
            "invalid scheduler configuration: max_tasks",
        ),
        (SchedulerError::InvalidSchedule, "invalid task schedule"),
        (SchedulerError::InvalidCron, "invalid cron schedule"),
        (SchedulerError::InvalidTimezone, "invalid IANA timezone"),
        (
            SchedulerError::RuntimeRequired,
            "a Tokio runtime with an enabled time driver is required",
        ),
        (
            SchedulerError::AlreadyInitialized,
            "scheduler is already initialized",
        ),
        (
            SchedulerError::NotInitialized,
            "scheduler is not initialized",
        ),
        (
            SchedulerError::TaskLimitExceeded,
            "scheduler task limit exceeded",
        ),
        (SchedulerError::Shutdown, "scheduler is shut down"),
    ];
    for (error, expected) in errors {
        assert_eq!(error.to_string(), expected);
        assert!(error.source().is_none());
    }
}

#[test]
fn validation_precedes_runtime_and_does_not_run_callbacks() {
    let scheduler = Scheduler::new(SchedulerConfig::default()).unwrap();
    let called = Arc::new(AtomicBool::new(false));

    let invalid = [
        TaskSchedule::interval(Duration::ZERO),
        TaskSchedule::cron("0 0 0 * *", "UTC"),
        TaskSchedule::cron("0 0 0 * * * *", "UTC"),
        TaskSchedule::cron("@hourly", "UTC"),
        TaskSchedule::cron("0 0 0 L * *", "UTC"),
        TaskSchedule::cron("0 0 0 ? * *", "UTC"),
    ];
    for schedule in invalid {
        let marker = Arc::clone(&called);
        assert!(matches!(
            scheduler.register(schedule, move || {
                marker.store(true, Ordering::SeqCst);
                async {}
            }),
            Err(SchedulerError::InvalidSchedule | SchedulerError::InvalidCron)
        ));
    }
    assert_eq!(
        scheduler.register(TaskSchedule::cron("0 0 0 * * *", "not/a-zone"), || async {}),
        Err(SchedulerError::InvalidTimezone)
    );
    assert_eq!(
        scheduler.register(TaskSchedule::cron("0 0 0 31 2 *", "UTC"), || async {}),
        Err(SchedulerError::InvalidCron)
    );
    let valid_256 = format!("0 0 0 * * *{}", " ".repeat(256 - "0 0 0 * * *".len()));
    assert_eq!(valid_256.len(), 256);
    assert_eq!(
        scheduler.register(TaskSchedule::cron(valid_256, "UTC"), || async {}),
        Err(SchedulerError::RuntimeRequired)
    );
    let valid_255 = format!("0 0 0 * * *{}", " ".repeat(255 - "0 0 0 * * *".len()));
    assert_eq!(valid_255.len(), 255);
    assert_eq!(
        scheduler.register(TaskSchedule::cron(valid_255, "UTC"), || async {}),
        Err(SchedulerError::RuntimeRequired)
    );
    let invalid_257 = format!("0 0 0 * * *{}", " ".repeat(257 - "0 0 0 * * *".len()));
    assert_eq!(
        scheduler.register(TaskSchedule::cron(invalid_257, "UTC"), || async {}),
        Err(SchedulerError::InvalidCron)
    );
    assert_eq!(
        scheduler.register(
            TaskSchedule::cron("0 0 0 * * *", "x".repeat(129)),
            || async {}
        ),
        Err(SchedulerError::InvalidTimezone)
    );
    for length in [127, 128] {
        let timezone = "界".repeat(length / 3) + &"x".repeat(length % 3);
        assert_eq!(timezone.len(), length);
        assert_eq!(
            scheduler.register(TaskSchedule::cron("0 0 0 * * *", timezone), || async {}),
            Err(SchedulerError::InvalidTimezone)
        );
    }
    assert!(!called.load(Ordering::SeqCst));
}

#[test]
fn runtime_and_time_driver_are_required_without_consuming_capacity() {
    let scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    assert_eq!(
        scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
        Err(SchedulerError::RuntimeRequired)
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let result = runtime
        .block_on(async { scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}) });
    assert_eq!(result, Err(SchedulerError::RuntimeRequired));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let task = scheduler
            .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
            .unwrap();
        assert!(scheduler.cancel(task).unwrap());
    });
}

#[test]
fn dropping_runtime_before_first_poll_releases_capacity() {
    let scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let _entered = runtime.enter();
        scheduler
            .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
            .unwrap();
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let _entered = runtime.enter();
    let replacement = scheduler
        .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
        .unwrap();
    assert!(scheduler.cancel(replacement).unwrap());
}

#[tokio::test(start_paused = true)]
async fn once_interval_capacity_cancel_and_shutdown_work() {
    let scheduler = Scheduler::new(SchedulerConfig::new(2).unwrap()).unwrap();
    let once_count = Arc::new(AtomicUsize::new(0));
    let interval_count = Arc::new(AtomicUsize::new(0));

    let once_counter = Arc::clone(&once_count);
    let once = scheduler
        .register(TaskSchedule::once(Duration::from_secs(5)), move || {
            let counter = Arc::clone(&once_counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        })
        .unwrap();
    let interval_counter = Arc::clone(&interval_count);
    let interval = scheduler
        .register(TaskSchedule::interval(Duration::from_secs(2)), move || {
            let counter = Arc::clone(&interval_counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        })
        .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(
        scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
        Err(SchedulerError::TaskLimitExceeded)
    );

    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(interval_count.load(Ordering::SeqCst), 0);
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(interval_count.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(3)).await;
    tokio::task::yield_now().await;
    assert_eq!(once_count.load(Ordering::SeqCst), 1);
    assert!(!scheduler.cancel(once).unwrap());
    assert!(scheduler.cancel(interval).unwrap());
    assert!(!scheduler.cancel(interval).unwrap());
    let cancelled_count = interval_count.load(Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(interval_count.load(Ordering::SeqCst), cancelled_count);

    let replacement = scheduler
        .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
        .unwrap();
    assert_ne!(replacement, once);
    scheduler.shutdown().unwrap();
    scheduler.shutdown().unwrap();
    assert!(!scheduler.cancel(replacement).unwrap());
    assert_eq!(
        scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
        Err(SchedulerError::Shutdown)
    );
}

#[tokio::test(start_paused = true)]
async fn slow_interval_callback_does_not_overlap() {
    let scheduler = Scheduler::new(SchedulerConfig::default()).unwrap();
    let running = Arc::new(AtomicUsize::new(0));
    let max_running = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_running = Arc::clone(&running);
    let callback_max = Arc::clone(&max_running);
    let callback_calls = Arc::clone(&calls);
    let id = scheduler
        .register(TaskSchedule::interval(Duration::from_secs(1)), move || {
            let running = Arc::clone(&callback_running);
            let max_running = Arc::clone(&callback_max);
            let calls = Arc::clone(&callback_calls);
            async move {
                let current = running.fetch_add(1, Ordering::SeqCst) + 1;
                max_running.fetch_max(current, Ordering::SeqCst);
                calls.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs(3)).await;
                running.fetch_sub(1, Ordering::SeqCst);
            }
        })
        .unwrap();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(3)).await;
    tokio::task::yield_now().await;
    assert_eq!(max_running.load(Ordering::SeqCst), 1);
    assert!(calls.load(Ordering::SeqCst) <= 2);
    assert!(scheduler.cancel(id).unwrap());
}

#[tokio::test]
async fn cron_task_runs_and_completion_releases_capacity() {
    let scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    let notify = Arc::new(tokio::sync::Notify::new());
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_notify = Arc::clone(&notify);
    let callback_calls = Arc::clone(&calls);
    scheduler
        .register(TaskSchedule::cron("* * * * * *", "UTC"), move || {
            let notify = Arc::clone(&callback_notify);
            let calls = Arc::clone(&callback_calls);
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                notify.notify_waiters();
            }
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), async {
        while calls.load(Ordering::SeqCst) < 2 {
            notify.notified().await;
        }
    })
    .await
    .expect("cron should trigger and calculate another occurrence");
    scheduler.shutdown().unwrap();

    let once_scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    let done = Arc::new(tokio::sync::Notify::new());
    let callback_done = Arc::clone(&done);
    let first = once_scheduler
        .register(TaskSchedule::once(Duration::ZERO), move || {
            let done = Arc::clone(&callback_done);
            async move { done.notify_one() }
        })
        .unwrap();
    done.notified().await;
    tokio::task::yield_now().await;
    let second = once_scheduler
        .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
        .unwrap();
    assert_ne!(first, second);
    assert!(once_scheduler.cancel(second).unwrap());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_aborts_at_await_boundary_and_races_with_shutdown() {
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    let scheduler = Scheduler::new(SchedulerConfig::default()).unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let callback_started = Arc::clone(&started);
    let callback_dropped = Arc::clone(&dropped);
    let task = scheduler
        .register(TaskSchedule::once(Duration::ZERO), move || {
            let started = Arc::clone(&callback_started);
            let dropped = Arc::clone(&callback_dropped);
            async move {
                let _drop_flag = DropFlag(dropped);
                started.notify_one();
                std::future::pending::<()>().await;
            }
        })
        .unwrap();
    started.notified().await;
    assert!(scheduler.cancel(task).unwrap());
    tokio::time::timeout(Duration::from_secs(1), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("abort should drop a callback suspended at an await boundary");

    for _ in 0..32 {
        let scheduler = Arc::new(Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap());
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let register_scheduler = Arc::clone(&scheduler);
        let register_barrier = Arc::clone(&barrier);
        let register = tokio::spawn(async move {
            register_barrier.wait().await;
            register_scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {})
        });
        let shutdown_scheduler = Arc::clone(&scheduler);
        let shutdown_barrier = Arc::clone(&barrier);
        let shutdown = tokio::spawn(async move {
            shutdown_barrier.wait().await;
            shutdown_scheduler.shutdown()
        });
        barrier.wait().await;
        let registered = register.await.unwrap();
        shutdown.await.unwrap().unwrap();
        if let Ok(task_id) = registered {
            assert!(!scheduler.cancel(task_id).unwrap());
        } else {
            assert_eq!(registered, Err(SchedulerError::Shutdown));
        }
        assert_eq!(
            scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
            Err(SchedulerError::Shutdown)
        );
    }
}

#[tokio::test]
async fn callback_panic_releases_capacity() {
    let scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let callback_started = Arc::clone(&started);
    scheduler
        .register(TaskSchedule::once(Duration::ZERO), move || {
            let started = Arc::clone(&callback_started);
            async move {
                started.notify_one();
                panic!("intentional scheduler callback panic test");
            }
        })
        .unwrap();
    started.notified().await;
    let replacement = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {}) {
                Ok(task_id) => break task_id,
                Err(SchedulerError::TaskLimitExceeded) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected scheduler error: {error}"),
            }
        }
    })
    .await
    .expect("panic cleanup should release capacity");
    assert!(scheduler.cancel(replacement).unwrap());
}

#[tokio::test]
async fn callback_can_reenter_shutdown_and_send_non_sync_closure_is_accepted() {
    let scheduler = Arc::new(Scheduler::new(SchedulerConfig::new(2).unwrap()).unwrap());
    let weak = Arc::downgrade(&scheduler);
    let stopped = Arc::new(tokio::sync::Notify::new());
    let callback_stopped = Arc::clone(&stopped);
    scheduler
        .register(TaskSchedule::once(Duration::ZERO), move || {
            let weak = weak.clone();
            let stopped = Arc::clone(&callback_stopped);
            async move {
                weak.upgrade().unwrap().shutdown().unwrap();
                stopped.notify_one();
            }
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), stopped.notified())
        .await
        .unwrap();
    assert_eq!(
        scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
        Err(SchedulerError::Shutdown)
    );

    let independent = Scheduler::new(SchedulerConfig::default()).unwrap();
    let cell = std::cell::Cell::new(0_u8);
    let task = independent
        .register(TaskSchedule::once(Duration::from_secs(60)), move || {
            cell.set(cell.get().saturating_add(1));
            async {}
        })
        .unwrap();
    assert!(independent.cancel(task).unwrap());
}

#[test]
fn multi_thread_runtime_drives_tasks() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let scheduler = Scheduler::new(SchedulerConfig::default()).unwrap();
        let done = Arc::new(tokio::sync::Notify::new());
        let callback_done = Arc::clone(&done);
        scheduler
            .register(TaskSchedule::once(Duration::ZERO), move || {
                let done = Arc::clone(&callback_done);
                async move { done.notify_one() }
            })
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), done.notified())
            .await
            .unwrap();
    });
}
