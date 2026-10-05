//! 树节点的拥有型模型与非递归后序转换。

/// 原始记录及其有序直接子节点。
///
/// [`super::build_forest`] 生成的节点按排序值、ID 升序排列；公开字段允许调用方自行构造或修改。
/// 对象本身采用递归表示，普通析构、派生 trait 或外部序列化仍可能递归遍历。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeNode<T> {
    /// 当前节点拥有的原始记录，不添加业务字段或改变记录内容。
    pub item: T,
    /// 按调用方所需顺序保存的直接子节点；空集合表示叶节点。
    pub children: Vec<TreeNode<T>>,
}

impl<T> TreeNode<T> {
    /// 消费整棵树，按原有子节点顺序进行可失败的后序转换。
    ///
    /// 每个子节点先转换为 `U`，随后将当前记录及有序的直接子节点结果交给 `map`。
    /// 首次转换失败时原样返回该错误，不再调用 `map`；先前回调的外部副作用不会回滚。
    ///
    /// 遍历使用显式栈，但失败清理时剩余节点或已生成 `U` 的析构、以及调用方构造的递归
    /// 输出仍可能使用调用栈。此方法不承诺任意深度下的析构或序列化安全。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use axutils::tree::TreeNode;
    ///
    /// let root = TreeNode {
    ///     item: 2,
    ///     children: vec![TreeNode { item: 3, children: vec![] }],
    /// };
    /// let sum = root.try_map(|value, children: Vec<i32>| {
    ///     Ok::<_, &'static str>(value + children.into_iter().sum::<i32>())
    /// });
    /// assert_eq!(sum, Ok(5));
    /// ```
    pub fn try_map<U, E>(self, mut map: impl FnMut(T, Vec<U>) -> Result<U, E>) -> Result<U, E> {
        /// 显式保存节点展开与回调阶段，代替递归函数调用。
        enum Frame<T> {
            /// 尚未展开的节点，拥有原始记录及未转换的子树。
            Visit(TreeNode<T>),
            /// 子节点遍历完成后调用当前记录的转换函数。
            Build {
                /// 当前节点拥有的原始记录。
                item: T,
                /// 当前节点的直接子节点数量，用于从结果栈末尾提取有序结果。
                children: usize,
            },
        }

        // 帧栈先压入父节点的组装动作，再反向压入子节点，保持从左到右的后序访问。
        let mut stack = vec![Frame::Visit(self)];
        let mut built = Vec::new();
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Visit(TreeNode { item, children }) => {
                    stack.push(Frame::Build {
                        item,
                        children: children.len(),
                    });
                    stack.extend(children.into_iter().rev().map(Frame::Visit));
                }
                Frame::Build { item, children } => {
                    // 每棵已转换子树恰好留下一个结果；失败立即传播且不再执行后续回调。
                    let children = built.split_off(built.len() - children);
                    built.push(map(item, children)?);
                }
            }
        }

        // 输入恰好是一棵树，成功处理全部帧后必定只剩根节点的转换结果。
        Ok(built.pop().expect("单棵树的后序转换必定产生一个根结果"))
    }
}
