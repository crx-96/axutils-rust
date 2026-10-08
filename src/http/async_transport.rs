//! 基于 `reqwest` 的异步 HTTP 执行路径。

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::{
    header::{HeaderName, HeaderValue},
    Method as AsyncMethod,
};
use tokio::runtime::Handle;
use tokio::time;

#[cfg(feature = "tracing")]
use crate::telemetry::http as http_trace;

use super::coalesce::AsyncFlight;
use super::policy::{self, AsyncLeaderGuard};
use super::prepared::{self, AttemptError, PreparedRequest};
use super::retry::RetryPolicy;
use super::{
    HttpClient, HttpError, HttpHeaders, HttpRequest, HttpResponse, HttpTransportErrorKind,
};

impl HttpClient {
    /// 异步执行请求。
    ///
    /// 该方法只在启用 `http-async` feature 时存在，并要求调用方已经运行在
    /// Tokio runtime 中；crate 不创建 runtime，也不会在异步入口中调用 `block_on`。
    pub async fn execute_async(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        // 统一记录已完成调用；取消 future 不会留下虚假的完成事件。
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = self.execute_async_inner(request).await;
        #[cfg(feature = "tracing")]
        http_trace::record_completion("async", &result, started);
        result
    }

    /// 完成本地准备并选择缓存、follower 或独立异步网络执行。
    async fn execute_async_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        // 使用调用方 runtime；缺少 runtime 时在创建等待器或访问网络前返回稳定错误。
        if Handle::try_current().is_err() {
            return Err(HttpError::RuntimeRequired);
        }

