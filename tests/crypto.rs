#![cfg(all(feature = "base64", feature = "md5", feature = "aes"))]

use axutils::{AesKey, AesMode, Base64Alphabet, Base64Options, CryptoUtils};

fn hex_bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2));
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(text, 16).unwrap()
        })
        .collect()
}

// RFC 4648 §10 test vectors, standard alphabet with padding.
#[test]
fn base64_rfc4648_standard_padded_vectors() {
    let cases: [(&str, &str); 7] = [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];
    for (plain, expected) in cases {
        let encoded = CryptoUtils::base64_encode(plain, Base64Options::STANDARD).unwrap();
        assert_eq!(encoded, expected);
        assert_eq!(
            CryptoUtils::base64_decode(expected, Base64Options::STANDARD).unwrap(),
            plain.as_bytes()
        );
    }
}

#[test]
fn base64_rfc4648_vectors_cover_all_option_combinations() {
    let cases: [(&str, &str, &str); 7] = [
        ("", "", ""),
        ("f", "Zg==", "Zg"),
        ("fo", "Zm8=", "Zm8"),
        ("foo", "Zm9v", "Zm9v"),
        ("foob", "Zm9vYg==", "Zm9vYg"),
        ("fooba", "Zm9vYmE=", "Zm9vYmE"),
        ("foobar", "Zm9vYmFy", "Zm9vYmFy"),
    ];
    for (options, padded) in [
        (Base64Options::STANDARD, true),
        (Base64Options::STANDARD_NO_PAD, false),
        (Base64Options::URL_SAFE, true),
        (Base64Options::URL_SAFE_NO_PAD, false),
    ] {
        for (input, padded_expected, no_pad_expected) in cases {
            let expected = if padded {
                padded_expected
            } else {
                no_pad_expected
            };
            let encoded = CryptoUtils::base64_encode(input, options).unwrap();
            assert_eq!(encoded, expected);
            assert_eq!(
                CryptoUtils::base64_decode(expected, options).unwrap(),
                input.as_bytes()
            );
        }
    }
}

#[test]
fn base64_url_safe_and_no_pad_options_are_independent() {
    let input: &[u8] = &[0xfb, 0xff, 0xfe];
    let url_no_pad = CryptoUtils::base64_encode(input, Base64Options::URL_SAFE_NO_PAD).unwrap();
    assert!(!url_no_pad.contains('+') && !url_no_pad.contains('/') && !url_no_pad.contains('='));
    assert_eq!(
        CryptoUtils::base64_decode(&url_no_pad, Base64Options::URL_SAFE_NO_PAD).unwrap(),
        input
    );
    assert_eq!(
        Base64Options::URL_SAFE_NO_PAD.alphabet(),
        Base64Alphabet::UrlSafe
    );
    assert!(!Base64Options::URL_SAFE_NO_PAD.padding());
}

// RFC 1321 §A.5, all 7 official MD5 test vectors.
#[test]
fn md5_rfc1321_test_suite() {
    let cases: [(&str, &str); 7] = [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ("abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            "abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
        (
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        (
            "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            "57edf4a22be3c955ac49da2e2107b67a",
        ),
    ];
    for (input, expected_hex) in cases {
        assert_eq!(
            CryptoUtils::md5_hex(input),
            expected_hex,
            "input = {input:?}"
        );
        let digest = CryptoUtils::md5(input);
        assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), expected_hex);
    }
}

