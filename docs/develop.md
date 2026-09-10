# axutils 开发与验收

本文档维护开发命令和验证范围选择。公共模块与 feature 契约见
[模块与 feature 定位](module-map.md)。

下面按验证目的分组，可按改动影响组合使用，不需要逐级执行。选择能检验原问题、受影响契约
和主要失败路径的检查；复杂或高风险变化增加相应证据，局部变化优先复用现有测试。
已有结果与当前代码、feature 和环境一致时可复用；新改动、失败或未解决疑点再触发补充验证。

规范入口见 [AGENTS.md](../AGENTS.md)，库级判断依据见
[Rust library 审查 Skill](skills/review-rust-library-change/SKILL.md)。以下命令提供不同范围的证据，
不替代对公共契约和实际行为的审查。

## 环境

- Rust / Cargo：1.95（项目 MSRV）
- Edition：2021
- 默认 feature：空
- 工作目录：仓库根目录

确认环境：

```bash
rustc --version
cargo --version
```

普通验证复用现有构建缓存。测量冷缓存或隔离并发任务时可设置独立 `CARGO_TARGET_DIR`；清理范围
限于本任务创建且已无用途的目录，保留用户进程和其他任务的有效缓存。

## 快速检查

适用于默认能力、可执行文档示例变化或提交前快速反馈。纯规则、措辞和格式维护通常检查文档结构、
链接及 `git diff --check` 即可；涉及库契约判断时结合项目审查 Skill 的对应主题选择验证范围。

```bash
cargo fmt --all -- --check
cargo check --lib --no-default-features
cargo test --no-default-features
cargo clippy --no-default-features --all-targets -- -D warnings
git diff --check
```

默认依赖树应只包含本 crate：

```bash
cargo tree --no-default-features --edges normal,build
```

断言与负向用例的维护依据见项目 Skill 的实施与迁移章节，检查失败时先查明契约与实现的差异。

## 领域验证

改动一个领域时，按影响选择最小语义 feature、相关组合和直接集成测试。下面是各领域可用命令，
选用与改动有关的部分；局部用例不足以证明行为或隔离关系时再扩大。

### FS 与 Config

```bash
cargo check --no-default-features --features fs-async
cargo check --no-default-features --features fs-temp
cargo check --no-default-features --features fs-temp-async
cargo test --no-default-features --features fs-async,fs-temp,fs-temp-async --test fs

cargo check --no-default-features --features config
cargo check --no-default-features --features config-yaml
cargo check --no-default-features --features config-toml
cargo check --no-default-features --features config-ini
cargo test --no-default-features --features config-async,config-yaml,config-toml,config-ini --test config
```

### HTTP

```bash
cargo test --no-default-features --features http --test http --test http_tls --test http_global
cargo test --no-default-features --features http-json --test http_serde
cargo test --no-default-features --features http-async,http-json --test http --test http_serde
cargo tree --no-default-features --features http --edges normal,build
cargo tree --no-default-features --features http-async --edges normal,build
```

依赖树预期：同步 `http` 不含 `reqwest`，`http-async` 包含它。

下游压缩 feature 合并的行为回归使用独立 fixture，分别验证未启用压缩和启用所有 provider 压缩
feature 的组合；仅访问测试自身启动的 loopback server。首次运行需准备 fixture 的依赖缓存：

```bash
cargo fetch --manifest-path tests/fixtures/http_compression/Cargo.toml
cargo test --no-default-features --test feature_matrix http_downstream_compression_contract -- --ignored --test-threads=1 --nocapture
```

同步入口验证 ureq 的下游解压边界；异步入口验证原始字节与编码 Header 保持不变。该用例也纳入
完整 ignored 矩阵。

### Redis

```bash
cargo test --no-default-features --features redis --test redis --test redis_global --test redis_serde
cargo test --no-default-features --features redis-cluster --test redis_cluster --test redis_global_cluster
cargo test --no-default-features --features redis-async --test redis_global_async
cargo test --no-default-features --features redis-cluster-async --test redis_global_cluster_async
```

