---
name: review-rust-library-change
description: 按 axutils 的影响分析、公共 API/feature/依赖边界、文档测试同步、安全审查和证据化标准，设计、实现、审查或验收源码、模块、公共 API、测试、文档及发布元数据变更。
---

# axutils Rust library 变更审查与验收

本 Skill 集中定义 `axutils` 的设计、实现、审查和交付验收标准，即“什么才算完成”；项目规则
声明强制入口，并保留仓库边界、授权及其他更具体约束，避免重复维护验收细则。工具类定位、
公共导出和能力边界以
[`docs/module-map.md`](../../module-map.md) 为准；[`docs/develop.md`](../../develop.md) 的读取条件见第 2 节。

## 1. 适用范围、术语和证据要求

设计、实现、审查或验收以下任一内容时，适用本标准：

- `src/` 中的实现、公共 API、错误语义、运行时行为或安全边界；
- 新增、删除、重命名或调整工具类、领域模块、公开导出、类型、方法、trait、枚举、常量、
  类型别名、静态项或宏；
- `Cargo.toml` 的 feature、依赖、依赖 feature、MSRV、发布白名单或 crate 元数据；
- 集成测试、feature/API 编译 fixture、依赖边界断言、公开 API 文档、README、`docs/examples/`
  或发布文件，以及相关回归排查或发布前检查。

纯翻译、简单措辞调整或与 Rust library 无关的文件操作不自动触发本 Skill；涉及本项目规则、
标准、模块定位或验收时仍必须触发并完整读取。

用词定义：

- **必须**：没有满足时不能宣称通过验收；
- **应**：默认要求，确有理由不能满足时必须记录原因、替代证据和剩余风险；
- **可以**：不构成验收门槛，但不得违反更高优先级的项目规则或用户要求；
- **证据**：可复核的源码、差异、测试输出、fixture 编译结果、文档编译结果、依赖树或
  发布包清单；“看起来正确”或“未发现问题”不算证据。

验收结论必须分别报告：

- 已通过的门槛及对应证据；
- 没有运行的检查及具体原因；
- 失败、预期失败和环境阻塞，不能把三者混写为成功；
- 尚未解决的问题、影响范围和后续风险。

## 2. 读取顺序和权威来源

开始实现或审查前，按顺序建立上下文：

1. 读取适用的 `AGENTS.md` 或更具体的目录规则；
2. 完整读取本 Skill；
3. 涉及具体工具类、领域模块、跨模块 API 或新增方法时，完整读取
   [`docs/module-map.md`](../../module-map.md)；
4. 读取 `Cargo.toml` 的 `[package]`、`[features]`、`[dependencies]` 和相关
   `dev-dependencies`，尤其确认当前 `version` 与 `rust-version`；
5. 读取目标源码、crate 根导出、调用方、已有测试、fixture、对应 `docs/examples/`、README
   相关段落和当前 `CHANGELOG.md` 条目；
6. 只有新增、删除、修改开发/发布命令，或需同步这些命令给开发人员时，才读取并更新
   [`docs/develop.md`](../../develop.md) 对应章节；普通实现、测试、审查和验收不读取该文件。

当不同文件对“当前行为”的描述不一致时，按以下顺序处理：

1. 用户明确要求和更具体的项目规则；
2. 当前源码、`Cargo.toml` 和可复现的测试/编译结果；
3. 本标准的流程和验收要求；
4. `docs/module-map.md`、API doc、`docs/examples/`、README、`docs/develop.md` 和 CHANGELOG 中的
   已记录说明。

发现文档与源码不一致时，不能用旧文档当作实现依据；应先确定当前行为，再在同一变更中
修正文档或补充明确的兼容性说明。

## 3. 当前库基线

本节仅概括当前仓库；版本、feature、依赖和公开导出始终以对应文件为准，不能以本节替代清单。

