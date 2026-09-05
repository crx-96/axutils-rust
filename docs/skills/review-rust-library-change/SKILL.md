---
name: review-rust-library-change
description: 当任务需要设计、实现、审查或验收 axutils 的 Rust 源码、公共 API、feature/依赖、契约测试、用户文档或发布内容时使用；纯规则、工具、格式和措辞维护不适用。
---

# axutils Rust library 变更审查与验收

本 Skill 定义 `axutils` 库级变更的架构约束、工作顺序和“完成”标准。具体模块、feature、文档
映射以 [`docs/module-map.md`](../../module-map.md) 为准；分层验证命令以
[`docs/develop.md`](../../develop.md) 为准。

## 1. 触发范围与证据

在首次作出相关设计、实现、审查或验收判断前完整读取本 Skill，适用情形包括：

- 修改或判断 `src/**` 的实现、公共 API、错误、运行时行为或安全边界；
- 新增、删除、改名或移动公共模块、类型、方法、trait、枚举、常量、宏或导出；
- 修改 feature、依赖、MSRV、docs.rs 配置、发布白名单或其他 crate 元数据；
- 用测试、fixture、API doc、README 或 `docs/examples/**` 定义或验证上述契约；
- 做跨领域重构、发布前检查或 library 级回归审查。

纯翻译、格式调整、规则或 Skill 自身维护、CI/开发工具维护、Git 操作，以及不判断 library
契约的测试基础设施调整不自动触发本 Skill；任务中一旦需要作出上述库级判断，再完整读取。

本文用词：

- **必须**：未满足就不能声明完成；
- **应**：默认要求；例外必须记录理由、替代证据和剩余风险；
- **证据**：可复核的源码、diff、编译/测试结果、诊断、依赖树或发布清单。主观判断不是证据。

交付必须区分“成功”“预期失败”“未运行”“环境阻塞”，并说明适用范围，不能把局部检查
描述成全量验收。

## 2. 读取顺序与权威来源

1. 读取适用的 `AGENTS.md`；
2. 完整读取本 Skill；
3. 涉及模块归属、公共路径、feature 或跨领域调用时完整读取 `docs/module-map.md`；
4. 读取 `Cargo.toml` 的 package、features、dependencies、docs.rs 和 lint 配置，确认版本与 MSRV；
5. 读取目标源码、领域入口、调用方、相关测试/fixture、API doc、对应示例和当前 CHANGELOG；
6. 只有命令或开发工作流变化时才读取并更新 `docs/develop.md`。

当前行为发生冲突时，权威顺序是：用户明确要求与更具体规则；源码、manifest 和可复现结果；
本 Skill 的设计/验收标准；module map、API doc、示例、README、develop 和历史记录。旧文档不能
覆盖当前源码事实，发现漂移必须在同一任务中收口。

## 3. 当前架构基线

- 本项目是单一 Rust library crate `axutils`，入口为 `src/lib.rs`；不得为分域而拆 workspace。
- `default = []`；不依赖第三方 crate 的基础工具默认可用。
- crate 根只声明公开领域模块，不重导出具体类型、错误或 `*Utils`。
- 领域类型、配置、错误和自由函数的 canonical path 是 `axutils::<domain>::Item`。
- 所有 `*Utils` 和工具支持类型的 canonical path 是 `axutils::utils::Item`；`utils` 叶实现模块私有。
- 领域实现内部可以继续按 `client`、`config`、`codec`、`policy`、`sync`、`async`、`global`
  等职责拆分，但不得形成外部可达的实现路径。
- 私有观测适配层是 `telemetry`；不得命名成会与外部 `tracing` crate 淆义的公开模块。
- 依赖方向固定为：

  ```text
  utils façade -> domain public API -> domain internals -> third-party crates
                                  -> private telemetry
  ```

  领域模块不得反向依赖 `utils`。

- 普通生产源码应以单一职责组织，约 600 行触发拆分评估；超过 800 行必须记录不可再拆的理由。
  不按行数机械切割，也不创建没有独立职责的碎片。
- library 不注册或选择进程全局 allocator；最终 binary 自行声明唯一 `#[global_allocator]`。
- 当前版本、edition 和 MSRV 始终以 `Cargo.toml` 为准；任务期间不得自行提升版本。