上述命令不会执行 ignored 的真实服务测试。

### SQLx、Scheduler 与 Axum

```bash
cargo check --no-default-features --features sqlx-postgres
cargo check --no-default-features --features sqlx-mysql
cargo test --no-default-features --features sqlx-sqlite --test sqlx

cargo test --no-default-features --features scheduler --test scheduler --test scheduler_global
cargo test --no-default-features --features axum --test axum
cargo test --no-default-features --features axum-governor --test axum
```

依赖树预期：SQLx 单 driver 与另外两个 driver 隔离；`scheduler` 单 feature 提供完整调度 API。

### 其他领域

```bash
cargo test --no-default-features --features jwt --test jwt --test jwt_codec --test jwt_global
cargo test --no-default-features --features email --test email_live
cargo test --no-default-features --features logging --test log_global --test log_conflict
cargo test --no-default-features --features tokio,task-group --test tokio
```

`email_live` 在该命令中只运行本地配置解析测试；网络用例保持 ignored。

## 完整非 live 验证

共享实现、跨模块依赖、公共路径迁移、feature 组合或发布级变化可能扩大回归面，按影响考虑本节
检查。影响广、边界难以收敛、准备发布或用户要求全量验收时，采用快速检查及本节完整集合，
覆盖 feature matrix 和 Markdown 示例。能明确限定影响的公共 API 或 feature 小改动，可先验证
相关正负 fixture、组合、依赖树与文档；证据有缺口时补充对应检查，按实际范围报告。

```bash
cargo check --all-features
cargo test --lib --all-features -- --test-threads=4
cargo test --tests --all-features -- --test-threads=4
cargo clippy --all-features --all-targets -- -D warnings
cargo doc --no-deps --all-features
cargo test --doc --all-features -- --test-threads=4
```

示例中的线程数用于缓解 Windows 并发 linker 的页面文件压力，可按资源和隔离需求调整，
测试集合保持一致。

### Feature/API/依赖矩阵

快速结构测试：

```bash
cargo test --no-default-features --test feature_matrix
```

完整 ignored 矩阵：

```bash
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1 --nocapture
```

矩阵使用统一 scratch fixture，并复用相同 feature/edge/invert 的 `cargo tree` 结果。覆盖范围包括：

- 默认正常依赖为空；
- 每个独立 feature 有对应 API；
- `tokio` 不开放其他领域异步 API；
- HTTP、Redis、SQLx、FS、Config、Axum 的分层；
- Scheduler 单 feature；
- canonical path 正向和旧根/公开叶路径负向；
- provider-only feature 与 allocator feature 已删除；
- 时间无后缀 API 已删除。

### Markdown 示例

枚举和 metadata 完整性：

```bash
cargo test --no-default-features --test docs_examples
```

编译全部示例：

```bash
cargo test --no-default-features --test docs_examples -- --ignored --test-threads=1 --nocapture
```

局部排查可设置：

```bash
AXUTILS_DOCS_EXAMPLE_FILTER=docs/examples/http.md \
  cargo test --no-default-features --test docs_examples -- --ignored --nocapture
```

PowerShell：

```powershell
$env:AXUTILS_DOCS_EXAMPLE_FILTER = "docs/examples/http.md"
cargo test --no-default-features --test docs_examples -- --ignored --nocapture
Remove-Item Env:AXUTILS_DOCS_EXAMPLE_FILTER
```

正向代码块按“axutils feature + 完整直接依赖语义”分组，一个 scratch crate 使用多个 bin 一次
检查；组失败后才逐 bin 回退。`compile_fail` 用例保持独立并匹配稳定诊断。

## 发布前检查

准备发布时，在完整非 live 验证的基础上检查本地包；仅调整发布清单等元数据时，可先运行相关
清单或打包检查，再依据影响判断其余范围：

```bash
cargo package --list
cargo package --allow-dirty
git diff --check
```