| 项目 | 当前基线与权威文件 |
| --- | --- |
| crate 类型 | Rust library crate，包名和 crate 名为 `axutils`，入口为 `src/lib.rs` |
| Rust 版本 | edition、`rust-version` 和 resolver 以 `Cargo.toml` 为准；当前 MSRV 为 `1.95` |
| 默认能力 | `default = []`；不依赖第三方 crate 的能力默认可用 |
| feature 组织 | 第三方能力通过显式 feature 提供；需要多个能力的 API 使用精确的组合 feature 守卫 |
| 公共导出 | crate 根、领域模块和 `utils` 兼容重导出由 `src/lib.rs` 与 `docs/module-map.md` 共同维护 |
| 工具类 | `*Utils` 入口位于 `src/utils/`；多文件实现位于同名前缀的 `src/<prefix>/` |
| 使用文档 | 每个公共能力单元对应 `docs/examples/<前缀>.md`，映射表位于 `docs/module-map.md` |
| 测试 | `src/` 单元测试、`tests/` 集成测试和 `tests/fixtures/` 编译 fixture 共同构成回归面；删除边界见第 4.3 节 |
| 慢速契约 | `tests/feature_matrix.rs` 的 ignored 测试覆盖 API、feature 和依赖边界；执行范围见第 8.3 节 |
| 真实外部服务 | `tests/email_live.rs`、`tests/redis_live.rs` 等默认 ignored；运行条件见第 9 节 |
| 发布白名单 | `Cargo.toml` 的 `package.include` 当前包含 `Cargo.toml`、README、CHANGELOG、LICENSE、`src/**` 和 `docs/examples/**` |
| docs.rs 文档 | `[package.metadata.docs.rs].features` 使用非 allocator 综合成功组合；排除规则见第 5.3 节 |
| 开发者文件 | 规则文件、`docs/develop.md`、`docs/module-map.md`、`docs/skills/**`、测试和本地配置不属于发布运行时内容 |
| 依赖锁定 | library crate 不把根目录 `Cargo.lock` 作为依赖版本策略提交；依赖下限由 manifest、MSRV 和无锁解析验证 |

以下固定组合语义必须持续保持：

- `regex` 提供 `RegUtils`，国际手机号校验还需要独立的 `libphonenumber`；
- 模板能力需要 `serde` 与 `strfmt` 或 `minijinja` 的显式组合；
- 日期后端 `chrono`、`time`、`jiff` 相互独立，同时启用多个后端时使用带后缀 API；
- 调度器 API 严格要求 `chrono + chrono_tz + tokio + croner`；16 种组合中只有完整组合导出
  `scheduler` 模块、领域类型和 `SchedulerUtils`。`croner` 是直接依赖的同名 provider feature，
  单独启用只会带入其内部 `chrono` 默认 feature（含 `clock`）、`derive_builder` 和 `strum`，不等于
  启用本 crate 的 `chrono` feature，也不得导出半套调度 API；
- 配置异步入口需要 `serde + tokio`；邮件异步入口需要 `lettre + tokio`；HTTP、Redis 的异步
  入口也分别受 `tokio` 组合守卫；
- `aes + base64` 才提供 AES 的 Base64 便捷入口；
- `mimalloc` 与 `rpmalloc` 是互斥的进程级全局分配器 feature，双 feature 是预期编译失败，
  不能把 `--all-features` 当作无条件成功的验收命令；
- `EmailUtils`、`HttpUtils`、`RedisUtils`、`JwtUtils` 和 `CryptoUtils` 的部分入口具有
  `OnceLock` 或进程级单例语义，必须把初始化、不可替换性、生命周期和并发边界写进文档与测试。

## 4. 变更影响分析和设计门槛

### 4.1 先分类，再决定验证范围

一次变更可以同时命中多个类别，验证范围取所有类别的并集：

| 类别 | 典型内容 | 至少需要关注 |
| --- | --- | --- |
| L：局部实现 | 私有函数、局部算法、无公共行为变化的重构 | 直接单元测试、格式、错误和边界 |
| B：行为变化 | 返回值、错误、默认值、限制、I/O 或并发语义变化 | API doc、对应文档、正常/边界/回归测试、CHANGELOG 判断 |
| P：公共 API | 导出、签名、可见性、trait、枚举、常量、类型别名、静态项、宏 | 完整导出路径、feature fixture、doctest、使用文档、兼容性审查 |
| F：feature/依赖 | `Cargo.toml`、`cfg`、可选依赖、依赖 feature、MSRV | 正负编译矩阵、依赖树、默认能力隔离、文档和 CHANGELOG 判断 |
| D：文档 | API doc、README、`docs/examples/`、module map | 本次变更涉及的全部 Rust 代码块、路径和 feature 说明 |
| S：安全/资源 | 密钥、网络、配置、文件、全局状态、重试、解析深度、分配器、unsafe | 敏感信息审查、边界/拒绝服务测试、外部副作用隔离和专门验证 |
| R：发布 | package 白名单、版本、CHANGELOG、publish dry-run | `cargo package --list`、发布清单、版本一致性和外部操作授权 |

