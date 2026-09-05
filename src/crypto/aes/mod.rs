//! AES-GCM / AES-CBC-PKCS7 后端（`aes`/`aes-gcm`/`cbc`/`zeroize` crate，feature = `aes`）。

mod cbc;
mod container;
mod gcm;
mod key;
mod mode;
mod random;

#[cfg(test)]
mod tests;

pub use key::{AesKey, AesKeyBits};
pub use mode::AesMode;

pub(crate) use container::{decrypt, decrypt_explicit_iv, encrypt, encrypt_explicit_iv};
