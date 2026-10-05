use axutils::concurrency::{AdmissionError, KeyedAdmission};
use std::{future::pending, time::Duration};
use tokio::{sync::oneshot, time::timeout};

#[tokio::test]
async fn canceling_a_task_releases_its_permit_when_the_future_is_dropped() {
    let admission = KeyedAdmission::new(1);
    let owner = admission.clone();
    let (entered, ready) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _permit = owner.try_enter("running").unwrap();
        entered.send(()).unwrap();
        pending::<()>().await;
    });
    timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        admission.try_enter("running").unwrap_err(),
        AdmissionError::Busy
    );

    task.abort();
    assert_eq!(
        admission.try_enter("running").unwrap_err(),
        AdmissionError::Busy
    );
    assert!(timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap_err()
        .is_cancelled());
    assert!(admission.try_enter("running").is_ok());
}
