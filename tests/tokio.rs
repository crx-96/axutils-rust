#![cfg(feature = "tokio")]

use axutils::{
    tokio::{TokioConfig, TokioError, TokioRuntimeFlavor},
    utils::TokioUtils,
};
use std::{panic::catch_unwind, time::Duration};

#[cfg(feature = "axum")]
use axutils::axum::AxumError;
#[cfg(feature = "task-group")]
use axutils::tokio::TokioTaskGroup;
#[cfg(feature = "task-group")]
use tokio::{sync::Barrier, task::yield_now};

#[test]
fn public_paths_and_config_boundaries_are_stable() {
    let _: TokioConfig = TokioConfig::new();
    let _: &str = std::any::type_name::<TokioUtils>();
    assert!(matches!(
        TokioConfig::new().with_worker_threads(Some(0)),
        Err(TokioError::InvalidConfig {
            field: "worker_threads"
        })
    ));
    assert!(matches!(
        TokioConfig::new().with_max_blocking_threads(0),
        Err(TokioError::InvalidConfig {
            field: "max_blocking_threads"
        })
    ));
    assert!(matches!(
        TokioConfig::new().with_shutdown_timeout(Duration::ZERO),
        Err(TokioError::InvalidConfig {
            field: "shutdown_timeout"
        })
    ));
    assert!(TokioConfig::new().with_worker_threads(Some(1_024)).is_ok());
    assert!(TokioConfig::new().with_worker_threads(Some(1_025)).is_err());
    assert!(TokioConfig::new().with_max_blocking_threads(4_096).is_ok());
    assert!(TokioConfig::new().with_max_blocking_threads(4_097).is_err());
    assert!(TokioConfig::new()
        .with_thread_name(Some("a".repeat(64)))
        .is_ok());
    assert!(TokioConfig::new()
        .with_thread_name(Some(String::new()))
        .is_err());
    assert!(TokioConfig::new()
        .with_thread_name(Some("a\0b".into()))
        .is_err());
    assert!(TokioConfig::new()
        .with_shutdown_timeout(Duration::from_secs(300))
        .is_ok());
    assert!(TokioConfig::new()
        .with_shutdown_timeout(Duration::from_secs(301))
        .is_err());
    assert!(TokioConfig::new()
        .with_flavor(TokioRuntimeFlavor::CurrentThread)
        .builder()
        .is_ok());
    assert!(TokioConfig::new()
        .with_flavor(TokioRuntimeFlavor::CurrentThread)
        .with_worker_threads(Some(1))
        .unwrap()
        .builder()
        .is_err());
}

#[test]
fn run_returns_value_rejects_nested_runtime_and_resumes_panic() {
    assert!(!TokioUtils::has_runtime());
    assert!(matches!(
        TokioUtils::try_current_handle(),
        Err(TokioError::RuntimeRequired)
    ));
    assert_eq!(
        TokioUtils::run(&TokioConfig::new(), async { 42 }).unwrap(),
        42
    );
    let panic = catch_unwind(|| {
        let _ = TokioUtils::run(&TokioConfig::new(), async { panic!("sentinel") });
    });
    assert!(panic.is_err());
    TokioUtils::run(&TokioConfig::new(), async {
        assert!(matches!(
            TokioUtils::build_runtime(&TokioConfig::new()),
            Err(TokioError::NestedRuntime)
        ));
    })
    .unwrap();
}

#[tokio::test]
async fn spawn_timeout_and_channel_use_current_runtime() {
    assert!(TokioUtils::has_runtime());
    assert_eq!(TokioUtils::spawn(async { 7 }).unwrap().await.unwrap(), 7);
    let handle = TokioUtils::try_current_handle().unwrap();
    assert_eq!(TokioUtils::spawn_on(&handle, async { 8 }).await.unwrap(), 8);
    assert_eq!(TokioUtils::spawn_blocking(|| 9).unwrap().await.unwrap(), 9);
    assert!(matches!(
        TokioUtils::bounded_mpsc::<u8>(0),
        Err(TokioError::InvalidConfig {
            field: "channel_capacity"
        })
    ));
    let (tx, mut rx) = TokioUtils::bounded_mpsc(1).unwrap();
    tx.send(9).await.unwrap();
    assert_eq!(rx.recv().await, Some(9));
    assert!(matches!(
        TokioUtils::timeout(Duration::from_millis(1), std::future::pending::<()>()).await,
        Err(TokioError::Timeout)
    ));
}

