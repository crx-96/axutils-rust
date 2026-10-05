# Tokio 工具

启用 `tokio` 提供 `axutils::tokio` 的 runtime 工具和单任务取消守卫，不会自动启用邮件、HTTP、配置、Redis、SQLx
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

## 单任务取消守卫

`TokioTaskGuard<T = ()>` 在 `tokio` feature 下可用，接管已有 `JoinHandle<T>`，不启动任务或
创建 runtime。其 `Drop` 对任务发出 `abort` 请求；放入 `Arc` 后，最后一个强引用释放时才
触发取消。任务结果 `T` 可以是任意类型，但守卫不读取结果。已完成的结果保留在句柄中，
释放守卫时一并丢弃；任务尚未完成时，结果及任务资源的清理由 Tokio 后续执行。

```rust
use std::{future::pending, sync::Arc};

use axutils::{
    tokio::{TokioConfig, TokioTaskGuard},
    utils::TokioUtils,
};

TokioUtils::run(&TokioConfig::new(), async {
    let task = TokioUtils::spawn(pending::<u32>()).unwrap();
    let owner = Arc::new(TokioTaskGuard::new(task));
    let last_owner = Arc::clone(&owner);
    drop(owner);
    assert!(!last_owner.is_finished());
    drop(last_owner); // 请求取消，不等待任务实际退出。
}).unwrap();
```

`is_finished` 观察任务是否实际退出，不判断成功、失败或取消；发出取消请求后仍可能返回
`false`。若需要在所有者释放后观察任务完成，可在交出 `JoinHandle` 前保留其 `abort_handle()`。
完成状态不保证结果中的资源已释放，确认特定资源清理时应让任务或结果提供释放通知。
需要任务结果时应直接管理 `JoinHandle`。

取消不会撤销已经发出的外部操作，不能强制终止已经开始的 `spawn_blocking` closure；普通
异步任务也需要 runtime 再次调度才能处理取消。任务守卫的释放语义独立于 `TokioTaskGroup`：
任务组仍保持 `Drop` 不 abort、显式协作取消和有界等待的原有契约。
