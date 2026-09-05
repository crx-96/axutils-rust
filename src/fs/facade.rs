//! 本地文件系统操作的无状态静态入口。
//!
//! `FsUtils` 不保存句柄、根目录、权限上下文、缓存或全局状态。同步方法默认可用且会阻塞
//! 当前线程；带 `_async` 后缀的方法只在相应能力 feature 下提供，是在调用时复制路径/内容
//! 并返回 owned future 的工厂函数，要求调用方持有 Tokio runtime。该库不会创建 runtime、调用
//! `block_on` 或把路径检查当作沙箱/授权保证。
//!
//! # Examples
//!
//! ```
//! use axutils::utils::FsUtils;
//!
//! let _tool = FsUtils;
//! ```

mod asynchronous;
mod sync;
mod transfer;

/// 本地文件系统操作的无状态静态入口。
#[derive(Debug, Clone, Copy, Default)]
pub struct FsUtils;
