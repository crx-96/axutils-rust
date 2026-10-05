use super::{AdmissionError, KeyedAdmission};
use std::panic::catch_unwind;

#[test]
fn poison_rejects_admission_but_does_not_prevent_permit_cleanup() {
    let admission = KeyedAdmission::new(1);
    let permit = admission.try_enter("held").unwrap();
    assert!(catch_unwind(|| {
        let _locked = admission.active.lock().unwrap();
        panic!("poison admission state");
    })
    .is_err());

    assert_eq!(
        admission.try_enter("held").unwrap_err(),
        AdmissionError::Unavailable
    );
    drop(permit);
    assert!(admission.active.lock().unwrap_err().into_inner().is_empty());
    assert_eq!(
        admission.try_enter("new").unwrap_err(),
        AdmissionError::Unavailable
    );
}
