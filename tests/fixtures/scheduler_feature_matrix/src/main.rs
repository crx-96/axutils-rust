#[cfg(feature = "all")]
fn main() {
    use axutils::scheduler::{
        Scheduler as DomainScheduler, SchedulerConfig as DomainSchedulerConfig,
        SchedulerError as DomainSchedulerError, TaskId as DomainTaskId,
        TaskSchedule as DomainTaskSchedule,
    };
    use axutils::utils::scheduler_utils::SchedulerUtils as NestedSchedulerUtils;
    use axutils::utils::SchedulerUtils as FacadeSchedulerUtils;
    use axutils::{
        Scheduler, SchedulerConfig, SchedulerError, SchedulerUtils, TaskId, TaskSchedule,
    };

    let _ = std::any::type_name::<Scheduler>();
    let _ = std::any::type_name::<SchedulerConfig>();
    let _ = std::any::type_name::<SchedulerError>();
    let _ = std::any::type_name::<TaskId>();
    let _ = std::any::type_name::<TaskSchedule>();
    let _ = std::any::type_name::<SchedulerUtils>();
    let _ = std::any::type_name::<DomainScheduler>();
    let _ = std::any::type_name::<DomainSchedulerConfig>();
    let _ = std::any::type_name::<DomainSchedulerError>();
    let _ = std::any::type_name::<DomainTaskId>();
    let _ = std::any::type_name::<DomainTaskSchedule>();
    let _ = std::any::type_name::<FacadeSchedulerUtils>();
    let _ = std::any::type_name::<NestedSchedulerUtils>();
}

#[cfg(any(
    feature = "none",
    feature = "chrono",
    feature = "chrono-tz",
    feature = "tokio",
    feature = "croner",
    feature = "chrono-chrono-tz",
    feature = "chrono-tokio",
    feature = "chrono-croner",
    feature = "chrono-tz-tokio",
    feature = "chrono-tz-croner",
    feature = "tokio-croner",
    feature = "chrono-chrono-tz-tokio",
    feature = "chrono-chrono-tz-croner",
    feature = "chrono-tokio-croner",
    feature = "chrono-tz-tokio-croner"
))]
fn main() {
    let _ = std::any::type_name::<axutils::Scheduler>();
}

#[cfg(feature = "negative-root-module-alias")]
fn main() {
    let _ = std::any::type_name::<axutils::scheduler_utils::SchedulerUtils>();
}

#[cfg(not(any(
    feature = "none",
    feature = "chrono",
    feature = "chrono-tz",
    feature = "tokio",
    feature = "croner",
    feature = "chrono-chrono-tz",
    feature = "chrono-tokio",
    feature = "chrono-croner",
    feature = "chrono-tz-tokio",
    feature = "chrono-tz-croner",
    feature = "tokio-croner",
    feature = "chrono-chrono-tz-tokio",
    feature = "chrono-chrono-tz-croner",
    feature = "chrono-tokio-croner",
    feature = "chrono-tz-tokio-croner",
    feature = "all",
    feature = "negative-root-module-alias"
)))]
fn main() {}
