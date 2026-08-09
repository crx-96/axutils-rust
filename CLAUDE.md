# axutils 项目协作规则

## 项目定位

这是一个 Rust library crate，包名和 crate 名均为 `axutils`。公共 API 位于
`src/lib.rs` 及其 feature 模块中。当前 `TimeUtils` 仅依赖标准库，属于默认能力；
`RegUtils` 模块及基础校验能力依赖第三方 `regex` crate，仅通过显式启用的 `regex` feature 提供；
国际手机号校验能力还需要显式同时启用独立的 `libphonenumber` feature。
SMTP 邮件能力通过显式 `lettre` feature 提供；异步发送还需要同时启用独立的 `tokio` feature。
工具类的职责边界、公共导出、依赖和适用范围维护在
[工具类定位文档](docs/module-map.md) 中。

## 仓库目录与 Git 提交边界

- `tests/`：集成测试、feature/API 矩阵测试、依赖边界断言及测试 fixture 目录。测试代码和
  fixture 应随实现提交，作为后续修改的回归保障；`tests/email_live.rs` 中的真实 SMTP 测试
  固定为 ignored，只有用户明确授权、环境变量和本地配置同时满足时才会发信。
- `CHANGELOG.md`：面向使用者的版本变更记录，仅记录源码、公共 API、运行时行为、错误与安全边界，以及
  直接面向使用者的兼容性变化，并随 crate 发布；不记录 GitHub Actions、CI/工作流、开发工具、文档整理
  或其他仅影响仓库维护流程的变更。
- `config/`：本地测试和开发配置目录，整体由 `.gitignore` 忽略。可以通过代码读取其中的测试
  配置，但不得提交 `email-test.toml`、密码、授权码、邮箱地址或其他凭据，也不得将其写入
  命令行、日志或文档。
- `tests/` 不属于运行时依赖；删除它会失去回归测试，删除前必须确认对应能力不再需要并同步清理目录引用。

## 模块使用文档

- 每个公共能力单元必须在 `docs/examples/<前缀>.md` 维护一份详细使用文档（映射关系见
  `docs/module-map.md` 末尾的「使用示例文档」小节）。文档命名取能力单元的前缀，**不带
  `Utils` 后缀**：工具类按去掉 `Utils` 后的前缀命名（`PathUtils` → `path.md`、
  `TimeUtils` → `time.md`、`FormatUtils` → `format.md`、`RegUtils` → `reg.md`、
  `RandomUtils` → `random.md`），领域模块按模块名命名并把对应的 `Utils` 静态入口合并进
  同一份文档（email 模块 + `EmailUtils` → `email.md`，config 模块 + `ConfigUtils` →
  `config.md`）。文档必须说明该单元的完整公共 API：模块、结构体、枚举、常量、错误类型，
  以及当前或未来可能出现的公共自由函数、trait、类型别名、静态项和宏及其 feature 依赖；
  每个本 crate 定义的公共自由函数和 inherent/associated 方法单独一节，写清签名、参数含义、
  返回值/错误语义、正常与边界示例、注意事项与限制。公共 trait 的必需方法与默认方法在 trait
  小节逐项说明；不能因为当前仓库尚无某类公共项，就把它排除在长期规则之外。
- 公共导出路径必须完整记录：对每个公开模块、类型、错误、枚举、常量、自由函数、trait、
  类型别名、静态项和宏，列出当前源码实际支持的所有 crate 根路径、领域模块路径、`utils`
  命名空间重导出路径和公开子模块直达路径，并标注 feature 守卫；如果某条路径只是兼容性/
  次级路径，也要保留并说明推荐路径。每条路径都必须经过当前源码或最小编译 fixture 验证；
  私有实现文件路径不能冒充公共导入路径，也不能因存在推荐路径而省略其他可访问公共路径。
- 新增公共类型、自由函数、trait/trait 方法或 inherent/associated 方法，或修改现有签名或行为
  时，必须在同一变更中同步更新对应 `docs/examples/` 文档；新增公共能力单元时必须新建对应
  文档，并同步更新
  `docs/module-map.md` 的映射表。
- `docs/examples/` 文档随 crate 发布（后续实施完成后 `Cargo.toml` 的 `package.include` 白名单必须
  包含 `docs/examples/**`），必须提交到 Git；发布前运行 `cargo package --list` 确认其在发布包内。
- `README.md` 只保留简短示例与能力概览，详细示例一律链接到 `docs/examples/` 对应文档，
  不在 README 中复制完整行为说明。
- 文档示例优先复用源码 API doc 中已通过 doctest 验证的示例，至少保证可编译；涉及 SMTP
  网络、进程级单例、外部文件、异步 runtime 或平台状态时，必须使用 `no_run`、函数指针/构造
  示例或明确的“不会执行外部副作用”说明，不能在验证或阅读示例时向真实 relay 发信。示例中
  只能使用 `example.com` 等保留域名和明显的占位凭据，不得出现真实账号、密码、授权码、真实
  邮箱地址或本地配置。
