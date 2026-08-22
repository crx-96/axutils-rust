# axutils 项目协作规则

项目实现、公共 API、feature/依赖、测试、文档和发布工作的详细设计与验收要求统一维护在
[`REVIEW_ACCEPTANCE.md`](REVIEW_ACCEPTANCE.md)，本文件不重复复制其细则。

## 强制入口

对以下任务，必须自动完整读取 [`REVIEW_ACCEPTANCE.md`](REVIEW_ACCEPTANCE.md)，再开始设计、实现、
审查或验收；不能只依据本文件的摘要：

- 修改或审查 `src/`、`Cargo.toml`、公共 API、错误语义、运行时行为或安全边界；
- 新增、删除、重命名或调整工具类、领域模块、公开导出、feature、依赖、测试、fixture、API doc、
  README、`docs/examples/`、发布白名单或 CHANGELOG；
- 进行回归排查、发布前检查，或用户要求“按标准审查/验收”。

涉及具体工具类、领域模块、跨模块 API 或新增方法时，在标准文档之后再完整读取
[`docs/module-map.md`](docs/module-map.md)。需要开发或发布命令时读取 [`develop.md`](develop.md)
的对应章节。若源码、规则、标准或文档描述不一致，按标准文档中的权威来源顺序收集证据并处理，
不能静默选择较宽松的解释。

## 项目级通用 Skill 触发规则

[`review-rust-library-change`](docs/skills/review-rust-library-change/SKILL.md) 是项目级、工具无关的
Rust library 变更工作流，不包含任何特定 Agent 工具的调用语法。

当任务涉及 Rust library 的实现、模块归属、公共 API、feature、依赖、集成测试、编译 fixture、
API 文档、README、`docs/examples/`、发布元数据、实现就绪性审查或验收时，必须在读取项目标准后
完整读取并使用该 Skill。它用于组织工作流；[`REVIEW_ACCEPTANCE.md`](REVIEW_ACCEPTANCE.md) 仍是
本项目具体的验收门槛和权威标准。

纯翻译、简单措辞调整或与 Rust library 无关的文件操作不自动触发该 Skill；但只要涉及本项目
规则、标准、模块定位或验收，仍必须读取 `REVIEW_ACCEPTANCE.md`。

## 项目定位与仓库边界

这是 Rust library crate `axutils`，crate 入口为 `src/lib.rs`。默认 feature 为空；不依赖第三方
crate 的能力默认可用，其他能力通过显式 feature 提供。工具类、领域模块、公共导出、feature、
依赖和适用范围的唯一定位清单是 [`docs/module-map.md`](docs/module-map.md)。

- `tests/` 和 `tests/fixtures/` 是回归测试与 feature/API/依赖契约的一部分，不能为通过当前测试而删除或放宽；
- `tests/email_live.rs`、`tests/redis_live.rs` 等真实外部服务测试固定为 ignored，只有用户明确授权、
  受控服务和被忽略的本地配置同时满足时才可运行；
- `config/` 只存本地测试配置，禁止提交密码、授权码、邮箱、节点地址或其他凭据，也不得写入命令行、
  日志、测试输出或文档；
- `CHANGELOG.md` 只记录源码、公共 API、运行时行为、错误/安全边界和直接面向使用者的兼容性变化；
  规则、CI、开发工具、Skill 和文档整理不写入 CHANGELOG；
- `Cargo.toml` 的 `package.include` 是发布白名单；开发者标准、规则、测试、`docs/skills/` 和本地配置
  不属于发布包；library crate 不把根目录 `Cargo.lock` 作为依赖版本策略提交。

## 基本工作约定

默认使用中文编写说明、审查意见、提交信息、注释和文档，Rust 标识符、API、命令和标准技术术语
保留原文。修改前先读取相关规则、标准、源码、测试和 Cargo 配置，确认根因和影响范围，保持最小
变更；不顺带重构无关代码，不擅自切换工具链、修改全局环境、发布、推送或运行真实外部服务。

版本号以 `Cargo.toml` 的 `[package].version` 为唯一来源。源码、公共 API、运行时行为、错误或安全
边界变更前必须读取当前版本并按标准判断 CHANGELOG；任务执行过程中不得自行提升版本号。
