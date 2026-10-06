//! 只执行有界、有限重试的 Redis DEL，不承担业务键生成、缓存回填或持久调度。

use std::{
    fmt,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use ::tokio::{task, time::Instant};

#[cfg(feature = "tracing")]
use crate::telemetry::redis_invalidation as invalidation_trace;
use crate::tokio::{facade::TokioUtils, TokioTaskGuard};

use super::{RedisClient, RedisError};

mod pending;
use pending::Pending;

#[cfg(test)]
mod tests;

/// Redis 内存失效队列的调用方预算；需要 `redis-invalidation` feature。
///
/// 不提供应用默认策略。构造队列时取得配置所有权，此后修改外部副本不会影响已有队列。
/// 队列预算不替代 [`super::RedisConfig`] 的单键、批量和连接预算；调用方应使二者相容。
#[derive(Clone, Debug)]
pub struct RedisInvalidationConfig {
    /// 待处理的不同键数量上限，不含最多一个在途批次；零表示拒绝所有新键。
    ///
    /// 达到上限仍可刷新已有键。失败批次回队时也受此限制；没有空位的旧重试被丢弃。
    /// 此参数不是总内存字节上限，键长度与调用方输入迭代器的大小应由调用方约束。
    pub capacity: usize,
    /// 每批最多删除的键数量；零在构造时归一为一，保证队列可以推进。
    pub batch_items: usize,
    /// 每批键字符串的 UTF-8 字节总预算，不含 RESP 开销。
    ///
    /// 首个键即使超限也单独成批，避免该键永远无法处理；零仍允许每批一个键。
    /// RedisClient 自身的键和批量上限仍生效，超出客户端上限会进入有限重试路径。
    pub batch_bytes: usize,
    /// 每批 DEL 的等待预算，包含连接获取；零允许立即超时，不能保证命令未执行。
    /// 构造时不可表示的单调时钟 deadline 返回 `InvalidConfig`。
    pub io_timeout: Duration,
    /// 初次失败及每次重试失败后的等待间隔；空列表表示只尝试一次。
    ///
    /// 总尝试次数最多为 `1 + retry_delays.len()`，零间隔允许立即重试；构造时任何不可
    /// 表示的 deadline 返回 `InvalidConfig`。新入队的同键请求重置此预算并立即到期。
    pub retry_delays: Vec<Duration>,
}

impl RedisInvalidationConfig {
    /// 提前拒绝无法转换为 deadline 的时间预算，避免配置在后台任务内导致时间加法溢出。
    fn validate(&self) -> Result<(), RedisError> {
        let now = Instant::now();
        // 配置检查不访问网络、不要求 runtime；运行时仍使用 checked_add 防御长期时钟推进。
        if now.checked_add(self.io_timeout).is_none() {
            return Err(RedisError::InvalidConfig {
                field: "io_timeout",
            });
        }
        if self
            .retry_delays
            .iter()
            .any(|delay| now.checked_add(*delay).is_none())
        {
            return Err(RedisError::InvalidConfig {
                field: "retry_delays",
            });
        }
        Ok(())
    }
}

/// 一次同步投递的计数和启动状态；接收不等于 Redis 删除成功。
///
/// 不保存输入键。后台 DEL 结果不经此值返回；启用 `tracing` 可观察脱敏失败事件。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use = "检查被拒绝的条目和 worker 启动错误；接收不表示删除已完成"]
pub struct RedisInvalidationEnqueue {
    /// 已接受的输入项数，包含重复键刷新；不是队列不同键数，也不是已删除数，计数饱和于 `usize::MAX`。
    pub accepted: usize,
    /// 因容量已满而拒绝的新键输入项数；允许部分接收，不回滚已经接收的项，计数饱和于 `usize::MAX`。
    pub rejected: usize,
    /// 需要启动 worker 却缺少 runtime context 时为 `Some(RedisError::RuntimeRequired)`。
    ///
    /// 已接受条目仍保留，可在可用 runtime 内再次调用 `enqueue`（允许空迭代器）重试启动。
    /// `None` 仅表示本次无需启动或登记成功，不能保证 runtime 存活、驱动可用或任务将执行。
    pub worker_error: Option<RedisError>,
}

/// 显式注入 RedisClient、按字符串键去重的有界内存失效队列。
///
/// 需要 `redis-invalidation` feature。`new` 不创建任务、不访问网络；首次有效投递惰性
/// 启动唯一 worker。同步 `enqueue` 使用短期互斥锁，不等待网络或容量，不提供异步背压。
///
/// # 去重、分批与重试
///
/// 去重只覆盖待处理映射：重复键重置重试次数与到期时间。取出的批次不占容量；在途期间
/// 同键再次入队会保留为新的请求，旧批次成功不会删除它，失败也不会覆盖它。到期键按
/// 字符串顺序选取，无 FIFO 或公平性保证。数量及字节预算见 [`RedisInvalidationConfig`]。
///
/// DEL 错误（包括本地校验、Cluster 跨 slot 错误）与超时均按配置有限重试。满队列拒绝新键；
/// 旧重试无法回队或次数耗尽时丢弃。返回值仅反馈当次投递，后台失败通过可选 `tracing`
/// 输出固定分类与计数，不输出键、内容或服务端原文；未开启时不发出这些诊断。
///
/// # 生命周期与取消
///
/// 调用方负责持续运行启用 I/O 和 time driver 的 Tokio runtime。缺少 runtime context
/// 时投递仍保留条目并报告启动错误；已有 worker 正常存活时允许 runtime 外投递。
/// 已退出 worker 在下次投递且仍有待处理项时重启，不自动创建 runtime 或监护任务。
/// 缺少 driver 可能使后台任务 panic；任务退出时尚在映射内的键保留，已取出的批次丢失。
/// 登记到已关闭或不再推进的 runtime 不保证执行；投递结果不是 worker 健康检查。
///
/// 队列不可克隆，可用 [`Arc`] 共享。最后一个拥有者释放时 [`TokioTaskGuard`] 请求 abort，
/// 不同步等待任务退出、不排空队列；已取出的资源由 runtime 后续调度释放，待处理内容丢弃。
/// 空队列用通知等待，不轮询；新请求也会中断退避等待。
/// 每批完成后主动让出执行权，使立即失败和零退避也保留调度与取消边界。
///
/// 这不是持久投递或强一致机制；超时、取消和拥有者释放均不能保证已发出的 Redis 命令未执行。
/// 调用方保留提交时机、TTL 等业务恢复策略；DEL 的重试也可能删除后来回填的新值。
pub struct RedisInvalidationQueue {
    /// 唯一消费者的所有权；固定先锁 worker 再锁 pending，串行化启动与并发投递。
    worker: Mutex<Option<TokioTaskGuard>>,
    /// 工作任务只共享待处理状态，不持有队列本身，避免所有权环使 Drop 无法取消。
    pending: Arc<Pending>,
    /// 调用方显式注入的客户端；worker 克隆共享其底层连接状态，不访问全局 client。
    client: RedisClient,
    /// 经校验的不可变策略，worker 与队列共享，避免每次启动复制退避列表。
    config: Arc<RedisInvalidationConfig>,
}

impl RedisInvalidationQueue {
    /// 接管客户端和策略并构造空队列；不要求 runtime，不访问 Redis。
    ///
    /// 时间预算不可表示时返回 [`RedisError::InvalidConfig`]，`field` 是 `io_timeout` 或
    /// `retry_delays`。其他零值的语义见配置字段，不隐式读取应用配置或生成缓存键。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use axutils::redis::{
    ///     RedisClient, RedisConfig, RedisError, RedisInvalidationConfig, RedisInvalidationQueue,
    /// };
    ///
    /// let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    /// let queue = RedisInvalidationQueue::new(client, RedisInvalidationConfig {
    ///     capacity: 16,
    ///     batch_items: 8,
    ///     batch_bytes: 4096,
    ///     io_timeout: Duration::from_secs(1),
    ///     retry_delays: vec![Duration::from_millis(100)],
    /// })?;
    /// // 同步示例没有 runtime：接受并保留键，报告未能启动 worker，不访问网络。
    /// let report = queue.enqueue(["cache:example".to_owned(), "cache:example".to_owned()]);
    /// assert_eq!(report.accepted, 2);
    /// assert_eq!(report.rejected, 0);
    /// assert_eq!(report.worker_error, Some(RedisError::RuntimeRequired));
    /// # Ok::<(), RedisError>(())
    /// ```
    pub fn new(
        client: RedisClient,
        mut config: RedisInvalidationConfig,
    ) -> Result<Self, RedisError> {
        // 在登记任何后台任务之前完成时间校验，并保留参考实现的零数量归一语义。
        config.validate()?;
        config.batch_items = config.batch_items.max(1);
        Ok(Self {
            worker: Mutex::new(None),
            pending: Arc::default(),
            client,
            config: Arc::new(config),
        })
    }

    /// 同步尝试投递拥有型键，返回接收、拒绝数及 worker 启动错误，不等待 DEL 完成。
    ///
    /// 按迭代顺序逐项处理，满队列时仍刷新已有键，丢弃新的不同键；空迭代器可重启保留
    /// 条目的 worker。键内容由 RedisClient 在实际删除时校验，接收不保证键有效。
    /// 该方法消费整个迭代器，调用方应提供有限且执行有界的输入。
    ///
    /// # Panics
    ///
    /// 输入迭代器的 panic 原样传播；之前已插入的项保留，后续投递可恢复被毒化的内部锁。
    /// 不要在输入迭代器中递归调用同一队列的 `enqueue`，否则会等待自身持有的互斥锁。
    pub fn enqueue(&self, keys: impl IntoIterator<Item = String>) -> RedisInvalidationEnqueue {
        // 投递与 worker 启动共用门闩，避免多个生产者同时启动消费者。
        let mut worker = self.worker.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut report, has_pending) = self.pending.enqueue(keys, self.config.capacity);
        if has_pending {
            // 不持条目锁创建任务；结束的守卫替换时无需等待旧任务结果。
            if worker.as_ref().is_none_or(TokioTaskGuard::is_finished) {
                match TokioUtils::spawn(Self::run(
                    Arc::clone(&self.pending),
                    self.client.clone(),
                    Arc::clone(&self.config),
                )) {
                    Ok(task) => *worker = Some(TokioTaskGuard::new(task)),
                    Err(_) => report.worker_error = Some(RedisError::RuntimeRequired),
                }
            }
            // notify_one 保留一个许可，覆盖队列检查与进入等待之间的投递。
            self.pending.ready.notify_one();
        }
        drop(worker);
        // subscriber 是调用方代码，事件放在所有队列锁之外，防止重入造成死锁。
        #[cfg(feature = "tracing")]
        {
            if report.rejected > 0 {
                invalidation_trace::record("queue_full", report.rejected, None);
            }
            if let Some(error) = &report.worker_error {
                invalidation_trace::record("runtime_unavailable", 0, Some(error));
            }
        }
        report
    }

    /// 唯一后台消费者：锁内取批、锁外等待 I/O、失败后按剩余预算回队。
    async fn run(pending: Arc<Pending>, client: RedisClient, config: Arc<RedisInvalidationConfig>) {
        // future 被实际 poll 后才登记退出观察；无 runtime 的启动失败不伪报 worker 退出。
        let _exit = WorkerExit;
        loop {
            let (batch, next_due) = pending.take_batch(&config, Instant::now());
            if batch.is_empty() {
                // 到期之前的新投递可提前唤醒；完全空闲时只等待通知，无周期性轮询。
                if let Some(due) = next_due {
                    let _ = TokioUtils::timeout(
                        due.saturating_duration_since(Instant::now()),
                        pending.ready.notified(),
                    )
                    .await;
                } else {
                    pending.ready.notified().await;
                }
                continue;
            }
            // 超时仅取消等待；底层已提交的 DEL 可能继续执行，所以失败只采用有限幂等重试。
            let result = if Instant::now().checked_add(config.io_timeout).is_none() {
                Err(RedisError::InvalidConfig {
                    field: "io_timeout",
                })
            } else {
                match TokioUtils::timeout(
                    config.io_timeout,
                    client.delete_many_async(batch.iter().map(|(key, _)| key)),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err(RedisError::Timeout),
                }
            };
            if let Err(_error) = result {
                #[cfg(feature = "tracing")]
                invalidation_trace::record("delete_failed", batch.len(), Some(&_error));
                pending.retry(batch, &config, Instant::now());
            }
            // 本地校验失败可能同步返回，零退避也没有等待；每批主动让出以免耗尽重试前
            // 独占 runtime 线程，并让拥有者释放后的 abort 能在下一次调度时生效。
            task::yield_now().await;
        }
    }
}

impl fmt::Debug for RedisInvalidationQueue {
    /// 只显示不可变预算，不获取队列锁或输出客户端、待处理键及在途内容。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisInvalidationQueue")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// 观察已开始 worker 的 future 被释放；只报告退出事实，不推断成功、panic 或取消原因。
struct WorkerExit;

impl Drop for WorkerExit {
    /// abort/runtime 退出/panic 清理时发出固定诊断，不进行网络恢复或重启。
    fn drop(&mut self) {
        #[cfg(feature = "tracing")]
        invalidation_trace::record("worker_stopped", 0, None);
    }
}
