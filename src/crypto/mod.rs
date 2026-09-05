//! 内存数据的编码、摘要与加解密能力。
//!
//! `crypto` 模块、[`crate::utils::CryptoUtils`]、[`CryptoError`] 基线变体（`OddHexLength`/
//! `InvalidHex`/`TextDecodeInvalid`/`OutputTooLarge`）、[`TextEncoding`]（含 `Utf8` 变体）与
//! 十六进制编解码在**任何 feature 组合下都可用**，不依赖任何第三方 crate。Base64、MD5、AES
//! 分别需要显式启用 `base64`、`md5`、`aes` feature；`encoding_rs` feature 为 [`TextEncoding`]
//! 追加六个 legacy 编码变体；同时启用 `aes` 与 `base64` 后，实例 [`AesCipher`] 额外提供
//! `encrypt_base64`/`decrypt_base64`。
//!
//! 本模块只负责“把内存中的一段数据安全地编码/摘要/加解密”：不提供非对称密码学、口令派生、
//! 密钥存储/轮换/封装策略或流式/文件接口；启用 `aes` 后，`AesCipher` 提供实例级可控密钥
//! 生命周期，`CryptoUtils` 的全局 AES 便捷入口则使用进程级单例。错误不回显明文、密文、密钥、
//! IV 或原始文本内容。

#[cfg(feature = "aes")]
mod aes;
#[cfg(feature = "base64")]
mod base64;
#[cfg(feature = "aes")]
mod cipher;
mod error;
pub(crate) mod facade;
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
#[cfg(feature = "aes")]
pub use cipher::AesCipher;