## 4. 影响分析与实施顺序

变更至少归入以下一类，验证范围取并集：

| 类别 | 典型内容 | 必查项 |
| --- | --- | --- |
| L 局部实现 | 私有算法、行为不变拆分 | 相关单元/集成测试、错误与资源边界 |
| B 行为 | 返回值、错误、默认值、I/O、并发 | API doc、边界/回归测试、CHANGELOG |
| P 公共 API | 路径、签名、可见性、trait | 正负 fixture、doctest、文档、兼容性 |
| F feature/依赖 | manifest、cfg、runtime、TLS、MSRV | 单 feature、组合、负向诊断、依赖树 |
| S 安全/资源 | 密钥、网络、文件、全局状态、解析预算 | 脱敏、上限、失败语义、隔离 |
| R 发布 | package、版本、发布元数据 | package 清单、版本、CHANGELOG、授权 |

修改前必须明确目标、非目标、公共契约、受影响 feature/依赖、外部副作用、测试范围和可观察
验收条件。先用调用链、输入、状态、错误或基准定位问题，不用试错替代根因分析。

大型结构重构按以下顺序执行：

1. 记录基线和迁移表，冻结正式测试与用户文档；
2. 在行为不变前提下拆分源码、修正依赖方向；
3. 建立 canonical API；若任务要求兼容阶段，先以 shim 验证新旧路径；
4. 保持原测试不变完成源码验收；
5. 再迁移测试、fixture 和测试性能结构；
6. 只删除预先列明并已有负向契约的兼容路径；
7. 最后更新 API doc、用户文档、规则、Skill 和 CHANGELOG。

普通局部改动不必机械套用全部波次，但必须保持“实现契约先稳定，再让测试和文档反映契约”。
不得为让测试通过而删除或放宽既有安全、错误或负向契约。

## 5. 模块、公共 API 与代码风格

### 5.1 归属与可见性

- 新能力放入负责其完整生命周期和领域语义的模块；`utils` 只提供明确的便利入口。
- `src/lib.rs` 不新增具体项重导出，也不创建 `prelude`。
- 无状态 `ConfigUtils`、`FsUtils`、`ConvertUtils`、`FormatUtils`、`PathUtils` 等可以提供便利操作，
  但只能从 `axutils::utils` 导入。
- 状态型 façade 只保留初始化、初始化状态和实例访问器。业务方法在领域实例上调用。
- 状态型基线包括 Email/HTTP/Redis/SQLx/JWT/Scheduler/Axum/Logging/Crypto；Crypto 仍可保留
  Hex/Base64/MD5 等无状态便利方法，Logging 只负责 subscriber 初始化/状态。
- JWT 的实例入口是公开 `axutils::jwt::JwtCodec`；不得迫使调用方依赖全局状态。
- 不新增公开 `utils::*_utils`、`domain::client`、`domain::global` 等叶实现路径。

新增、删除、改名或改变职责时，同步更新领域入口、module map、fixture 和用户文档。

### 5.2 路径与命名

- 类型在无歧义时可直接 `use`；`execute`、`parse`、`record_client_init` 等通用函数不能裸导入。
- 跨模块函数通过有业务含义的模块限定符调用，例如 `sqlx_trace::record_client_init`、
  `transfer::copy_file_with`。
- 遇到 `sqlx`、`redis` 等名称冲突时使用 `sqlx_trace`、`redis_trace` 等明确别名。
- `use` 可以写完整来源；普通表达式和签名路径最多两个 segment。
- 负向编译 fixture 为验证旧路径或缺失能力而写出的目标表达式可以使用完整路径；该例外只覆盖
  被验证的契约，不扩展到 fixture 的其他代码、普通测试、源码或可执行文档示例。
- `clippy::absolute_paths` 必须为 deny；`clippy.toml` 的 segment 上限和标准库豁免不得被绕过。
- 错误用 `Result`/`Option` 显式传播。对不可信输入、配置、网络或文件不得使用未记录的 panic。
- `unsafe` 只允许最小范围使用，并紧邻说明安全不变量、平台条件和验证证据。

