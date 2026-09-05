//! 去重、完成缓存与共享执行结果策略。

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(feature = "http-async")]
use super::coalesce::AsyncFlight;
use super::coalesce::{
    RequestKey, SyncFlight, MAX_COALESCE_KEY_BODY_BYTES, MAX_COALESCE_KEY_HEADERS_BYTES,
};
use super::prepared::{transport_error, PreparedRequest};
use super::{HttpClient, HttpError, HttpMethod, HttpResponse, HttpTransportErrorKind};

/// 返回本次调用在总 deadline 前仍可使用的传输时间。
pub(super) fn remaining_until(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// 判断当前尝试之后是否仍可按请求策略进行重试。
pub(super) fn can_retry(prepared: &PreparedRequest, attempts: u32) -> bool {
    prepared.retry_policy.can_retry_method(&prepared.method)
        && attempts < prepared.retry_policy.max_retries()
}

/// 构造总 deadline 用尽时的统一传输错误。
pub(super) fn deadline_error(prepared: &PreparedRequest, attempts: u32) -> HttpError {
    transport_error(
        HttpTransportErrorKind::Timeout,
        attempts,
        attempts >= prepared.retry_policy.max_retries(),
    )
}

impl HttpClient {
    /// 为满足安全和容量条件的请求构造 single-flight/cache key。
    pub(super) fn coalesce_key(&self, prepared: &PreparedRequest) -> Option<RequestKey> {
        let safe_repeatable = prepared.method.is_idempotent_safe() && prepared.body.is_none();
        if !prepared.deduplication_policy.is_enabled()
            || (!safe_repeatable && !prepared.deduplication_opt_in)
        {
            return None;
        }
        if prepared.headers.total_bytes() > MAX_COALESCE_KEY_HEADERS_BYTES
            || prepared
                .body
                .as_ref()
                .is_some_and(|body| body.len() > MAX_COALESCE_KEY_BODY_BYTES)
        {
            return None;
        }
        Some(RequestKey {
            method: prepared.method.clone(),
            url: prepared.url.as_str().to_owned(),
            headers: prepared.headers.entries().to_vec(),
            body: prepared.body.clone(),
            retry_policy: prepared.retry_policy.clone(),
            deduplication_policy: prepared.deduplication_policy.clone(),
        })
    }

    /// 发布同步 leader 的结果，并在满足策略时写入完成缓存。
    fn finish_sync(
        &self,
        key: &RequestKey,
        flight: &Arc<SyncFlight>,
        prepared: &PreparedRequest,
        result: &Result<HttpResponse, HttpError>,
    ) {
        let mut state = recover_lock(&self.sync_state);
        if prepared.deduplication_policy.cache_enabled()
            && result
                .as_ref()
                .ok()
                .is_some_and(|response| cache_eligible(prepared, response))
        {
            let response = result.as_ref().expect("successful response checked above");
            state.cache.insert(
                key.clone(),
                response.clone(),
                Instant::now() + prepared.deduplication_policy.ttl(),
                prepared.deduplication_policy.max_completed_entries(),
                prepared.deduplication_policy.max_cached_body_bytes(),
            );
        }
        state.in_flight.remove(key);
        flight.publish(result.clone());
    }

    /// 发布异步 leader 的结果，并在满足策略时写入完成缓存。
    #[cfg(feature = "http-async")]
    fn finish_async(
        &self,
        key: &RequestKey,
        flight: &Arc<AsyncFlight>,
        prepared: &PreparedRequest,
        result: &Result<HttpResponse, HttpError>,
    ) {
        let mut state = recover_lock(&self.async_state);
        if prepared.deduplication_policy.cache_enabled()
            && result
                .as_ref()
                .ok()
                .is_some_and(|response| cache_eligible(prepared, response))
        {
            let response = result.as_ref().expect("successful response checked above");
            state.cache.insert(
                key.clone(),
                response.clone(),
                Instant::now() + prepared.deduplication_policy.ttl(),
                prepared.deduplication_policy.max_completed_entries(),
                prepared.deduplication_policy.max_cached_body_bytes(),
            );
        }
        state.in_flight.remove(key);
        flight.publish(result.clone());
    }
}

/// 同步 leader 的 RAII guard，确保 panic 或提前返回不会让 follower 永久等待。
pub(super) struct SyncLeaderGuard<'a> {
    client: &'a HttpClient,
    key: RequestKey,
    flight: Arc<SyncFlight>,
    pub(super) prepared: PreparedRequest,
    finished: bool,
}