### 4.2 修改前的最小结论

在写代码前，至少明确并记录：

- 目标行为、非目标范围和可观察的验收条件；
- 目标模块的职责边界、已有相似 API、公共导出和调用链；
- 受影响的 feature、依赖、异步 runtime、全局状态、文件/网络/进程副作用；
- 要修改或新增的测试、fixture、API doc、`docs/examples/`、module map、README 和 CHANGELOG；
- 最小验证命令、预期成功/预期失败结果，以及无法执行时的替代证据。

问题定位必须先收集调用链、错误日志、输入/状态和实际路径，确认根因及影响范围后再修改；
不得以试错、顺手重构或扩大错误处理范围代替定位。

### 4.3 最小变更和范围控制

- 只修改实现目标和验收所需的文件；
- 不因为“以后可能需要”引入抽象、依赖、公共导出、错误分支或配置；
- 保留现有兼容路径，除非用户明确要求删除且完成 Breaking 评估；
- 与目标无关的格式化、命名重构和依赖升级应拆分，不混入当前变更；
- 测试、fixture 和文档是实现的一部分，不能为了让测试通过而删除或放宽契约。

## 5. 模块、公共 API、feature 和依赖设计标准

### 5.1 模块归属和职责

- 新增或调整工具类、领域模块、公共方法前必须先读 `docs/module-map.md`；
- 每个能力单元必须有单一、清晰的职责边界，并写明“负责”和“不负责”的范围；
- `*Utils` 工具类入口统一放在 `src/utils/<前缀>_utils.rs`。单文件可以包含完整实现；一旦需要
  多个关联文件或私有子模块，相关实现统一放入同名前缀的 `src/<前缀>/`，`src/utils/` 只保留
  工具类入口，且不得改变已有公共导出路径；
- 新能力应优先复用已有公共基础设施，避免新增与现有 `*Utils`、领域客户端或后端重复的 API；
- 新增、删除、重命名或职责变化必须同步更新 module map 的源文件、导出、feature/依赖、
  边界和使用文档映射。

### 5.2 公共导出和签名

- `src/lib.rs`、领域 `mod.rs` 和 `utils/mod.rs` 中的 `pub`、`pub use`、`pub mod` 必须与文档一致；
- 对每个公开模块、结构体、枚举、错误、常量、自由函数、trait、类型别名、静态项和宏，列出
  当前源码实际支持的所有 crate 根、领域模块、`utils` 重导出和公开子模块直达路径；
- 兼容或次级路径可以保留，但必须标注推荐路径；私有实现文件路径不能冒充公共导入路径；
- 每条路径都必须由源码、最小编译 fixture 或公开 API 测试验证；
- 直接出现在公共签名中的第三方类型必须明确其依赖 crate、feature、版本边界和调用方责任；
- 新增方法优先使用清晰、可组合的通用入口；不要为了隐藏 feature 差异暴露不稳定的别名；
- 多后端能力优先采用一个通用方法加 feature 精确控制的枚举参数；只有后端输入类型本身
  不可统一时，才使用带后缀的方法；不能新增“只启用一个后端时才存在”的无后缀别名，除非
  已有同类兼容约定且接受多后端同时启用时该别名消失。

### 5.3 feature 和可选依赖

- 第三方依赖必须 `optional = true`。单一第三方 crate 提供的能力，其公开 feature 默认必须与直接可选
  依赖同名并通过 `dep:<name>` 映射；同一提供方的多个 middleware 可以共用该 feature。只有用户
  确需独立选择上游能力时，才增加与提供方和上游 feature 明确对应的映射，并记录命名及依赖边界；
  不得为 Axum 包装层发明 `axum-*` feature。只有多个第三方 crate 必须共同组成能力时，才可使用
  语义聚合 feature，并在文档和依赖树中列出全部提供方；
- 上游 feature 转发必须在 manifest 中使用 Cargo 的 `package-name/feature-name` 语法，文档列出实际启用项。
  Cargo feature 属于当前 package；下游启用本 crate 的提供方同名 feature，不直接操作传递依赖。
  例如：`tower-http = ["dep:tower-http", "tower-http/cors", "tower-http/timeout"]`，下游使用
  `features = ["tower-http"]`；
