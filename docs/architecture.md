# axutils 架构基线

本文件规定本仓库后续开发使用的结构与边界。当前能力清单见 [module map](module-map.md)，
库级兼容性与安全判断见 [审查 Skill](skills/review-rust-library-change/SKILL.md)，
验证命令见 [develop](develop.md)。模块增加时更新能力清单；只有结构或职责约定改变时修改本文件。

## 固定边界

- 采用单一 library crate，入口为 `src/lib.rs`。新增能力优先放入对应领域；拆成 workspace 或独立
  crate 需要独立发布、依赖隔离、复用或构建收益等实际依据，并作为架构变更说明兼容性与迁移。
- 默认 feature 为空，默认生产依赖树不包含第三方 crate。可选能力通过语义 feature 显式启用，
  各 feature 应可组合，`--all-features` 共同构建是现有契约。
- Client、配置、错误、模型和领域自由函数的 canonical path 为 `axutils::<domain>::Item`。
  `*Utils` 的 canonical path 为 `axutils::utils::Type`；仅工具自身专用、没有独立领域归属的
  支持类型（如 `TemplateEngine`、`LetterCase`）跟随该入口。
- crate 根声明模块、feature gate 和 crate 文档，不平铺重导出类型，不设置 `prelude` 或兼容
  别名入口。领域内部实现和 `utils` 叶模块对下游保持私有。
- runtime、日志 subscriber、全局 allocator 和真实服务配置由调用方决定。库不在加载时初始化
  进程状态；普通异步 API 使用调用方 runtime。专用 runtime/日志工具只通过显式 API 启动，
  `#[global_allocator]` 由最终 binary 声明。

## 目录职责

以下是路径模式，按实际能力创建，不要求每个领域生成全套文件或空目录。

```text
src/
  lib.rs                      crate 文档、模块声明和 feature gate
  <domain>/
    mod.rs                    领域入口、模块声明、公开类型重导出
    client.rs / codec.rs      实例入口，按领域选择实际名称
    config.rs / error.rs      配置与错误；规模很小时可保留在入口
    global.rs / facade.rs     可选的全局入口或无状态便利操作
    <capability>.rs           具体职责；增长后配同名子目录
    <capability>/             按协议、算法、同步/异步等职责展开
  utils/
    mod.rs                    工具公共入口及 feature gate
    <domain>_utils.rs         已有领域工具的重导出叶
    <tool>_utils.rs            独立无状态工具及必要的私有子模块
  telemetry/                  私有、按领域划分的 tracing 事件适配
tests/
  <domain>.rs                 按领域组织的集成测试 target
  <domain>/                  该 target 专用的用例与辅助模块
  support/                   多个 target 共用的测试支撑
  fixtures/                  静态样本与独立下游编译 crate
docs/
  architecture.md             本基线
  module-map.md              当前能力映射
  develop.md                 开发与验证命令
  rules/personal.md          用户确认的项目个人偏好（AGENTS 登记必读）
  examples/                  下游用户示例与说明
  skills/                    本仓库设计与审查 Skill
config/                      被忽略的本地测试配置
```

生产源码留在 `src/`，测试支撑不得被生产模块反向导入。构建产物、索引和临时工作记录不充当源码
或规范来源；Git 忽略与发布范围分别由 `.gitignore` 和 manifest 的 `package.include` 决定。

## 入口与依赖方向

公共使用关系如下，调用方可以直接使用领域实例：

```text
调用方 ──→ axutils::<domain>::Item ──→ 领域私有实现 ──→ 第三方 crate
   └────→ axutils::utils::XxxUtils ──→ 对应领域实例或无状态操作
领域实现 ──→ telemetry::<domain> ──→ tracing 事件
```

- `utils/mod.rs` 统一重导出。已有领域的 `*_utils.rs` 叶只聚合入口，实现留在领域内部；
  `FormatUtils`、`PathUtils`、`RandomUtils`、`RegUtils` 等独立无状态工具可在 `utils` 内实现。
  新工具若引入 client、领域模型、配置生命周期或多后端 I/O，应建立领域模块，避免把业务堆入工具叶。
- `global.rs` 管理全局实例，`facade.rs` 提供无状态便利操作；两者按实际职责择需使用。
  现有 Crypto/Logging 的 `facade.rs` 同时承担其专属初始化入口，可沿用；增加独立职责时再拆分。
  工具类型可以定义在领域私有模块中，但只通过 `utils` 暴露；领域业务实现不反向调用 `utils`。
- 领域间复用应有明确语义，通过领域入口或范围受限的内部接口完成，不经全局 façade 获取依赖，
  不导入其他领域的 transport/backend 私有细节。没有实际共同语义时保留局部实现，避免新增
  含混的 `common`、`helpers` 或 `manager` 集合；共同抽象在拥有其语义的模块维护。
