//! `ConvertUtils` 静态工具类的兼容性导出。
//!
//! 与 `ConfigUtils` 一样，`ConvertUtils` 的面向调用方工具类位于 `src/utils/`，不保存全局
//! 可变状态，也不需要实例化。整数、浮点和 UUID 的后端实现分别位于
//! [`crate::convert`] 子模块；本文件只维护 `utils` 命名空间下的工具类公共入口。

/// 提供整数、浮点数和 UUID 转换入口的无状态工具类。
///
/// `ConvertUtils` 不保存解析器、全局 buffer 或运行时状态。可用的关联方法由对应 feature
/// 决定：`itoa` 提供整数方法，`ryu`/`zmij` 提供浮点方法，`uuid` 提供 UUID 方法。
///
/// # Examples
///
/// ```
/// use axutils::utils::convert_utils::ConvertUtils;
///
/// let _utils = ConvertUtils;
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ConvertUtils;
