# Scheduler 调度器使用文档

## Feature、导出路径与职责边界

调度器 API 只在同时启用 `chrono`、`chrono_tz`、`tokio` 和 `croner` 四项 feature 时存在：

```toml
[dependencies]
axutils = { version = "0.1.0", features = ["chrono", "chrono_tz", "tokio", "croner"] }
tokio = { version = "1.53.1", features = ["rt-multi-thread", "time", "macros"] }
```

领域类型支持以下全部公开路径：

- 推荐的 crate 根路径：`axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskId, TaskSchedule}`；
- 领域模块路径：`axutils::scheduler::{Scheduler, SchedulerConfig, SchedulerError, TaskId, TaskSchedule}`；
- 全局 facade：`axutils::SchedulerUtils`、`axutils::utils::SchedulerUtils` 和
  `axutils::utils::scheduler_utils::SchedulerUtils`。

领域类型不从 `axutils::utils` 重导出，也不存在 `axutils::scheduler_utils` 根模块别名。
公共签名只使用 `std::time::Duration` 和 axutils 自有类型；`chrono`、`chrono-tz`、
`croner` 与 Tokio `JoinHandle` 都是内部实现细节，不由 axutils 重导出。

`Scheduler` 负责进程内、非持久化的一次、固定间隔和 IANA 时区 cron 触发。它不创建
Tokio runtime，不调用 `block_on`，不安装 signal handler，也不提供持久化、失败重试、
callback 超时、任务结果或跨进程去重。callback 的业务错误、阻塞工作、内部 `spawn`、
重试和资源限额由调用方管理。

## 运行时与通用执行语义

`Scheduler::new`、`SchedulerUtils::init`、`cancel` 和 `shutdown` 不需要 runtime。
`register` 必须在当前线程已进入、且开启了 time driver 的 Tokio runtime 中调用；仅有
runtime 但未调用 `enable_time`/`enable_all` 也会返回 `SchedulerError::RuntimeRequired`。
返回错误时不会执行 callback，也不会占用活动任务容量。current-thread 和 multi-thread
runtime 都可使用；调用方必须让 runtime 存活到任务完成或被关闭。

同一任务的 callback 总是串行执行。`Interval` 使用 monotonic timer，首次执行在一个完整
period 之后，并使用 `MissedTickBehavior::Skip`；callback 太慢时会跳过错过的 tick，
不会并行追赶。`Cron` 在每次 callback 完成后重新按当前墙上时间计算下一次
occurrence，也不会积压无界触发。timer 不是实时保证，实际执行可能晚于目标时刻。

## `SchedulerConfig`

`pub struct SchedulerConfig { pub max_tasks: usize }`，实现 `Clone`、`Debug`、`Eq` 和
`PartialEq`。`max_tasks` 只限制该调度器注册表中的活动任务，不限制 callback 自行
启动的工作。一次性任务正常完成、callback panic、任务取消或调度器关闭后，对应的
活动记录会被移除。长时间 callback 会在其存活期内占用一个槽位。

### `SchedulerConfig::new`

**签名：** `pub fn new(max_tasks: usize) -> Result<Self, SchedulerError>`。允许范围是
`1..=4096`；越界返回 `InvalidConfig { field: "max_tasks" }`，不创建 runtime 或后台任务。

```rust
use axutils::{SchedulerConfig, SchedulerError};

let config = SchedulerConfig::new(32)?;
assert_eq!(config.max_tasks, 32);
assert!(matches!(
    SchedulerConfig::new(0),
    Err(SchedulerError::InvalidConfig { field: "max_tasks" })
));
# Ok::<(), SchedulerError>(())
```

### `SchedulerConfig::default`

**签名：** `fn default() -> SchedulerConfig`（`Default` trait）。默认 `max_tasks` 为 `256`，
不返回错误。字段公开，因此即使调用方用结构体字面量绕过 `new`，
`Scheduler::new` 仍会再次校验。

```rust
use axutils::{Scheduler, SchedulerConfig, SchedulerError};

assert_eq!(SchedulerConfig::default().max_tasks, 256);
assert!(matches!(
    Scheduler::new(SchedulerConfig { max_tasks: 4097 }),
    Err(SchedulerError::InvalidConfig { field: "max_tasks" })
));
```

## `TaskSchedule`

`TaskSchedule` 实现 `Clone`、`Debug`、`Eq` 和 `PartialEq`，且完整公开三个拥有型变体：

- `Once(Duration)`：指定延迟后异步执行一次；`Duration::ZERO` 合法；
- `Interval(Duration)`：基于 monotonic timer 的固定间隔；零间隔在注册时拒绝；
- `Cron { expression: String, timezone: String }`：按六段秒级 cron 和显式 IANA 时区运行。