- 上述同名 provider 规则同样适用于调度器：直接提供 cron 解析/计算的 `croner` 依赖使用
  `croner = ["dep:croner"]`，不得另建 `scheduler` 别名；`Scheduler` 仍通过源码中的
  `chrono + chrono_tz + tokio + croner` 精确组合守卫表达跨 provider API 前置条件；
- 依赖版本使用 Cargo 默认 caret 兼容约束，不在 `version` 中使用 `=` 精确锁定补丁版本；
- 默认 feature 保持为空；不依赖第三方 crate 的方法直接可用，不增加 feature 守卫或可选依赖；
- 单项 feature 默认保持独立，不自动启用其他项目 feature；只有公共 API 基础设施必需的内部
  适配依赖可以在同一 feature 中聚合，并必须记录原因；
- 模块、类型、方法、导入、测试、fixture 和文档示例使用最窄的 `cfg` 守卫；一个 API 需要
  多个能力时使用 `cfg(all(...))`，不能用过宽的模块级守卫掩盖缺失组合；
- 同时验证 feature 的“应存在”和“不应存在”两侧：单独启用后端不能意外导出基础 API，
  缺少组合 feature 的方法必须得到稳定、可识别的编译诊断；
- 新增 feature 或修改依赖边界时，`Cargo.toml`、源码和 fixture 必须一致；文档及 CHANGELOG 同步
  按第 7.2、7.4 节执行；
- 新增、删除、重命名 feature，或修改其依赖映射、组合前提、公共导出时，必须检查
  `[package.metadata.docs.rs].features`。清单必须覆盖应展示的可选公共 API，并排除 `mimalloc`、
  `rpmalloc` 及其他负向契约或不能共同成功构建的组合；无需更新时，验收报告必须说明理由。
  只有 feature 名称集合变化会修改面向开发人员的非 allocator 综合命令时，才按第 2 节读取并同步
  `docs/develop.md` 对应清单；
- feature 选择不得悄悄改变 TLS、代理、OpenSSL/native-tls、Tokio runtime、全局分配器等
  安全或平台边界，依赖树必须验证实际结果。

### 5.4 全局状态、异步和外部副作用

- `OnceLock` 或其他进程级单例必须说明一次初始化、重复初始化、不可替换、生命周期、并发和
  测试隔离语义；不能向调用方暴露误导性的 reset/replace 假象；
- library crate 的普通异步 API 不隐式创建 Tokio runtime，不在异步入口内部 `block_on`；runtime
  默认由调用方提供，生产依赖和测试依赖分别声明所需的 Tokio feature。若后续提供专门、显式 opt-in
  的 runtime 构建或运行 API，则允许该 API 创建并拥有一个 runtime，但必须记录 runtime 的创建、
  `block_on`、关闭、嵌套调用错误和任务生命周期语义；其他普通异步 API 仍不得偷偷创建第二个 runtime。
- 构造配置、客户端或纯解析对象默认不访问网络；文件、网络、SMTP、Redis、进程级分配器等
  外部副作用必须在 API doc、测试和验收报告中明确；
- 真实外部服务测试的 ignored、配置、授权和副作用边界统一按第 9 节执行；
- 连接、重试、缓存、去重、锁租约、事务、超时和响应/请求大小必须有有限边界，并写出超限
  和失败语义；
- `mimalloc`/`rpmalloc` 这类进程级能力不得新增运行时切换 API；互斥 feature、下游重复注册
  和目标平台的 native linker 前提必须单独验证。

## 6. Rust 代码风格和实现质量

### 6.1 可读性和一致性

- 运行 `cargo fmt --all -- --check`，遵循 Rust 2021 命名和惯用写法；
- 说明、审查意见、提交信息、API doc 和项目文档默认使用中文；Rust 标识符、命令、API 名称
  和标准技术术语保留原文；
- 新增或修改的封装方法必须有与可见性匹配的注释：公共方法使用 doc comment 说明用途、输入、
  返回值、错误、feature、限制和副作用；私有方法说明职责及非显然的不变量或设计原因；
- 使用 `Result`/`Option` 表达失败或缺失，错误必须可追踪；不得以静默吞错、无依据强制转换或日志
  代替错误语义。`unwrap`、`expect`、`panic!` 仅用于已证明不可达的内部不变量或测试，并在代码/
  文档中说明理由；不可信输入、配置、网络、文件或用户数据不得触发未记录的 panic；
