#![cfg(feature = "aes")]

use std::sync::{Arc, Barrier};
use std::thread;

use axutils::{
    crypto::{AesKey, AesMode, CryptoError},
    utils::CryptoUtils,
};

#[test]
fn global_aes_lifecycle_exposes_the_initialized_cipher() {
    assert!(!CryptoUtils::aes_is_initialized());
    assert!(matches!(
        CryptoUtils::cipher(),
        Err(CryptoError::NotInitialized)
    ));

    assert!(matches!(
        CryptoUtils::aes_init_from_bytes([0u8; 15], AesMode::Gcm),
        Err(CryptoError::InvalidKeyLength { length: 15 })
    ));
    assert!(!CryptoUtils::aes_is_initialized());

    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        CryptoUtils::aes_init_from_bytes([0x11; 16], AesMode::Gcm)
    });
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        CryptoUtils::aes_init_from_bytes([0x22; 16], AesMode::CbcPkcs7)
    });
    barrier.wait();
    let first_result = first.join().unwrap();
    let second_result = second.join().unwrap();
    assert_eq!(
        [first_result.is_ok(), second_result.is_ok()]
            .into_iter()
            .filter(|success| *success)
            .count(),
        1
    );

    let mode = if first_result.is_ok() {
        AesMode::Gcm
    } else {
        AesMode::CbcPkcs7
    };
    assert!(CryptoUtils::aes_is_initialized());
    assert!(matches!(
        CryptoUtils::aes_init(AesKey::from_bytes([0x55; 16]).unwrap(), AesMode::Gcm),
        Err(CryptoError::AlreadyInitialized)
    ));
    let cipher = CryptoUtils::cipher().expect("the winning initializer installs a cipher");
    assert_eq!(cipher.mode(), mode);
    let debug = format!("{cipher:?}");
    assert!(!debug.contains("17"));
    assert!(!debug.contains("34"));
}
