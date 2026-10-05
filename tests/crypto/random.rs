use axutils::{crypto::CryptoError, utils::CryptoUtils};

#[test]
fn public_system_random_outputs_have_requested_lengths_and_alphabets() {
    for length in [0, 1, 16, 65, 257] {
        assert_eq!(
            CryptoUtils::secure_random_bytes(length).unwrap().len(),
            length
        );
        let digits = CryptoUtils::secure_random_digits(length).unwrap();
        assert_eq!(digits.len(), length);
        assert!(digits.bytes().all(|byte| byte.is_ascii_digit()));
        let hex = CryptoUtils::secure_random_hex(length).unwrap();
        assert_eq!(hex.len(), length * 2);
        assert!(hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_eq!(CryptoUtils::hex_decode(&hex).unwrap().len(), length);
    }
}

#[test]
fn public_random_api_reports_capacity_errors() {
    assert!(matches!(
        CryptoUtils::secure_random_bytes(usize::MAX),
        Err(CryptoError::OutputTooLarge { .. })
    ));
    assert!(matches!(
        CryptoUtils::secure_random_digits(usize::MAX),
        Err(CryptoError::OutputTooLarge { .. })
    ));
    assert!(matches!(
        CryptoUtils::secure_random_hex(usize::MAX / 2 + 1),
        Err(CryptoError::OutputTooLarge { .. })
    ));
}
