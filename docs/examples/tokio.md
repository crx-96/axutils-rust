# Tokio 工具使用文档

## Feature、导出路径与责任边界

启用 `tokio` 后，下列类型同时从 crate 根与领域模块导出：

- `axutils::{TokioConfig, TokioRuntimeFlavor, TokioError, TokioShutdownReason}`
- `axutils::tokio::{TokioConfig, TokioRuntimeFlavor, TokioError, TokioShutdownReason}`
- 自由函数仅为 `axutils::tokio::wait_for_shutdown`。
- `TokioUtils` 的三条路径为 `axutils::TokioUtils`、`axutils::utils::TokioUtils`、`axutils::utils::tokio_utils::TokioUtils`。
- `TokioTaskGroup` 还要求 `tokio-util`，路径为 `axutils::TokioTaskGroup` 与 `axutils::tokio::TokioTaskGroup`。

签名中的 `tokio::runtime::{Builder, Runtime, Handle}`、`tokio::task::JoinHandle`、`tokio::sync::mpsc` 和 `tokio_util::sync::CancellationToken` 不由 axutils 重导出；调用方需要直接依赖兼容版本。普通异步 API 使用当前 runtime；只有 `build_runtime`/`run` 显式创建并拥有 runtime。`spawn_blocking` closure 一旦开始便不能强制停止。

## `TokioRuntimeFlavor`

`pub enum TokioRuntimeFlavor { MultiThread, CurrentThread }`，要求 `tokio`。`MultiThread` 使用多线程调度器；`CurrentThread` 只在驱动 runtime 的线程调度任务。枚举当前未标记 `non_exhaustive`。

```rust
use axutils::TokioRuntimeFlavor;
assert_ne!(TokioRuntimeFlavor::MultiThread, TokioRuntimeFlavor::CurrentThread);
```

## `TokioConfig`

字段私有；以下方法均要求 `tokio`，仅修改或读取本地配置，不启动线程、不访问网络。

### `TokioConfig::new`

**签名：** `pub fn new() -> Self`。无参数；返回默认配置；不返回错误。默认值为 `MultiThread`、worker 数沿用 Tokio 默认、blocking 上限 512、线程名 `axutils-runtime`、IO/time driver 开启、shutdown timeout 30 秒。

```rust
use axutils::{TokioConfig, TokioRuntimeFlavor};
let c = TokioConfig::new();
assert_eq!(c.flavor(), TokioRuntimeFlavor::MultiThread);
assert_eq!(c.worker_threads(), None);
```

### `TokioConfig::default`

**签名：** `fn default() -> TokioConfig`（`Default` trait）。返回值、feature 和副作用与 `new` 相同；不返回错误。

```rust
use axutils::TokioConfig;
assert_eq!(TokioConfig::default().max_blocking_threads(), 512);
```

### `TokioConfig::with_flavor`

**签名：** `pub fn with_flavor(self, v: TokioRuntimeFlavor) -> Self`。`v` 选择调度器；返回更新后的配置；不返回错误。`CurrentThread` 与显式 worker 数的冲突延迟到 `builder` 检查。

```rust
use axutils::{TokioConfig, TokioRuntimeFlavor};
let c = TokioConfig::new().with_flavor(TokioRuntimeFlavor::CurrentThread);
assert_eq!(c.flavor(), TokioRuntimeFlavor::CurrentThread);
```

### `TokioConfig::with_worker_threads`

**签名：** `pub fn with_worker_threads(self, v: Option<usize>) -> Result<Self, TokioError>`。`None` 使用 Tokio 默认；`Some(n)` 仅允许 `1..=1024`。越界返回 `InvalidConfig { field: "worker_threads" }`；不创建线程。

```rust
use axutils::{TokioConfig, TokioError};
assert!(matches!(TokioConfig::new().with_worker_threads(Some(0)),
    Err(TokioError::InvalidConfig { field: "worker_threads" })));
```

### `TokioConfig::with_max_blocking_threads`

**签名：** `pub fn with_max_blocking_threads(self, v: usize) -> Result<Self, TokioError>`。`v` 允许 `1..=4096`；越界返回 `InvalidConfig { field: "max_blocking_threads" }`。它只限制 blocking 线程池上限，不限制单个 closure 的时间或内存。

