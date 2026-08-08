//! HTTP 客户端执行器。

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ureq::RequestExt;
use url::Url;

#[cfg(all(feature = "http", feature = "tokio"))]
use super::coalesce::{AsyncFlight, AsyncState};
use super::coalesce::{
    RequestKey, SyncFlight, SyncState, MAX_COALESCE_KEY_BODY_BYTES, MAX_COALESCE_KEY_HEADERS_BYTES,
};
use super::config::{DeduplicationPolicy, HttpConfig};
use super::headers::HttpHeaders;
use super::request::{HttpMethod, HttpRequest};
use super::response::HttpResponse;
use super::retry::RetryPolicy;
use super::{HttpError, HttpTransportErrorKind};

/// HTTP 客户端。
///
/// 客户端持有独立的同步和异步连接池；同步入口使用 `ureq`，异步入口使用 `reqwest`。
/// 两者都关闭系统代理、自动重定向、自动压缩和隐式重试，并且不会把第三方错误文本
/// 直接暴露给调用方。
pub struct HttpClient {
    config: HttpConfig,
    sync_agent: ureq::Agent,
    sync_state: Mutex<SyncState>,
    #[cfg(all(feature = "http", feature = "tokio"))]
    async_client: reqwest::Client,
    #[cfg(all(feature = "http", feature = "tokio"))]
    async_state: Mutex<AsyncState>,
}

impl HttpClient {
    /// 根据配置创建客户端。
    pub fn new(config: HttpConfig) -> Result<Self, HttpError> {
        let sync_agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .proxy(None)
            .max_redirects(0)
            .allow_non_standard_methods(true)
            .max_idle_connections_per_host(config.max_idle_connections_per_host())
            .max_idle_connections(config.max_idle_connections_per_host().saturating_mul(4))
            .max_idle_age(config.idle_connection_timeout())
            .timeout_global(Some(config.request_timeout()))
            .timeout_connect(Some(config.connect_timeout()))
            .accept_encoding("")
            .user_agent("")
            .build()
            .new_agent();

        #[cfg(all(feature = "http", feature = "tokio"))]
        let async_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .retry(reqwest::retry::never())
            .no_proxy()
            .no_gzip()
            .timeout(config.request_timeout())
            .connect_timeout(config.connect_timeout())
            .pool_idle_timeout(config.idle_connection_timeout())
            .pool_max_idle_per_host(config.max_idle_connections_per_host())
            .build()
            .map_err(|_| HttpError::ClientBuild)?;

        Ok(Self {
            config,
            sync_agent,
            sync_state: Mutex::new(SyncState::new()),
            #[cfg(all(feature = "http", feature = "tokio"))]
            async_client,
            #[cfg(all(feature = "http", feature = "tokio"))]
            async_state: Mutex::new(AsyncState::new()),
        })
    }

