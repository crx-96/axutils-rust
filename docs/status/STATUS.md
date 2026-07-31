# 当前任务状态

## 目标

删除仓库内的 `.github/` GitHub Actions 工作流，并同步清理规则、开发文档、README、CHANGELOG
和任务状态中关于内置 CI 的说明。

## 范围

- `.github/workflows/ci.yml`
- `AGENTS.md`、`CLAUDE.md`、`develop.md` 和 `README.md`
- 根目录 `CHANGELOG.md`
- `docs/status/STATUS.md` 当前任务记录

## 阶段

1. [完成] 读取 `.github/` 内容、工作区状态和所有相关文档引用
2. [完成] 更新规则、开发文档、README 和 `0.1.0` CHANGELOG
3. [完成] 删除 `.github/` 目录
4. [完成] 验证目录、引用、格式和工作区差异

## 已确认问题

- `.github/` 当前只包含 `workflows/ci.yml`，没有其他发布、凭据或运行时文件。
- 删除后，仓库不再提供内置 GitHub Actions CI；验证命令仍保留在 `develop.md`，可由维护者或外部 CI 执行。
- 本次变更已记录在 `CHANGELOG.md` 的当前 `0.1.0` 条目中。

## 风险与阻塞

- 工作区原有未跟踪的 `.claude/` 目录未修改，也不纳入本次变更。
- 删除 `.github/` 后不会影响 crate 运行时源码，但仓库将失去内置的自动化跨平台检查。

## 验证记录

已通过：确认 `.github/` 不存在、文档不再保留过时的内置 CI 引用，运行
`cargo fmt --all -- --check`、`cargo package --allow-dirty --list` 和 `git diff --check`。
发布文件清单包含 `CHANGELOG.md`、`README.md`、`LICENSE`、`Cargo.toml` 和 `src/**`，不包含
`.github/` 或其他开发者文档。

## 相关路径

`.github/workflows/ci.yml`、`AGENTS.md`、`CLAUDE.md`、`develop.md`、`README.md`、`CHANGELOG.md`、
`docs/status/STATUS.md`
