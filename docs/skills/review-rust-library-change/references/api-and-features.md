# API、feature 与演进

领域归属、canonical path 和 façade 职责见 [架构基线](../../../architecture.md)。
本主题用于公共 API、可见性、错误类型、命名、迁移及 feature/依赖变更。

## 实施与迁移

局部变更围绕受影响契约实施。结构重构可按依赖和风险分阶段，下列做法用于保留可复核的迁移
过程，可以合并或调整先后，不构成额外审批：

- 用现有实现、测试和用户文档建立基线；跨路径迁移较多时记录对应关系，保留原断言语义。
- 将行为不变的职责拆分、依赖方向修正与行为/API 迁移区分，便于发现回归和判断差异来源。
- 建立 canonical API，优先运行原回归；已授权的 API 迁移使旧调用无法编译时，可同步迁移调用
  并用对应关系证明原行为仍被覆盖。测试、fixture、性能 harness 和文档随稳定的契约同步。
- 兼容别名和 shim 用于用户已要求的兼容目标，不因重构顺带引入。清理兼容路径时核对授权的
  迁移范围，并保留对应的负向契约。
- API doc、用户文档和 CHANGELOG 反映最终行为；规则或 Skill 的维护按用户请求范围处理。

测试失败用于检验实现和假设；保留既有负向、边界、错误与安全契约，避免通过删除断言、放宽
预期或额外启用 feature 掩盖问题。用户已授权改变契约时，说明前后行为并验证新的适用边界。

## 路径与命名

- 类型无歧义时可直接 `use`；`execute`、`parse`、`record_client_init` 等通用函数通过有业务含义
  的模块限定符调用，例如 `sqlx_trace::record_client_init`、`transfer::copy_file_with`。
- `sqlx`、`redis` 等名称冲突可用 `sqlx_trace`、`redis_trace` 等明确别名区分。
- `use` 写明来源，普通表达式与签名使用简短路径。当前 `clippy::absolute_paths = deny`，
  segment 上限和标准库豁免以 `clippy.toml` 为准；适配代码时保留这项 lint 的检查作用。
- 负向编译 fixture 可在验证旧路径或缺失能力的目标表达式中使用完整路径；其余 fixture 代码、
  普通测试、源码和可执行文档示例沿用正常路径风格。
- 错误通过 `Result`/`Option` 显式传播；不可信输入、配置、网络或文件失败按 API 契约返回错误，
  panic 语义在存在的接口中说明原因与触发条件。
- 使用 `unsafe` 时收敛作用范围，并就近说明安全不变量、平台条件和验证依据。

## 公共类型与兼容性

- API 的借用、所有权转移和共享方式对应真实生命周期；只读输入优先借用，消费或保留输入、转移
  资源及跨任务持有时按需要接收拥有型值。`Clone` 对 client、guard 和 handle 的共享、复制或释放
  语义应明确。
- 公共错误保留可供调用方分支处理的分类，沿用各领域的类型化错误；第三方错误的 `Display`、
  `Debug` 和 `source()` 同样属于脱敏边界。允许忽略清理错误的路径说明可观察结果与恢复方式。
- 公共兼容性核对覆盖路径和签名之外的 enum 穷尽匹配、公开字段、trait bounds、`Send`/`Sync`、
  默认值、错误分类及 feature 可用性。只有需要封装不变量或预留扩展时才选用私有字段、builder
  或 `#[non_exhaustive]`，不为统一风格改写现有类型。

## Feature 与依赖

公共 feature 按用户可获得的能力命名，每个独立可选 feature 对应可用公共 API。能力与 provider
映射集中在 [module map](../../../module-map.md) 的能力 feature 表，判断时结合 manifest 核对相关行及依赖方向。

能力分层、后端聚合与时间方法的稳定后缀沿用该映射；`--all-features` 共同构建成功是兼容性契约。

设计与验证关注：

- 可选第三方依赖使用 `optional = true`、`dep:name` 和相关上游 feature 转发。
- `serde`、`lettre`、`croner`、`tower-http`、`tempfile` 等内部 provider 通过能力 feature 聚合，
  避免作为缺少独立用户能力的公开开关。
- 依赖、edition 和 MSRV 的升级按当前任务范围处理；普通实现修改沿用现有版本策略。
- `cfg` 对应实际能力，模块、类型、方法、测试、fixture 和文档保持一致。
- 按受影响能力检验 API 存在与缺失两侧，负向用例核对目标符号的诊断。
- 核对默认正常依赖为空，以及 runtime、TLS、provider 在相关 feature 间的隔离。
- Cargo feature 会在下游依赖图中合并；关闭代理、重定向、解压、隐式重试等行为承诺，应由实例
  配置保障，不能仅依赖本 crate 的 `default-features = false`。涉及该边界时，用下游 fixture
  额外启用相关上游 feature，核对行为仍与本库文档一致。
- feature 变化时核对 docs.rs 清单、module map、相关文档和 matrix，并判断 CHANGELOG 的同步范围。