        // 重试与 follower 等待共用本次调用 deadline，不为每次重试重置预算。
        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("async", &prepared.method, "direct");
            return self.execute_network_async(&prepared, deadline).await;
        };

        // 只在内存角色判定期间持同步锁；任何网络或 follower 等待都发生在锁释放后。
        let (flight, leader, cached, bypass) = {
            let mut state = policy::recover_lock(&self.async_state);
            let cached = if prepared.deduplication_policy.cache_enabled() {
                state.cache.get(&key, Instant::now())
            } else {
                None
            };
            if cached.is_some() {
                (Arc::new(AsyncFlight::new()), false, cached, false)
            } else if let Some(existing) = state.in_flight.get(&key) {
                (Arc::clone(existing), false, None, false)
            } else if state.in_flight.len() >= prepared.deduplication_policy.max_inflight_keys() {
                (Arc::new(AsyncFlight::new()), false, None, true)
            } else {
                let flight = Arc::new(AsyncFlight::new());
                state.in_flight.insert(key.clone(), Arc::clone(&flight));
                (flight, true, None, false)
            }
        };

        // 新键超过容量时独立执行；缓存命中直接返回，follower 只等待共享结果。
        if bypass {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("async", &prepared.method, "capacity_bypass");
            return self.execute_network_async(&prepared, deadline).await;
        }
        if let Some(response) = cached {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("async", &prepared.method, "cache_hit");
            return Ok(response);
        }
        if !leader {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("async", &prepared.method, "follower");
            return flight.wait(deadline).await;
        }

        #[cfg(feature = "tracing")]
        http_trace::record_dispatch("async", &prepared.method, "leader");

        // RAII 守卫随 future 取消而释放在途键并通知 follower，网络操作不会被后台续跑。
        let guard = AsyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_async(&guard.prepared, deadline).await;
        guard.finish(result)
    }

    /// 在单一总 deadline 内执行有限异步尝试，保留网络次数与本地错误的区别。
    async fn execute_network_async(
        &self,
        prepared: &PreparedRequest,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let mut retries = 0;
        let mut attempts = 0;
        loop {
            // 每次尝试仅获得尚未消耗的总预算，避免慢请求通过重试无限延长。
            let remaining = policy::remaining_until(deadline);
            if remaining.is_zero() {
                return Err(policy::deadline_error(prepared, attempts));
            }
            attempts += 1;
            match self.run_async_attempt(prepared, remaining).await {
                Ok(response) => {
                    // 可重试状态直接释放响应并退避，不额外读取即将丢弃的正文。
                    let status = response.status().as_u16();
                    if policy::can_retry(prepared, attempts)
                        && prepared.retry_policy.should_retry_status(status)
                    {
                        drop(response);
                        retries += 1;
                        self.wait_for_retry_async(
                            &prepared.retry_policy,
                            retries,
                            deadline,
                            attempts,
                        )
                        .await?;
                        continue;
                    }
                    // 正文读取的本地限额失败立即返回；网络中断仍受方法和次数策略约束。
                    match read_async_response(
                        response,
                        self.config.max_response_body_bytes(),
                        attempts,
                    )
                    .await
                    {
                        Ok(response) => return Ok(response),
                        Err(AttemptError::Local(error)) => return Err(error),
                        Err(AttemptError::Transport(kind)) => {
                            if policy::can_retry(prepared, attempts) {
                                retries += 1;
                                self.wait_for_retry_async(
                                    &prepared.retry_policy,
                                    retries,
                                    deadline,
                                    attempts,
                                )
                                .await?;
                                continue;
                            }
                            return Err(prepared::transport_error(
                                kind,
                                attempts,
                                attempts >= prepared.retry_policy.max_retries(),
                            ));
                        }
                    }
                }
                Err(AttemptError::Local(error)) => return Err(error),
                Err(AttemptError::Transport(kind)) => {
                    // exhausted 只反映网络尝试次数，方法不允许重试本身不会把它置为 true。
                    if policy::can_retry(prepared, attempts) {
                        retries += 1;
                        self.wait_for_retry_async(
                            &prepared.retry_policy,
                            retries,
                            deadline,
                            attempts,
                        )
                        .await?;
                        continue;
                    }
                    return Err(prepared::transport_error(
                        kind,
                        attempts,
                        attempts >= prepared.retry_policy.max_retries(),
                    ));
                }
            }
        }
    }

    /// 将模型转换为一次 reqwest 请求，单次 timeout 使用总预算的剩余值。
    async fn run_async_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<reqwest::Response, AttemptError> {
        // 公开 Custom 变体可直接构造，因此方法和 header 在 provider 边界再次校验。
        let method = AsyncMethod::from_bytes(prepared.method.as_str().as_bytes())
            .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "method" }))?;
        let mut builder = self
            .async_client
            .request(method, prepared.url.as_str())
            .timeout(remaining);
        for entry in prepared.headers.entries() {
            let name = HeaderName::from_bytes(entry.name.as_bytes())
                .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderName))?;
            let value = HeaderValue::from_bytes(&entry.value)
                .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderValue))?;
            builder = builder.header(name, value);
        }
        // reqwest 请求拥有正文；重试所需的原始缓冲区继续由 PreparedRequest 持有。
        if let Some(body) = &prepared.body {
            builder = builder.body(body.clone());
        }
        builder
            .send()
            .await
            .map_err(|error| AttemptError::Transport(map_reqwest_error(&error)))
    }

    /// 仅在总预算能容纳退避时异步等待，返回后重新确认 deadline。
    async fn wait_for_retry_async(
        &self,
        policy: &RetryPolicy,
        retry_number: u32,
        deadline: Instant,
        attempts: u32,
    ) -> Result<(), HttpError> {
        // 不让退避突破总预算，也不把预算不足误报为已经用尽网络尝试次数。
        let delay = policy.delay_for_retry(retry_number);
        let remaining = policy::remaining_until(deadline);
        if delay >= remaining {
            #[cfg(feature = "tracing")]
            http_trace::record_retry("async", retry_number, attempts, delay, "timeout");
            return Err(prepared::transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                attempts >= policy.max_retries(),
            ));
        }
        // 等待可被取消；调度延迟可能超过目标值，因此醒来后重新检查剩余时间。
        time::sleep(delay).await;
        if Instant::now() >= deadline {
            #[cfg(feature = "tracing")]
            http_trace::record_retry("async", retry_number, attempts, delay, "timeout");
            return Err(prepared::transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                attempts >= policy.max_retries(),
            ));
        }
        #[cfg(feature = "tracing")]
        http_trace::record_retry("async", retry_number, attempts, delay, "scheduled");
        Ok(())
    }
}

/// 读取有界响应 header 和正文；未知 Content-Length 时按 chunk 累计限制。
async fn read_async_response(
    mut response: reqwest::Response,
    limit: usize,
    attempts: u32,
) -> Result<HttpResponse, AttemptError> {
    // 先处理有界 header 与长度提示，避免为明显超限正文继续分配。
    let status = response.status().as_u16();
    let mut headers = HttpHeaders::new();
    for (name, value) in response.headers() {
        headers
            .append_internal(name.as_str(), value.as_bytes())
            .map_err(AttemptError::Local)?;
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
    }
    // 每次追加前检查剩余字节预算，不能仅信任服务端声明的长度。
    let mut body = Vec::with_capacity(limit.min(8192));
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AttemptError::Transport(map_reqwest_error(&error)))?
    {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(HttpResponse::new(status, headers, body, attempts))
}

/// 使用 reqwest 的稳定类别接口脱敏；连接阶段 TLS 失败保持 Connection。
fn map_reqwest_error(error: &reqwest::Error) -> HttpTransportErrorKind {
    // reqwest 没有独立 TLS 判定接口，不用错误字符串猜测证书或握手失败。
    if error.is_timeout() {
        HttpTransportErrorKind::Timeout
    } else if error.is_connect() {
        HttpTransportErrorKind::Connection
    } else if error.is_request() {
        HttpTransportErrorKind::Protocol
    } else {
        HttpTransportErrorKind::Other
    }
}
