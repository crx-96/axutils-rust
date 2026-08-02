# 当前任务状态

## 目标

按 `docs/plans/backend-dispatch-style-audit.md` 完成 `FormatUtils` 模板后端公共 API 的风格统一：
新增显式 `TemplateEngine` 分派入口，移除旧后缀方法和仅单后端存在的无后缀别名，并同步
feature/API 矩阵、测试、用户文档、历史计划、变更日志和项目规则；实施后完成二次审查，直到
没有新的可行动问题。

## 范围

- `src/utils/format_utils.rs`：`TemplateEngine`、统一入口、私有后端实现及行为测试。
- `src/utils/mod.rs`、`src/lib.rs`：精确 feature 守卫和公共重导出。
- `README.md`、`docs/module-map.md`、`CHANGELOG.md`、`docs/plans/format-utils-template.md`。
- `tests/feature_matrix.rs` 与 `tests/fixtures/format_feature_matrix/` 的正/负向 API 矩阵。
- `AGENTS.md`、`CLAUDE.md`：固化多后端公共 API 设计规则。

## 阶段

1. [完成] 读取项目规则、执行文档、Cargo 版本、现有实现、测试、导出和文档；确认方案 A。
2. [完成] 实施统一 API、测试 fixture 和全部同步文档。
3. [完成] 执行 feature 检查、格式化、测试、doctest、clippy、doc、发布包清单和 diff 检查。
4. [完成] 二次审查 API 可见性、旧名称残留、feature 条件、文档一致性和风格，修复后复验。

## 已确认结论

- `Cargo.toml` 当前版本为 `0.1.0`；执行文档已确认 crate 尚未发布，因此移除旧 API 不新增
  `Breaking` 条目。
- `TimeUtils` 因各后端日期类型不兼容保留后缀模式；本次只改造 `FormatUtils`。
- 根 `Cargo.toml` 的依赖、版本和 feature 不需修改；测试 fixture 的直接 `serde` 依赖只服务于测试。
- 工作区已有未跟踪 `.claude/`，与本任务无关，保持不动。

## 风险与验证计划

- 重点验证 `serde + strfmt`、`serde + minijinja`、双后端及缺失 feature 时的类型/方法/枚举变体可见性。
- `TemplateEngine`、统一入口和三条公共访问路径（定义路径及两条重导出路径）必须使用同一外层
  `cfg`；后端实现必须保持原有
  回退、空输出、序列化、嵌套和 Unicode 行为。
- 任务结束前清理本次测试产生且无后续用途的临时内容；不删除既有计划、状态或用户文件。

## 当前验证结果

- `cargo check --no-default-features`、仅 `serde`、仅 `strfmt`、仅 `minijinja`、`serde+strfmt`、
  `serde+minijinja`、`serde+strfmt+minijinja` 全部通过。
- `cargo test --all-features`、`cargo test --doc --all-features`、`cargo clippy --all-targets
  --all-features -- -D warnings`、`cargo test --no-default-features`、`cargo doc --no-deps
  --all-features`、`cargo fmt --all -- --check`、`git diff --check` 全部通过。
- `cargo package --allow-dirty --list` 未包含测试、文档、规则和开发文件；仅包含发布白名单内容。
- 新增模板 fixture 的三种正向组合和六种负向可见性/变体检查均通过；真实 SMTP 测试仍按规则 ignored。

## 最终审查结论

当前源码、测试、公共导出、feature 守卫、README、crate 文档、模块定位、CHANGELOG、历史计划和
项目规则已相互一致；当前用户可见面没有遗留的旧模板 API 或旧分派规则引用。仅历史计划和执行
文档中的方案对比/追溯段保留旧名称，且已明确标注其历史或说明用途。

## 相关路径

`docs/plans/backend-dispatch-style-audit.md`、`src/utils/format_utils.rs`、`src/utils/mod.rs`、
`src/lib.rs`、`README.md`、`docs/module-map.md`、`CHANGELOG.md`、
`docs/plans/format-utils-template.md`、`tests/feature_matrix.rs`、
`tests/fixtures/format_feature_matrix/`、`AGENTS.md`、`CLAUDE.md`