    /// 返回客户端配置。
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }

    /// 同步执行请求。
    ///
    /// 在启用了 `tokio` feature 的进程中，如果当前线程已经处于 Tokio runtime，方法会
    /// 返回 [`HttpError::BlockingInAsyncRuntime`]，避免同步网络调用阻塞异步执行器。
    pub fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        #[cfg(feature = "tokio")]
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(HttpError::BlockingInAsyncRuntime);
        }

        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            return self.execute_network_sync(&prepared, deadline);
        };

        let (flight, leader, cached) = {
            let mut state = recover_lock(&self.sync_state);
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
                return self.execute_network_sync(&prepared, deadline);
            } else {
                let flight = Arc::new(SyncFlight::new());
                state.in_flight.insert(key.clone(), Arc::clone(&flight));
                (flight, true, None)
            }
        };

        if let Some(response) = cached {
            return Ok(response);
        }
        if !leader {
            return flight.wait(deadline);
        }

        let guard = SyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_sync(&guard.prepared, deadline);
        guard.finish(result)
    }

    /// 异步执行请求。
    ///
    /// 该方法只在同时启用 `http` 与 `tokio` feature 时存在，并要求调用方已经运行在
    /// Tokio runtime 中；crate 不创建 runtime，也不会在异步入口中调用 `block_on`。
    #[cfg(all(feature = "http", feature = "tokio"))]
    pub async fn execute_async(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        if tokio::runtime::Handle::try_current().is_err() {
            return Err(HttpError::RuntimeRequired);
        }

        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            return self.execute_network_async(&prepared, deadline).await;
        };

        let (flight, leader, cached, bypass) = {
            let mut state = recover_lock(&self.async_state);
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
            return self.execute_network_async(&prepared, deadline).await;
        }
        if let Some(response) = cached {
            return Ok(response);
        }
        if !leader {
            return flight.wait(deadline).await;
        }

        let guard = AsyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_async(&guard.prepared, deadline).await;
        guard.finish(result)
    }

    fn prepare(&self, request: HttpRequest) -> Result<PreparedRequest, HttpError> {
        let url = request.resolve(self.config.base_url_ref())?;
        let headers = HttpHeaders::merge(self.config.default_headers(), request.headers())?;
        let body = request.body().map(ToOwned::to_owned);
        if let Some(body) = &body {
            if body.len() > self.config.max_request_body_bytes() {
                return Err(HttpError::RequestBodyTooLarge {
                    limit: self.config.max_request_body_bytes(),
                });
            }
        }
        Ok(PreparedRequest {
            url,
            method: request.method().clone(),
            headers,
            body,
            timeout: request.timeout().unwrap_or(self.config.request_timeout()),
            retry_policy: request
                .retry_policy()
                .cloned()
                .unwrap_or_else(|| self.config.retry_policy().clone()),
            deduplication_policy: request
                .deduplication_policy()
                .cloned()
                .unwrap_or_else(|| self.config.deduplication_policy().clone()),
            deduplication_opt_in: request.deduplication_policy().is_some(),
        })
    }

    fn coalesce_key(&self, prepared: &PreparedRequest) -> Option<RequestKey> {
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
            timeout: prepared.timeout,
            retry_policy: prepared.retry_policy.clone(),
            deduplication_policy: prepared.deduplication_policy.clone(),
        })
    }

    fn execute_network_sync(
        &self,
        prepared: &PreparedRequest,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let mut retries = 0;
        let mut attempts = 0;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(transport_error(
                    HttpTransportErrorKind::Timeout,
                    attempts,
                    true,
                ));
            }
            attempts += 1;
            match self.run_sync_attempt(prepared, remaining) {
                Ok(response) => {
                    let status = response.status().as_u16();
                    if prepared.retry_policy.can_retry_method(&prepared.method)
                        && attempts < prepared.retry_policy.max_retries()
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
                            if prepared.retry_policy.can_retry_method(&prepared.method)
                                && attempts < prepared.retry_policy.max_retries()
                            {
                                retries += 1;
                                self.wait_for_retry(
                                    &prepared.retry_policy,
                                    retries,
                                    deadline,
                                    attempts,
                                )?;
                                continue;
                            }
                            return Err(transport_error(
                                kind,
                                attempts,
                                !prepared.retry_policy.can_retry_method(&prepared.method)
                                    || attempts >= prepared.retry_policy.max_retries(),
                            ));
                        }
                    }
                }
                Err(AttemptError::Local(error)) => return Err(error),
                Err(AttemptError::Transport(kind)) => {
                    if prepared.retry_policy.can_retry_method(&prepared.method)
                        && attempts < prepared.retry_policy.max_retries()
                    {
                        retries += 1;
                        self.wait_for_retry(&prepared.retry_policy, retries, deadline, attempts)?;
                        continue;
                    }
                    return Err(transport_error(
                        kind,
                        attempts,
                        !prepared.retry_policy.can_retry_method(&prepared.method)
                            || attempts >= prepared.retry_policy.max_retries(),
                    ));
                }
            }
        }
    }

    fn run_sync_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<ureq::http::Response<ureq::Body>, AttemptError> {
        if let Some(body) = &prepared.body {
            let request = build_ureq_request(
                &prepared.method,
                &prepared.url,
                &prepared.headers,
                body.clone(),
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
        let remaining = deadline.saturating_duration_since(Instant::now());
        if delay >= remaining {
            return Err(transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                true,
            ));
        }
        std::thread::sleep(delay);
        if Instant::now() >= deadline {
            return Err(transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                true,
            ));
        }
        Ok(())
    }

    #[cfg(all(feature = "http", feature = "tokio"))]
    async fn execute_network_async(
        &self,
        prepared: &PreparedRequest,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let mut retries = 0;
        let mut attempts = 0;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(transport_error(
                    HttpTransportErrorKind::Timeout,
                    attempts,
                    true,
                ));
            }
            attempts += 1;
            match self.run_async_attempt(prepared, remaining).await {
                Ok(response) => {
                    let status = response.status().as_u16();
                    if prepared.retry_policy.can_retry_method(&prepared.method)
                        && attempts < prepared.retry_policy.max_retries()
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
                            if prepared.retry_policy.can_retry_method(&prepared.method)
                                && attempts < prepared.retry_policy.max_retries()
                            {
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
                            return Err(transport_error(
                                kind,
                                attempts,
                                !prepared.retry_policy.can_retry_method(&prepared.method)
                                    || attempts >= prepared.retry_policy.max_retries(),
                            ));
                        }
                    }
                }
                Err(AttemptError::Local(error)) => return Err(error),
                Err(AttemptError::Transport(kind)) => {
                    if prepared.retry_policy.can_retry_method(&prepared.method)
                        && attempts < prepared.retry_policy.max_retries()
                    {
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
                    return Err(transport_error(
                        kind,
                        attempts,
                        !prepared.retry_policy.can_retry_method(&prepared.method)
                            || attempts >= prepared.retry_policy.max_retries(),
                    ));
                }
            }
        }
    }

    #[cfg(all(feature = "http", feature = "tokio"))]
    async fn run_async_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<reqwest::Response, AttemptError> {
        let method = reqwest::Method::from_bytes(prepared.method.as_str().as_bytes())
            .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "method" }))?;
        let mut builder = self
            .async_client
            .request(method, prepared.url.as_str())
            .timeout(remaining);
        for entry in prepared.headers.entries() {
            let name = reqwest::header::HeaderName::from_bytes(entry.name.as_bytes())
                .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderName))?;
            let value = reqwest::header::HeaderValue::from_bytes(&entry.value)
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

    #[cfg(all(feature = "http", feature = "tokio"))]
    async fn wait_for_retry_async(
        &self,
        policy: &RetryPolicy,
        retry_number: u32,
        deadline: Instant,
        attempts: u32,
    ) -> Result<(), HttpError> {
        let delay = policy.delay_for_retry(retry_number);
        let remaining = deadline.saturating_duration_since(Instant::now());
        if delay >= remaining {
            return Err(transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                true,
            ));
        }
        tokio::time::sleep(delay).await;
        if Instant::now() >= deadline {
            return Err(transport_error(
                HttpTransportErrorKind::Timeout,
                attempts,
                true,
            ));
        }
        Ok(())
    }

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
            let response = result.as_ref().expect("checked above");
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

    #[cfg(all(feature = "http", feature = "tokio"))]
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
            let response = result.as_ref().expect("checked above");
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