- `unsafe` 只在确有必要时使用；必须有紧邻的安全不变量说明、最小边界、平台条件和测试，
  并在审查报告中单独列出。

### 6.2 性能、资源和拒绝服务边界

新增方法或改变解析、网络、加密、随机、缓存、锁和全局状态时，必须评估：

- 时间复杂度、空间复杂度、临时分配和拷贝次数；
- 输入长度、文件大小、递归深度、模板展开、数组/别名数量、批量数量和 key 数量上限；
- 重试次数、退避、超时、并发、连接池、缓存和完成状态的上界；
- 锁竞争、租约过期、取消、任务阻塞和跨 await 持有资源的风险；
- 错误路径是否泄露明文、密钥、凭据、配置内容、原始响应或其他敏感数据。

需要限制时采用安全默认值和显式错误，不把无界输入交给第三方后端；若暂时不能提供限制，
必须在 API doc 中写明调用方责任并补充风险记录。

### 6.3 领域安全约束

- `CryptoUtils` 的密钥、明文、密文、IV/nonce 和原始错误不得写入日志或错误文本；
- MD5 只能用于非对抗性摘要，不得用于密码、签名或安全认证；CBC 没有完整性认证，不能把它
  描述成认证加密；
- `RandomUtils` 不是密码学安全随机源，不用于密码、token、密钥或安全 nonce；AES 的随机
  IV/nonce/密钥生成必须使用操作系统随机源；
- JWT payload 不是加密内容；算法、key、claims、过期/生效时间和全局初始化语义必须明确；
- 配置解析错误不回显原始配置内容；敏感 Header、Cookie、认证信息和跨 origin 行为必须有
  明确的合并与过滤规则；
- 任何新增网络能力都要审查代理、重定向、TLS、SSRF、请求/响应上限和隐式重试边界，不能
  把当前库未承诺的安全能力写成已保证。

## 7. 文档和变更记录同步标准

### 7.1 公共能力文档

每个公共能力单元必须在 `docs/examples/<前缀>.md` 有详细使用文档。工具类文档使用同名前缀，
去掉 `Utils` 后缀并按文件名规范转换，例如 `XxxxUtils` 对应 `docs/examples/xxxx.md`。同一领域的
模块、类型和静态工具入口可以合并到同一文件，完整映射以 `docs/module-map.md` 为准。

文档必须覆盖：

- 公开模块、结构体、枚举、错误类型、常量、自由函数、trait、类型别名、静态项、宏和重要
  trait 实现；
- 每个本 crate 定义的公共自由函数、inherent/associated 方法和 feature-gated 方法的独立小节；
- 方法签名、参数含义、返回值和错误语义、正常示例、边界示例、限制、feature 和副作用；
- 所有实际支持的公开导入路径，包括推荐路径和兼容路径；
- 枚举的所有公开变体、`#[non_exhaustive]` 约束、直接依赖类型和调用方必须承担的生命周期/
  runtime/资源责任。

不能用“几个方法合并一个示例”代替逐项覆盖，也不能只写私有实现文件路径。

### 7.2 API doc、README 和 module map

- 新增或修改公共方法、签名、行为、安全边界或 feature 守卫时，必须同步更新 API doc（含
  `# Examples` doctest）、`docs/examples/`、对应测试和必要的 module map；
- 工具类或公共能力完成后，必须在同一变更中向 `README.md` 添加或更新简要说明和简短示例，
  并链接对应的 `docs/examples/<前缀>.md`；README 不复制长期维护的完整 API 说明；
- 新增、删除、重命名或职责变化的工具类/公共模块必须更新 `docs/module-map.md` 的定位清单、
  导出路径、feature/依赖、职责边界、主要场景和使用示例映射；
- `Cargo.toml`、源码、fixture、README、API doc、`docs/examples/` 和 module map 中的 feature 守卫
  与模块路径必须互相一致；
- `docs/examples/` 是随 crate 发布的使用文档，必须纳入 Git；发布前用 `cargo package --list`
  按 module map 的映射逐项确认，不能只检查文件是否在工作区存在；
- 仅维护开发流程、规则或说明时不写 CHANGELOG；是否记录以第 7.4 节为准。

### 7.3 文档代码块验证

Markdown 不会被 `cargo test --doc` 自动收集。新建或修改 README、`docs/examples/` 或其中的
Rust 代码块时，必须：