三个构造方法只拥有输入，不解析 cron，也不检查 runtime；调度参数在 `register`
中校验。公开变体可直接构造，语义与对应构造方法相同。

### `TaskSchedule::once`

**签名：** `pub fn once(delay: Duration) -> Self`。`delay` 是执行前的经过时间；零值会创建
“尽快异步执行”的任务，不在注册函数内同步调用 callback。

```rust
use std::time::Duration;
use axutils::TaskSchedule;

assert_eq!(TaskSchedule::once(Duration::from_secs(2)), TaskSchedule::Once(Duration::from_secs(2)));
assert!(matches!(TaskSchedule::once(Duration::ZERO), TaskSchedule::Once(value) if value.is_zero()));
```

### `TaskSchedule::interval`

**签名：** `pub fn interval(period: Duration) -> Self`。构造时允许任意 `Duration`；
`Duration::ZERO` 会在 `register` 时返回 `InvalidSchedule`。

```rust
use std::time::Duration;
use axutils::TaskSchedule;

assert_eq!(
    TaskSchedule::interval(Duration::from_secs(5)),
    TaskSchedule::Interval(Duration::from_secs(5))
);
let deferred_error = TaskSchedule::interval(Duration::ZERO);
assert!(matches!(deferred_error, TaskSchedule::Interval(value) if value.is_zero()));
```

### `TaskSchedule::cron`

**签名：** `pub fn cron(expression: impl Into<String>, timezone: impl Into<String>) -> Self`。
构造时只转换为拥有的 `String`；表达式、时区和第一个未来 occurrence 延迟到注册时校验。

```rust
use axutils::TaskSchedule;

let schedule = TaskSchedule::cron("0 0 9 * * *", "Asia/Shanghai");
assert_eq!(schedule, TaskSchedule::Cron {
    expression: "0 0 9 * * *".to_owned(),
    timezone: "Asia/Shanghai".to_owned(),
});

// 边界输入也只会被拥有；错误在 register 中返回。
assert!(matches!(TaskSchedule::cron("@hourly", "Not/AZone"), TaskSchedule::Cron { .. }));
```

## Cron 表达式、长度与 DST

cron 字段顺序固定为 `秒 分 时 日 月 周`，必须恰好有六个空白分隔字段。语义为
POSIX/Vixie weekday，DOM（月中日）和 DOW（周中日）同时受限时按 **OR** 解释。
支持 Croner POSIX 解析器接受的列表、范围、步进以及 `MON`/`JAN` 等标准名称；
不支持五段、七段、nickname，也拒绝 `L`、`W`、`#`、`+`、`?`、`@` 扩展。

cron 表达式最多 `256` 字节，时区字符串最多 `128` 字节；超限分别返回
`InvalidCron` 和 `InvalidTimezone`。时区必须是 `UTC`、`Asia/Shanghai`、
`America/New_York` 等 IANA 标识符，不是 `+08:00` 一类固定偏移。注册时会同步解析并搜索
第一个严格晚于当前时间的秒对齐 occurrence；无可表示的未来时间也返回 `InvalidCron`。
`Once(Duration::ZERO)` 是明确的立即异步触发入口。

时区只影响 `Cron`。计算会把当前 UTC 时间转到任务时区，在本地时间上寻找下次 occurrence，
再转回 UTC 计算 sleep。遇到 spring-forward gap 时，固定时刻任务在当天 gap 后第一个
有效时刻执行；遇到 fall-back overlap 时，同一重复墙上时刻只执行一次，不额外补跑
secondary occurrence。已创建的 sleep 不会因系统墙上时钟跳变自动改期；完成 callback 后
才按新的当前时间重算。时区数据库编译进 `chrono-tz` 依赖，不在运行期动态下载更新。

## `TaskId`

`TaskId` 是字段私有的不透明标识，实现 `Clone`、`Copy`、`Debug`、`Eq`、`PartialEq`
和 `Hash`，因此可作为 `HashMap`/`HashSet` key。它由每个 `Scheduler` 单调分配，完成后
不在该实例内复用；不是跨调度器或跨进程的 UUID，应与产生它的调度器一起使用。
内部计数器耗尽时，新注册返回 `TaskLimitExceeded`，不复用旧 ID。

```rust
use std::collections::HashSet;
use axutils::TaskId;

fn traits(id: TaskId) {
    let copied = id;
    let cloned = id.clone();
    assert_eq!(copied, cloned);
    let mut ids = HashSet::new();
    ids.insert(id);
    assert!(ids.contains(&id));
    let _debug = format!("{id:?}");
}
# let _ = traits as fn(TaskId);
```