```rust
use axutils::TokioConfig;
assert_eq!(TokioConfig::new().with_max_blocking_threads(1).unwrap().max_blocking_threads(), 1);
```

### `TokioConfig::with_thread_name`

**签名：** `pub fn with_thread_name(self, v: Option<String>) -> Result<Self, TokioError>`。`None` 不设置名称；字符串须为 1..=64 字节且不含 NUL，否则返回 `InvalidConfig { field: "thread_name" }`。

```rust
use axutils::{TokioConfig, TokioError};
assert!(matches!(TokioConfig::new().with_thread_name(Some(String::new())),
    Err(TokioError::InvalidConfig { field: "thread_name" })));
```

### `TokioConfig::with_io_enabled`

**签名：** `pub fn with_io_enabled(self, v: bool) -> Self`。返回更新后的配置；不返回错误。关闭 IO driver 后使用依赖它的 Tokio API 可能 panic，责任由调用方承担。

```rust
use axutils::TokioConfig;
assert!(!TokioConfig::new().with_io_enabled(false).io_enabled());
```

### `TokioConfig::with_time_enabled`

**签名：** `pub fn with_time_enabled(self, v: bool) -> Self`。返回更新后的配置；不返回错误。关闭 time driver 后 timer API 可能 panic。

```rust
use axutils::TokioConfig;
assert!(!TokioConfig::new().with_time_enabled(false).time_enabled());
```

### `TokioConfig::with_shutdown_timeout`

**签名：** `pub fn with_shutdown_timeout(self, v: Duration) -> Result<Self, TokioError>`。`v` 必须大于零且不超过 300 秒，否则返回 `InvalidConfig { field: "shutdown_timeout" }`。该预算用于 `run` 销毁 runtime；超时后已开始的 blocking 工作仍可能继续。

```rust
use std::time::Duration;
use axutils::{TokioConfig, TokioError};
assert!(matches!(TokioConfig::new().with_shutdown_timeout(Duration::ZERO),
    Err(TokioError::InvalidConfig { field: "shutdown_timeout" })));
```

### `TokioConfig::flavor`

**签名：** `pub fn flavor(&self) -> TokioRuntimeFlavor`。返回调度器选择；无错误、分配或副作用。

```rust
use axutils::{TokioConfig, TokioRuntimeFlavor};
assert_eq!(TokioConfig::new().flavor(), TokioRuntimeFlavor::MultiThread);
```

### `TokioConfig::worker_threads`

**签名：** `pub fn worker_threads(&self) -> Option<usize>`。返回显式值或 `None`；不返回 Tokio 实际采用的线程数。

```rust
use axutils::TokioConfig;
assert_eq!(TokioConfig::new().with_worker_threads(Some(2)).unwrap().worker_threads(), Some(2));
```

### `TokioConfig::max_blocking_threads`

**签名：** `pub fn max_blocking_threads(&self) -> usize`。返回配置上限；无错误或副作用。

```rust
use axutils::TokioConfig;
assert_eq!(TokioConfig::new().max_blocking_threads(), 512);
```

### `TokioConfig::thread_name`

**签名：** `pub fn thread_name(&self) -> Option<&str>`。返回借用，生命周期不超过 `self`；无错误或副作用。

```rust
use axutils::TokioConfig;
assert_eq!(TokioConfig::new().thread_name(), Some("axutils-runtime"));
```

### `TokioConfig::io_enabled`

**签名：** `pub fn io_enabled(&self) -> bool`。只读取配置，不探测 runtime driver。

```rust
use axutils::TokioConfig;
assert!(TokioConfig::new().io_enabled());
```

### `TokioConfig::time_enabled`

**签名：** `pub fn time_enabled(&self) -> bool`。只读取配置，不探测 runtime driver。

```rust
use axutils::TokioConfig;
assert!(TokioConfig::new().time_enabled());
```

### `TokioConfig::shutdown_timeout`

**签名：** `pub fn shutdown_timeout(&self) -> Duration`。返回 `run` 使用的销毁预算；它不是任务 deadline。

```rust
use std::time::Duration;
use axutils::TokioConfig;
assert_eq!(TokioConfig::new().shutdown_timeout(), Duration::from_secs(30));
```

### `TokioConfig::builder`