#[cfg(feature = "task-group")]
#[test]
fn open_task_group_rejects_spawn_without_runtime() {
    let group = TokioTaskGroup::new();
    assert!(matches!(
        group.spawn(async {}),
        Err(TokioError::RuntimeRequired)
    ));
}

#[cfg(feature = "task-group")]
#[test]
fn task_group_timeout_works_without_tokio_time_driver() {
    let config = TokioConfig::new().with_time_enabled(false);
    let result = TokioUtils::run(&config, async {
        let group = TokioTaskGroup::new();
        group.spawn(std::future::pending::<()>()).unwrap();
        group.shutdown(Duration::from_millis(1)).await
    })
    .unwrap();
    assert!(matches!(
        result,
        Err(TokioError::TaskGroupShutdownTimeout { remaining_tasks: 1 })
    ));
}

#[test]
fn provider_errors_have_redacted_debug() {
    let sensitive = "secret-provider-message";
    let runtime = TokioError::RuntimeBuild(std::io::Error::other(sensitive));
    assert!(!format!("{runtime:?}").contains(sensitive));
    #[cfg(feature = "axum")]
    {
        let io = AxumError::Io(std::io::Error::other(sensitive));
        assert!(!format!("{io:?}").contains(sensitive));
    }
}

#[cfg(feature = "task-group")]
#[tokio::test]
async fn task_group_close_is_linearized_and_shutdown_is_bounded() {
    let _: TokioTaskGroup = TokioTaskGroup::new();
    let group = TokioTaskGroup::new();
    let token = group.cancellation_token();
    group.spawn(async move { token.cancelled().await }).unwrap();
    group.shutdown(Duration::from_secs(1)).await.unwrap();
    assert!(group.is_closed());
    assert!(matches!(
        group.spawn(async {}),
        Err(TokioError::TaskGroupClosed)
    ));

    let blocked = TokioTaskGroup::new();
    blocked.spawn(std::future::pending::<()>()).unwrap();
    assert!(matches!(
        blocked.shutdown(Duration::from_secs(301)).await,
        Err(TokioError::InvalidConfig {
            field: "task_group_grace"
        })
    ));
    assert!(matches!(
        blocked.shutdown(Duration::ZERO).await,
        Err(TokioError::TaskGroupShutdownTimeout { remaining_tasks: 1 })
    ));
}

#[cfg(feature = "task-group")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_group_close_race_and_drop_abort_semantics_are_explicit() {
    use std::sync::Arc;
    let group = TokioTaskGroup::new();
    let clone = group.clone();
    drop(clone);
    let token = group.cancellation_token();
    let handle = group
        .spawn({
            let token = token.clone();
            async move { token.cancelled().await }
        })
        .unwrap();
    group.close();
    assert!(!token.is_cancelled());
    assert!(!handle.is_finished());
    group.cancel();
    handle.await.unwrap();
    group.shutdown(Duration::from_secs(1)).await.unwrap();

    let racing = TokioTaskGroup::new();
    let barrier = Arc::new(Barrier::new(2));
    let spawn_task = {
        let racing = racing.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            racing.spawn(async {})
        })
    };
    barrier.wait().await;
    racing.close();
    let _ = spawn_task.await.unwrap();
    assert!(matches!(
        racing.spawn(async {}),
        Err(TokioError::TaskGroupClosed)
    ));

    let aborted = TokioTaskGroup::new();
    let handle = aborted.spawn(std::future::pending::<()>()).unwrap();
    handle.abort();
    let _ = handle.await;
    yield_now().await;
    aborted.shutdown(Duration::from_secs(1)).await.unwrap();
    assert_eq!(aborted.remaining_tasks(), 0);
}
