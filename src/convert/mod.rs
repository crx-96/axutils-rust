//! 字符串与整数、浮点数、UUID 之间的显式转换。
//!
//! [`crate::utils::ConvertUtils`] 本身始终可用；具体转换方法和 buffer 类型按 `itoa`、`ryu`、`zmij`、
//! `uuid` feature 独立开放。实现子模块是私有的，调用方只应使用本模块导出的公共类型和
//! 方法。

pub(crate) mod facade;
#[cfg(any(feature = "ryu", feature = "zmij"))]
mod float;
#[cfg(feature = "itoa")]
mod integer;
#[cfg(feature = "uuid")]
mod uuid;

#[cfg(feature = "itoa")]
pub use integer::{IntegerBuffer, IntegerValue};

#[cfg(any(feature = "ryu", feature = "zmij"))]
pub use float::{FloatBuffer, FloatFormat, FloatValue};

#[cfg(feature = "uuid")]
pub use uuid::UuidBuffer;
