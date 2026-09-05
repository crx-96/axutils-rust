//! 基于 `ureq` 的同步 HTTP 执行路径。

use std::io::{Error, ErrorKind, Read};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use ureq::{
    http::{
        HeaderName as SyncHeaderName, HeaderValue as SyncHeaderValue, Method as SyncMethod,
        Request as SyncRequest, Response as SyncResponse,
    },
    Body as SyncBody, Error as SyncError, RequestExt,
};
use url::Url;

#[cfg(feature = "tracing")]
use crate::telemetry::http as http_trace;
#[cfg(feature = "http-async")]
use tokio::runtime::Handle;

use super::coalesce::SyncFlight;
use super::policy::{self, SyncLeaderGuard};
use super::prepared::{self, AttemptError, PreparedRequest};
use super::retry::RetryPolicy;
use super::{
    HttpClient, HttpError, HttpHeaders, HttpRequest, HttpResponse, HttpTransportErrorKind,
};

impl HttpClient {
    /// 同步执行请求。
    ///
    /// 在启用了 `http-async` feature 的进程中，如果当前线程已经处于 Tokio runtime，方法会
    /// 返回 [`HttpError::BlockingInAsyncRuntime`]，避免同步网络调用阻塞异步执行器。
    pub fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = self.execute_sync_inner(request);
        #[cfg(feature = "tracing")]
        http_trace::record_completion("sync", &result, started);
        result
    }

    fn execute_sync_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        #[cfg(feature = "http-async")]
        if Handle::try_current().is_ok() {
            return Err(HttpError::BlockingInAsyncRuntime);
        }

        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("sync", &prepared.method, "direct");
            return self.execute_network_sync(&prepared, deadline);
        };

        let (flight, leader, cached) = {
            let mut state = policy::recover_lock(&self.sync_state);
            let cached = if prepared.deduplication_policy.cache_enabled() {
                state.cache.get(&key, Instant::now())
            } else {
                None
            };
            if cached.is_some() {
                (Arc::new(SyncFlight::new()), false, cached)
            } else if let Some(existing) = state.in_flight.get(&key) {
                (Arc::clone(existing), false, None)
            } else if state.in_flight.len() >= prepared.deduplication_policy.max_inflight_keys() {
                drop(state);
                #[cfg(feature = "tracing")]
                http_trace::record_dispatch("sync", &prepared.method, "capacity_bypass");
                return self.execute_network_sync(&prepared, deadline);
            } else {
                let flight = Arc::new(SyncFlight::new());
                state.in_flight.insert(key.clone(), Arc::clone(&flight));
                (flight, true, None)
            }
        };

        if let Some(response) = cached {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("sync", &prepared.method, "cache_hit");
            return Ok(response);
        }
        if !leader {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("sync", &prepared.method, "follower");
            return flight.wait(deadline);
        }

        #[cfg(feature = "tracing")]
        http_trace::record_dispatch("sync", &prepared.method, "leader");

        let guard = SyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_sync(&guard.prepared, deadline);
        guard.finish(result)
    }

    fn execute_network_sync(
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
            match self.run_sync_attempt(prepared, remaining) {
                Ok(response) => {
                    let status = response.status().as_u16();
                    if policy::can_retry(prepared, attempts)
                        && prepared.retry_policy.should_retry_status(status)
                    {
                        drop(response);
                        retries += 1;
                        self.wait_for_retry(&prepared.retry_policy, retries, deadline, attempts)?;
                        continue;
                    }
                    match read_sync_response(
                        response,
                        self.config.max_response_body_bytes(),
                        attempts,
                    ) {
                        Ok(response) => return Ok(response),
                        Err(AttemptError::Local(error)) => return Err(error),
                        Err(AttemptError::Transport(kind)) => {
                            if policy::can_retry(prepared, attempts) {
                                retries += 1;
                                self.wait_for_retry(
                                    &prepared.retry_policy,
                                    retries,
                                    deadline,
                                    attempts,
                                )?;
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
                        self.wait_for_retry(&prepared.retry_policy, retries, deadline, attempts)?;
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

    fn run_sync_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<SyncResponse<SyncBody>, AttemptError> {
        if let Some(body) = &prepared.body {
            let request = build_ureq_request(
                &prepared.method,
                &prepared.url,
                &prepared.headers,
                body.as_slice(),
            )?;
            request
                .with_agent(&self.sync_agent)
                .configure()
                .timeout_global(Some(remaining))
                .timeout_connect(Some(remaining.min(self.config.connect_timeout())))
                .run()
                .map_err(|error| AttemptError::Transport(map_ureq_error(&error)))
        } else {
            let request =
                build_ureq_request(&prepared.method, &prepared.url, &prepared.headers, ())?;
            request
                .with_agent(&self.sync_agent)
                .configure()
                .timeout_global(Some(remaining))
                .timeout_connect(Some(remaining.min(self.config.connect_timeout())))
                .run()
                .map_err(|error| AttemptError::Transport(map_ureq_error(&error)))
        }
    }

    fn wait_for_retry(
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
            http_trace::record_retry("sync", retry_number, attempts, delay, "timeout");
            return Err(prepared::transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                attempts >= policy.max_retries(),
            ));
        }
        thread::sleep(delay);
        if Instant::now() >= deadline {
            #[cfg(feature = "tracing")]
            http_trace::record_retry("sync", retry_number, attempts, delay, "timeout");
            return Err(prepared::transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                attempts >= policy.max_retries(),
            ));
        }
        #[cfg(feature = "tracing")]
        http_trace::record_retry("sync", retry_number, attempts, delay, "scheduled");
        Ok(())
    }
}

fn build_ureq_request<S: ureq::AsSendBody>(
    method: &super::HttpMethod,
    url: &Url,
    headers: &HttpHeaders,
    body: S,
) -> Result<SyncRequest<S>, AttemptError> {
    let method = SyncMethod::from_bytes(method.as_str().as_bytes())
        .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "method" }))?;
    let mut builder = SyncRequest::builder().method(method).uri(url.as_str());
    for entry in headers.entries() {
        let name = SyncHeaderName::from_bytes(entry.name.as_bytes())
            .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderName))?;
        let value = SyncHeaderValue::from_bytes(&entry.value)
            .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderValue))?;
        builder = builder.header(name, value);
    }
    builder
        .body(body)
        .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "request" }))
}