## `SchedulerError`

`SchedulerError` 实现 `Clone`、`Copy`、`Debug`、`Eq`、`PartialEq`、`Display` 和
`std::error::Error`；当前错误没有 `source`。枚举标记了 `#[non_exhaustive]`，在 crate 外
`match` 时必须保留 wildcard：

```rust
use axutils::SchedulerError;

fn classify(error: SchedulerError) -> &'static str {
    match error {
        SchedulerError::RuntimeRequired => "runtime",
        SchedulerError::Shutdown => "shutdown",
        _ => "other", // non_exhaustive 枚举必须保留 wildcard
    }
}
# assert_eq!(classify(SchedulerError::RuntimeRequired), "runtime");
```

当前九个变体及语义如下：

| 变体 | 产生条件 | `Display` |
| --- | --- | --- |
| `InvalidConfig { field: &'static str }` | `max_tasks` 不在 `1..=4096` | `invalid scheduler configuration: {field}` |
| `InvalidSchedule` | `Interval` 的 period 为零 | `invalid task schedule` |
| `InvalidCron` | cron 字段、语法、长度、未来 occurrence 或时间转换无效 | `invalid cron schedule` |
| `InvalidTimezone` | IANA 时区无效或超过 128 字节 | `invalid IANA timezone` |
| `RuntimeRequired` | 当前无 Tokio runtime 或未开启 time driver | `a Tokio runtime with an enabled time driver is required` |
| `AlreadyInitialized` | 全局 `SchedulerUtils` 已成功初始化 | `scheduler is already initialized` |
| `NotInitialized` | 在全局初始化前调用转发方法 | `scheduler is not initialized` |
| `TaskLimitExceeded` | 活动任务达到 `max_tasks` 或 ID 计数器耗尽 | `scheduler task limit exceeded` |
| `Shutdown` | 对已关闭实例注册新任务 | `scheduler is shut down` |

错误文本是脱敏分类，不回显 cron、时区原文、任务 ID、callback 值或上游错误。
`InvalidConfig` 仅携带静态字段名，其余当前变体都是 unit variant。

## `Scheduler`

`Scheduler` 拥有独立生命周期，可安全地 `Send + Sync`，但不实现 `Clone`。需跨组件共享时由
调用方使用 `Arc<Scheduler>`。

### `Scheduler::new`

**签名：** `pub fn new(config: SchedulerConfig) -> Result<Self, SchedulerError>`。再次校验
`max_tasks`，然后只构造内存状态；不需要当前 runtime，不启动任务，不访问网络或文件。

```rust
use axutils::{Scheduler, SchedulerConfig, SchedulerError};

let scheduler = Scheduler::new(SchedulerConfig::new(8)?)?;
assert!(matches!(
    Scheduler::new(SchedulerConfig { max_tasks: 0 }),
    Err(SchedulerError::InvalidConfig { field: "max_tasks" })
));
scheduler.shutdown()?;
# Ok::<(), SchedulerError>(())
```

### `Scheduler::register`

**签名：**

```text
pub fn register<F, Fut>(&self, schedule: TaskSchedule, callback: F)
    -> Result<TaskId, SchedulerError>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static
```

注册不同步调用 callback；成功时立即在当前 runtime 的 handle 上 spawn 任务并返回 ID。
callback 可通过 `move` 捕获拥有值；因为签名是 `Fn`，重复任务应在每次调用中 clone
所需句柄。callback 返回 `()`，业务错误必须在 callback 内处理。

错误顺序是：已关闭时先返回 `Shutdown`；否则校验 schedule、cron、时区和首个
occurrence；再检查 runtime/time driver；最后再检查关闭竞态、容量和 ID。任一错误都不会
留下半注册任务。

```rust,no_run
use std::{sync::Arc, time::Duration};
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
let scheduler = Scheduler::new(SchedulerConfig::default())?;
let state = Arc::new(String::from("snapshot"));
let task_state = Arc::clone(&state);
let id = scheduler.register(TaskSchedule::interval(Duration::from_secs(60)), move || {
    let state = Arc::clone(&task_state);
    async move {
        let _ = state; // 在此处执行应用工作并处理业务错误
    }
})?;
assert!(scheduler.cancel(id)?);

// 边界：零 interval 在注册时返回错误。
assert!(matches!(
    scheduler.register(TaskSchedule::interval(Duration::ZERO), || async {}),
    Err(SchedulerError::InvalidSchedule)
));
# Ok(())
# }
```

### `Scheduler::cancel`