**签名：** `pub fn builder(&self) -> Result<tokio::runtime::Builder, TokioError>`。返回已应用本配置的原生 Builder，调用方可继续设置 axutils 未封装的选项。`CurrentThread + Some(worker_threads)` 返回 `InvalidConfig { field: "worker_threads" }`；仅本地组装，不启动线程。

```rust
use axutils::{TokioConfig, TokioError, TokioRuntimeFlavor};
let c = TokioConfig::new().with_flavor(TokioRuntimeFlavor::CurrentThread)
    .with_worker_threads(Some(1)).unwrap();
assert!(matches!(c.builder(), Err(TokioError::InvalidConfig { field: "worker_threads" })));
```

## `TokioUtils`

### `TokioUtils::try_current_handle`

**签名：** `pub fn try_current_handle() -> Result<tokio::runtime::Handle, TokioError>`。返回当前 context 的 Handle；线程不在 runtime/`EnterGuard` 中时返回 `RuntimeRequired`。Handle 不延长 owner runtime 的可用承诺。

```rust
use axutils::{TokioError, TokioUtils};
assert!(matches!(TokioUtils::try_current_handle(), Err(TokioError::RuntimeRequired)));
```

### `TokioUtils::has_runtime`

**签名：** `pub fn has_runtime() -> bool`。只探测当前线程是否有 runtime context；不保证 runtime owner 会继续存活。

```rust
use axutils::TokioUtils;
assert!(!TokioUtils::has_runtime());
```

### `TokioUtils::spawn`

**签名：** `pub fn spawn<F>(f: F) -> Result<JoinHandle<F::Output>, TokioError> where F: Future + Send + 'static, F::Output: Send + 'static`。在当前 Handle 登记任务；缺少 context 返回 `RuntimeRequired`。返回原生 JoinHandle，任务 panic/取消由 await 后的 `JoinError` 表示。

```rust
use axutils::{TokioError, TokioUtils};
assert!(matches!(TokioUtils::spawn(async { 1 }), Err(TokioError::RuntimeRequired)));
```

### `TokioUtils::spawn_on`

**签名：** `pub fn spawn_on<F>(h: &Handle, f: F) -> JoinHandle<F::Output> where F: Future + Send + 'static, F::Output: Send + 'static`。使用显式 Handle，无 axutils 错误返回；Handle 对应 runtime 必须仍可接受任务。

```rust
use axutils::{TokioConfig, TokioUtils};
let rt = TokioUtils::build_runtime(&TokioConfig::new()).unwrap();
let h = TokioUtils::spawn_on(rt.handle(), async { 7 });
assert_eq!(rt.block_on(h).unwrap(), 7);
```

### `TokioUtils::spawn_blocking`

**签名：** `pub fn spawn_blocking<F, T>(f: F) -> Result<JoinHandle<T>, TokioError> where F: FnOnce() -> T + Send + 'static, T: Send + 'static`。在当前 runtime 的 blocking 池登记；缺少 context 返回 `RuntimeRequired`。取消或丢弃 JoinHandle 不能停止已开始的 closure。

```rust
use axutils::{TokioError, TokioUtils};
assert!(matches!(TokioUtils::spawn_blocking(|| 1), Err(TokioError::RuntimeRequired)));
```

### `TokioUtils::timeout`

**签名：** `pub async fn timeout<F>(d: Duration, f: F) -> Result<F::Output, TokioError> where F: Future`。在当前 runtime 的 time driver 上等待；超时返回 `Timeout` 并丢弃 future。它不保证底层外部操作已取消；time driver 关闭时 Tokio 可能 panic。

```rust
# async fn demo() {
use std::{future, time::Duration};
use axutils::{TokioError, TokioUtils};
assert!(matches!(TokioUtils::timeout(Duration::ZERO, future::pending::<()>()).await,
    Err(TokioError::Timeout)));
# }
```

### `TokioUtils::bounded_mpsc`

**签名：** `pub fn bounded_mpsc<T>(n: usize) -> Result<(mpsc::Sender<T>, mpsc::Receiver<T>), TokioError>`。容量只允许 `1..=1_000_000`；越界返回 `InvalidConfig { field: "channel_capacity" }`。返回 Tokio 有界 channel，不创建任务。

```rust
use axutils::{TokioError, TokioUtils};
assert!(matches!(TokioUtils::bounded_mpsc::<u8>(0),
    Err(TokioError::InvalidConfig { field: "channel_capacity" })));
```

