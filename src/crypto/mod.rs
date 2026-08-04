//! 内存数据的编码、摘要与加解密能力。
//!
//! `crypto`/`crypto_utils` 模块、[`crate::CryptoUtils`]、[`CryptoError`] 基线变体（`OddHexLength`/
//! `InvalidHex`/`TextDecodeInvalid`/`OutputTooLarge`）、[`TextEncoding`]（含 `Utf8` 变体）与
//! 十六进制编解码在**任何 feature 组合下都可用**，不依赖任何第三方 crate。Base64、MD5、AES
//! 分别需要显式启用 `base64`、`md5`、`aes` feature；`encoding_rs` feature 为 [`TextEncoding`]
//! 追加六个 legacy 编码变体；同时启用 `aes` 与 `base64` 后额外提供
//! `aes_encrypt_base64`/`aes_decrypt_base64`。
//!
//! 本模块只负责“把内存中的一段数据安全地编码/摘要/加解密”：不提供非对称密码学、口令派生、
//! 密钥生命周期管理或流式/文件接口；错误不回显明文、密文、密钥、IV 或原始文本内容。

#[cfg(feature = "aes")]
mod aes;
#[cfg(feature = "base64")]
mod base64;
mod error;
mod hex;
#[cfg(feature = "md5")]
mod md5;
mod text;

pub use error::CryptoError;
pub use text::TextEncoding;

#[cfg(feature = "aes")]
pub use aes::{AesKey, AesKeyBits, AesMode};
#[cfg(feature = "base64")]
pub use base64::{Base64Alphabet, Base64Options};

pub(crate) use hex::{
    decode as hex_decode, encode_lower as hex_encode_lower, encode_upper as hex_encode_upper,
};

#[cfg(feature = "md5")]
pub(crate) use hex::encode_lower_fixed;

#[cfg(feature = "base64")]
pub(crate) use base64::{decode as base64_decode, encode as base64_encode};

#[cfg(feature = "md5")]
pub(crate) use md5::digest as md5_digest;

#[cfg(feature = "aes")]
pub(crate) use aes::{
    decrypt as aes_decrypt_container, decrypt_explicit_iv as aes_decrypt_explicit_iv,
    encrypt as aes_encrypt_container, encrypt_explicit_iv as aes_encrypt_explicit_iv,
};
