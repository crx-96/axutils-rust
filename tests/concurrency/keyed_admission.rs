use axutils::concurrency::{AdmissionError, KeyedAdmission};
use std::{
    panic::catch_unwind,
    sync::{Arc, Barrier},
    thread,
};

#[test]
fn clones_share_capacity_and_duplicate_keys_take_priority() {
    let admission = KeyedAdmission::new(2);
    let other_owner = admission.clone();
    let first = admission.try_enter("Record").unwrap();
    let second = other_owner.try_enter(String::from("record")).unwrap();
    assert_eq!(
        other_owner.try_enter("Record").unwrap_err(),
        AdmissionError::Busy
    );
    assert_eq!(
        admission.try_enter("other").unwrap_err(),
        AdmissionError::Full
    );
    drop(first);
    let replacement = other_owner.try_enter("Record").unwrap();
    drop((replacement, second));
    assert!(admission.try_enter("other").is_ok());
}

#[test]
fn zero_capacity_rejects_new_keys_and_separate_instances_are_independent() {
    assert_eq!(
        KeyedAdmission::new(0).try_enter("").unwrap_err(),
        AdmissionError::Full
    );
    let first = KeyedAdmission::new(1);
    let second = KeyedAdmission::new(1);
    let first_permit = first.try_enter("").unwrap();
    let second_permit = second.try_enter("").unwrap();
    drop(first);
    drop(first_permit);
    drop(second_permit);
    assert!(second.try_enter("").is_ok());
}

#[test]
fn one_competing_thread_owns_the_same_key_until_all_contenders_finish() {
    let admission = KeyedAdmission::new(8);
    let start = Arc::new(Barrier::new(8));
    let attempted = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let admission = admission.clone();
            let start = Arc::clone(&start);
            let attempted = Arc::clone(&attempted);
            thread::spawn(move || {
                start.wait();
                let result = admission.try_enter("shared");
                attempted.wait();
                match result {
                    Ok(_permit) => true,
                    Err(error) => {
                        assert_eq!(error, AdmissionError::Busy);
                        false
                    }
                }
            })
        })
        .collect();
    let winners = workers
        .into_iter()
        .map(|worker| usize::from(worker.join().unwrap()))
        .sum::<usize>();
    assert_eq!(winners, 1);
    assert!(admission.try_enter("shared").is_ok());
}

#[test]
fn different_keys_compete_for_a_shared_total_limit() {
    let admission = KeyedAdmission::new(3);
    let start = Arc::new(Barrier::new(8));
    let attempted = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|id| {
            let admission = admission.clone();
            let start = Arc::clone(&start);
            let attempted = Arc::clone(&attempted);
            thread::spawn(move || {
                start.wait();
                let result = admission.try_enter(id.to_string());
                attempted.wait();
                match result {
                    Ok(_permit) => true,
                    Err(error) => {
                        assert_eq!(error, AdmissionError::Full);
                        false
                    }
                }
            })
        })
        .collect();
    let winners = workers
        .into_iter()
        .map(|worker| usize::from(worker.join().unwrap()))
        .sum::<usize>();
    assert_eq!(winners, 3);
}

#[test]
fn early_error_and_unwind_release_the_key_without_poisoning_state() {
    fn failing_operation(admission: &KeyedAdmission) -> Result<(), &'static str> {
        let _permit = admission.try_enter("operation").unwrap();
        Err("operation failed")
    }

    let admission = KeyedAdmission::new(1);
    assert_eq!(failing_operation(&admission), Err("operation failed"));
    assert!(admission.try_enter("operation").is_ok());
    assert!(catch_unwind(|| {
        let _permit = admission.try_enter("operation").unwrap();
        panic!("operation panicked");
    })
    .is_err());
    assert!(admission.try_enter("operation").is_ok());
}

#[test]
fn debug_and_errors_do_not_expose_keys() {
    let sensitive = "private-business-key";
    let admission = KeyedAdmission::new(1);
    let permit = admission.try_enter(sensitive).unwrap();
    let error = admission.try_enter(sensitive).unwrap_err();
    assert!(!format!("{admission:?}").contains(sensitive));
    assert!(!format!("{permit:?}").contains(sensitive));
    assert!(!format!("{error:?} {error}").contains(sensitive));
}