### `TokioUtils::build_runtime`

**签名：** `pub fn build_runtime(c: &TokioConfig) -> Result<Runtime, TokioError>`。只允许在 runtime context 外构建拥有型 Runtime；嵌套时返回 `NestedRuntime`，配置冲突返回 `InvalidConfig`，后端构建失败返回 `RuntimeBuild(io::Error)`。调用方必须在 async context 外 drop，或消费原生有限/后台 shutdown API。

```rust
use axutils::{TokioConfig, TokioUtils};
let runtime = TokioUtils::build_runtime(&TokioConfig::new()).unwrap();
runtime.shutdown_background();
```

### `TokioUtils::run`

**签名：** `pub fn run<F>(c: &TokioConfig, f: F) -> Result<F::Output, TokioError> where F: Future`。在 context 外创建 runtime、`block_on` 一次 future，并在正常返回或 unwind 时执行配置的有限 `shutdown_timeout`；嵌套/构建错误同 `build_runtime`。future panic 在清理后继续传播；`panic=abort` 不受保证，已开始 blocking 工作仍可能存活。

```rust
use axutils::{TokioConfig, TokioUtils};
assert_eq!(TokioUtils::run(&TokioConfig::new(), async { 42 }).unwrap(), 42);
```

### `TokioUtils::wait_for_shutdown`

**签名：** `pub async fn wait_for_shutdown() -> Result<TokioShutdownReason, TokioError>`。转发领域自由函数；等待 Ctrl+C，Unix 还等待 SIGTERM。注册失败返回 `Signal(io::Error)`；会挂起至信号到达，不创建 runtime。

```rust,no_run
# async fn demo() -> Result<(), axutils::TokioError> {
let reason = axutils::TokioUtils::wait_for_shutdown().await?;
println!("shutdown: {reason:?}");
# Ok(()) }
```

## `axutils::tokio::wait_for_shutdown`

**签名：** `pub async fn wait_for_shutdown() -> Result<TokioShutdownReason, TokioError>`，要求 `tokio`。语义、错误与副作用同上；它是领域模块中的唯一公共自由函数。

```rust,no_run
# async fn demo() -> Result<(), axutils::TokioError> {
let _ = axutils::tokio::wait_for_shutdown().await?;
# Ok(()) }
```

## `TokioTaskGroup`（`tokio + tokio-util`）

该 feature 直接使用 `futures-timer` 实现独立 grace timer，因此 `TokioConfig::with_time_enabled(false)` 时 shutdown timeout 仍工作。

clone 共享同一个 TaskTracker、CancellationToken 和关闭门闩。Drop 不 close、不 cancel、也不 abort 任务。

### `TokioTaskGroup::new`

**签名：** `pub fn new() -> Self`。创建开放且没有任务的组；不返回错误，不登记任务。

```rust
use axutils::TokioTaskGroup;
let group = TokioTaskGroup::new();
assert!(!group.is_closed());
assert_eq!(group.remaining_tasks(), 0);
```

### `TokioTaskGroup::default`

**签名：** `fn default() -> TokioTaskGroup`（`Default` trait）。返回值、feature 和副作用与 `new` 相同。

```rust
use axutils::TokioTaskGroup;
assert!(!TokioTaskGroup::default().is_closed());
```

### `TokioTaskGroup::cancellation_token`

**签名：** `pub fn cancellation_token(&self) -> CancellationToken`。返回共享状态的 token clone；只广播协作式取消，任务必须自行观察它。

```rust
use axutils::TokioTaskGroup;
let group = TokioTaskGroup::new();
let token = group.cancellation_token();
group.cancel();
assert!(token.is_cancelled());
```

### `TokioTaskGroup::is_closed`

**签名：** `pub fn is_closed(&self) -> bool`。读取线性化关闭门闩；无错误。`cancel` 不会令其变为 true。

```rust
use axutils::TokioTaskGroup;
let group = TokioTaskGroup::new();
group.cancel();
assert!(!group.is_closed());
```

### `TokioTaskGroup::remaining_tasks`

**签名：** `pub fn remaining_tasks(&self) -> usize`。返回 tracker 当前观测值；不是新的同步保证，读取后数量可立即变化。

```rust
use axutils::TokioTaskGroup;
assert_eq!(TokioTaskGroup::new().remaining_tasks(), 0);
```