1. 在临时 scratch crate 中通过 `#![doc = include_str!("...")]` 收集本次涉及的完整文档；
2. 按实际 feature 组合运行 `cargo test --doc`，覆盖普通、`no_run` 和必要的 `compile_fail` 块；
3. 对需要证明“未启用某 feature 时 API 不存在”的案例，运行预期失败的最小 fixture，核对
   诊断目标而不是只检查非零退出；
4. 使用 `example.com` 等保留域名和明显占位凭据；SMTP、Redis、文件写入、外部进程和平台状态
   示例必须 `no_run`、只构造对象或明确不会执行外部副作用；
5. 验证后删除 scratch crate 和本次产生且无用途的构建/日志临时文件。

“本次涉及”以修改文件为界：局部修改 README 只收集 README；修改某个
`docs/examples/<name>.md` 则编译该文件的全部 Rust 代码块。`docs_examples_are_complete` 可以快速
双向枚举全部文档 metadata；运行 ignored 的 `compile_docs_examples_offline` 时，必须用
`AXUTILS_DOCS_EXAMPLE_FILTER` 限定受影响文档。只有命中第 8.4 节或用户明确要求全量验证时，才可
不设过滤器并编译全部 `docs/examples/`；harness 支持全量模式不构成扩大局部验证的理由。

### 7.4 版本和 CHANGELOG

- 每次源码、公共 API、运行时行为、错误或安全边界变更前，先读取 `Cargo.toml` 的
  `[package].version`；
- 在当前版本条目下按 `Added`、`Changed`、`Fixed`、`Breaking` 等类别记录用户可见变化；
- 任务执行过程中不得自行提升或修改版本号，也不得为尚未发布的变化预先创建未来版本条目；
- 只有发布流程由用户明确要求时才更新版本号，并同步 CHANGELOG；
- GitHub Actions、CI、开发工具、规则文件、Skill、文档整理和其他仅影响仓库维护流程的变化
  不写入 CHANGELOG。

## 8. 测试、feature 矩阵和验证标准

### 8.1 测试内容要求

每个新增或修改的封装方法都必须有测试覆盖其适用的正常、边界和错误行为；私有方法可以通过
上层可观察行为间接覆盖，公共方法还必须通过公开导入路径或 doctest 验证。测试覆盖以行为和
契约为准，不以无依据的覆盖率百分比替代。按影响范围继续覆盖以下测试层次：

- **单元测试**：正常输入、空值/最小值/最大值、非法输入、错误类型、平台分支和资源上限；
- **集成测试**：从公开导入路径调用，覆盖跨模块行为、错误传播、全局初始化和并发语义；
- **编译 fixture**：验证公共路径和 feature 组合“应成功”；验证缺少 feature、错误组合、
  禁止依赖和禁止导出“应失败”，并检查诊断中包含目标符号或依赖；
- **依赖边界**：使用 `cargo tree` 或等价结果确认 optional 依赖、弱依赖、TLS/runtime、
  transitive feature 和默认依赖没有越界；
- **文档测试**：验证本次修改涉及的全部文档 Rust 代码块，而不是只抽查一个示例；
- **真实服务测试**：按第 9 节运行；默认验收不依赖它们。

全局单例、进程 allocator、live service 或时间/环境相关测试必须隔离状态、串行执行或用独立
进程/临时目录，不能因运行顺序偶然通过。

### 8.2 按影响范围选择命令

先运行与影响范围一致的最小充分验证：直接相关单元/集成测试、受影响 feature、fixture、doctest
和依赖边界；普通单 feature/依赖调整默认止于对应组合、filtered matrix 和依赖树。只有变更跨模块
或多个能力单元，涉及高影响 feature/依赖边界、公共安全边界，改变默认 feature、共享/传递依赖或
TLS/runtime/allocator 等安全/平台边界，无法局部证明影响，涉及发布，或用户明确要求完整验证时，
才执行第 8.4 节。扩大前必须记录触发原因、增加范围和预计耗时；不得仅因完整清单存在就默认执行。