impl<'a> SyncLeaderGuard<'a> {
    pub(super) fn new(
        client: &'a HttpClient,
        key: RequestKey,
        flight: Arc<SyncFlight>,
        prepared: PreparedRequest,
    ) -> Self {
        Self {
            client,
            key,
            flight,
            prepared,
            finished: false,
        }
    }

    pub(super) fn finish(
        mut self,
        result: Result<HttpResponse, HttpError>,
    ) -> Result<HttpResponse, HttpError> {
        self.finished = true;
        self.client
            .finish_sync(&self.key, &self.flight, &self.prepared, &result);
        result
    }
}

impl Drop for SyncLeaderGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.client.finish_sync(
                &self.key,
                &self.flight,
                &self.prepared,
                &Err(HttpError::CoalescedRequestCancelled),
            );
        }
    }
}

/// 异步 leader 的 RAII guard，保持与同步路径相同的取消发布语义。
#[cfg(feature = "http-async")]
pub(super) struct AsyncLeaderGuard<'a> {
    client: &'a HttpClient,
    key: RequestKey,
    flight: Arc<AsyncFlight>,
    pub(super) prepared: PreparedRequest,
    finished: bool,
}

#[cfg(feature = "http-async")]
impl<'a> AsyncLeaderGuard<'a> {
    pub(super) fn new(
        client: &'a HttpClient,
        key: RequestKey,
        flight: Arc<AsyncFlight>,
        prepared: PreparedRequest,
    ) -> Self {
        Self {
            client,
            key,
            flight,
            prepared,
            finished: false,
        }
    }

    pub(super) fn finish(
        mut self,
        result: Result<HttpResponse, HttpError>,
    ) -> Result<HttpResponse, HttpError> {
        self.finished = true;
        self.client
            .finish_async(&self.key, &self.flight, &self.prepared, &result);
        result
    }
}

#[cfg(feature = "http-async")]
impl Drop for AsyncLeaderGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.client.finish_async(
                &self.key,
                &self.flight,
                &self.prepared,
                &Err(HttpError::CoalescedRequestCancelled),
            );
        }
    }
}

/// 判断响应是否可以进入受限的完成缓存。
pub(super) fn cache_eligible(prepared: &PreparedRequest, response: &HttpResponse) -> bool {
    if !matches!(prepared.method, HttpMethod::Get | HttpMethod::Head)
        || prepared.body.is_some()
        || !response.is_success()
    {
        return false;
    }
    if prepared.headers.iter().any(|(name, _)| {
        name == "authorization"
            || name == "cookie"
            || name == "range"
            || name.starts_with("if-")
            || name == "pragma"
    }) {
        return false;
    }
    if prepared.headers.iter().any(|(name, value)| {
        name == "cache-control"
            && (contains_header_token(value, b"no-store")
                || contains_header_token(value, b"no-cache"))
    }) {
        return false;
    }
    if response.headers().contains("set-cookie") {
        return false;
    }
    if response
        .headers()
        .iter()
        .any(|(name, value)| name == "vary" && contains_header_token(value, b"*"))
    {
        return false;
    }
    !response.headers().iter().any(|(name, value)| {
        name == "cache-control"
            && (contains_header_token(value, b"no-store")
                || contains_header_token(value, b"no-cache"))
    })
}

fn contains_header_token(value: &[u8], expected: &[u8]) -> bool {
    value.split(|byte| *byte == b',').any(|part| {
        let token = part
            .iter()
            .copied()
            .map(|byte| byte.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let end = token
            .iter()
            .position(|byte| *byte == b'=')
            .unwrap_or(token.len());
        let mut start = 0;
        let mut stop = end;
        while start < stop && token[start].is_ascii_whitespace() {
            start += 1;
        }
        while stop > start && token[stop - 1].is_ascii_whitespace() {
            stop -= 1;
        }
        token[start..stop]
            .iter()
            .copied()
            .eq(expected.iter().copied())
    })
}

pub(super) fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
