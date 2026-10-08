//! HTTP single-flight 和有限完成缓存的内部状态。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Instant;

#[cfg(feature = "http-async")]
use tokio::{sync::Notify, time as tokio_time};

use super::headers::HeaderEntry;
use super::request::HttpMethod;
use super::DeduplicationPolicy;
use super::{HttpError, HttpResponse, RetryPolicy};

/// 单个合并键允许复制的 header 名和值总字节数。
pub(super) const MAX_COALESCE_KEY_HEADERS_BYTES: usize = 64 * 1024;
/// 单个合并键允许复制的正文总字节数，超过时绕过去重。
pub(super) const MAX_COALESCE_KEY_BODY_BYTES: usize = 64 * 1024;

/// 描述可共享执行结果的方法、目标、内容及重试/缓存策略；各调用者的等待预算独立管理。
#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct RequestKey {
    /// 精确方法，避免不同操作被合并。
    pub(super) method: HttpMethod,
    /// 规范化后的完整地址，包含查询参数。
    pub(super) url: String,
    /// 顺序敏感的最终 header 集合，区分认证或内容协商变体。
    pub(super) headers: Vec<HeaderEntry>,
    /// 显式允许合并时保存的有限正文；None 与空正文不等同。
    pub(super) body: Option<Vec<u8>>,
    /// 最终重试策略，防止不同尝试预算共用结果。
    pub(super) retry_policy: RetryPolicy,
    /// 最终合并/缓存策略，防止不同 TTL 和容量策略共用结果。
    pub(super) deduplication_policy: DeduplicationPolicy,
}

/// 已完成响应及其显式 TTL 到期时间。
pub(super) struct CacheEntry {
    /// 成功响应，正文通过 Arc 与返回值共享。
    response: HttpResponse,
    /// 完成时计算的单调时钟到期点。
    expires_at: Instant,
}

/// 同时按条目数和正文总字节限制的插入顺序缓存。
pub(super) struct CompletedCache {
    /// 请求键到响应的有界索引。
    entries: HashMap<RequestKey, CacheEntry>,
    /// 插入顺序队列，用于按最早插入条目逐出；不是 LRU。
    order: VecDeque<RequestKey>,
    /// 缓存内所有响应正文长度之和，用于总字节预算。
    body_bytes: usize,
}

impl CompletedCache {
    /// 创建不预分配响应容量的空缓存。
    pub(super) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            body_bytes: 0,
        }
    }

    /// 返回未到期的共享响应；命中到期项时同步清理索引、顺序队列和字节计数。
    pub(super) fn get(&mut self, key: &RequestKey, now: Instant) -> Option<HttpResponse> {
        // TTL 使用单调时间，读取不会续期或改变逐出顺序。
        let expired = self
            .entries
            .get(key)
            .map(|entry| entry.expires_at <= now)
            .unwrap_or(false);
        if expired {
            self.remove(key);
            return None;
        }
        self.entries.get(key).map(|entry| entry.response.clone())
    }

    /// 在容量预算内插入响应，按最早插入项逐出；超过单项预算时直接跳过。
    pub(super) fn insert(
        &mut self,
        key: RequestKey,
        response: HttpResponse,
        expires_at: Instant,
        max_entries: usize,
        max_body_bytes: usize,
    ) {
        // 大于整体正文预算的单项不能进入缓存，避免为永远放不下的项逐出其他结果。
        let response_body_len = response.body().len();
        if response_body_len > max_body_bytes || max_entries == 0 {
            return;
        }
        // 覆盖旧键前先撤销它的计数，再逐出足够的旧项满足两种独立预算。
        self.remove(&key);
        while self.entries.len() >= max_entries
            || self.body_bytes.saturating_add(response_body_len) > max_body_bytes
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.remove(&oldest);
        }
        // 预算检查完成后统一提交顺序、索引和字节计数。
        self.body_bytes += response_body_len;
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            CacheEntry {
                response,
                expires_at,
            },
        );
    }

    /// 删除存在的项并维护容量不变量；不存在时无副作用。
    fn remove(&mut self, key: &RequestKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.body_bytes = self.body_bytes.saturating_sub(entry.response.body().len());
            if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
                self.order.remove(index);
            }
        }
    }
}

/// 同步执行路径在短临界区内共享的合并索引和完成缓存。
pub(super) struct SyncState {
    /// 当前执行中的同步 leader，follower 克隆对应 flight 后离开状态锁。
    pub(super) in_flight: HashMap<RequestKey, Arc<SyncFlight>>,
    /// 与同步执行路径独立绑定的有限完成缓存。
    pub(super) cache: CompletedCache,
}