**签名：** `pub fn cancel(&self, task_id: TaskId) -> Result<bool, SchedulerError>`。找到活动记录时
先从注册表移除，然后发出 Tokio abort 请求并返回 `Ok(true)`；已完成、已取消或未知
ID 返回 `Ok(false)`。当前实例方法保留 `Result` 是为了统一契约，实现不产生错误。

`Ok(true)` 只表示记录已移除且取消请求已发出，不会等待 callback。已经越过一个
`await` 取消点的 callback 可能正常完成；阻塞线程或不让出控制权的 CPU 循环不能被安全强制停止。

```rust,no_run
use std::time::Duration;
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
let scheduler = Scheduler::new(SchedulerConfig::default())?;
let id = scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
assert!(scheduler.cancel(id)?);
assert!(!scheduler.cancel(id)?); // 边界：重复取消
# Ok(())
# }
```

### `Scheduler::shutdown`

**签名：** `pub fn shutdown(&self) -> Result<(), SchedulerError>`。非阻塞地标记关闭、移除所有
活动记录并发出 abort 请求。方法幂等，重复调用成功；关闭后新注册返回
`Shutdown`，但 `cancel` 已移除的 ID 仍返回 `Ok(false)`。关闭不等待 callback，不需要 runtime。

```rust
use std::time::Duration;
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

let scheduler = Scheduler::new(SchedulerConfig::default())?;
scheduler.shutdown()?;
scheduler.shutdown()?; // 幂等
assert!(matches!(
    scheduler.register(TaskSchedule::once(Duration::ZERO), || async {}),
    Err(SchedulerError::Shutdown)
));
# Ok::<(), SchedulerError>(())
```

### `Scheduler::drop`

`Drop` 不是可直接调用的公共方法，但它是重要生命周期契约。丢弃实例会复用
`shutdown` 协议，非阻塞地取消全部活动任务。后台任务只弱引用调度器状态，不会因自身未结束
而延长 `Scheduler` 生命周期。显式 `shutdown` 仍是推荐的应用退出路径，因为它使关闭时点
和错误处理更清晰。

```rust,no_run
use std::time::Duration;
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
{
    let scheduler = Scheduler::new(SchedulerConfig::default())?;
    let _id = scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
} // 边界：离开作用域会发出取消请求，但不等待 callback。
# Ok(())
# }
```

## `SchedulerUtils`

`SchedulerUtils` 是基于进程级 `OnceLock<Scheduler>` 的全局便捷入口。成功的首次初始化
永久占用全局位置；不提供 getter、reset 或 replace。需要多实例、可替换配置、测试隔离或
可控生命周期时，应直接持有 `Scheduler`。全局状态会跨同一进程的测试保留，测试不应假定
能够重置。

### `SchedulerUtils::init`

**签名：** `pub fn init(config: SchedulerConfig) -> Result<(), SchedulerError>`。先校验配置，
再竞争写入 `OnceLock`；无效配置返回 `InvalidConfig` 且不消耗初始化机会。成功后再调用
返回 `AlreadyInitialized`。初始化不创建任务，因此不需要 runtime。

```rust,no_run
use axutils::{SchedulerConfig, SchedulerError, SchedulerUtils};

assert!(matches!(
    SchedulerUtils::init(SchedulerConfig { max_tasks: 0 }),
    Err(SchedulerError::InvalidConfig { field: "max_tasks" })
));
SchedulerUtils::init(SchedulerConfig::default())?;
assert!(matches!(
    SchedulerUtils::init(SchedulerConfig::default()),
    Err(SchedulerError::AlreadyInitialized)
));
# Ok::<(), SchedulerError>(())
```

### `SchedulerUtils::is_initialized`

**签名：** `pub fn is_initialized() -> bool`。只表示全局位置是否曾成功写入；
`shutdown` 后仍返回 `true`，它不表示调度器仍接受新任务。

```rust,no_run
use axutils::{SchedulerConfig, SchedulerError, SchedulerUtils};

let before = SchedulerUtils::is_initialized();
if !before {
    SchedulerUtils::init(SchedulerConfig::default())?;
}
assert!(SchedulerUtils::is_initialized());
SchedulerUtils::shutdown()?;
assert!(SchedulerUtils::is_initialized()); // 边界：关闭不重置 OnceLock
# Ok::<(), SchedulerError>(())
```

### `SchedulerUtils::register`

**签名：**

```text
pub fn register<F, Fut>(schedule: TaskSchedule, callback: F)
    -> Result<TaskId, SchedulerError>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static
```

转发到全局实例，未初始化先返回 `NotInitialized`；否则与 `Scheduler::register` 共享
完整的 schedule、runtime、容量、关闭和 callback 语义。