#[test]
fn aes_cbc_nist_sp800_38a_vectors_include_pkcs7_block() {
    let plaintext = hex_bytes(
        "6bc1bee22e409f96e93d7e117393172a\
         ae2d8a571e03ac9c9eb76fac45af8e51\
         30c81c46a35ce411e5fbc1191a0a52ef\
         f69f2445df4f9b17ad2b417be66c3710",
    );
    let iv = hex_bytes("000102030405060708090a0b0c0d0e0f");
    let cases = [
        (
            "2b7e151628aed2a6abf7158809cf4f3c",
            "7649abac8119b246cee98e9b12e9197d\
             5086cb9b507219ee95db113a917678b2\
             73bed6b8e3c1743b7116e69e22229516\
             3ff1caa1681fac09120eca307586e1a7\
             8cb82807230e1321d3fae00d18cc2012",
        ),
        (
            "8e73b0f7da0e6452c810f32b809079e562f8ead2522c6b7b",
            "4f021db243bc633d7178183a9fa071e8\
             b4d9ada9ad7dedf4e5e738763f69145a\
             571b242012fb7ae07fa9baac3df102e0\
             08b0e27988598881d920a9e64f5615cd\
             612ccd79224b350935d45dd6a98f8176",
        ),
        (
            "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4",
            "f58c4c04d6e5f1ba779eabfb5f7bfbd6\
             9cfc4e967edb808d679f777bc6702c7d\
             39f23369a9d9bacfa530e26304231461\
             b2eb05e2c39be9fcda6c19078c6a9d1b\
             3f461796d6b0d6b2e0c2a72b4d80e644",
        ),
    ];

    for (key_hex, expected_hex) in cases {
        let key = AesKey::from_bytes(hex_bytes(key_hex)).unwrap();
        let expected = hex_bytes(expected_hex);
        let ciphertext =
            CryptoUtils::aes_encrypt_with_iv(&plaintext, &key, &iv, AesMode::CbcPkcs7).unwrap();
        assert_eq!(ciphertext, expected, "key = {key_hex}");
        assert_eq!(
            CryptoUtils::aes_decrypt_with_iv(&ciphertext, &key, &iv, AesMode::CbcPkcs7).unwrap(),
            plaintext,
            "key = {key_hex}"
        );
    }
}

#[test]
fn aes_gcm_nist_vectors_cover_all_key_sizes() {
    let iv = hex_bytes("cafebabefacedbaddecaf888");
    let cases = [
        (
            "feffe9928665731c6d6a8f9467308308",
            "3247184b3c4f69a44dbcd22887bbb418",
        ),
        (
            "feffe9928665731c6d6a8f9467308308feffe9928665731c",
            "c835aa88aebbc94f5a02e179fdcfc3e4",
        ),
        (
            "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308",
            "fd2caa16a5832e76aa132c1453eeda7e",
        ),
    ];

    for (key_hex, expected_hex) in cases {
        let key = AesKey::from_bytes(hex_bytes(key_hex)).unwrap();
        let expected = hex_bytes(expected_hex);
        let ciphertext = CryptoUtils::aes_encrypt_with_iv([], &key, &iv, AesMode::Gcm).unwrap();
        assert_eq!(ciphertext, expected, "key = {key_hex}");
        assert_eq!(
            CryptoUtils::aes_decrypt_with_iv(&ciphertext, &key, &iv, AesMode::Gcm).unwrap(),
            Vec::<u8>::new(),
            "key = {key_hex}"
        );
    }
}

#[test]
fn aes_cbc_is_deterministic_for_fixed_key_iv_and_correct_across_key_sizes() {
    // 65 bytes: not a multiple of the 16-byte block size, exercising PKCS#7 padding across an
    // odd trailing partial block.
    let plaintext: Vec<u8> = (0u8..65).collect();
    let iv = [0u8; 16];

    for key_len in [16usize, 24, 32] {
        let key = AesKey::from_bytes(vec![0x11u8; key_len]).unwrap();
        let a = CryptoUtils::aes_encrypt_with_iv(&plaintext, &key, &iv, AesMode::CbcPkcs7).unwrap();
        let b = CryptoUtils::aes_encrypt_with_iv(&plaintext, &key, &iv, AesMode::CbcPkcs7).unwrap();
        // Fixed key/IV must be fully deterministic (unlike the random-IV container path).
        assert_eq!(a, b);
        // 65-byte plaintext pads up to 80 bytes (5 blocks).
        assert_eq!(a.len(), 80);
        assert_eq!(
            CryptoUtils::aes_decrypt_with_iv(&a, &key, &iv, AesMode::CbcPkcs7).unwrap(),
            plaintext
        );

        // A different key must produce different ciphertext for the same plaintext/IV.
        let other_key = AesKey::from_bytes(vec![0x22u8; key_len]).unwrap();
        let c = CryptoUtils::aes_encrypt_with_iv(&plaintext, &other_key, &iv, AesMode::CbcPkcs7)
            .unwrap();
        assert_ne!(a, c);
    }
}