### `TokioTaskGroup::spawn`

**签名：** `pub fn spawn<F>(&self, f: F) -> Result<JoinHandle<F::Output>, TokioError> where F: Future + Send + 'static, F::Output: Send + 'static`。在当前 runtime 登记并跟踪任务；close 后稳定返回 `TaskGroupClosed`。未在 runtime context 中时返回 `RuntimeRequired`。

```rust
use axutils::{TokioError, TokioTaskGroup};
let group = TokioTaskGroup::new();
group.close();
assert!(matches!(group.spawn(async {}), Err(TokioError::TaskGroupClosed)));
```

### `TokioTaskGroup::spawn_blocking`

**签名：** `pub fn spawn_blocking<F, T>(&self, f: F) -> Result<JoinHandle<T>, TokioError> where F: FnOnce() -> T + Send + 'static, T: Send + 'static`。在当前 runtime blocking 池登记并跟踪；close 后返回 `TaskGroupClosed`。取消 token、shutdown 超时或丢弃 handle 都不能停止已开始的 closure。

```rust
use axutils::{TokioError, TokioTaskGroup};
let group = TokioTaskGroup::new();
group.close();
assert!(matches!(group.spawn_blocking(|| 1), Err(TokioError::TaskGroupClosed)));
```

### `TokioTaskGroup::close`

**签名：** `pub fn close(&self)`。禁止后续登记并关闭 tracker；幂等、不 cancel、不 abort。它与 spawn 在同一 mutex 临界区线性化。

```rust
use axutils::TokioTaskGroup;
let group = TokioTaskGroup::new();
group.close();
group.close();
assert!(group.is_closed());
```

### `TokioTaskGroup::cancel`

**签名：** `pub fn cancel(&self)`。广播 token；幂等、不 close、不等待、不 abort。

```rust
use axutils::TokioTaskGroup;
let group = TokioTaskGroup::new();
let token = group.cancellation_token();
group.cancel();
assert!(token.is_cancelled());
```

### `TokioTaskGroup::shutdown`

**签名：** `pub async fn shutdown(&self, grace: Duration) -> Result<(), TokioError>`。先 close、再 cancel，并在 `0..=300s` 内等待 tracker 清空；大于 300 秒返回 `InvalidConfig { field: "task_group_grace" }` 且在此错误分支不会 close/cancel。预算耗尽返回 `TaskGroupShutdownTimeout { remaining_tasks }`。`grace = 0` 合法；该 deadline 只覆盖任务组等待，不强制终止任务。

```rust
# async fn demo() {
use std::time::Duration;
use axutils::{TokioError, TokioTaskGroup};
let group = TokioTaskGroup::new();
assert!(matches!(group.shutdown(Duration::from_secs(301)).await,
    Err(TokioError::InvalidConfig { field: "task_group_grace" })));
assert!(!group.is_closed());
# }
```

## `TokioShutdownReason`

`#[non_exhaustive] pub enum TokioShutdownReason`，要求 `tokio`：

- `CtrlC`：所有平台的 Ctrl+C。
- `SigTerm`：仅 Unix 编译目标存在，表示 SIGTERM。

调用方 match 必须保留 wildcard；无公开字段。

```rust
use axutils::TokioShutdownReason;
let reason = TokioShutdownReason::CtrlC;
assert!(matches!(reason, TokioShutdownReason::CtrlC));
```

## `TokioError`

`#[non_exhaustive] pub enum TokioError`，要求 `tokio`；调用方 match 必须保留 wildcard。公开变体与字段为：

- `InvalidConfig { field: &'static str }`
- `RuntimeRequired`
- `NestedRuntime`
- `RuntimeBuild(std::io::Error)`
- `Join(tokio::task::JoinError)`
- `Timeout`
- `Signal(std::io::Error)`
- `TaskGroupClosed`（产生它的任务组 API 还要求 `tokio-util`）
- `TaskGroupShutdownTimeout { remaining_tasks: usize }`（同上）

`RuntimeBuild`、`Join`、`Signal` 通过 `Error::source` 暴露底层错误；Display 不回显任务 payload。

```rust
use axutils::TokioError;
let error = TokioError::InvalidConfig { field: "worker_threads" };
assert!(error.to_string().contains("worker_threads"));
```
