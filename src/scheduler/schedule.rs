//! 已校验调度的时间锚点与串行执行；不管理注册表或任务所有权。

use super::{cron::CronSchedule, SchedulerError, TaskSchedule};
use std::{future::Future, time::Duration};
use tokio::time::{self, Instant, MissedTickBehavior};

/// 注册前已完成校验的调度参数，首次触发统一保存 monotonic deadline。
pub(super) enum ValidatedSchedule {
    /// 仅执行一次的既定时刻。
    Once(Instant),
    /// 固定周期，首次时刻不受后台首次 poll 的延迟影响。
    Interval {
        /// 注册时计算的首次时刻。
        start: Instant,
        /// 已验证非零且首次 deadline 可表示的间隔。
        period: Duration,
    },
    /// Cron 规则及注册时的首次 deadline；后续仍按墙钟重新计算。
    Cron(Box<CronSchedule>, Instant),
}

/// 在发布任何后台任务前验证参数并确定首次 deadline。
pub(super) fn validate(schedule: TaskSchedule) -> Result<ValidatedSchedule, SchedulerError> {
    // 在发布任务前校验并保存 deadline，避免后台再次做不受检的时间加法。
    let now = Instant::now();
    match schedule {
        TaskSchedule::Once(delay) => now
            .checked_add(delay)
            .map(ValidatedSchedule::Once)
            .ok_or(SchedulerError::InvalidSchedule),
        TaskSchedule::Interval(period) if period.is_zero() => Err(SchedulerError::InvalidSchedule),
        TaskSchedule::Interval(period) => now
            .checked_add(period)
            .map(|start| ValidatedSchedule::Interval { start, period })
            .ok_or(SchedulerError::InvalidSchedule),
        TaskSchedule::Cron {
            expression,
            timezone,
        } => {
            let schedule = CronSchedule::parse(&expression, &timezone)?;
            let deadline = cron_deadline(&schedule)?;
            Ok(ValidatedSchedule::Cron(Box::new(schedule), deadline))
        }
    }
}

/// 将当前墙钟下的下一次 Cron 触发映射到 monotonic deadline，计算耗时不重新延长预算。
fn cron_deadline(schedule: &CronSchedule) -> Result<Instant, SchedulerError> {
    let now = Instant::now();
    let delay = schedule.delay_from_now()?;
    now.checked_add(delay).ok_or(SchedulerError::InvalidCron)
}

/// 按调度串行调用业务 future；取消由外层任务的 abort/Drop 处理。
pub(super) async fn run<F, Fut>(schedule: ValidatedSchedule, callback: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = ()>,
{
    match schedule {
        ValidatedSchedule::Once(deadline) => {
            // 等待既定时刻；callback 串行运行，取消沿 Tokio future 的释放传播。
            time::sleep_until(deadline).await;
            callback().await;
        }
        ValidatedSchedule::Interval { start, period } => {
            // 跳过错过的 tick，避免慢回调产生追赶风暴；同一回调不会重叠执行。
            let mut interval = time::interval_at(start, period);
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                callback().await;
            }
        }
        ValidatedSchedule::Cron(schedule, mut deadline) => loop {
            // 首次 deadline 锚定注册时刻，后台首次 poll 晚到时不重新开始相对等待。
            time::sleep_until(deadline).await;
            callback().await;
            // 回调结束后重新读取墙钟；不存在未来有效触发点时结束并释放登记。
            let Ok(next_deadline) = cron_deadline(&schedule) else {
                break;
            };
            deadline = next_deadline;
        },
    }
}
