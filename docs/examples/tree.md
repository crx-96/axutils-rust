# 树与森林

`axutils::tree` 默认可用，不需要启用 feature 或引入第三方依赖。`build_forest` 消费平铺记录，
由调用方提取节点 ID、可选父 ID 和排序值；ID 和排序值只需实现 `Ord`，不要求 `Copy` 或 `Clone`。
`None` 父 ID 表示根节点，每组同级节点（包括根节点）按排序值、ID 升序排列。

```rust
use axutils::tree::{self, TreeBuildError};

// 记录格式由调用方决定；此处依次为 ID、父 ID、排序值和显示名称。
let rows = vec![
    (3_u32, Some(1), 0_i16, "乙"),
    (1, None, 0, "根"),
    (2, Some(1), 0, "甲"),
];
let mut forest = tree::build_forest(rows, 2, |row| row.0, |row| row.1, |row| row.2)?;
assert_eq!(forest[0].children[0].item.3, "甲");
assert_eq!(forest[0].children[1].item.3, "乙");

// 后序转换保留子节点顺序，先得到子结果，再处理当前节点。
let root = forest.pop().unwrap();
let text = root.try_map(|row, children: Vec<String>| {
    Ok::<_, TreeBuildError>(format!("{}[{}]", row.3, children.join(",")))
})?;
assert_eq!(text, "根[甲[],乙[]]");
# Ok::<(), TreeBuildError>(())
```

构建拒绝 `DuplicateId`、`MissingParent`、`Cycle` 和 `DepthExceeded`，错误中不保存节点 ID 或
业务记录。自环和与合法根树并列的独立环都返回错误，不会只返回可达节点而静默丢弃其余记录。
当输入同时违反多类规则时，不依赖错误的报告顺序。

根深度为 1，上限包含该层。空输入在任意上限下返回空森林；非空输入在上限为 0 时立即返回
`DepthExceeded`，不会调用元数据提取函数。调用方根据后续使用方式决定合理深度与输入数量限制。

```rust
use axutils::tree::{self, TreeBuildError};

let empty = tree::build_forest(Vec::<u32>::new(), 0, |id| *id, |_| None, |_| 0);
assert!(empty.unwrap().is_empty());
let rejected = tree::build_forest(vec![1_u32], 0, |id| *id, |_| None, |_| 0);
assert_eq!(rejected.unwrap_err(), TreeBuildError::DepthExceeded);
```

`TreeNode::try_map` 消费整棵树，按从左到右的后序调用转换函数，原样传播第一次错误并停止后续
转换；已发生的回调副作用不会回滚。节点公开的 `item` 与 `children` 字段也支持自行构造和修改。

构建和转换本身使用显式栈，构建只在结构与深度校验通过后组装递归输出。然而 `TreeNode` 以及
调用方生成的递归输出的序列化、派生 trait、普通析构，和转换失败时剩余对象的清理仍可能递归。
**显式栈不意味着返回对象自动支持任意深度。** 不应仅因构建成功就把极深对象交给任意序列化器。