- 新建或修改 `docs/examples/` 或 `README.md` 时，必须对该次变更涉及文档的**全部** Rust
  代码块做编译验证，不能用抽查代表性片段代替。Markdown 本身不由 `cargo test --doc` 自动
  收集；应在临时 scratch
  crate 中通过 `#![doc = include_str!(...)]` 配合 `cargo test --doc`，按文档实际 feature 组合
  验证普通、`no_run` 和必要的 `compile_fail` 代码块，并在完成后删除 scratch 目录。需要证明
  API 在某个 feature 组合下不存在时，还要运行预期失败的最小编译 fixture 并核对诊断目标。
- 文档维护触发条件不仅包括新增方法：还包括任一公共项的新增、删除、重命名、可见性变化、
  签名变化、返回值或错误语义变化、feature 守卫变化、公开常量或枚举变体变化，以及会改变
  已记录行为/安全边界的实现修改。每个当前或未来由本 crate 定义的公共自由函数和 inherent/associated 方法
  （包括 feature-gated 方法和单后端别名）必须在对应文档中拥有独立小节和至少一个针对该项的
  调用示例；把多个函数/方法合并成一个示例不能代替逐项覆盖。公共 trait 的方法、公开常量、
  枚举变体、类型别名、静态项、宏、`#[non_exhaustive]` 约束和重要 trait 实现也必须在导出清单
  或专门小节中说明。

## 修改约定

- 默认使用中文编写说明、审查意见、提交信息和文档；代码中的 Rust 标识符遵循 Rust 命名规范。
- 修改前先读取相关源码、测试和 Cargo 配置，保持最小变更，不顺带重构无关内容。
- 每次修改或增加功能时，必须先读取 `Cargo.toml` 中 `[package]` 的 `version`，再在 `CHANGELOG.md`
  中新增或更新对应版本条目；仅对源码、公共 API、运行时行为、错误与安全边界，以及直接面向使用者的
  兼容性变化按 `Added`、`Changed`、`Fixed`、`Breaking` 等类别记录。GitHub Actions、CI/工作流、开发工具、
  文档整理和其他仅影响仓库维护流程的变更不得写入 `CHANGELOG.md`。
- 版本号以 `Cargo.toml` 的 `[package].version` 为唯一来源，与 `CHANGELOG.md` 版本条目一一对应；
  无论变更是否破坏兼容，任务执行过程中一律**不得**自行提升或改动版本号、也不得为尚未发布的变化
  预先新增未来版本条目。crate 未发布到 crates.io 时，破坏性变更直接在当前版本条目内改写/补充记录。
  版本号调整只发生在发布流程中，由用户在发布时显式修改 `Cargo.toml` 并同步 `CHANGELOG.md`。
- 新增方法必须以性能和安全为优先：评估时间/空间复杂度、内存分配、锁或全局状态、输入规模、panic/拒绝服务和敏感数据风险，采用安全默认值，避免无界资源消耗、隐式副作用和静默吞错；性能或安全边界需要时应补充相应测试或基准验证，并在 API doc 中说明重要限制。
- 新增公共方法时，必须同时添加 API doc、`# Examples` doctest 和覆盖正常/边界情况的测试。
- `src/utils/` 仅承载单文件或少量紧密相关实现；当某个工具类需要拆分、创建多个关联源文件或私有子模块时，必须在 `src/<领域>/` 下创建独立目录与文件，而不是继续堆放到 `src/utils/`。目录名使用对应领域的小写 snake_case；例如 `TimeUtils` 的关联实现放在 `src/time/`。保留既有公共导出路径，并只创建任务实际需要的目录和文件。
- 新增、删除或重命名工具类/公共模块时，必须同步更新 [工具类定位文档](docs/module-map.md)，记录源文件、公共导出、feature/依赖、职责边界和主要使用场景。
- 处理具体工具类、跨模块 API 或新增方法前，必须先阅读 [工具类定位文档](docs/module-map.md)；若定位信息已过期，应在同一变更中修正。
- 新增需要第三方包的能力时，依赖必须标记为 `optional = true`，并优先使用与依赖名一致的 feature，通过 `dep:<dependency-name>` 映射。
- 不依赖第三方包的方法属于默认能力，不添加 feature 守卫，直接从 crate 根模块默认导出。
- feature 守卫、模块路径、README 示例和开发者文档必须保持一致；默认 feature 只包含不依赖第三方包的能力，当前正则能力统一使用 `regex` feature。
- 模块、类型和方法的 feature 守卫应按其直接能力和依赖分别控制：模块或类型只由自身所属能力的 feature 导出，不因其他可选依赖 feature 单独启用而扩大公共 API。
- 一个 API 同时依赖多个可选能力时，使用 `#[cfg(all(feature = "...", feature = "..."))]` 精确限制实现、导入、测试和文档示例；不要用更宽的模块级守卫替代方法级约束。
- 可选依赖对应的 feature 应保持独立：后端或单项能力 feature 只能通过 `dep:<dependency-name>` 启用其同名直接依赖，禁止引用、自动启用或聚合其他项目 feature。一个 API 需要多个可选能力时，由用户显式启用全部 feature，并在代码中使用精确的 `cfg(all(...))` 限制；多 feature 组合及其公共 API 矩阵必须同步记录在工具类定位文档、README、CHANGELOG 和 API doc 中。仅作为公共 API 基础设施的 feature 可以启用其不可单独使用的内部适配依赖，但不得启用其他项目 feature。
- 依赖版本约束必须使用最低兼容版本的 Cargo 默认 caret 写法（例如 `version = "0.11.22"`），不得在 `version` 字段使用等号前缀来精确锁定补丁版本。本项目是 library crate，不提交 `Cargo.lock` 作为依赖版本策略；安全下限和兼容性通过 manifest、MSRV 与无锁 fresh-resolution 验证。若未来确有技术原因需要精确版本，必须先取得用户明确确认，并在计划、状态和 `CHANGELOG.md` 中记录原因；当前项目不设此类例外。
- 一个能力存在多个可选第三方后端时，优先设计为“一个通用方法 + 内部按 feature 匹配的枚举
  参数”（参见 `ConfigLoader`/`ConfigUtils` 对 `ConfigFormat` 的处理）：枚举各变体按自身依赖
  的 feature 精确 `#[cfg(...)]` 守卫，未启用的后端既不出现在枚举里也不出现在公共方法签名里；
  各后端的具体实现下沉为模块私有函数，不作为独立的公共方法暴露。仅当各后端要求的入参具体
  类型本身随后端变化（例如不同第三方 crate 各自的日期/时间类型），导致无法用同一个方法签名
  覆盖所有后端时，才回退为“公共方法名按后端后缀区分”（参见 `TimeUtils` 的
  `format_date_chrono`/`format_date_time`/`format_date_jiff`）；此时应避免新增“仅当且仅当
  只启用一个后端 feature 时才存在”的无后缀别名，除非已有同类先例（如 `TimeUtils` 现有别名）
  且团队接受该别名在多后端同时启用时从公共 API 消失的行为。

