// Positive fixtures: baseline (hex + TextEncoding::Utf8) must compile under every feature
// combination, including no crypto feature at all (decision A2); each combination additionally
// exercises the capability newly unlocked by its feature set.

fn assert_baseline() {
    use axutils::{CryptoUtils, TextEncoding};
    let _: axutils::CryptoUtils = CryptoUtils::default();
    let _: axutils::utils::CryptoUtils = CryptoUtils::default();
    let _: axutils::crypto_utils::CryptoUtils = CryptoUtils::default();
    let _: axutils::utils::crypto_utils::CryptoUtils = CryptoUtils::default();
    let _: axutils::crypto::CryptoError = axutils::crypto::CryptoError::OddHexLength { length: 1 };
    let _: axutils::crypto::TextEncoding = axutils::crypto::TextEncoding::Utf8;
    let encoded = CryptoUtils::hex_encode([0x00, 0xff]).expect("hex encode");
    assert_eq!(encoded, "00ff");
    assert_eq!(CryptoUtils::hex_decode(&encoded).expect("hex decode"), vec![0x00, 0xff]);
    assert_eq!(TextEncoding::Utf8.decode(b"hi").expect("utf8 decode"), "hi");
}

#[cfg(any(
    feature = "aes-only",
    feature = "base64-aes",
    feature = "md5-aes",
    feature = "aes-encoding-rs",
    feature = "base64-md5-aes",
    feature = "base64-aes-encoding-rs",
    feature = "md5-aes-encoding-rs",
    feature = "all",
))]
fn assert_aes_api() {
    use axutils::{AesCipher, AesKey, AesMode, CryptoError, CryptoUtils};

    let key = AesKey::from_bytes([0x00; 16]).expect("AES key");
    let _: axutils::crypto::AesKey = key;
    let _: axutils::crypto::AesKeyBits = axutils::AesKeyBits::Aes128;
    let _: axutils::crypto::AesMode = AesMode::Gcm;
    let _: axutils::AesCipher = AesCipher::from_key_bytes([0x01; 16], AesMode::Gcm).unwrap();
    let _: axutils::crypto::AesCipher = AesCipher::from_key_bytes([0x02; 16], AesMode::Gcm).unwrap();
    let _: axutils::crypto::CryptoError = CryptoError::NotInitialized;
    let _: axutils::crypto::CryptoError = CryptoError::AlreadyInitialized;

    CryptoUtils::aes_init_from_bytes([0x03; 16], AesMode::Gcm).expect("global AES init");
    let ciphertext = CryptoUtils::aes_encrypt_hex("hi").expect("global encrypt");
    assert_eq!(CryptoUtils::aes_decrypt_hex(&ciphertext).unwrap(), b"hi");

    let cipher = AesCipher::from_key_bytes([0x04; 16], AesMode::CbcPkcs7).unwrap();
    let ciphertext = cipher.encrypt("instance").unwrap();
    assert_eq!(cipher.decrypt(&ciphertext).unwrap(), b"instance");

    #[cfg(any(
        feature = "base64-aes",
        feature = "base64-md5-aes",
        feature = "base64-aes-encoding-rs",
        feature = "all",
    ))]
    {
        let encoded = CryptoUtils::aes_encrypt_base64("hi", axutils::Base64Options::STANDARD)
            .expect("global base64 encrypt");
        assert_eq!(
            CryptoUtils::aes_decrypt_base64(&encoded, axutils::Base64Options::STANDARD).unwrap(),
            b"hi"
        );
        let encoded = cipher
            .encrypt_base64("instance", axutils::Base64Options::URL_SAFE_NO_PAD)
            .unwrap();
        assert_eq!(
            cipher
                .decrypt_base64(&encoded, axutils::Base64Options::URL_SAFE_NO_PAD)
                .unwrap(),
            b"instance"
        );
    }
}

#[cfg(feature = "none")]
fn main() {
    assert_baseline();
}

#[cfg(feature = "encoding-rs-only")]
fn main() {
    assert_baseline();
    use axutils::TextEncoding;
    let bytes = TextEncoding::Gbk.encode("你好").expect("gbk encode");
    assert_eq!(TextEncoding::Gbk.decode(bytes).expect("gbk decode"), "你好");
}

