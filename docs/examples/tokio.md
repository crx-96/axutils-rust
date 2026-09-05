# Tokio 工具

启用 `tokio` 只提供 `axutils::tokio` 的 runtime 工具，不会自动启用邮件、HTTP、配置、Redis、SQLx
或其他领域的异步 API。任务组需要额外启用 `task-group`。

```toml
[dependencies]
axutils = { version = "1.0", features = ["tokio"] }
```

部分公共签名保留 Tokio 原生的 `Handle`、`Runtime`、`mpsc` 和 `JoinHandle` 类型；应用若要在
签名中命名这些类型或使用其扩展 API，应直接依赖兼容的 Tokio 1.x。`task-group` 还公开
`tokio-util` 的 `CancellationToken` 语义，需要命名该类型时应直接依赖兼容的 tokio-util 0.7.x。

## runtime 与当前上下文

`TokioUtils` 只能从 `axutils::utils` 导入。普通操作使用调用方已有 runtime；只有 `build_runtime`
和 `run` 会明确创建拥有型 runtime，并且在嵌套 runtime 中返回错误。

```rust
use axutils::{
    tokio::{TokioConfig, TokioError},
    utils::TokioUtils,
};

fn run_work() -> Result<u32, TokioError> {
    TokioUtils::run(&TokioConfig::new(), async { 42 })
}
```

`spawn` 和 `spawn_blocking` 在缺少 runtime context 时返回 `TokioError::RuntimeRequired`。
`timeout` 直接使用 Tokio time driver，必须在启用了 time driver 的 runtime 中创建并 poll；缺少
runtime/time driver 时会遵循 Tokio 的 panic 语义，而不是转换成 `TokioError`。timeout 到期只会
丢弃被包装的 future，不能作为强制停止 blocking 任务的手段。

```rust,no_run
use std::time::Duration;

use axutils::{tokio::TokioError, utils::TokioUtils};

async fn bounded_wait() -> Result<(), TokioError> {
    TokioUtils::timeout(Duration::from_secs(1), async {}).await
}
```

## 任务组

`task-group` 是独立能力，适合管理一组在同一 runtime 中执行的任务；它不使其他领域的 feature
变为可用。

```toml
[dependencies]
axutils = { version = "1.0", features = ["task-group"] }
```

```rust,no_run
use std::time::Duration;

use axutils::tokio::{TokioError, TokioTaskGroup};

async fn grouped_work() -> Result<(), TokioError> {
    let group = TokioTaskGroup::new();
    let task = group.spawn(async { 42 })?;
    let _answer = task.await.expect("task does not panic");
    group.shutdown(Duration::from_secs(1)).await
}
```

应用应在其 shutdown 流程中给任务组有限的 grace period，并显式处理尚未完成或 panic 的任务。
