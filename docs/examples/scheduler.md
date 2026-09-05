# 调度器

`scheduler` 是完整调度能力的单一 feature，包含 Tokio、时间、时区和 cron 支持；不需要调用方手工
组合多个 provider feature。启用它不会替其他领域开启异步 API。

```toml
[dependencies]
axutils = { version = "1.0", features = ["scheduler"] }
tokio = { version = "1", features = ["rt-multi-thread", "time"] }
```

## 实例 API

`Scheduler::new` 只校验配置；注册任务需要调用方提供带 time driver 的 Tokio runtime。任务闭包必须
快速返回 future，并由应用自行处理任务内部的业务错误、幂等性和重试。

```rust,no_run
use std::time::Duration;

use axutils::scheduler::{Scheduler, SchedulerConfig, SchedulerError, TaskSchedule};

async fn schedule_once() -> Result<(), SchedulerError> {
    let scheduler = Scheduler::new(SchedulerConfig::new(128)?)?;
    let _task_id = scheduler.register(TaskSchedule::once(Duration::from_secs(30)), || async {
        // 执行短小、可取消的业务任务。
    })?;
    scheduler.shutdown()
}
```

`TaskSchedule::interval` 要求非零周期；`TaskSchedule::cron` 接收 cron 表达式和 IANA 时区。关闭后
不能继续注册任务，`SchedulerError` 会保留该生命周期错误而非静默忽略。`cancel` 和 `shutdown`
返回只表示已经发出 abort 请求并更新状态，不等待任务 future 的清理完成；清理由 Tokio 后续调度
推进。业务若要求资源释放或工作完成确认，必须另建 acknowledgement/graceful shutdown 协议。

## 进程级入口

`SchedulerUtils` 只提供一次初始化、状态和实例访问。成功后即使 shutdown，全局位置仍保持已初始化，
不能 reset 或 replace。

```rust
use axutils::{
    scheduler::{SchedulerConfig, SchedulerError},
    utils::SchedulerUtils,
};

fn initialize() -> Result<(), SchedulerError> {
    SchedulerUtils::init(SchedulerConfig::new(128)?)?;
    let _scheduler = SchedulerUtils::scheduler()?;
    Ok(())
}
```

需要不同任务容量、独立关闭时机或测试隔离时，直接保存 `Scheduler` 实例。