## 6. Feature 与依赖

公共 feature 以用户可获得的能力命名，不以内部 provider crate 命名。每个可独立选择的 feature
单独启用后都必须提供可使用的公共 API；具体清单和依赖映射以 manifest/module map 为准。

固定契约：

- `phone-validation` 包含正则基础和国际号码 provider。
- `template-strfmt`、`template-minijinja` 各自聚合所需 Serde 与模板后端。
- `fs-async`、`fs-temp`、`fs-temp-async` 分别控制异步 FS、同步临时资源和异步临时资源。
- `config` 提供 JSON/`.env` 基础；YAML/TOML/INI/async 由对应 `config-*` feature 增量开启。
- `email` 与 `email-async`、`http` 与 `http-async`/`http-json` 明确分层；同步 `http` 依赖树
  不得包含 `reqwest`。
- Redis 由 `redis`、`redis-cluster`、`redis-async`、`redis-cluster-async` 表达四层能力。
- `sqlx-postgres`、`sqlx-mysql`、`sqlx-sqlite` 支持单 driver；`sqlx` 是三 driver 聚合入口。
- `tokio` 只提供 Tokio 工具，不自动打开 FS、Config、Email、HTTP、Redis 或 SQLx 异步 API。
- `task-group` 增加任务组；`scheduler` 单 feature 即提供完整调度能力。
- `axum` 提供基础 server；Tower、Tower HTTP 和 Governor 扩展使用各自 `axum-*` 能力 feature。
- `chrono`、`time`、`jiff` 后端方法始终保留明确后缀；API 名称不随启用后端数量改变。
- 所有最终 feature 必须可以共同构建，`--all-features` 是成功契约。

设计和验收要求：

- 可选第三方依赖保持 `optional = true`，使用 `dep:name` 和必要的上游 feature 转发；
- 不公开 `serde`、`lettre`、`croner`、`tower-http`、`tempfile` 等 provider-only feature；
- 不在当前任务中顺带升级依赖、edition 或 MSRV；
- `cfg` 使用最窄的能力守卫；模块、类型、方法、测试、fixture 和文档必须一致；
- 同时验证 API “应存在”和“不应存在”两侧，并检查负向诊断中的目标符号；
- 验证无 feature 的正常依赖树为空，验证 runtime/TLS/provider 没有跨能力泄漏；
- feature 变化必须同步 docs.rs feature 清单、module map、相关文档、matrix 和 CHANGELOG 判断。

## 7. 性能、资源、安全与全局状态

结构复用优先使用普通泛型和共享纯逻辑；没有测量证据时不为消除少量重复引入 trait object、
`async_trait`、boxing 或新的缓存。性能结论必须记录同工具链、相同冷/热缓存条件和命令。

解析、网络、加密、缓存、锁、任务和全局状态变更必须评估：

- 时间/空间复杂度、临时分配和复制；
- 输入、文件、递归、模板展开、批量、缓存、重试、超时和并发上限；
- 锁竞争、跨 await 持有资源、取消、关闭和租约语义；
- 错误/日志是否泄露密钥、token、凭据、配置、Header、明文或原始响应。

`OnceLock` 等单例必须覆盖成功、重复初始化、并发竞争、失败不占位、不可替换、实例访问和关闭后
行为。普通异步 API 不隐式创建 runtime 或 `block_on`；runtime 由调用方提供。构造纯配置/客户端
默认不访问网络，任何例外必须在 API doc 和测试中明确。

领域安全底线：

- AES 密钥、明文、IV/nonce 不进入错误或日志；MD5 不用于密码/签名；CBC 不是认证加密。
- `RandomUtils` 不是密码学安全随机源；安全密钥和 nonce 使用操作系统随机源。
- JWT payload 不是加密内容；算法、key、时间和解析预算边界必须明确。
- 配置错误不回显原始配置；网络能力需审查代理、重定向、TLS、SSRF、重试与大小限制。
- 不在缺少证据时改变 Config/JWT 安全预检、HTTP 顺序/缓存、Redis 锁/事务、Scheduler 锁或
  Crypto 临时 buffer 等既有语义。

## 8. 测试与验证

