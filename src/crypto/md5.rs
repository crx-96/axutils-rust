//! MD5 摘要后端（`md-5` crate，通过 `md5` feature 启用，别名为 `::md5::`）。

use ::md5::{Digest, Md5};

pub(crate) fn digest(input: &[u8]) -> [u8; 16] {
    let mut hasher = Md5::new();
    hasher.update(input);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super as md5_digest;
    use crate::crypto::facade::CryptoUtils;

    // RFC 1321 §A.5 test suite (all 7 official vectors).
    #[test]
    fn rfc1321_test_suite() {
        let cases: [(&[u8], &str); 7] = [
            (b"", "d41d8cd98f00b204e9800998ecf8427e"),
            (b"a", "0cc175b9c0f1b6a831c399e269772661"),
            (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
            (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                b"abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ];
        for (input, expected_hex) in cases {
            let hex = CryptoUtils::hex_encode(md5_digest::digest(input)).unwrap();
            assert_eq!(hex, expected_hex, "input = {input:?}");
        }
    }

    #[test]
    fn digest_returns_16_bytes() {
        assert_eq!(md5_digest::digest(b"anything").len(), 16);
    }
}
