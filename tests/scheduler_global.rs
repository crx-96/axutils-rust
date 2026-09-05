#![cfg(feature = "scheduler")]

use std::{sync::Arc, time::Duration};

use axutils::scheduler::{SchedulerConfig, SchedulerError, TaskSchedule};
use axutils::utils::SchedulerUtils;
use tokio::runtime::Builder as RuntimeBuilder;

#[test]
fn global_scheduler_has_one_irreversible_lifecycle() {
    assert!(!SchedulerUtils::is_initialized());
    assert!(matches!(
        SchedulerUtils::scheduler(),
        Err(SchedulerError::NotInitialized)
    ));
    assert_eq!(
        SchedulerUtils::init(SchedulerConfig { max_tasks: 0 }),
        Err(SchedulerError::InvalidConfig { field: "max_tasks" })
    );
    assert!(!SchedulerUtils::is_initialized());

    let barrier = Arc::new(std::sync::Barrier::new(2));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let barrier = Arc::clone(&barrier);
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            SchedulerUtils::init(SchedulerConfig::default())
        }));
    }
    let results = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(SchedulerError::AlreadyInitialized)))
            .count(),
        1
    );
    assert!(SchedulerUtils::is_initialized());
    assert_eq!(
        SchedulerUtils::init(SchedulerConfig { max_tasks: 0 }),
        Err(SchedulerError::InvalidConfig { field: "max_tasks" })
    );

    let runtime = RuntimeBuilder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let scheduler = SchedulerUtils::scheduler().unwrap();
        let id = scheduler
            .register(TaskSchedule::once(Duration::from_secs(60)), || async {})
            .unwrap();
        assert!(scheduler.cancel(id).unwrap());
    });
    let scheduler = SchedulerUtils::scheduler().unwrap();
    scheduler.shutdown().unwrap();
    scheduler.shutdown().unwrap();
    assert!(SchedulerUtils::is_initialized());
    assert_eq!(
        runtime.block_on(async {
            SchedulerUtils::scheduler()
                .unwrap()
                .register(TaskSchedule::once(Duration::ZERO), || async {})
        }),
        Err(SchedulerError::Shutdown)
    );
    assert_eq!(
        SchedulerUtils::init(SchedulerConfig::default()),
        Err(SchedulerError::AlreadyInitialized)
    );
}
