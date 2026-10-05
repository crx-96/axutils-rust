use super::{bytes_with, digits_with, hex_with};
use crate::crypto::CryptoError;
use std::error::Error;

#[test]
fn zero_lengths_never_touch_source() {
    assert!(bytes_with(0, |_| panic!("source accessed"))
        .unwrap()
        .is_empty());
    assert_eq!(digits_with(0, |_| panic!("source accessed")).unwrap(), "");
    assert_eq!(hex_with(0, |_| panic!("source accessed")).unwrap(), "");
}

#[test]
fn rejected_tail_is_resampled_and_leading_zero_is_preserved() {
    let mut values = [250, 251, 252, 253, 254, 255, 0, 9, 10, 249].into_iter();
    let mut calls = 0;
    let result = digits_with(4, |bytes| {
        calls += 1;
        for byte in bytes {
            *byte = values.next().expect("exactly the required samples");
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(result, "0909");
    assert_eq!(calls, 3);
    assert_eq!(values.next(), None);
}

#[test]
fn accepted_byte_domain_has_equal_digit_counts() {
    let mut source = 0_u8..250;
    let digits = digits_with(250, |bytes| {
        for byte in bytes {
            *byte = source.next().unwrap();
        }
        Ok(())
    })
    .unwrap();
    for digit in b'0'..=b'9' {
        assert_eq!(digits.bytes().filter(|&value| value == digit).count(), 25);
    }
}

#[test]
fn random_bytes_and_hex_preserve_exact_source_data() {
    let fill = |bytes: &mut [u8]| {
        bytes.copy_from_slice(&[0, 15, 16, 255]);
        Ok(())
    };
    assert_eq!(bytes_with(4, fill).unwrap(), [0, 15, 16, 255]);
    assert_eq!(hex_with(4, fill).unwrap(), "000f10ff");
}

#[test]
fn source_failures_return_only_error_even_after_partial_progress() {
    let fail = |bytes: &mut [u8]| {
        bytes.fill(7);
        Err(CryptoError::RandomSource)
    };
    assert_eq!(bytes_with(4, fail), Err(CryptoError::RandomSource));
    assert_eq!(hex_with(4, fail), Err(CryptoError::RandomSource));
    let mut calls = 0;
    assert_eq!(
        digits_with(2, |bytes| {
            calls += 1;
            if calls == 1 {
                bytes.copy_from_slice(&[0, 255]);
                Ok(())
            } else {
                Err(CryptoError::RandomSource)
            }
        }),
        Err(CryptoError::RandomSource)
    );
    assert_eq!(calls, 2);
    let error = CryptoError::RandomSource;
    assert_eq!(
        error.to_string(),
        "operating system random source unavailable"
    );
    assert_eq!(format!("{error:?}"), "RandomSource");
    assert!(error.source().is_none());
}

#[test]
fn impossible_capacity_and_hex_overflow_fail_before_sampling() {
    for length in [usize::MAX, isize::MAX as usize + 1] {
        assert!(matches!(
            bytes_with(length, |_| panic!("source accessed")),
            Err(CryptoError::OutputTooLarge { .. })
        ));
        assert!(matches!(
            digits_with(length, |_| panic!("source accessed")),
            Err(CryptoError::OutputTooLarge { .. })
        ));
    }
    for length in [usize::MAX, usize::MAX / 2 + 1, isize::MAX as usize / 2 + 1] {
        assert_eq!(
            hex_with(length, |_| panic!("source accessed")),
            Err(CryptoError::OutputTooLarge {
                operation: "secure_random_hex"
            })
        );
    }
}
