# axutils 项目协作规则

项目实现、公共 API、feature/依赖、测试、文档和发布的详细设计、工作流与验收要求，统一维护在
[`review-rust-library-change`](docs/skills/review-rust-library-change/SKILL.md) Skill；本文件不重复其细则。

## 强制入口与文档职责

[`review-rust-library-change`](docs/skills/review-rust-library-change/SKILL.md) 是项目级、工具无关的
Rust library 变更工作流与权威审查验收标准，不包含特定 Agent 工具调用语法。对以下任何任务，
必须在读取适用项目规则后、开始设计、实现、审查或验收前完整读取并使用该 Skill，不得只依据
本文件摘要：

- 修改或审查 `src/`、`Cargo.toml`、公共 API、错误语义、运行时行为或安全边界；
- 新增、删除、重命名或调整工具类、领域模块、公开导出、类型、方法、trait、枚举、常量、类型别名、
  静态项、宏、feature、依赖、测试、fixture、API doc、README、`docs/examples/`、发布白名单、
  发布元数据或 CHANGELOG；
- 对上述内容进行实现就绪性审查、回归排查、发布前检查，或用户要求“按标准审查/验收”。

纯翻译、简单措辞调整或与 Rust library 无关的文件操作不自动触发该 Skill；但涉及本项目规则、
标准、模块定位或验收时，仍必须完整读取。

涉及具体工具类、领域模块、跨模块 API 或新增方法时，在标准文档之后再完整读取
[`docs/module-map.md`](docs/module-map.md)。[`docs/develop.md`](docs/develop.md) 只说明开发/发布命令，
不是 Agent 常规实现、测试或验收的必读上下文；只有任务新增、删除、修改这些命令，或需向开发人员
同步命令变化时，才读取并更新对应章节。源码、规则、标准或文档描述不一致时，按 Skill 规定的
权威来源顺序收集证据并处理，不得静默采用较宽松解释。

## 项目定位与仓库边界

这是 Rust library crate `axutils`，入口为 `src/lib.rs`；默认 feature 为空，不依赖第三方 crate 的
能力默认可用，其他能力通过显式 feature 提供。工具类、领域模块、公共导出、feature、依赖和
适用范围以 [`docs/module-map.md`](docs/module-map.md) 为唯一定位清单。

- `tests/` 和 `tests/fixtures/` 是回归测试与 feature/API/依赖契约的一部分；不得为通过当前测试而删除或放宽；
- `tests/email_live.rs`、`tests/redis_live.rs` 等真实外部服务测试固定为 ignored，只有用户明确授权、
  受控服务和被忽略的本地配置同时满足时才可运行；
- `config/` 仅存本地测试配置；禁止提交密码、授权码、邮箱、节点地址或其他凭据，也不得写入命令行、
  日志、测试输出或文档；
- `CHANGELOG.md` 仅记录源码、公共 API、运行时行为、错误/安全边界及直接面向使用者的兼容性变化；
  规则、CI、开发工具、Skill 和文档整理不写入其中；
- `Cargo.toml` 的 `package.include` 是发布白名单；开发者标准、规则、测试、`docs/skills/` 和本地配置
  不属于发布包；library crate 不将根目录 `Cargo.lock` 作为依赖版本策略提交。

## 基本工作约定

说明、审查意见、提交信息、注释和文档默认使用中文；Rust 标识符、API、命令和标准技术术语保留
原文。修改前须读取相关规则、标准、源码、测试和 Cargo 配置，确认根因与影响范围并保持最小变更；
不得顺带重构无关代码，或擅自切换工具链、修改全局环境、发布、推送、运行真实外部服务。

版本号以 `Cargo.toml` 的 `[package].version` 为唯一来源。源码、公共 API、运行时行为、错误或安全
边界变更前，必须读取当前版本并按标准判断 CHANGELOG；任务期间不得自行提升版本号。
