use super::*;
use tokio::runtime::Builder as RuntimeBuilder;

#[test]
fn task_id_overflow_does_not_consume_capacity() {
    let shared = Shared::new(SchedulerConfig::new(1).unwrap());
    shared.lock().next_task_id = u64::MAX;
    assert_eq!(shared.reserve(), Err(SchedulerError::TaskLimitExceeded));
    assert!(shared.lock().tasks.is_empty());
}

#[test]
fn prepublication_completion_cancel_and_shutdown_do_not_restore_capacity() {
    for action in 0..3 {
        let shared = Arc::new(Shared::new(SchedulerConfig::new(1).unwrap()));
        let task_id = shared.reserve().unwrap();
        assert_eq!(shared.reserve(), Err(SchedulerError::TaskLimitExceeded));
        match action {
            0 => drop(TaskCleanup {
                shared: Arc::downgrade(&shared),
                task_id,
            }),
            1 => assert!(shared.cancel(task_id)),
            _ => shared.shutdown(),
        }
        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        let task = runtime.spawn(std::future::pending::<()>());
        shared.publish(task_id, task.abort_handle());
        assert!(shared.lock().tasks.is_empty());
        assert!(runtime.block_on(task).unwrap_err().is_cancelled());
        if action != 2 {
            assert!(shared.reserve().is_ok());
        }
    }
}

#[test]
fn synchronous_task_drop_can_reenter_registry_after_reservation() {
    struct Reenter(Arc<Shared>);
    impl Drop for Reenter {
        fn drop(&mut self) {
            self.0.shutdown();
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let shared = Arc::new(Shared::new(SchedulerConfig::new(1).unwrap()));
        let id = shared.reserve().unwrap();
        let runtime = RuntimeBuilder::new_current_thread().build().unwrap();
        let handle = runtime.handle().clone();
        drop(runtime);
        let reenter = Reenter(Arc::clone(&shared));
        let cleanup = TaskCleanup {
            shared: Arc::downgrade(&shared),
            task_id: id,
        };
        let task = handle.spawn(async move {
            let _reenter = reenter;
            let _cleanup = cleanup;
            std::future::pending::<()>().await;
        });
        shared.publish(id, task.abort_handle());
        assert!(shared.lock().tasks.is_empty());
        tx.send(()).unwrap();
    });
    rx.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.join().unwrap();
}
