# axutils 项目协作规则

## 项目定位

这是一个 Rust library crate，包名和 crate 名均为 `axutils`。公共 API 位于
`src/lib.rs` 及其 feature 模块中。当前 `TimeUtils` 仅依赖标准库，属于默认能力；
`RegUtils` 依赖第三方 `regex` crate，仅通过显式启用的 `regex` feature 提供。

## 修改约定

- 默认使用中文编写说明、审查意见、提交信息和文档；代码中的 Rust 标识符遵循 Rust 命名规范。
- 修改前先读取相关源码、测试和 Cargo 配置，保持最小变更，不顺带重构无关内容。
- 新增公共方法时，必须同时添加 API doc、`# Examples` doctest 和覆盖正常/边界情况的测试。
- 新增需要第三方包的能力时，依赖必须标记为 `optional = true`，并优先使用与依赖名一致的 feature，通过 `dep:<dependency-name>` 映射。
- 不依赖第三方包的方法属于默认能力，不添加 feature 守卫，直接从 crate 根模块默认导出。
- feature 守卫、模块路径、README 示例和开发者文档必须保持一致；默认 feature 只包含不依赖第三方包的能力，当前正则能力统一使用 `regex` feature。

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

`Cargo.toml` 的 `package.include` 是发布包白名单。`README.md`、`LICENSE`、`Cargo.toml`
和 `src/**` 可以随包发布；`develop.md`、本文件和 `docs/skills/**` 仅供仓库开发使用，
不要调整白名单将它们带入 crates.io。

## 项目专属 skill

当前任务不需要额外的项目专属 skill，因此暂不创建 `docs/skills/`。如果后续确实需要
可复用的项目工作流，应在 `docs/skills/<skill-name>/SKILL.md` 创建说明，并在本文件中
加入对应链接后再使用。