`cargo package --allow-dirty` 构建本地包；`cargo publish` 的真实发布操作按 AGENTS 的已有授权
边界处理。本地检查通过提供质量证据，不增加发布授权。

发布包应包含：

- `Cargo.toml`
- `README.md`
- `CHANGELOG.md`
- `LICENSE`
- `src/**`
- `docs/examples/**`

开发内容留在仓库，发布白名单排除：

- `tests/**`
- `config/**`
- `AGENTS.md`
- `docs/develop.md`
- `docs/module-map.md`
- `docs/skills/**`
- `docs/plans/**`
- `docs/status/**`

根目录 `Cargo.lock` 的提交约定见 AGENTS。

## 规范审查的补充检查

公共 API 文档维护或项目规范审查时，可用以下命令检查缺失说明和 Rustdoc 警告，并单独运行示例：

```bash
cargo rustdoc --no-default-features --lib -- -D warnings -W missing_docs
cargo rustdoc --all-features --lib -- -D warnings -W missing_docs
cargo test --doc --all-features -- --test-threads=4
```

这是补充诊断入口，不修改 manifest 的 lint 等级。结合 Rustdoc 渲染结果检查示例是否显示核心调用，
并核对关键断言是否实际运行；编译成功、`no_run` 和运行成功分别报告。

本地已有依赖缓存时可附加 `--offline`，此时结果只证明当前已解析依赖集；缺少缓存属于环境缺口。
检查 MSRV 时核对实际 `rustc --version`，验证平台也按实际环境记录。

涉及第三方 feature 合并时，在独立下游 fixture 中同时依赖 `axutils` 和相关 provider，并额外启用
会改变行为的 provider feature。现有 `--all-features` 仅覆盖本 crate 声明的组合，不能证明所有下游
组合都已验证。真实外部访问继续遵循 Live 测试条件，本地复现优先使用 loopback 或内存 fixture。

规范审查按问题报告触发条件、影响、对应条款、位置和证据；没有运行的矩阵、平台分支或 live
场景列为未验证，不因工具返回成功或用例被 ignored 而视为通过。

## 性能测量

比较 feature 或 harness 性能时对齐工具链、命令和缓存条件，区分冷构建与热构建，确保结果
具有可比性。

PowerShell 示例：

```powershell
$target = Join-Path $env:TEMP "axutils-http-sync-bench"
$elapsed = Measure-Command {
    cargo check --no-default-features --features http --target-dir $target
}
$elapsed.TotalSeconds
```

根据比较目的记录：

- 工具链和目标平台；
- feature 集；
- 冷/热缓存；
- wall-clock 时间；
- Cargo 子进程或唯一依赖树调用数；
- 是否存在并发任务。

性能目标按当前任务与可复核基线确定；提升以保留安全、边界和负向契约为前提，历史任务的降幅
作为参考，不直接用作当前验收目标。

## Live 测试

以下测试默认 ignored：

- SMTP；
- Redis 单机；
- Redis Cluster。

运行 live 场景时核对用户对此服务与操作的明确授权、受控服务、被忽略的本地配置，以及值为 `1`
的一次性 opt-in 环境变量，然后单独执行；这些条件作用于真实外部访问，普通“全量测试”使用
非 live 集合，保持 live 用例的 `#[ignore]`。敏感信息按 AGENTS 脱敏处理，缺配置或未运行时如实
记录状态，不把跳过记为成功。

## 失败处理

- 保留诊断所需的 stdout/stderr 和失败命令，并按 AGENTS 处理敏感内容；区分源码问题、fixture
  问题、资源不足与环境权限问题，依据原因选择下一步。
- feature 负向用例以目标 rustc 诊断作为预期失败证据，其他编译失败继续排查。
- 页面文件或并发 linker 资源不足时可降低测试线程或使用独立 target，保留测试集合与语义。
- 临时目录清理失败时说明遗留内容和位置，妥善处理本任务的凭据、日志或大型 target。
