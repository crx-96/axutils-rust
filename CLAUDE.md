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

## 修改约定

- 默认使用中文编写说明、审查意见、提交信息和文档；代码中的 Rust 标识符遵循 Rust 命名规范。
- 修改前先读取相关源码、测试和 Cargo 配置，保持最小变更，不顺带重构无关内容。
- 每次修改或增加功能时，必须先读取 `Cargo.toml` 中 `[package]` 的 `version`，再在 `CHANGELOG.md`
  中新增或更新对应版本条目；仅对源码、公共 API、运行时行为、错误与安全边界，以及直接面向使用者的
  兼容性变化按 `Added`、`Changed`、`Fixed`、`Breaking` 等类别记录。GitHub Actions、CI/工作流、开发工具、
  文档整理和其他仅影响仓库维护流程的变更不得写入 `CHANGELOG.md`。
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

## 验证命令

提交代码前至少运行：

```powershell
cargo fmt --all -- --check
cargo test --all-features
cargo test --doc --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
```

影响发布内容或 Cargo 配置时，还要运行 `cargo package --list`，确认开发者文件未进入
发布包；详细发布步骤见 [develop.md](develop.md)。

## 发布边界

`Cargo.toml` 的 `package.include` 是发布包白名单。`README.md`、`CHANGELOG.md`、`LICENSE`、
`Cargo.toml` 和 `src/**` 可以随包发布；`develop.md`、`AGENTS.md`、`CLAUDE.md` 和 `docs/skills/**`
仅供仓库开发使用，不要调整白名单将它们带入 crates.io。

## 项目专属 skill

当前任务不需要额外的项目专属 skill，因此暂不创建 `docs/skills/`。如果后续确实需要
可复用的项目工作流，应在 `docs/skills/<skill-name>/SKILL.md` 创建说明，并在 `AGENTS.md` 和
`CLAUDE.md` 中加入对应链接后再使用。

## 规则文件同步

本文件是 [AGENTS.md](AGENTS.md) 在 Claude Code 中读取的同步副本，两者项目约定必须保持
一致（Claude Code 读取本文件，其他工具读取 `AGENTS.md`）。修改本文件后必须同步更新
`AGENTS.md`（反之亦然），避免不同工具读取到不一致的项目约定。
