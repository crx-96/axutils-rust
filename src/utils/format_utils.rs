//! 提供持续时间格式化、字符串脱敏、轻量文本处理和运行时模板渲染的工具。

mod duration;
mod mask;
#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
mod template;
mod text;

/// 无状态的格式化、字符串脱敏和文本处理工具。
///
/// HTML 转义与字面标记替换默认可用且分别显式调用；模板渲染通过对应 template feature 启用。
#[derive(Debug, Clone, Copy, Default)]
pub struct FormatUtils;

#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
pub use template::TemplateEngine;
