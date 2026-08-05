#![cfg(all(feature = "aes", feature = "base64"))]

use std::sync::{Arc, Barrier};
use std::thread;

use axutils::{AesCipher, AesKey, AesMode, Base64Options, CryptoError, CryptoUtils};

#[test]
fn global_aes_initializes_once_and_interoperates_with_instances() {
    assert!(!CryptoUtils::aes_is_initialized());
    assert!(matches!(
        CryptoUtils::aes_mode(),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_encrypt("valid plaintext"),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_decrypt([]),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_encrypt_with_iv("plaintext", &[]),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_decrypt_with_iv([], &[]),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_encrypt_hex("plaintext"),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_decrypt_hex("not-hex"),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_encrypt_base64("plaintext", Base64Options::STANDARD),
        Err(CryptoError::NotInitialized)
    ));
    assert!(matches!(
        CryptoUtils::aes_decrypt_base64("!", Base64Options::STANDARD),
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

    let (mode, key_bytes) = if first_result.is_ok() {
        (AesMode::Gcm, [0x11; 16])
    } else {
        (AesMode::CbcPkcs7, [0x22; 16])
    };
    assert_eq!(CryptoUtils::aes_mode().unwrap(), mode);
    assert!(CryptoUtils::aes_is_initialized());
    assert!(matches!(
        CryptoUtils::aes_init(AesKey::from_bytes([0x55; 16]).unwrap(), AesMode::Gcm),
        Err(CryptoError::AlreadyInitialized)
    ));
    assert_eq!(CryptoUtils::aes_mode().unwrap(), mode);

    let cipher = AesCipher::from_key_bytes(key_bytes, mode).unwrap();
    let debug = format!("{cipher:?}");
    assert!(!debug.contains("17"));
    assert!(!debug.contains("34"));

    let container = CryptoUtils::aes_encrypt("global container").unwrap();
    assert_eq!(cipher.decrypt(&container).unwrap(), b"global container");
    let instance_container = cipher.encrypt("instance container").unwrap();
    assert_eq!(
        CryptoUtils::aes_decrypt(&instance_container).unwrap(),
        b"instance container"
    );

    let iv = vec![0u8; mode.iv_length()];
    let explicit = CryptoUtils::aes_encrypt_with_iv("global explicit", &iv).unwrap();
    assert_eq!(
        cipher.decrypt_with_iv(&explicit, &iv).unwrap(),
        b"global explicit"
    );
    let instance_explicit = cipher.encrypt_with_iv("instance explicit", &iv).unwrap();
    assert_eq!(
        CryptoUtils::aes_decrypt_with_iv(&instance_explicit, &iv).unwrap(),
        b"instance explicit"
    );

    let hex = CryptoUtils::aes_encrypt_hex("global hex").unwrap();
    assert_eq!(cipher.decrypt_hex(&hex).unwrap(), b"global hex");
    let instance_hex = cipher.encrypt_hex("instance hex").unwrap();
    assert_eq!(
        CryptoUtils::aes_decrypt_hex(&instance_hex).unwrap(),
        b"instance hex"
    );

    let standard =
        CryptoUtils::aes_encrypt_base64("global base64", Base64Options::STANDARD).unwrap();
    assert_eq!(
        cipher
            .decrypt_base64(&standard, Base64Options::STANDARD)
            .unwrap(),
        b"global base64"
    );
    let instance_base64 = cipher
        .encrypt_base64("instance base64", Base64Options::URL_SAFE_NO_PAD)
        .unwrap();
    assert_eq!(
        CryptoUtils::aes_decrypt_base64(&instance_base64, Base64Options::URL_SAFE_NO_PAD).unwrap(),
        b"instance base64"
    );

    let sentinel = b"SENTINEL_GLOBAL_AES_PLAINTEXT";
    let mut tampered = CryptoUtils::aes_encrypt(sentinel).unwrap();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    let error = CryptoUtils::aes_decrypt(&tampered).unwrap_err();
    assert!(!format!("{error}").contains("SENTINEL_GLOBAL_AES_PLAINTEXT"));
    assert!(!format!("{error:?}").contains("SENTINEL_GLOBAL_AES_PLAINTEXT"));
}
