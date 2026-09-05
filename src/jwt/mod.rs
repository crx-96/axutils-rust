//! 固定算法、受限 claims 和脱敏错误边界的 JSON Web Signature 能力。
//!
//! 该模块只在 `jwt` feature 下公开。它支持 JWS 签名与验证，不提供 JWE payload 加密、
//! JWKS、撤销列表或运行时密钥轮换。

mod algorithm;
mod claims;
mod clock;
mod codec;
mod config;
mod error;
pub(crate) mod global;
mod header;
mod key;

pub use algorithm::JwtAlgorithm;
pub use codec::JwtCodec;
pub use config::{JwtConfig, JwtValidation};
pub use error::JwtError;
pub use key::{JwtSigningKey, JwtVerificationKey};

pub(crate) use algorithm::EcCurve;
pub(crate) use algorithm::KeyFamily;
pub(crate) use config::JwtConfigParts;