```rust,no_run
use std::time::Duration;
use axutils::{SchedulerConfig, SchedulerError, SchedulerUtils, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
// 边界：若全局入口尚未初始化，这个调用返回 NotInitialized。
let _before_init = SchedulerUtils::register(TaskSchedule::once(Duration::ZERO), || async {});

if !SchedulerUtils::is_initialized() {
    SchedulerUtils::init(SchedulerConfig::default())?;
}
let id = SchedulerUtils::register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
let _ = SchedulerUtils::cancel(id)?;
# Ok(())
# }
```

### `SchedulerUtils::cancel`

**签名：** `pub fn cancel(task_id: TaskId) -> Result<bool, SchedulerError>`。未初始化返回
`NotInitialized`；否则与实例 `cancel` 一样，活动 ID 返回 `true`，已移除或未知 ID 返回
`false`，且只发出非阻塞取消请求。

```rust,no_run
use std::time::Duration;
use axutils::{SchedulerConfig, SchedulerError, SchedulerUtils, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
if !SchedulerUtils::is_initialized() {
    SchedulerUtils::init(SchedulerConfig::default())?;
}
let id = SchedulerUtils::register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
assert!(SchedulerUtils::cancel(id)?);
assert!(!SchedulerUtils::cancel(id)?); // 边界：重复取消
# Ok(())
# }
```

### `SchedulerUtils::shutdown`

**签名：** `pub fn shutdown() -> Result<(), SchedulerError>`。未初始化返回 `NotInitialized`；
初始化后幂等转发实例关闭。关闭只取消任务并拒绝新注册，不清空 `OnceLock`，
因此关闭后既不能重新 `init`，也不能替换配置。

```rust,no_run
use axutils::{SchedulerConfig, SchedulerError, SchedulerUtils};

if !SchedulerUtils::is_initialized() {
    SchedulerUtils::init(SchedulerConfig::default())?;
}
SchedulerUtils::shutdown()?;
SchedulerUtils::shutdown()?; // 幂等
assert!(SchedulerUtils::is_initialized());
assert!(matches!(
    SchedulerUtils::init(SchedulerConfig::default()),
    Err(SchedulerError::AlreadyInitialized)
));
# Ok::<(), SchedulerError>(())
```

## 容量、取消、panic 与关闭清理

活动任务达到 `max_tasks` 时，新注册返回 `TaskLimitExceeded`。任务运行 callback 时
不持有调度器状态锁，callback 可调用 `cancel` 或 `shutdown`。一次性任务完成、callback
panic、Tokio abort 或 cron 运行期无法计算下一次 occurrence 时，任务结束且不自动重试；
完成清理与 `cancel`/`shutdown` 使用同一任务 ID 移除协议，不重复释放容量。

调用方应在销毁 runtime 前显式调用 `shutdown`。runtime 提前销毁时，调度任务会随 runtime
结束，进程内未持久化的调度信息不会自动恢复。callback panic 的报告由应用的 Tokio/日志
策略决定；调度器不捕获业务 panic 也不重启该任务。

```rust,no_run
use std::time::Duration;
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

# async fn example() -> Result<(), SchedulerError> {
let scheduler = Scheduler::new(SchedulerConfig::new(1)?)?;
let first = scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
assert!(matches!(
    scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {}),
    Err(SchedulerError::TaskLimitExceeded)
));
assert!(scheduler.cancel(first)?); // 移除记录后立即释放注册容量
let second = scheduler.register(TaskSchedule::once(Duration::from_secs(60)), || async {})?;
assert!(scheduler.cancel(second)?);
# Ok(())
# }
```

## 完整示例：按上海时区串行执行业务任务

```rust,no_run
use std::sync::Arc;
use axutils::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

#[derive(Clone)]
struct Service;

impl Service {
    async fn clean_expired(&self) -> Result<(), ()> {
        Ok(())
    }
}

# async fn example() -> Result<(), SchedulerError> {
let scheduler = Arc::new(Scheduler::new(SchedulerConfig::default())?);
let service = Service;
let task_id = scheduler.register(
    TaskSchedule::cron("0 0 0 * * *", "Asia/Shanghai"),
    move || {
        let service = service.clone();
        async move {
            if let Err(error) = service.clean_expired().await {
                // 调用方记录脱敏错误或实现有界重试。
                let _ = error;
            }
        }
    },
)?;

// 应用自己的 signal/退出流程决定何时取消或关闭。
let _removed = scheduler.cancel(task_id)?;
scheduler.shutdown()?;
# Ok(())
# }
```
