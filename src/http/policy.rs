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
        // 默认只合并无正文的安全方法；其他请求需要请求级显式选择，不能由全局策略误启用。
        let safe_repeatable = prepared.method.is_idempotent_safe() && prepared.body.is_none();
        if !prepared.deduplication_policy.is_enabled()
            || (!safe_repeatable && !prepared.deduplication_opt_in)
        {
            return None;
        }
        // 键复制本身也有容量预算；超出预算只绕过去重，不改变请求能否执行。
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
        // 同一状态锁下完成缓存写入和在途删除，使新请求不会落在两个状态之间。
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
        // 失败也必须撤销占位并发布结果，让 follower 获得同一稳定错误。
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
        // 异步网络阶段不持锁；只有发布时短暂同步修改索引、缓存和完成槽。
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
        // 取消或网络失败同样释放键并唤醒 follower，不缓存错误结果。
        state.in_flight.remove(key);
        flight.publish(result.clone());
    }
}

/// 同步 leader 的 RAII guard，确保 panic 或提前返回不会让 follower 永久等待。
pub(super) struct SyncLeaderGuard<'a> {
    /// 借用拥有 in-flight 索引和缓存的客户端。
    client: &'a HttpClient,
    /// 本次 leader 已占用的请求键。
    key: RequestKey,
    /// 所有 follower 等待的共享完成槽。
    flight: Arc<SyncFlight>,
    /// 本次 leader 独占的已验证请求，供传输与缓存策略读取。
    pub(super) prepared: PreparedRequest,
    /// 是否已经显式发布完成；否则 Drop 发布取消。
    finished: bool,
}

impl<'a> SyncLeaderGuard<'a> {
    /// 接管已经登记的同步 leader，占位清理责任由返回的守卫持有。
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

    /// 发布正常返回的成功或错误结果，并关闭 Drop 的取消兜底。
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
    /// 网络执行异常离开时撤销占位并告知 follower，避免其等待至无谓超时。
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
    /// 借用拥有异步 in-flight 索引和缓存的客户端。
    client: &'a HttpClient,
    /// 本次异步 leader 已占用的请求键。
    key: RequestKey,
    /// 异步 follower 共享的完成槽。
    flight: Arc<AsyncFlight>,
    /// leader 独占的已验证请求。
    pub(super) prepared: PreparedRequest,
    /// 是否已显式发布完成；取消 future 时为 false。
    finished: bool,
}

#[cfg(feature = "http-async")]
impl<'a> AsyncLeaderGuard<'a> {
    /// 接管异步 leader；守卫随执行 future 一起被取消或正常完成。
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

    /// 发布异步执行结果并标记已完成，防止 Drop 再发布取消。
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
    /// 被取消的 leader 仍同步清理共享状态；不创建后台重试或继续请求。
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
    // 仅缓存无正文 GET/HEAD 的成功响应，不把写请求、错误响应或任意 opt-in 直接视为可缓存。
    if !matches!(prepared.method, HttpMethod::Get | HttpMethod::Head)
        || prepared.body.is_some()
        || !response.is_success()
    {
        return false;
    }
    // 认证、cookie、范围与条件请求有额外上下文语义，不进入完成缓存。
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
    // 尊重响应禁止缓存指令和不可复用标记，尤其不能重复传播服务端设置的 cookie。
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

/// 在逗号分隔的 header 指令中查找忽略大小写的 token，忽略其可选参数与周围空白。
fn contains_header_token(value: &[u8], expected: &[u8]) -> bool {
    // Cache-Control 的 no-cache 可带字段参数；只比较等号之前的指令名称。
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

/// 恢复被异常执行污染的短临界区锁，使后续清理与结果发布仍可继续。
pub(super) fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
