use std::{future, sync::mpsc, thread, time::Duration};

use axutils::tokio::TokioTaskGroup;
use tokio::runtime::Builder;

struct ReenterOnDrop {
    group: TokioTaskGroup,
    observed: mpsc::Sender<usize>,
}

impl Drop for ReenterOnDrop {
    fn drop(&mut self) {
        self.group.close();
        self.observed.send(self.group.remaining_tasks()).unwrap();
    }
}

fn assert_shutdown_runtime_drops_outside_gate(blocking: bool) {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let runtime = Builder::new_current_thread().build().unwrap();
        let handle = runtime.handle().clone();
        drop(runtime);
        let _entered = handle.enter();
        let group = TokioTaskGroup::new();
        let (drop_sender, drop_receiver) = mpsc::channel();
        let reenter = ReenterOnDrop {
            group: group.clone(),
            observed: drop_sender,
        };
        if blocking {
            drop(group.spawn_blocking(move || drop(reenter)).unwrap());
        } else {
            drop(
                group
                    .spawn(async move {
                        let _reenter = reenter;
                        future::pending::<()>().await;
                    })
                    .unwrap(),
            );
        }
        assert_eq!(drop_receiver.try_recv().unwrap(), 1, "析构期间仍应被计数");
        assert!(group.is_closed());
        assert_eq!(group.remaining_tasks(), 0);
        sender.send(()).unwrap();
    });
    receiver
        .recv_timeout(Duration::from_secs(3))
        .expect("任务析构重入任务组不能死锁");
    worker.join().unwrap();
}

#[test]
fn shutdown_runtime_drops_async_future_outside_group_gate() {
    assert_shutdown_runtime_drops_outside_gate(false);
}

#[test]
fn shutdown_runtime_drops_blocking_closure_outside_group_gate() {
    assert_shutdown_runtime_drops_outside_gate(true);
}
