#[cfg(not(panic = "abort"))]
compile_error!("this downstream regression requires panic=abort");

use axutils::scheduler::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};
use std::time::Duration;
use tokio::{runtime::Builder, sync::Notify, time};

fn main() {
    let scheduler = Scheduler::new(SchedulerConfig::new(1).unwrap()).unwrap();
    let missing = Builder::new_current_thread().build().unwrap();
    let unknown = Builder::new_current_thread().enable_time().build().unwrap();
    scheduler
        .with_time_runtime(&mut Builder::new_current_thread(), |runtime| {
            for foreign in [&missing, &unknown] {
                let _entered = foreign.enter();
                assert_eq!(
                    scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
                    Err(SchedulerError::RuntimeRequired)
                );
            }
            runtime.block_on(async {
                let fired = std::sync::Arc::new(Notify::new());
                let callback = fired.clone();
                scheduler
                    .register(TaskSchedule::once(Duration::from_millis(1)), move || {
                        let callback = callback.clone();
                        async move { callback.notify_one() }
                    })
                    .unwrap();
                time::timeout(Duration::from_secs(3), fired.notified())
                    .await
                    .unwrap();
                let id = scheduler
                    .register(TaskSchedule::interval(Duration::from_secs(60)), || async {})
                    .unwrap();
                assert!(scheduler.cancel(id).unwrap());
            });
        })
        .unwrap();
}
