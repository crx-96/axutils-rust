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
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = self.execute_async_inner(request).await;
        #[cfg(feature = "tracing")]
        http_trace::record_completion("async", &result, started);
        result
    }

    async fn execute_async_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        if Handle::try_current().is_err() {
            return Err(HttpError::RuntimeRequired);
        }

        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("async", &prepared.method, "direct");
            return self.execute_network_async(&prepared, deadline).await;
        };

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

        let guard = AsyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_async(&guard.prepared, deadline).await;
        guard.finish(result)
    }

    async fn execute_network_async(
        &self,
        prepared: &PreparedRequest,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let mut retries = 0;
        let mut attempts = 0;
        loop {
            let remaining = policy::remaining_until(deadline);
            if remaining.is_zero() {
                return Err(policy::deadline_error(prepared, attempts));
            }
            attempts += 1;
            match self.run_async_attempt(prepared, remaining).await {
                Ok(response) => {
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

    async fn run_async_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<reqwest::Response, AttemptError> {
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
        if let Some(body) = &prepared.body {
            builder = builder.body(body.clone());
        }
        builder
            .send()
            .await
            .map_err(|error| AttemptError::Transport(map_reqwest_error(&error)))
    }

    async fn wait_for_retry_async(
        &self,
        policy: &RetryPolicy,
        retry_number: u32,
        deadline: Instant,
        attempts: u32,
    ) -> Result<(), HttpError> {
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

async fn read_async_response(
    mut response: reqwest::Response,
    limit: usize,
    attempts: u32,
) -> Result<HttpResponse, AttemptError> {
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

fn map_reqwest_error(error: &reqwest::Error) -> HttpTransportErrorKind {
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
