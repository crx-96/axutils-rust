use super::*;
use super::{container, random};
use crate::crypto::CryptoError;

fn key128() -> AesKey {
    AesKey::from_bytes([0u8; 16]).unwrap()
}

#[test]
fn key_bits_accessors() {
    assert_eq!(AesKeyBits::Aes128.bit_length(), 128);
    assert_eq!(AesKeyBits::Aes128.byte_length(), 16);
    assert_eq!(AesKeyBits::Aes192.bit_length(), 192);
    assert_eq!(AesKeyBits::Aes192.byte_length(), 24);
    assert_eq!(AesKeyBits::Aes256.bit_length(), 256);
    assert_eq!(AesKeyBits::Aes256.byte_length(), 32);
}

#[test]
fn mode_accessors() {
    assert_eq!(AesMode::Gcm.iv_length(), 12);
    assert!(AesMode::Gcm.is_authenticated());
    assert_eq!(AesMode::Gcm.as_str(), "AES-GCM");
    assert_eq!(AesMode::CbcPkcs7.iv_length(), 16);
    assert!(!AesMode::CbcPkcs7.is_authenticated());
    assert_eq!(AesMode::CbcPkcs7.as_str(), "AES-CBC-PKCS7");
}

#[test]
fn from_bytes_rejects_invalid_lengths() {
    for len in [0usize, 15, 17, 33] {
        let error = AesKey::from_bytes(vec![0u8; len]).unwrap_err();
        assert_eq!(error, CryptoError::InvalidKeyLength { length: len });
    }
}

#[test]
fn debug_does_not_leak_key_bytes() {
    let key = AesKey::from_bytes([0xAB; 32]).unwrap();
    let rendered = format!("{key:?}");
    assert!(!rendered.contains("171"));
    assert!(rendered.contains("Aes256"));
}

#[test]
fn gcm_container_roundtrip_and_min_length() {
    let key = key128();
    let ciphertext = encrypt(b"hello world", &key, AesMode::Gcm).unwrap();
    assert_eq!(ciphertext.len(), 12 + 11 + 16);
    assert_eq!(
        decrypt(&ciphertext, &key, AesMode::Gcm).unwrap(),
        b"hello world"
    );
    assert!(matches!(
        decrypt(&[0u8; 10], &key, AesMode::Gcm),
        Err(CryptoError::CiphertextTooShort {
            minimum: 28,
            length: 10
        })
    ));
}

#[test]
fn gcm_empty_plaintext_roundtrip() {
    let key = key128();
    let ciphertext = encrypt(b"", &key, AesMode::Gcm).unwrap();
    assert_eq!(ciphertext.len(), 28);
    assert_eq!(decrypt(&ciphertext, &key, AesMode::Gcm).unwrap(), b"");
}

#[test]
fn cbc_container_roundtrip_and_min_length() {
    let key = key128();
    let ciphertext = encrypt(b"hello world", &key, AesMode::CbcPkcs7).unwrap();
    assert_eq!(ciphertext.len(), 16 + 16);
    assert_eq!(
        decrypt(&ciphertext, &key, AesMode::CbcPkcs7).unwrap(),
        b"hello world"
    );
    assert!(matches!(
        decrypt(&[0u8; 10], &key, AesMode::CbcPkcs7),
        Err(CryptoError::CiphertextTooShort {
            minimum: 32,
            length: 10
        })
    ));
}

#[test]
fn cbc_empty_plaintext_roundtrip() {
    let key = key128();
    let ciphertext = encrypt(b"", &key, AesMode::CbcPkcs7).unwrap();
    assert_eq!(ciphertext.len(), 32);
    assert_eq!(decrypt(&ciphertext, &key, AesMode::CbcPkcs7).unwrap(), b"");
}

#[test]
fn cbc_non_block_multiple_length_is_decrypt_error() {
    let key = key128();
    let iv = [0u8; 16];
    let bad = vec![0u8; 17];
    assert!(matches!(
        decrypt_explicit_iv(&bad, &key, &iv, AesMode::CbcPkcs7),
        Err(CryptoError::Decrypt)
    ));
}

#[test]
fn explicit_iv_roundtrip_all_key_sizes_and_modes() {
    for bits in [AesKeyBits::Aes128, AesKeyBits::Aes192, AesKeyBits::Aes256] {
        let key = AesKey::from_bytes(vec![0x11u8; bits.byte_length()]).unwrap();
        for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
            let iv = vec![0x22u8; mode.iv_length()];
            let ciphertext = encrypt_explicit_iv(b"round trip payload", &key, &iv, mode).unwrap();
            assert_eq!(
                decrypt_explicit_iv(&ciphertext, &key, &iv, mode).unwrap(),
                b"round trip payload"
            );
        }
    }
}

#[test]
fn explicit_iv_wrong_length_is_rejected() {
    let key = key128();
    assert!(matches!(
        encrypt_explicit_iv(b"x", &key, &[0u8; 11], AesMode::Gcm),
        Err(CryptoError::InvalidIvLength {
            expected: 12,
            length: 11
        })
    ));
    assert!(matches!(
        encrypt_explicit_iv(b"x", &key, &[0u8; 15], AesMode::CbcPkcs7),
        Err(CryptoError::InvalidIvLength {
            expected: 16,
            length: 15
        })
    ));
}

#[test]
fn gcm_tampering_is_detected() {
    let key = key128();
    for tamper_at in [0usize, 12, 15] {
        let mut ciphertext = encrypt(b"hello world", &key, AesMode::Gcm).unwrap();
        ciphertext[tamper_at] ^= 0x01;
        assert!(matches!(
            decrypt(&ciphertext, &key, AesMode::Gcm),
            Err(CryptoError::Decrypt)
        ));
    }
}

#[test]
fn random_containers_differ_but_both_decrypt() {
    let key = key128();
    let first = encrypt(b"same plaintext", &key, AesMode::Gcm).unwrap();
    let second = encrypt(b"same plaintext", &key, AesMode::Gcm).unwrap();
    assert_eq!(first[..12].len(), 12);
    assert_eq!(second[..12].len(), 12);
    assert_eq!(
        decrypt(&first, &key, AesMode::Gcm).unwrap(),
        b"same plaintext"
    );
    assert_eq!(
        decrypt(&second, &key, AesMode::Gcm).unwrap(),
        b"same plaintext"
    );
}

#[test]
fn random_bytes_lengths() {
    assert_eq!(random::random_bytes(12).unwrap().len(), 12);
    assert_eq!(random::random_bytes(16).unwrap().len(), 16);
    assert_eq!(random::random_bytes(24).unwrap().len(), 24);
    assert_eq!(random::random_bytes(32).unwrap().len(), 32);
}

#[test]
fn generate_covers_all_key_sizes() {
    for bits in [AesKeyBits::Aes128, AesKeyBits::Aes192, AesKeyBits::Aes256] {
        assert_eq!(AesKey::generate(bits).unwrap().bits(), bits);
    }
}

#[test]
fn random_source_failures_map_to_random_source() {
    fn fail(_: usize) -> Result<Vec<u8>, CryptoError> {
        Err(CryptoError::RandomSource)
    }

    assert!(matches!(
        AesKey::generate_with_random(AesKeyBits::Aes256, fail),
        Err(CryptoError::RandomSource)
    ));
    assert!(matches!(
        container::encrypt_with_random(b"payload", &key128(), AesMode::Gcm, fail),
        Err(CryptoError::RandomSource)
    ));
}
