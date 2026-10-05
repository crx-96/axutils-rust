//! 森林关系校验、确定性排序及显式栈后序组装。

use std::collections::{btree_map::Entry, BTreeMap};

use super::{TreeBuildError, TreeNode};

/// 校验阶段缓存的记录元数据，避免重复调用关系或排序提取函数。
struct IndexedNode<T, Id, Sort> {
    /// 最终移入输出树节点的原始记录。
    item: T,
    /// 原始父 ID；`None` 表示根节点，只在建立邻接表时使用。
    parent_id: Option<Id>,
    /// 调用方提供的同级排序值，使用其 `Ord` 升序关系。
    sort_order: Sort,
    /// 节点 ID 在有序索引中的名次，排序值相等时用于比较 ID 而无需克隆 ID。
    id_rank: usize,
}

/// 将拥有型平铺记录构建为完整森林，并按排序值、ID 升序排列每组同级节点。
///
/// `id`、`parent_id` 和 `sort_order` 从记录提取节点标识、可选父标识和排序值；成功时每个
/// 提取函数对每条记录只调用一次。ID 和排序值仅需实现 `Ord`，无需 `Copy` 或 `Clone`。
/// 原始记录移入输出节点，`None` 父 ID 表示根节点，根深度为 1。
///
/// 空输入总是返回空森林，包括 `max_depth == 0`；非空输入且上限为零时立即返回
/// [`TreeBuildError::DepthExceeded`]，不调用提取函数。其他输入拒绝重复 ID、缺失父节点、
/// 自环、独立环和超深节点，绝不静默丢弃不可达记录；同时存在多类错误时不承诺报告顺序。
///
/// 索引和排序使用 `O(n log n)` 次量级的比较及 `O(n)` 辅助空间；构建过程采用显式栈，
/// 在通过结构和深度校验后才组装递归输出。该保证不延伸到记录或输出的析构、序列化、
/// 派生 trait 等操作；调用方应按后续用途选择合理的深度上限。
///
/// # 示例
///
/// ```rust
/// use axutils::tree::{self, TreeBuildError};
///
/// let records = vec![(3, Some(1), 0), (2, Some(1), 0), (1, None, 0)];
/// let forest = tree::build_forest(records, 2, |row| row.0, |row| row.1, |row| row.2)?;
/// assert_eq!(forest[0].item.0, 1);
/// assert_eq!(forest[0].children[0].item.0, 2);
/// assert_eq!(forest[0].children[1].item.0, 3);
/// # Ok::<(), TreeBuildError>(())
/// ```
pub fn build_forest<T, Id: Ord, Sort: Ord>(
    items: Vec<T>,
    max_depth: usize,
    mut id: impl FnMut(&T) -> Id,
    mut parent_id: impl FnMut(&T) -> Option<Id>,
    mut sort_order: impl FnMut(&T) -> Sort,
) -> Result<Vec<TreeNode<T>>, TreeBuildError> {
    // 空集合没有节点深度；非空零上限在提取元数据前失败，定义统一的零值行为。
    if items.is_empty() {
        return Ok(Vec::new());
    }
    if max_depth == 0 {
        return Err(TreeBuildError::DepthExceeded);
    }

    // 唯一 ID 索引指向连续的内部位置，后续遍历只保存 usize，不要求克隆业务 ID。
    let mut ids = BTreeMap::new();
    let mut nodes = Vec::with_capacity(items.len());
    for item in items {
        match ids.entry(id(&item)) {
            Entry::Occupied(_) => return Err(TreeBuildError::DuplicateId),
            Entry::Vacant(entry) => {
                entry.insert(nodes.len());
            }
        }
        nodes.push(IndexedNode {
            parent_id: parent_id(&item),
            sort_order: sort_order(&item),
            item,
            id_rank: 0,
        });
    }

    // 有序映射的遍历名次等价于 ID 顺序，作为同级排序的确定性次关键字。
    for (rank, &index) in ids.values().enumerate() {
        nodes[index].id_rank = rank;
    }

    // 校验每条父关系后建立邻接表；每个非根节点只有一个父节点。
    let mut children = vec![Vec::new(); nodes.len()];
    let mut roots = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        match node.parent_id.as_ref() {
            Some(parent) => {
                let parent_index = ids.get(parent).ok_or(TreeBuildError::MissingParent)?;
                children[*parent_index].push(index);
            }
            None => roots.push(index),
        }
    }
    drop(ids);

    // 根节点与所有孩子集合共用同一排序规则，输出顺序不依赖输入记录的排列。
    for siblings in children.iter_mut().chain([&mut roots]) {
        siblings.sort_unstable_by(|&left, &right| {
            nodes[left]
                .sort_order
                .cmp(&nodes[right].sort_order)
                .then_with(|| nodes[left].id_rank.cmp(&nodes[right].id_rank))
        });
    }

    // 单父节点图中，环所在部分无法从根到达；完整后序计数能够发现所有独立环和自环。
    let mut stack: Vec<_> = roots.iter().rev().map(|&index| (index, false, 1)).collect();
    let mut postorder = Vec::with_capacity(nodes.len());
    while let Some((index, expanded, depth)) = stack.pop() {
        if expanded {
            postorder.push(index);
            continue;
        }
        stack.push((index, true, depth));
        if !children[index].is_empty() {
            // 只有仍有下一层时检查上限；先比较再加一，同时避免 usize 深度运算溢出。
            if depth == max_depth {
                return Err(TreeBuildError::DepthExceeded);
            }
            stack.extend(
                children[index]
                    .iter()
                    .rev()
                    .map(|&child| (child, false, depth + 1)),
            );
        }
    }
    if postorder.len() != nodes.len() {
        return Err(TreeBuildError::Cycle);
    }

    // 校验完成后才转移记录所有权；Option 槽位使后序能够取走记录与已完成子树。
    let mut records: Vec<_> = nodes.into_iter().map(|node| Some(node.item)).collect();
    let mut built: Vec<Option<TreeNode<T>>> = (0..records.len()).map(|_| None).collect();
    for index in postorder {
        let descendants = children[index]
            .iter()
            .map(|&child| {
                built[child]
                    .take()
                    .expect("后序组装时子树已完成且仅取走一次")
            })
            .collect();
        built[index] = Some(TreeNode {
            item: records[index]
                .take()
                .expect("后序序列中每条记录恰好出现一次"),
            children: descendants,
        });
    }

    // 根节点不会被其他节点取走；按已排序的根序列移动所有完整树。
    Ok(roots
        .into_iter()
        .map(|index| built[index].take().expect("每个根节点恰好保留一棵完整树"))
        .collect())
}