| 影响范围 | 最小验证 | 需要增加的验证 |
| --- | --- | --- |
| 仅私有局部实现 | `cargo fmt --all -- --check`；直接相关单元/集成测试；`git diff --check` | 触及资源、安全、并发或跨平台时增加相应边界和 feature 检查 |
| 公共行为或错误语义 | 上述检查 + 相关 feature 测试 + API doc/doctest | 本次涉及的 `docs/examples/<name>.md` 全部代码块、CHANGELOG 判断、公开路径和回归测试 |
| 公共导出、签名或模块 | 相关单元/集成测试 + API doc/doctest + 对应正负 fixture | 对应的 filtered ignored matrix、受影响文档 feature 和兼容性审查；不因仅新增方法默认运行完整矩阵 |
| feature、依赖、Cargo 配置、TLS/runtime 或 MSRV | 相关组合的 `check/test/doc`、正负 fixture、filtered ignored matrix 和依赖树 | 默认能力隔离、fresh-resolution、传递 feature 和平台前提；命中高影响条件或局部证据不足时才扩大完整矩阵 |
| 跨模块、公共安全边界或发布 | 完整项目验证清单 | `cargo package --list`、必要的 publish dry-run；外部发布仍需明确授权 |

局部验证不得被描述为完整或全量验证。未命中完整验证触发条件时，不得用与变更无关的全仓测试
替代影响分析。命令失败时保留关键输出，区分代码失败、环境缺失、预期负向失败和未授权外部操作。

### 8.3 本项目的固定契约验证

`tests/feature_matrix.rs` 的 ignored 测试是慢速契约。局部公共 API 变更默认只运行目标模块对应的
测试函数，例如：

```powershell
cargo test --no-default-features --test feature_matrix <相关测试函数名> -- --ignored --test-threads=1
```

只有 feature/依赖变更影响默认能力、共享/传递依赖、安全或平台边界、多个能力单元，或者涉及
跨模块 API/行为、公共安全边界、发布、局部 fixture 无法证明契约，或用户明确要求完整验证时，
才运行全部 ignored matrix：

```powershell
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
```

选择 filtered matrix 时必须确认对应测试覆盖本次变更的公开路径、feature 组合和预期失败诊断；
缺少覆盖时先补 fixture/matrix，再运行对应测试，不能用过滤参数跳过本应验证的相关组合。

新增 feature 时，必须在 `tests/fixtures/` 增加或调整正向和负向最小 crate，并在矩阵中记录：

- 空 feature、相关单 feature 和每个必要组合的预期成功；
- 缺少基础 feature、缺少后端 feature、错误依赖组合和禁止导出的预期失败；
- 依赖树中不应出现的包、feature 或 TLS/runtime 后端；
- 公开路径、方法签名和条件编译诊断的稳定识别 token。

当前 allocator 矩阵必须分别验证 `mimalloc`、`rpmalloc` 的成功路径，以及
`mimalloc + rpmalloc` 和下游重复注册的预期失败；因此以下命令不能作为成功验收：

```powershell
cargo check --no-default-features --features mimalloc,rpmalloc
```

它只有在非零退出且包含预期诊断时才算通过负向契约。对于同样会触发 allocator 冲突的
`--all-features`，必须按 feature 分组和负向组合验证，不能把全 feature 成功作为项目要求。

### 8.4 完整验证清单

命中第 8.2 节的完整验证条件时，执行并报告本清单；否则停留在最小充分验证。
`docs/develop.md` 仍只按第 2 节的命令变更条件读取，不是完整验证的前置。清单至少包括：

```powershell
cargo fmt --all -- --check
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
cargo test --no-default-features
cargo test --no-default-features --features mimalloc
cargo test --no-default-features --features rpmalloc
cargo test --no-default-features --features mimalloc --doc
cargo test --no-default-features --features rpmalloc --doc
cargo clippy --all-targets --no-default-features --features mimalloc -- -D warnings
cargo clippy --all-targets --no-default-features --features rpmalloc -- -D warnings
cargo package --list
git diff --check
```

再按变更涉及的能力补充 `lettre`、`tokio`、`serde`、模板、日期、配置、crypto、JWT、HTTP、
Redis、转换和编码等组合的 `cargo check/test/doc` 与 `cargo tree`。当前能力的具体组合以
`Cargo.toml`、目标源码、直接相关测试和 `tests/feature_matrix.rs` 为准。

调度器变更还必须运行 scheduler 集成测试、doctest、clippy 和依赖树检查，并通过 ignored
feature matrix 验证第 3 节规定的全部 16 种组合；除完整组合外，其余 15 种必须由稳定诊断证明
API 不存在。依赖树还必须确认
生产 Tokio feature 精确包含 `fs`、`io-util`、`net`、`rt`、`rt-multi-thread`、`signal`、`sync`、
`time`（及其平台传递 feature），不由本 crate 的 `tokio` feature 启用 `macros`，并记录 Croner
带入的 `chrono/clock`、`derive_builder` 和 `strum`。

