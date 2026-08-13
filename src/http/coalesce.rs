//! HTTP single-flight 和有限完成缓存的内部状态。

use std::collections::{HashMap, VecDeque};
use std::sync::{Condvar, Mutex};
use std::time::Instant;

use super::config::DeduplicationPolicy;
use super::headers::HeaderEntry;
use super::request::HttpMethod;
use super::{HttpError, HttpResponse, RetryPolicy};

pub(crate) const MAX_COALESCE_KEY_HEADERS_BYTES: usize = 64 * 1024;
pub(crate) const MAX_COALESCE_KEY_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct RequestKey {
    pub(crate) method: HttpMethod,
    pub(crate) url: String,
    pub(crate) headers: Vec<HeaderEntry>,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) retry_policy: RetryPolicy,
    pub(crate) deduplication_policy: DeduplicationPolicy,
}

pub(crate) struct CacheEntry {
    response: HttpResponse,
    expires_at: Instant,
}

pub(crate) struct CompletedCache {
    entries: HashMap<RequestKey, CacheEntry>,
    order: VecDeque<RequestKey>,
    body_bytes: usize,
}

impl CompletedCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            body_bytes: 0,
        }
    }

    pub(crate) fn get(&mut self, key: &RequestKey, now: Instant) -> Option<HttpResponse> {
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

    pub(crate) fn insert(
        &mut self,
        key: RequestKey,
        response: HttpResponse,
        expires_at: Instant,
        max_entries: usize,
        max_body_bytes: usize,
    ) {
        let response_body_len = response.body().len();
        if response_body_len > max_body_bytes || max_entries == 0 {
            return;
        }
        self.remove(&key);
        while self.entries.len() >= max_entries
            || self.body_bytes.saturating_add(response_body_len) > max_body_bytes
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.remove(&oldest);
        }
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

    fn remove(&mut self, key: &RequestKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.body_bytes = self.body_bytes.saturating_sub(entry.response.body().len());
            if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
                self.order.remove(index);
            }
        }
    }
}

pub(crate) struct SyncState {
    pub(crate) in_flight: HashMap<RequestKey, std::sync::Arc<SyncFlight>>,
    pub(crate) cache: CompletedCache,
}

impl SyncState {
    pub(crate) fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            cache: CompletedCache::new(),
        }
    }
}

pub(crate) struct SyncFlight {
    result: Mutex<Option<Result<HttpResponse, HttpError>>>,
    condition: Condvar,
}

impl SyncFlight {
    pub(crate) fn new() -> Self {
        Self {
            result: Mutex::new(None),
            condition: Condvar::new(),
        }
    }

    pub(crate) fn publish(&self, result: Result<HttpResponse, HttpError>) {
        let mut guard = recover_lock(&self.result);
        if guard.is_none() {
            *guard = Some(result);
            self.condition.notify_all();
        }
    }

    pub(crate) fn wait(&self, deadline: Instant) -> Result<HttpResponse, HttpError> {
        let mut guard = recover_lock(&self.result);
        loop {
            if let Some(result) = guard.as_ref() {
                return result.clone();
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
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

#[cfg(all(feature = "http", feature = "tokio"))]
pub(crate) struct AsyncState {
    pub(crate) in_flight: HashMap<RequestKey, std::sync::Arc<AsyncFlight>>,
    pub(crate) cache: CompletedCache,
}

#[cfg(all(feature = "http", feature = "tokio"))]
impl AsyncState {
    pub(crate) fn new() -> Self {
        Self {
            in_flight: HashMap::new(),
            cache: CompletedCache::new(),
        }
    }
}

#[cfg(all(feature = "http", feature = "tokio"))]
pub(crate) struct AsyncFlight {
    result: Mutex<Option<Result<HttpResponse, HttpError>>>,
    notify: tokio::sync::Notify,
}

#[cfg(all(feature = "http", feature = "tokio"))]
impl AsyncFlight {
    pub(crate) fn new() -> Self {
        Self {
            result: Mutex::new(None),
            notify: tokio::sync::Notify::new(),
        }
    }

    pub(crate) fn publish(&self, result: Result<HttpResponse, HttpError>) {
        let mut guard = recover_lock(&self.result);
        if guard.is_none() {
            *guard = Some(result);
            self.notify.notify_waiters();
        }
    }

    pub(crate) async fn wait(&self, deadline: Instant) -> Result<HttpResponse, HttpError> {
        loop {
            if let Some(result) = recover_lock(&self.result).as_ref() {
                return result.clone();
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
            let mut notified = Box::pin(self.notify.notified());
            notified.as_mut().enable();
            if let Some(result) = recover_lock(&self.result).as_ref() {
                return result.clone();
            }
            if tokio::time::timeout(remaining, notified).await.is_err() {
                return Err(HttpError::CoalescedWaitTimeout);
            }
        }
    }
}

fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
