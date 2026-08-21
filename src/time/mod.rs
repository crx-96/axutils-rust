//! `TimeUtils` 的按需日期格式化实现。

mod error;
mod offset;
mod template;

#[cfg(feature = "chrono")]
mod chrono;
#[cfg(feature = "jiff")]
mod jiff;
#[cfg(feature = "time")]
#[allow(clippy::module_inception)]
mod time;

pub use error::TimeError;
pub use offset::{TimeZoneOffset, TimeZoneOffsetError};
pub use template::{TimeFormatError, TimeFormatToken, TimeValueKind};
