use std::{
    collections::HashMap,
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard, Weak,
    },
    time::Duration,
};

use tokio::{
    runtime::Handle,
    task::AbortHandle,
    time::{self, Instant, MissedTickBehavior},
};

use super::{cron::CronSchedule, SchedulerConfig, SchedulerError, TaskSchedule};

/// 单个 [`Scheduler`](super::Scheduler) 内不复用的任务标识。
///
/// # Examples
///
/// ```rust
/// # #[cfg(all(feature="chrono",feature="chrono_tz",feature="tokio",feature="croner"))] {
/// let _cancel: fn(&axutils::Scheduler, axutils::TaskId)
///     -> Result<bool, axutils::SchedulerError> = axutils::Scheduler::cancel;
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TaskId(u64);

pub(crate) struct Shared {
    state: Mutex<State>,
}

struct State {
    shutdown: bool,
    max_tasks: usize,
    next_task_id: u64,
    tasks: HashMap<TaskId, AbortHandle>,
}

enum ValidatedSchedule {
    Once(Duration),
    Interval(Duration),
    Cron(Box<CronSchedule>, Duration),
}

impl Shared {
    pub(crate) fn new(config: SchedulerConfig) -> Self {
        Self {
            state: Mutex::new(State {
                shutdown: false,
                max_tasks: config.max_tasks,
                next_task_id: 1,
                tasks: HashMap::new(),
            }),
        }
    }

    pub(crate) fn register<F, Fut>(
        self: &Arc<Self>,
        schedule: TaskSchedule,
        callback: F,
    ) -> Result<TaskId, SchedulerError>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if self.lock().shutdown {
            return Err(SchedulerError::Shutdown);
        }
        let schedule = validate_schedule(schedule)?;
        let handle = runtime_with_time_driver()?;

        let mut state = self.lock();
        if state.shutdown {
            return Err(SchedulerError::Shutdown);
        }
        if state.tasks.len() >= state.max_tasks {
            return Err(SchedulerError::TaskLimitExceeded);
        }
        let task_id = TaskId(state.next_task_id);
        state.next_task_id = state
            .next_task_id
            .checked_add(1)
            .ok_or(SchedulerError::TaskLimitExceeded)?;

        let lifecycle = Arc::new(TaskLifecycle {
            shared: Arc::downgrade(self),
            task_id,
            published: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        });
        let cleanup = TaskCleanup {
            lifecycle: Arc::clone(&lifecycle),
        };
        let task = handle.spawn(async move {
            let _cleanup = cleanup;
            run(schedule, callback).await;
        });
        publish_task(&mut state, &lifecycle, task.abort_handle());
        drop(task);
        Ok(task_id)
    }

    pub(crate) fn cancel(&self, task_id: TaskId) -> bool {
        let mut state = self.lock();
        if let Some(abort) = state.tasks.remove(&task_id) {
            abort.abort();
            true
        } else {
            false
        }
    }

    pub(crate) fn shutdown(&self) {
        let mut state = self.lock();
        state.shutdown = true;
        for (_, task) in state.tasks.drain() {
            task.abort();
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn publish_task(state: &mut State, lifecycle: &TaskLifecycle, abort: AbortHandle) {
    state.tasks.insert(lifecycle.task_id, abort);
    lifecycle.published.store(true, Ordering::SeqCst);
    if lifecycle.finished.load(Ordering::SeqCst) {
        state.tasks.remove(&lifecycle.task_id);
    }
}

struct TaskLifecycle {
    shared: Weak<Shared>,
    task_id: TaskId,
    published: AtomicBool,
    finished: AtomicBool,
}

struct TaskCleanup {
    lifecycle: Arc<TaskLifecycle>,
}

impl Drop for TaskCleanup {
    fn drop(&mut self) {
        self.lifecycle.finished.store(true, Ordering::SeqCst);
        if self.lifecycle.published.load(Ordering::SeqCst) {
            if let Some(shared) = self.lifecycle.shared.upgrade() {
                shared.lock().tasks.remove(&self.lifecycle.task_id);
            }
        }
    }
}

fn validate_schedule(schedule: TaskSchedule) -> Result<ValidatedSchedule, SchedulerError> {
    match schedule {
        TaskSchedule::Once(delay) => Ok(ValidatedSchedule::Once(delay)),
        TaskSchedule::Interval(period) if period.is_zero() => Err(SchedulerError::InvalidSchedule),
        TaskSchedule::Interval(period) => Ok(ValidatedSchedule::Interval(period)),
        TaskSchedule::Cron {
            expression,
            timezone,
        } => {
            let schedule = CronSchedule::parse(&expression, &timezone)?;
            let first_delay = schedule.delay_from_now()?;
            Ok(ValidatedSchedule::Cron(Box::new(schedule), first_delay))
        }
    }
}

fn runtime_with_time_driver() -> Result<Handle, SchedulerError> {
    let handle = Handle::try_current().map_err(|_| SchedulerError::RuntimeRequired)?;
    let timer_available = {
        let _entered = handle.enter();
        catch_unwind(AssertUnwindSafe(|| time::sleep(Duration::ZERO))).is_ok()
    };
    if timer_available {
        Ok(handle)
    } else {
        Err(SchedulerError::RuntimeRequired)
    }
}

async fn run<F, Fut>(schedule: ValidatedSchedule, callback: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    match schedule {
        ValidatedSchedule::Once(delay) => {
            time::sleep(delay).await;
            callback().await;
        }
        ValidatedSchedule::Interval(period) => {
            let mut interval = time::interval_at(Instant::now() + period, period);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                callback().await;
            }
        }
        ValidatedSchedule::Cron(schedule, mut delay) => loop {
            time::sleep(delay).await;
            callback().await;
            let Ok(next_delay) = schedule.delay_from_now() else {
                break;
            };
            delay = next_delay;
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_before_publication_is_compensated() {
        let shared = Arc::new(Shared::new(SchedulerConfig::new(1).unwrap()));
        let lifecycle = Arc::new(TaskLifecycle {
            shared: Arc::downgrade(&shared),
            task_id: TaskId(1),
            published: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        });
        drop(TaskCleanup {
            lifecycle: Arc::clone(&lifecycle),
        });

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let task = runtime.spawn(std::future::pending::<()>());
        publish_task(&mut shared.lock(), &lifecycle, task.abort_handle());

        assert!(lifecycle.finished.load(Ordering::SeqCst));
        assert!(shared.lock().tasks.is_empty());
    }

    #[test]
    fn task_id_overflow_does_not_consume_capacity() {
        let shared = Arc::new(Shared::new(SchedulerConfig::new(1).unwrap()));
        shared.lock().next_task_id = u64::MAX;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let result = runtime.block_on(async {
            shared.register(TaskSchedule::once(Duration::from_secs(60)), || async {})
        });
        assert_eq!(result, Err(SchedulerError::TaskLimitExceeded));
        assert!(shared.lock().tasks.is_empty());
    }
}
