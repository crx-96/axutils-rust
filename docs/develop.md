# axutils 开发与验收

本文档只维护当前有效的开发命令和验收层级。公共模块与 feature 契约见
[模块与 feature 定位](module-map.md)。

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

不要为普通验证执行 `cargo clean`。需要测量冷缓存或隔离并发任务时，为命令设置独立
`CARGO_TARGET_DIR`，并只清理该任务自己创建的目录。

## 一级：快速门禁

适用于默认能力、文档小改或提交前快速反馈：

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

不得通过放宽断言、删除负向用例或开启额外 feature 来让快速门禁通过。

## 二级：领域门禁

改动一个领域时，至少验证其最小语义 feature、组合 feature 和直接集成测试。示例：

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

`http` 的树中不得出现 `reqwest`；`http-async` 才应包含它。

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

SQLx 单 driver 的依赖树不得包含另外两个 driver。`scheduler` 单 feature 必须提供完整调度 API。

### 其他领域

```bash
cargo test --no-default-features --features jwt --test jwt --test jwt_codec --test jwt_global
cargo test --no-default-features --features email --test email_live
cargo test --no-default-features --features logging --test log_global --test log_conflict
cargo test --no-default-features --features tokio,task-group --test tokio
```

`email_live` 在该命令中只运行本地配置解析测试；网络用例保持 ignored。

## 三级：完整非 live 门禁

源码、公共 API、feature、依赖或共享行为变化完成后运行：

```bash
cargo check --all-features
cargo test --lib --all-features -- --test-threads=4
cargo test --tests --all-features -- --test-threads=4
cargo clippy --all-features --all-targets -- -D warnings
cargo doc --no-deps --all-features
cargo test --doc --all-features -- --test-threads=4
```

Windows 上限制 doctest 线程数可减少并发 linker 的页面文件压力；这不改变测试集合。

### Feature/API/依赖矩阵

快速结构测试：

```bash
cargo test --no-default-features --test feature_matrix
```

完整 ignored 矩阵：

```bash
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1 --nocapture
```

矩阵使用统一 scratch fixture，并复用相同 feature/edge/invert 的 `cargo tree` 结果。它至少验证：

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

## 四级：发布前门禁

发布前在三级门禁基础上运行：

```bash
cargo package --list
cargo package --allow-dirty
git diff --check
```

`cargo package --allow-dirty` 只构建本地包，不等于发布；不要运行 `cargo publish`，除非用户明确
授权。

发布包应包含：

- `Cargo.toml`
- `README.md`
- `CHANGELOG.md`
- `LICENSE`
- `src/**`
- `docs/examples/**`

不应包含：

- `tests/**`
- `config/**`
- `AGENTS.md`
- `docs/develop.md`
- `docs/module-map.md`
- `docs/skills/**`
- `docs/plans/**`
- `docs/status/**`

根目录 `Cargo.lock` 不是 library 的依赖版本策略，不应作为发布内容提交。

## 性能测量

比较 feature 或 harness 性能时固定工具链、命令和缓存条件。不要用一次冷构建和一次热构建直接
比较。

PowerShell 示例：

```powershell
$target = Join-Path $env:TEMP "axutils-http-sync-bench"
$elapsed = Measure-Command {
    cargo check --no-default-features --features http --target-dir $target
}
$elapsed.TotalSeconds
```

至少记录：

- 工具链和目标平台；
- feature 集；
- 冷/热缓存；
- wall-clock 时间；
- Cargo 子进程或唯一依赖树调用数；
- 是否存在并发任务。

本轮 harness 的目标是：文档编译 Cargo 子进程较逐 block 模型减少至少 50%，feature matrix 的重复
Cargo/依赖树调用减少至少 30%。不得靠删除安全、边界或负向契约达成。

## Live 测试

以下测试默认 ignored：

- SMTP；
- Redis 单机；
- Redis Cluster。

只有用户明确授权、服务受控、所需本地配置存在并且对应环境变量精确为 `1` 时才可单独运行。不要
把凭据放入命令行、日志、fixture、文档或提交内容。普通“全量测试”不包含 live 测试，也不应临时
取消其 `#[ignore]`。

## 失败处理

- 先保留完整 stdout/stderr 和失败命令，区分源码问题、fixture 问题、资源不足与环境权限问题。
- feature 负向用例必须匹配目标 rustc 诊断，不能把任意编译失败当作成功。
- 页面文件或并发 linker 资源不足时，降低测试线程或使用独立 target；不要删测试或修改语义。
- 临时目录清理失败应显式报告；不得静默遗留凭据、日志或大型 target。
