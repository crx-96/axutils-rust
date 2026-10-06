//! 待处理键的短临界区状态转换；网络与定时等待均由外层 worker 执行。

use std::{
    collections::{btree_map::Entry, BTreeMap},
    sync::{Mutex, PoisonError},
};

use ::tokio::{sync::Notify, time::Instant};

#[cfg(feature = "tracing")]
use crate::telemetry::redis_invalidation as invalidation_trace;

use super::{RedisInvalidationConfig, RedisInvalidationEnqueue};

/// 一个已取出、不计入待处理容量的批次；顺序为键的字符串顺序。
pub(super) type Batch = Vec<(String, Attempt)>;

/// 生产者与唯一消费者共享的映射及可保留许可的唤醒信号。
#[derive(Default)]
pub(super) struct Pending {
    /// 最新的待处理失效请求；键离开映射即成为在途项，容量可由新请求使用。
    entries: Mutex<BTreeMap<String, Attempt>>,
    /// 唤醒唯一 worker；没有等待者时保留一次许可，避免空队列竞态丢失唤醒。
    pub(super) ready: Notify,
}

/// 一个键的下一次尝试状态；不记录业务实体或缓存内容。
pub(super) struct Attempt {
    /// 已安排的重试次数，同时作为下一次退避间隔下标；初次投递为零。
    retries: usize,
    /// 允许删除的单调时钟时刻；使用 Tokio 时钟以保持超时、退避与暂停时间一致。
    due: Instant,
}

impl Pending {
    /// 在一个临界区内逐项去重、重置到期与预算，并返回容量反馈及是否仍有条目。
    pub(super) fn enqueue(
        &self,
        keys: impl IntoIterator<Item = String>,
        capacity: usize,
    ) -> (RedisInvalidationEnqueue, bool) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let mut report = RedisInvalidationEnqueue::default();
        // 只拒绝新增不同键；重复投递即使满容量也代表一项新的失效意图。
        for key in keys {
            if entries.len() >= capacity && !entries.contains_key(&key) {
                report.rejected = report.rejected.saturating_add(1);
                continue;
            }
            entries.insert(
                key,
                Attempt {
                    retries: 0,
                    due: Instant::now(),
                },
            );
            report.accepted = report.accepted.saturating_add(1);
        }
        (report, !entries.is_empty())
    }

    /// 取出到期键并计算剩余最早 deadline；首个超大键可单独成批以保持可推进性。
    pub(super) fn take_batch(
        &self,
        config: &RedisInvalidationConfig,
        now: Instant,
    ) -> (Batch, Option<Instant>) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let mut keys = Vec::new();
        let mut bytes = 0;
        // 键顺序与原实现一致；遇到预算边界结束当前批，不从后面挑选更小的键填空。
        for (key, _) in entries.iter().filter(|(_, attempt)| attempt.due <= now) {
            if keys.len() >= config.batch_items
                || (!keys.is_empty() && key.len() > config.batch_bytes.saturating_sub(bytes))
            {
                break;
            }
            keys.push(key.clone());
            bytes = bytes.saturating_add(key.len());
        }
        // 移出映射后释放容量；在途同键再次投递会创建独立的最新条目。
        let batch = keys
            .into_iter()
            .filter_map(|key| entries.remove(&key).map(|attempt| (key, attempt)))
            .collect();
        (batch, entries.values().map(|attempt| attempt.due).min())
    }

    /// 为旧失败批次安排下一次退避；已有同键总是代表更新的请求，不覆盖或延迟它。
    pub(super) fn retry(&self, batch: Batch, config: &RedisInvalidationConfig, now: Instant) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let mut full = 0usize;
        let mut exhausted = 0usize;
        let mut unrepresentable = 0usize;
        for (key, attempt) in batch {
            // 先判新请求优先，避免已被新请求取代的失败误报容量或耗尽。
            if entries.contains_key(&key) {
                continue;
            }
            let Some(delay) = config.retry_delays.get(attempt.retries) else {
                exhausted += 1;
                continue;
            };
            if entries.len() >= config.capacity {
                full += 1;
                continue;
            }
            let Some(due) = now.checked_add(*delay) else {
                // 构造时虽已校验，长期运行仍防御 deadline 溢出；丢弃该旧请求而不 panic。
                unrepresentable += 1;
                continue;
            };
            // 在同一把锁下只填空位，保持新请求的重试次数和到期时间不受旧失败影响。
            if let Entry::Vacant(slot) = entries.entry(key) {
                slot.insert(Attempt {
                    retries: attempt.retries + 1,
                    due,
                });
            }
        }
        drop(entries);
        // 锁外按分类汇总诊断，不把键或每项内容传给 subscriber。
        for (_event, count) in [
            ("queue_full", full),
            ("retry_exhausted", exhausted),
            ("retry_deadline_unrepresentable", unrepresentable),
        ] {
            if count > 0 {
                #[cfg(feature = "tracing")]
                invalidation_trace::record(_event, count, None);
            }
        }
    }
}