## 9. 安全、配置和外部操作验收

- `config/` 整体被 `.gitignore` 忽略，只能存本地测试配置；不得提交账号、密码、授权码、邮箱、
  节点地址或其他凭据，也不得把它们写进命令行、日志、测试输出、文档或 fixture；
- 默认不运行真实 SMTP/Redis 测试，不发送邮件、不修改远程数据、不连接不受控服务；需要运行
  时必须有用户明确授权、受控服务、一次性环境开关和被忽略的本地配置；
- live 测试缺配置时必须明确失败或跳过的原因，不得伪装成成功；
- 任何错误、诊断和审查报告只输出证明问题所需的最少信息；敏感值使用占位符或脱敏摘要；
- 依赖升级、系统工具链切换、全局安装、环境变量修改、发布、推送和发送外部消息不属于默认
  验收动作，必须单独确认授权和影响范围；
- 破坏性删除、覆盖、移动或清理前先确认绝对路径和目标范围，优先使用可恢复方式，不得清理
  用户已有的未提交改动、未知来源文件或其他任务产物。

## 10. 审查清单和“完成”定义

审查时按以下顺序逐项给出结论，任何一个适用门槛缺少证据都不能标记为完全通过：

| 门 | 通过条件 | 证据示例 |
| --- | --- | --- |
| 需求与范围 | 目标、非目标、影响范围和验收条件明确，差异没有无关改动 | diff、文件清单、影响分析 |
| 结构与职责 | 放置位置、复用关系和 module map 一致，没有重复或越界职责 | `docs/module-map.md`、调用链、源码 |
| 代码质量 | 方法注释、rustfmt、命名、错误处理、复杂度、资源边界和 unsafe 说明满足标准 | 源码、fmt/clippy 输出、设计说明 |
| 公共 API | 导出路径、签名、可见性、feature 守卫、兼容性和错误语义均有证据 | `src/lib.rs`、领域模块、fixture、API doc |
| feature/依赖 | 成功/失败组合和依赖树均符合预期，无隐式依赖或安全后端越界 | feature matrix、`cargo tree`、Cargo.toml |
| 文档同步 | API doc、doctest、`docs/examples`、README、module map、docs.rs feature 清单、CHANGELOG（如适用）同步 | 文档差异、scratch doc test、综合 rustdoc、映射表 |
| 测试 | 每个变更方法及其正常、边界、错误、集成、公开路径和适用的负向 fixture 已覆盖 | 测试源码与命令输出 |
| 安全与副作用 | 敏感信息不泄露，资源有界，外部副作用被隔离并记录 | 安全审查、忽略测试配置、错误输出 |
| 发布边界 | package 白名单正确，开发者文件/凭据/测试不进入包，版本和 CHANGELOG 一致 | `cargo package --list` |
| 交付报告 | 明确通过项、未运行项、失败项、预期失败、风险和后续动作 | 最终验收报告 |

达到以下条件才可以交付：

1. 适用门全部通过，或每个例外都有明确原因、替代证据、用户接受的风险和后续动作；
2. 没有未处理的公共 API、feature/依赖、文档路径、敏感信息或行为回归问题；
3. 测试和验证命令的范围与变更影响一致，没有把局部结果包装成全量结果；
4. 任何预期失败只在对应负向契约中出现，并且诊断目标已核对；
5. 交付说明可以让另一位维护者仅凭文件、差异和命令输出复核结论。

## 11. Skill 与项目规则的维护

- `AGENTS.md` 必须链接本 Skill，并与第 1、2 节的完整触发范围、例外和读取要求保持一致；
- 可跨任务复用的稳定流程只维护在本项目级、工具无关的 Skill，不写入特定 Agent 工具语法；
- 修改本 Skill 时检查规则链接、触发描述和 module map 引用；`docs/develop.md` 仍按第 2 节处理。
  Skill、规则或纯文档整理本身不提升版本、不写 CHANGELOG；公共实现变更仍按第 7.4 节处理；
- 标准与当前实现冲突时，按第 2 节权威顺序，以源码和可复现证据定位后同步修正；不得静默采用
  较宽松解释。