## 验证命令

验证范围必须与变更影响面匹配。默认只运行直接相关的测试，不要求每次添加或修改内容都执行
全量测试；测试不足以覆盖影响面时，再逐级扩大范围，并在交付说明中写明实际运行的命令和未运行的检查。

- 只修改单个模块的实现、私有逻辑或局部测试，且没有改变公共 API、feature、依赖和跨模块行为时，运行对应的库单元测试或集成测试目标，优先使用测试过滤器；例如 `cargo test --no-default-features --lib utils::path_utils::tests`。
- 修改 feature-gated 模块时，至少运行对应 feature 与测试目标；例如 `cargo test --no-default-features --features redis --test redis`。修改 HTTP、邮件、配置等异步能力时，同时覆盖实际涉及的 feature 组合。
- 修改公共导出、签名、feature 守卫、可选依赖、依赖边界或跨模块调用时，在直接测试之外运行受影响的 feature/API 矩阵；本项目的 `tests/feature_matrix.rs` 属于慢速测试，使用 `cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1` 显式执行。
- 修改 API 文档、README 或 `docs/examples/` 时，按项目文档规则编译本次变更涉及文档的全部 Rust 代码块；新增或修改公共 API 的 doctest 不能用普通单元测试替代。
- 只有跨模块重构、公共行为或安全边界变化、Cargo 配置/依赖变更、发布前验证，或相关测试无法可靠隔离时，才运行全量验证。

全量验证命令如下：

```powershell
cargo fmt --all -- --check
cargo test --all-features --tests
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
cargo test --doc --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
```

影响发布内容或 Cargo 配置时，还要运行 `cargo package --list`，确认开发者文件未进入
发布包；详细发布步骤见 [develop.md](develop.md)。

## 发布边界

`Cargo.toml` 的 `package.include` 是发布包白名单。`README.md`、`CHANGELOG.md`、`LICENSE`、
`Cargo.toml`、`src/**` 和 `docs/examples/**` 可以随包发布；`develop.md`、`AGENTS.md`、
`CLAUDE.md` 以及 `docs/` 中的 `docs/plans/**`、`docs/status/**`、`docs/skills/**` 仅供仓库开发
使用，不要调整白名单将它们带入 crates.io。

## 项目专属 skill

当前任务不需要额外的项目专属 skill，因此暂不创建 `docs/skills/`。如果后续确实需要
可复用的项目工作流，应在 `docs/skills/<skill-name>/SKILL.md` 创建说明，并在 `AGENTS.md` 和
`CLAUDE.md` 中加入对应链接后再使用。

## 规则文件同步

本文件是 [AGENTS.md](AGENTS.md) 在 Claude Code 中读取的同步副本，两者项目约定必须保持
一致（Claude Code 读取本文件，其他工具读取 `AGENTS.md`）。修改本文件后必须同步更新
`AGENTS.md`（反之亦然），避免不同工具读取到不一致的项目约定。
