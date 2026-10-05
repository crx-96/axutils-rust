//! 平铺记录的完整森林构建与可失败的后序转换；默认可用，不依赖第三方 crate。
//!
//! ID、父 ID、排序值及允许深度由调用方提供，本模块只验证树结构。
//! 构建与转换使用显式栈，但返回的 [`TreeNode`] 仍是递归对象：序列化、派生 trait、
//! 析构以及转换结果的处理不因此支持任意深度。调用方应按后续操作设置合理深度上限。

mod build;
mod error;
mod node;

pub use build::build_forest;
pub use error::TreeBuildError;
pub use node::TreeNode;