impl fmt::Debug for HttpClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpClient")
            .field("config", &self.config)
            .finish()
    }
}

struct PreparedRequest {
    url: Url,
    method: HttpMethod,
    headers: HttpHeaders,
    body: Option<Vec<u8>>,
    timeout: Duration,
    retry_policy: RetryPolicy,
    deduplication_policy: DeduplicationPolicy,
    deduplication_opt_in: bool,
}

enum AttemptError {
    Transport(HttpTransportErrorKind),
    Local(HttpError),
}

struct SyncLeaderGuard<'a> {
    client: &'a HttpClient,
    key: RequestKey,
    flight: Arc<SyncFlight>,
    prepared: PreparedRequest,
    finished: bool,
}

impl<'a> SyncLeaderGuard<'a> {
    fn new(
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

    fn finish(
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

#[cfg(all(feature = "http", feature = "tokio"))]
struct AsyncLeaderGuard<'a> {
    client: &'a HttpClient,
    key: RequestKey,
    flight: Arc<AsyncFlight>,
    prepared: PreparedRequest,
    finished: bool,
}

#[cfg(all(feature = "http", feature = "tokio"))]
impl<'a> AsyncLeaderGuard<'a> {
    fn new(
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

    fn finish(
        mut self,
        result: Result<HttpResponse, HttpError>,
    ) -> Result<HttpResponse, HttpError> {
        self.finished = true;
        self.client
            .finish_async(&self.key, &self.flight, &self.prepared, &result);
        result
    }
}

#[cfg(all(feature = "http", feature = "tokio"))]
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

fn build_ureq_request<S: ureq::AsSendBody>(
    method: &HttpMethod,
    url: &Url,
    headers: &HttpHeaders,
    body: S,
) -> Result<ureq::http::Request<S>, AttemptError> {
    let method = ureq::http::Method::from_bytes(method.as_str().as_bytes())
        .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "method" }))?;
    let mut builder = ureq::http::Request::builder()
        .method(method)
        .uri(url.as_str());
    for entry in headers.entries() {
        let name = ureq::http::HeaderName::from_bytes(entry.name.as_bytes())
            .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderName))?;
        let value = ureq::http::HeaderValue::from_bytes(&entry.value)
            .map_err(|_| AttemptError::Local(HttpError::InvalidHeaderValue))?;
        builder = builder.header(name, value);
    }
    builder
        .body(body)
        .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "request" }))
}