fn read_sync_response(
    response: SyncResponse<SyncBody>,
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
        .body()
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
    }
    let mut reader = response
        .into_body()
        .into_with_config()
        .limit(limit.saturating_add(1) as u64)
        .reader();
    let mut body = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| AttemptError::Transport(map_sync_body_error(&error)))?;
        if read == 0 {
            break;
        }
        if read > limit.saturating_sub(body.len()) {
            return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Ok(HttpResponse::new(status, headers, body, attempts))
}

fn map_ureq_error(error: &SyncError) -> HttpTransportErrorKind {
    match error {
        SyncError::Timeout(_) => HttpTransportErrorKind::Timeout,
        SyncError::Tls(_) | SyncError::Rustls(_) => HttpTransportErrorKind::Tls,
        SyncError::Protocol(_) => HttpTransportErrorKind::Protocol,
        // ureq 3.4 wraps Rustls certificate/hostname failures as InvalidData I/O errors.
        SyncError::Io(error) if error.kind() == ErrorKind::InvalidData => {
            HttpTransportErrorKind::Tls
        }
        SyncError::Io(_)
        | SyncError::HostNotFound
        | SyncError::ConnectionFailed
        | SyncError::ConnectProxyFailed(_) => HttpTransportErrorKind::Connection,
        _ => HttpTransportErrorKind::Other,
    }
}

fn map_sync_body_error(error: &Error) -> HttpTransportErrorKind {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<SyncError>())
        .map(map_ureq_error)
        .unwrap_or(HttpTransportErrorKind::Other)
}