#[cfg(feature = "base64-only")]
fn main() {
    assert_baseline();
    use axutils::{Base64Options, CryptoUtils};
    let _: axutils::crypto::Base64Options = Base64Options::STANDARD;
    let _: axutils::crypto::Base64Alphabet = Base64Options::STANDARD.alphabet();
    let encoded = CryptoUtils::base64_encode("foobar", Base64Options::STANDARD).unwrap();
    assert_eq!(encoded, "Zm9vYmFy");
}

#[cfg(feature = "md5-only")]
fn main() {
    assert_baseline();
    use axutils::CryptoUtils;
    assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
}

#[cfg(feature = "aes-only")]
fn main() {
    assert_baseline();
    assert_aes_api();
}

#[cfg(feature = "base64-md5")]
fn main() {
    assert_baseline();
    use axutils::{Base64Options, CryptoUtils};
    let _ = CryptoUtils::base64_encode("x", Base64Options::STANDARD).unwrap();
    let _ = CryptoUtils::md5_hex("x");
}

#[cfg(feature = "base64-aes")]
fn main() {
    assert_baseline();
    assert_aes_api();
}

#[cfg(feature = "base64-encoding-rs")]
fn main() {
    assert_baseline();
    use axutils::{Base64Options, CryptoUtils, TextEncoding};
    let _ = CryptoUtils::base64_encode_text("你好", TextEncoding::Gbk, Base64Options::STANDARD).unwrap();
}

#[cfg(feature = "md5-aes")]
fn main() {
    assert_baseline();
    let _ = axutils::CryptoUtils::md5_hex("x");
    assert_aes_api();
}

#[cfg(feature = "md5-encoding-rs")]
fn main() {
    assert_baseline();
    use axutils::{CryptoUtils, TextEncoding};
    let _ = CryptoUtils::md5_hex_text("你好", TextEncoding::Gbk).unwrap();
}

#[cfg(feature = "aes-encoding-rs")]
fn main() {
    assert_baseline();
    assert_aes_api();
    use axutils::TextEncoding;
    let bytes = TextEncoding::Gbk.encode("你好").unwrap();
    assert_eq!(TextEncoding::Gbk.decode(bytes).unwrap(), "你好");
}

#[cfg(feature = "base64-md5-aes")]
fn main() {
    assert_baseline();
    let _ = axutils::CryptoUtils::md5_hex("x");
    assert_aes_api();
}

#[cfg(feature = "base64-md5-encoding-rs")]
fn main() {
    assert_baseline();
    use axutils::{Base64Options, CryptoUtils, TextEncoding};
    let _ = CryptoUtils::md5_hex_text("你好", TextEncoding::Gbk).unwrap();
    let _ = CryptoUtils::base64_encode_text("你好", TextEncoding::Gbk, Base64Options::STANDARD).unwrap();
}

#[cfg(feature = "base64-aes-encoding-rs")]
fn main() {
    assert_baseline();
    assert_aes_api();
    use axutils::TextEncoding;
    let bytes = TextEncoding::Gbk.encode("你好").unwrap();
    assert_eq!(TextEncoding::Gbk.decode(bytes).unwrap(), "你好");
}

#[cfg(feature = "md5-aes-encoding-rs")]
fn main() {
    assert_baseline();
    use axutils::TextEncoding;
    let _ = axutils::CryptoUtils::md5_hex_text("你好", TextEncoding::Gbk).unwrap();
    assert_aes_api();
}

#[cfg(feature = "all")]
fn main() {
    assert_baseline();
    use axutils::TextEncoding;
    let _ = axutils::CryptoUtils::md5_hex_text("你好", TextEncoding::Gbk).unwrap();
    assert_aes_api();
}

// Negative fixtures: each references exactly one API that must NOT exist under the given
// feature combination. The diagnostic token asserted by the test harness must appear in the
// resulting compiler error.

#[cfg(feature = "negative-none-base64")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-none-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-none-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-none-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-none-aes-errors")]
fn main() {
    let _ = axutils::CryptoError::NotInitialized;
    let _ = axutils::CryptoError::AlreadyInitialized;
}

#[cfg(feature = "negative-none-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-encoding-rs-only-base64")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-encoding-rs-only-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-encoding-rs-only-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-encoding-rs-only-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-base64-only-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-base64-only-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-base64-only-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-base64-only-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-md5-only-base64")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-md5-only-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-md5-only-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-md5-only-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-aes-only-base64")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-aes-only-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-aes-only-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-aes-only-aes-base64-combo")]
fn main() {
    let _ = axutils::CryptoUtils::aes_encrypt_base64;
}