#[test]
fn aes_gcm_authenticates_and_rejects_tampering_for_all_key_sizes() {
    for key_len in [16usize, 24, 32] {
        let key = AesKey::from_bytes(vec![0x42u8; key_len]).unwrap();
        let nonce = vec![0x24u8; 12];
        let plaintext = b"NIST GCM key-length coverage payload";
        let ciphertext =
            CryptoUtils::aes_encrypt_with_iv(plaintext, &key, &nonce, AesMode::Gcm).unwrap();
        assert_eq!(ciphertext.len(), plaintext.len() + 16);
        assert_eq!(
            CryptoUtils::aes_decrypt_with_iv(&ciphertext, &key, &nonce, AesMode::Gcm).unwrap(),
            plaintext
        );

        for tamper_at in [0usize, ciphertext.len() / 2, ciphertext.len() - 1] {
            let mut tampered = ciphertext.clone();
            tampered[tamper_at] ^= 0x01;
            assert!(
                CryptoUtils::aes_decrypt_with_iv(&tampered, &key, &nonce, AesMode::Gcm).is_err()
            );
        }
    }
}

#[test]
fn error_display_and_debug_never_echo_sentinel_secrets() {
    const SENTINEL_PLAINTEXT: &[u8] = b"SENTINEL_PLAINTEXT_MARKER";
    const SENTINEL_KEY: [u8; 16] = [0x99; 16];

    let key = AesKey::from_bytes(SENTINEL_KEY).unwrap();
    assert!(!format!("{key:?}").contains("153")); // 0x99 decimal, would only appear if leaked

    let ciphertext = CryptoUtils::aes_encrypt(SENTINEL_PLAINTEXT, &key, AesMode::Gcm).unwrap();
    let mut tampered = ciphertext.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    let err = CryptoUtils::aes_decrypt(&tampered, &key, AesMode::Gcm).unwrap_err();
    let rendered_display = format!("{err}");
    let rendered_debug = format!("{err:?}");
    assert!(!rendered_display
        .as_bytes()
        .windows(SENTINEL_PLAINTEXT.len())
        .any(|w| w == SENTINEL_PLAINTEXT));
    assert!(!rendered_debug
        .as_bytes()
        .windows(SENTINEL_PLAINTEXT.len())
        .any(|w| w == SENTINEL_PLAINTEXT));

    let bad_hex_err = CryptoUtils::hex_decode("zz").unwrap_err();
    assert!(!format!("{bad_hex_err}").contains("SENTINEL"));
}

#[test]
fn aes_hex_and_base64_convenience_methods_share_the_same_container_layout() {
    let key = AesKey::from_bytes([0x07u8; 32]).unwrap();
    for mode in [AesMode::Gcm, AesMode::CbcPkcs7] {
        let plaintext = "convenience method payload";
        let hex = CryptoUtils::aes_encrypt_hex(plaintext, &key, mode).unwrap();
        let container = CryptoUtils::hex_decode(&hex).unwrap();
        assert_eq!(
            CryptoUtils::aes_decrypt(&container, &key, mode).unwrap(),
            plaintext.as_bytes()
        );

        let b64 = CryptoUtils::aes_encrypt_base64(plaintext, &key, mode, Base64Options::STANDARD)
            .unwrap();
        let container2 = CryptoUtils::base64_decode(&b64, Base64Options::STANDARD).unwrap();
        assert_eq!(
            CryptoUtils::aes_decrypt(&container2, &key, mode).unwrap(),
            plaintext.as_bytes()
        );
    }
}