测试覆盖以行为和契约为准：

- 单元测试覆盖正常、边界、错误、资源上限和平台分支；
- 集成测试只通过 canonical 公共路径调用，覆盖跨模块、错误传播和全局并发语义；
- 编译 fixture 验证成功路径、旧路径/旧 feature/缺能力 API 的稳定失败诊断；
- `cargo tree` 验证 optional 依赖、runtime、TLS 和 provider 边界；
- doctest 和 Markdown harness 覆盖公开示例；
- live 测试不属于默认验收。

Feature matrix 使用声明式 case、共享 runner、统一领域 fixture 和依赖树缓存。同一 feature 集的
`cargo tree` 只执行一次；正向场景尽量批量检查，只有需要独立诊断的负向场景单独运行。

Markdown harness 使用“文档默认 feature/直接依赖 + 邻接 fence override”描述契约。完全相同
feature 和直接依赖组合的正向代码块合并到同一 scratch crate；`compile_fail` 独立运行并核对
诊断。新增 fence 必须被双向枚举，未闭合 fence、未声明的活动 `cfg` 和敏感值必须失败。

最小充分验证按影响范围选择；跨模块、公共路径、feature/依赖或发布级变化执行完整非 live 门禁：

```powershell
cargo fmt --all -- --check
cargo check --no-default-features
cargo test --no-default-features
cargo check --all-features
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
cargo test --no-default-features --test docs_examples
cargo test --no-default-features --test docs_examples -- --ignored --test-threads=1
cargo package --list
git diff --check
```

再按具体领域执行最小 feature 组合和依赖树断言。MSRV 需在 Rust 1.95 工具链验证；当前工具链
不是 1.95 时必须如实报告，不能用更高版本结果冒充 MSRV 证据。

## 9. 文档、CHANGELOG 与发布包

- 公共项必须有中文 API doc，说明用途、参数、返回、错误、feature、限制和副作用。
- `docs/examples/<domain>.md` 只使用 canonical path 和语义 feature，覆盖领域模型、典型流程、
  错误/安全/runtime 边界；不公开私有实现路径。
- README 只保留定位、短示例、feature 概览和文档链接，不复制完整方法清单。
- module map 只维护模块、职责、canonical path、feature 和文档映射，不复制所有方法。
- 开发命令按快速、领域、完整、发布四级维护在 `docs/develop.md`。
- 外部 I/O 示例必须用保留域名/占位凭据，并标为 `no_run` 或只构造不产生副作用的对象。
- 当前版本的用户可见路径、feature、运行时、错误、安全与 allocator 移除写入 CHANGELOG；
  测试组织、性能 harness、规则和纯文档整理不写入。
- `package.include` 只包含库源码和用户文档；测试、fixture、规则、Skill、develop、module map、
  本地配置和凭据不得进入发布包。
- 未经用户明确授权不得改版本、发布、推送或连接真实外部服务。

## 10. Live、敏感信息与完成定义

`tests/email_live.rs`、Redis/Redis Cluster live 测试保持 ignored。只有用户明确授权、受控服务、
一次性 opt-in 开关和被忽略的本地配置同时满足时才能运行；缺配置不能伪装成成功。错误、命令、
日志、fixture 和文档只输出最少必要信息并使用占位符。

可以声明完成的条件：

1. 架构依赖方向、canonical path、薄 façade、语义 feature 和 allocator 边界均有源码证据；
2. 每项公共/feature 变化同时有正向和必要的负向契约；
3. 适用测试、Clippy、rustdoc、文档 harness、依赖树和发布清单通过；
4. API doc、README、领域文档、module map、develop、规则、Skill 和 CHANGELOG 已按职责同步；
5. 无未处理的行为回归、安全弱化、敏感信息、外部副作用或无关改动；
6. 最终报告列明实际命令、结果、未运行项和剩余风险，另一位维护者可据此复核。

维护本 Skill 时必须完整读取它，并检查 `AGENTS.md` 的触发入口、module map 与 develop 链接。
Skill 或规则自身的纯维护不改版本、不写 CHANGELOG；与源码公共契约一起收口时，CHANGELOG 只记录
用户可见变化。