#[cfg(feature = "negative-aes-base64-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-aes-base64-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-base64-encoding-rs-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-base64-encoding-rs-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-base64-encoding-rs-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-base64-md5-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-base64-md5-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-base64-md5-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-base64-md5-encoding-rs")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-base64-md5-encoding-rs-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-md5-aes-base64-combo")]
fn main() {
    let _ = axutils::CryptoUtils::aes_encrypt_base64;
}

#[cfg(feature = "negative-md5-aes-legacy-encoding")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-md5-aes-encoding-rs")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-md5-aes-encoding-rs-base64-combo")]
fn main() {
    let _ = axutils::CryptoUtils::aes_encrypt_base64;
}

#[cfg(feature = "negative-md5-encoding-rs-base64")]
fn main() {
    let _ = axutils::CryptoUtils::base64_encode;
}

#[cfg(feature = "negative-md5-encoding-rs-aes")]
fn main() {
    let _ = axutils::AesKey::from_bytes;
}

#[cfg(feature = "negative-md5-encoding-rs-aescipher")]
fn main() {
    let _ = axutils::AesCipher::from_key_bytes;
}

#[cfg(feature = "negative-aes-encoding-rs-base64-combo")]
fn main() {
    let _ = axutils::CryptoUtils::aes_encrypt_base64;
}

#[cfg(feature = "negative-aes-encoding-rs-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(feature = "negative-base64-md5-aes-encoding-rs")]
fn main() {
    let _ = axutils::TextEncoding::Gbk;
}

#[cfg(feature = "negative-base64-aes-encoding-rs-md5")]
fn main() {
    let _ = axutils::CryptoUtils::md5;
}

#[cfg(not(any(
    feature = "none",
    feature = "encoding-rs-only",
    feature = "base64-only",
    feature = "md5-only",
    feature = "aes-only",
    feature = "base64-md5",
    feature = "base64-aes",
    feature = "base64-encoding-rs",
    feature = "md5-aes",
    feature = "md5-encoding-rs",
    feature = "aes-encoding-rs",
    feature = "base64-md5-aes",
    feature = "base64-md5-encoding-rs",
    feature = "base64-aes-encoding-rs",
    feature = "md5-aes-encoding-rs",
    feature = "all",
    feature = "negative-none-base64",
    feature = "negative-none-md5",
    feature = "negative-none-aes",
    feature = "negative-none-aescipher",
    feature = "negative-none-aes-errors",
    feature = "negative-none-legacy-encoding",
    feature = "negative-encoding-rs-only-base64",
    feature = "negative-encoding-rs-only-md5",
    feature = "negative-encoding-rs-only-aes",
    feature = "negative-encoding-rs-only-aescipher",
    feature = "negative-base64-only-md5",
    feature = "negative-base64-only-aes",
    feature = "negative-base64-only-aescipher",
    feature = "negative-base64-only-legacy-encoding",
    feature = "negative-md5-only-base64",
    feature = "negative-md5-only-aes",
    feature = "negative-md5-only-aescipher",
    feature = "negative-md5-only-legacy-encoding",
    feature = "negative-aes-only-base64",
    feature = "negative-aes-only-md5",
    feature = "negative-aes-only-legacy-encoding",
    feature = "negative-aes-only-aes-base64-combo",
    feature = "negative-aes-base64-md5",
    feature = "negative-aes-base64-legacy-encoding",
    feature = "negative-base64-encoding-rs-md5",
    feature = "negative-base64-encoding-rs-aes",
    feature = "negative-base64-encoding-rs-aescipher",
    feature = "negative-base64-md5-aes",
    feature = "negative-base64-md5-aescipher",
    feature = "negative-base64-md5-legacy-encoding",
    feature = "negative-base64-md5-encoding-rs",
    feature = "negative-base64-md5-encoding-rs-aescipher",
    feature = "negative-md5-aes-base64-combo",
    feature = "negative-md5-aes-legacy-encoding",
    feature = "negative-md5-aes-encoding-rs",
    feature = "negative-md5-aes-encoding-rs-base64-combo",
    feature = "negative-md5-encoding-rs-base64",
    feature = "negative-md5-encoding-rs-aes",
    feature = "negative-md5-encoding-rs-aescipher",
    feature = "negative-aes-encoding-rs-base64-combo",
    feature = "negative-aes-encoding-rs-md5",
    feature = "negative-base64-md5-aes-encoding-rs",
    feature = "negative-base64-aes-encoding-rs-md5",
)))]
fn main() {}