- `telemetry` 仅在 `tracing` 下编译。它可以读取领域结果、错误和统计 getter 来生成脱敏事件；
  这是类型引用，不代表允许反向触发领域 I/O、初始化、重试或改变业务结果。领域无需启用
  `logging` 即可发出事件，事件适配不安装 subscriber、不获取全局 client。
- 默认用私有模块和最小可见性；父级/兄弟协作优先 `pub(super)` 或受限路径，确实跨领域使用时
  才使用 `pub(crate)`。不为通过拆分后的编译而直接改成 `pub`。

## 状态与能力隔离

状态型 façade 只保留初始化、初始化状态和实例访问，业务操作在返回的实例上完成。成功初始化
后不能 reset/replace，初始化失败不占位；关闭后行为由实例契约决定。多配置、隔离测试或可控
销毁场景使用实例。Logging 只负责 subscriber 初始化/状态，Crypto 的无状态编码操作继续保留；
各入口的实际方法见 module map 的状态型 façade 表。

同步和异步 API 按领域能力 gate；单独启用 `tokio` 不开放其他领域的异步接口。多个后端在同一
领域内适配，公共模型、校验与策略尽量共享，同步/异步 transport 分开执行 I/O。新增抽象应能
减少真实重复或隔离变化，不为使所有领域外形一致而引入 trait、boxing 或统一全局容器。

## 文件拆分约定

- 顶层领域沿用 `<domain>/mod.rs`；其内部沿用相邻代码的 `name.rs + name/` 或 `name/mod.rs`
  形式，同一模块不同时创建两种入口。新增目录体现模块所有权，通常通过正常 `mod` 解析。
- `mod.rs` 以领域说明、声明和重导出为主；紧凑且内聚的类型可就地定义。client/codec 文件维护
  实例和操作入口，协议执行、格式解析、校验、重试或独立生命周期增长后放入各自子模块。
- 依据独立修改原因、依赖差异、测试边界和阅读成本拆分。配置声明、错误枚举、API doc 占比高
  的长文件可以保留；同步/异步实现或多阶段流程已经影响理解时，按职责拆分并共享纯校验逻辑。
  行数和就近归属的个人风格见 [personal](rules/personal.md#路径与目录风格)。
- 拆分时维护公共路径、feature gate、错误与资源行为，缩小内部可见性；原有测试断言和文档
  示例随之核对。模块或方法位置改变，不应顺带改变对外 API。

## 测试组织

- 私有行为使用就近单元测试，规模增长后放在模块下的 `tests.rs`；公共行为通过 `tests/` 的
  canonical path 验证。测试路径风格见 Skill 的 API 主题。
- 一个领域集成 target 较大时，保留 `tests/<domain>.rs` 入口，把用例放入 `tests/<domain>/`
  的职责子模块（如同步、异步、传输、临时资源）。不要只为缩短文件就增加顶层 test binary。
  当前 `#[path]` 测试入口可沿用，移动时保留模块解析和 feature gate。
- 测试进程隔离是独立拆 target 的依据。全局单例、subscriber 冲突、allocator 测量及 live 场景
  等需要隔离的现有 target 保持独立，不能为了统一目录合并后改变初始化顺序或并发条件。
- 仅一个 target 使用的 helper 放在其私有子模块；多个 target 复用时才放入 `tests/support/`。
  fixture crate 用于检验下游路径、feature 合并和依赖边界，不改造为生产 workspace 成员。

## 演进与验收

当前职责拆分体现三类稳定边界：公开模型与内部解析器、共享校验与不同执行后端、状态登记与实际
任务执行。具体文件位置由 module map 维护。拆分后先检查是否减少了状态协调及修改牵连，再检查
文件是否便于阅读；公共类型不因私有模块迁移而改变导入路径。

新增能力时先确定领域归属、公共入口、状态所有者和最小 feature，再按需创建文件。改变模块映射
时更新 module map；改变公共行为时按 Skill 同步 API doc、示例、CHANGELOG 和相关正负契约。

本基线约束后续开发和本次确需改动的区域，不把既有命名、文件长度或历史布局自动判为缺陷。
局部、行为不变的职责拆分可随相关需求完成。改变公共路径、进程状态、crate 边界或 feature 契约
时，说明触发原因、收益、兼容性和验证依据；已有任务授权覆盖时继续实施，有实质选择缺口时再询问。

规则审查检查链接、归属和与源码的对应关系；实际重构运行受影响测试，公共路径或 feature 变化
补相应 fixture。检查命令由 develop 维护，不能用文档整理完成代替源码重构或全库合规结论。

语言与构建依据：[Rust 模块文件规则](https://doc.rust-lang.org/book/ch07-05-separating-modules-into-different-files.html)、
[Cargo 测试 target](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#integration-tests)、
[Cargo feature 合并](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification)。
本项目的入口和命名约定是在这些机制之上的本地选择。