fn read_sync_response(
    response: ureq::http::Response<ureq::Body>,
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
        let read = std::io::Read::read(&mut reader, &mut buffer)
            .map_err(|error| AttemptError::Transport(map_sync_body_error(&error)))?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read]);
        if body.len() > limit {
            return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
        }
    }
    Ok(HttpResponse::new(status, headers, body, attempts))
}

#[cfg(all(feature = "http", feature = "tokio"))]
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
        body.extend_from_slice(&chunk);
        if body.len() > limit {
            return Err(AttemptError::Local(HttpError::ResponseTooLarge { limit }));
        }
    }
    Ok(HttpResponse::new(status, headers, body, attempts))
}

fn map_ureq_error(error: &ureq::Error) -> HttpTransportErrorKind {
    match error {
        ureq::Error::Timeout(_) => HttpTransportErrorKind::Timeout,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => HttpTransportErrorKind::Tls,
        ureq::Error::Protocol(_) => HttpTransportErrorKind::Protocol,
        ureq::Error::Io(_)
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectionFailed
        | ureq::Error::ConnectProxyFailed(_) => HttpTransportErrorKind::Connection,
        _ => HttpTransportErrorKind::Other,
    }
}

fn map_sync_body_error(error: &std::io::Error) -> HttpTransportErrorKind {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ureq::Error>())
        .map(map_ureq_error)
        .unwrap_or(HttpTransportErrorKind::Other)
}

#[cfg(all(feature = "http", feature = "tokio"))]
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

fn transport_error(kind: HttpTransportErrorKind, attempts: u32, exhausted: bool) -> HttpError {
    HttpError::Transport {
        kind,
        attempts,
        exhausted,
    }
}

fn cache_eligible(prepared: &PreparedRequest, response: &HttpResponse) -> bool {
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

fn recover_lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
