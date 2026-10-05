use axutils::tokio::TokioTaskGuard;
use std::{
    future::pending,
    sync::{mpsc, Arc},
    time::Duration,
};
use tokio::{
    sync::oneshot::{self, error::TryRecvError},
    task::{self, yield_now},
    time::timeout,
};

#[tokio::test]
async fn only_the_last_arc_owner_requests_abort_and_drop_does_not_wait() {
    let (started, ready) = oneshot::channel();
    let (alive, mut released) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        let _alive = alive;
        started.send(()).unwrap();
        pending::<Result<u32, &'static str>>().await
    });
    let completion = task.abort_handle();
    let owner = Arc::new(TokioTaskGuard::new(task));
    let last_owner = Arc::clone(&owner);
    timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();

    drop(owner);
    assert!(!last_owner.is_finished());
    yield_now().await;
    assert!(matches!(released.try_recv(), Err(TryRecvError::Empty)));

    drop(last_owner);
    assert!(!completion.is_finished());
    assert!(matches!(released.try_recv(), Err(TryRecvError::Empty)));
    assert!(timeout(Duration::from_secs(2), released)
        .await
        .unwrap()
        .is_err());
    assert!(completion.is_finished());
}

#[tokio::test]
async fn finished_state_observes_natural_completion_with_generic_output() {
    struct OutputWithoutDebug {
        _alive: oneshot::Sender<()>,
    }

    let (alive, mut released) = oneshot::channel();
    let guard = TokioTaskGuard::new(tokio::spawn(
        async move { OutputWithoutDebug { _alive: alive } },
    ));
    timeout(Duration::from_secs(2), async {
        while !guard.is_finished() {
            yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(guard.is_finished());
    assert!(format!("{guard:?}").contains("is_finished: true"));
    assert!(matches!(released.try_recv(), Err(TryRecvError::Empty)));
    drop(guard);
    assert!(timeout(Duration::from_secs(2), released)
        .await
        .unwrap()
        .is_err());
}

#[tokio::test]
async fn started_blocking_closure_finishes_only_after_its_own_release() {
    let (started, ready) = oneshot::channel();
    let (release, wait) = mpsc::channel();
    let (output, mut output_released) = oneshot::channel::<()>();
    let task = task::spawn_blocking(move || {
        started.send(()).unwrap();
        wait.recv().unwrap();
        output
    });
    let completion = task.abort_handle();
    let guard = TokioTaskGuard::new(task);
    timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();

    drop(guard);
    yield_now().await;
    assert!(!completion.is_finished());
    assert!(matches!(
        output_released.try_recv(),
        Err(TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    timeout(Duration::from_secs(2), async {
        while !completion.is_finished() {
            yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(timeout(Duration::from_secs(2), output_released)
        .await
        .unwrap()
        .is_err());
}
