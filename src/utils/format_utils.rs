//! 提供持续时间格式化、字符串脱敏和运行时模板渲染的工具。

mod duration;
mod mask;
#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
mod template;

/// 格式化与字符串脱敏工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct FormatUtils;

#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
pub use template::TemplateEngine;