impl SyncState {
    /// 创建空的同步合并状态，不建立网络连接。
    pub(super) fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            cache: CompletedCache::new(),
        }
    }
}

/// 同步 leader 的单次完成槽，允许多个 follower 以各自 deadline 等待。
pub(super) struct SyncFlight {
    /// leader 首次发布的共享结果；None 表示尚未完成。
    result: Mutex<Option<Result<HttpResponse, HttpError>>>,
    /// 结果发布后唤醒同步等待者，不负责驱动网络调用。
    condition: Condvar,
}

impl SyncFlight {
    /// 创建尚未发布结果的同步执行槽。
    pub(super) fn new() -> Self {
        Self {
            result: Mutex::new(None),
            condition: Condvar::new(),
        }
    }

    /// 仅首次发布生效，随后唤醒所有同步 follower。
    pub(super) fn publish(&self, result: Result<HttpResponse, HttpError>) {
        // 结果与通知受同一临界区保护，等待者不会错过已经写入的完成值。
        let mut guard = recover_lock(&self.result);
        if guard.is_none() {
            *guard = Some(result);
            self.condition.notify_all();
        }
    }

    /// 等待共享结果直到自身 deadline；超时不会取消 leader。
    pub(super) fn wait(&self, deadline: Instant) -> Result<HttpResponse, HttpError> {
        let mut guard = recover_lock(&self.result);
        loop {
            // 每次唤醒都先读结果，再判断预算，兼容虚假唤醒与完成/超时竞争。
            if let Some(result) = guard.as_ref() {
                return result.clone();
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
            // 等待自动释放 mutex；恢复后同时检查计时结果与完成值。
            let (next_guard, wait_result) = self
                .condition
                .wait_timeout(guard, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard = next_guard;
            if wait_result.timed_out() && guard.is_none() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
        }
    }
}

#[cfg(feature = "http-async")]
/// 异步执行路径专属的合并状态，不与同步连接池共享在途请求。
pub(super) struct AsyncState {
    /// 当前执行中的异步 leader，不与同步路径共享请求。
    pub(super) in_flight: HashMap<RequestKey, Arc<AsyncFlight>>,
    /// 与异步执行路径绑定的有限完成缓存。
    pub(super) cache: CompletedCache,
}

#[cfg(feature = "http-async")]
impl AsyncState {
    /// 创建空的异步合并状态，不要求 runtime 或启动任务。
    pub(super) fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            cache: CompletedCache::new(),
        }
    }
}

#[cfg(feature = "http-async")]
/// 异步 leader 的单次完成槽；通知只用于唤醒，结果始终从 mutex 读取。
pub(super) struct AsyncFlight {
    /// leader 首次发布的共享结果；取消同样发布稳定错误。
    result: Mutex<Option<Result<HttpResponse, HttpError>>>,
    /// 通知异步 follower 重新读取结果，结果本身保存在 mutex 中。
    notify: Notify,
}

#[cfg(feature = "http-async")]
impl AsyncFlight {
    /// 创建尚未发布结果的异步执行槽。
    pub(super) fn new() -> Self {
        Self {
            result: Mutex::new(None),
            notify: Notify::new(),
        }
    }

    /// 首次发布结果并通知所有已登记的异步等待者，重复发布无副作用。
    pub(super) fn publish(&self, result: Result<HttpResponse, HttpError>) {
        let mut guard = recover_lock(&self.result);
        if guard.is_none() {
            *guard = Some(result);
            self.notify.notify_waiters();
        }
    }

    /// 以 follower 自身 deadline 等待结果，取消或超时都不影响 leader。
    pub(super) async fn wait(&self, deadline: Instant) -> Result<HttpResponse, HttpError> {
        loop {
            // 快速命中已发布结果，或拒绝已经用尽自身预算的等待。
            if let Some(result) = recover_lock(&self.result).as_ref() {
                return result.clone();
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
            // 先登记通知，再重新读取结果，关闭读取和订阅之间的丢失唤醒窗口。
            let mut notified = Box::pin(self.notify.notified());
            notified.as_mut().enable();
            if let Some(result) = recover_lock(&self.result).as_ref() {
                return result.clone();
            }
            if tokio_time::timeout(remaining, notified).await.is_err() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
        }
    }
}

/// 恢复短临界区的互斥守卫，确保 leader 异常后 follower 仍可读取取消结果。
fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
