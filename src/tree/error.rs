//! 不携带输入记录或节点 ID 的树结构错误。

use std::{error::Error, fmt};

/// 平铺记录无法形成完整森林时的结构错误；不包含记录内容或业务标识。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TreeBuildError {
    /// 两条或更多记录具有相同的节点 ID。
    DuplicateId,
    /// 记录引用的父节点 ID 不存在于本次输入中。
    MissingParent,
    /// 存在自环或独立环，导致部分节点无法从任何根节点到达。
    Cycle,
    /// 非空输入的允许深度为零，或某个节点的深度超过上限；根深度为 1。
    DepthExceeded,
}

impl fmt::Display for TreeBuildError {
    /// 输出结构类别说明，不回显输入记录、节点 ID 或其他业务数据。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DuplicateId => "存在重复的树节点 ID",
            Self::MissingParent => "树节点引用了不存在的父节点",
            Self::Cycle => "树节点关系中存在环",
            Self::DepthExceeded => "树节点深度超过允许上限",
        };
        formatter.write_str(message)
    }
}

impl Error for TreeBuildError {}
