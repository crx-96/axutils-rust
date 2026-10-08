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
        // 统一观测入口只记录脱敏结果和耗时，不改变执行/重试结果。
        #[cfg(feature = "tracing")]
        let started = Instant::now();
        let result = self.execute_sync_inner(request);
        #[cfg(feature = "tracing")]
        http_trace::record_completion("sync", &result, started);
        result
    }

    /// 完成本地准备并选择缓存、follower 或独立同步网络执行。
    fn execute_sync_inner(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        // 在已启用异步能力时拒绝当前 Tokio 上下文中的阻塞调用，避免占用 worker。
        #[cfg(feature = "http-async")]
        if Handle::try_current().is_ok() {
            return Err(HttpError::BlockingInAsyncRuntime);
        }

        // 预算从本地准备完成后开始；同一调用的重试和 follower 等待共用这个 deadline。
        let prepared = self.prepare(request)?;
        let deadline = Instant::now() + prepared.timeout;
        let Some(key) = self.coalesce_key(&prepared) else {
            #[cfg(feature = "tracing")]
            http_trace::record_dispatch("sync", &prepared.method, "direct");
            return self.execute_network_sync(&prepared, deadline);
        };

        // 短临界区内选定角色；容量不足时先释放状态锁，再独立发起网络请求。
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

        // 缓存结果直接返回；follower 只等待 leader，不复制网络操作。
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

        // 守卫接管在途键，正常/异常退出都能发布结果并清理占位。
        let guard = SyncLeaderGuard::new(self, key, flight, prepared);
        let result = self.execute_network_sync(&guard.prepared, deadline);
        guard.finish(result)
    }

    /// 在同一个总 deadline 内执行有限尝试，并读取有界响应。
    fn execute_network_sync(
        &self,
        prepared: &PreparedRequest,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpError> {
        let mut retries = 0;
        let mut attempts = 0;
        loop {
            // 每次尝试只使用总预算的剩余部分，不能让重试重新获得完整 timeout。
            let remaining = policy::remaining_until(deadline);
            if remaining.is_zero() {
                return Err(policy::deadline_error(prepared, attempts));
            }
            attempts += 1;
            match self.run_sync_attempt(prepared, remaining) {
                Ok(response) => {
                    // 可重试状态无需读取正文，先释放本次响应再进入退避。
                    let status = response.status().as_u16();
                    if policy::can_retry(prepared, attempts)
                        && prepared.retry_policy.should_retry_status(status)
                    {
                        drop(response);
                        retries += 1;
                        self.wait_for_retry(&prepared.retry_policy, retries, deadline, attempts)?;
                        continue;
                    }
                    // 读取失败区分本地大小/校验错误与可重试的网络中断。
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
                    // 方法可重试且还有次数时才重发；exhausted 只表示次数预算是否耗尽。
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

    /// 生成一次 ureq 请求，传输预算不超过调用方剩余 deadline。
    fn run_sync_attempt(
        &self,
        prepared: &PreparedRequest,
        remaining: Duration,
    ) -> Result<SyncResponse<SyncBody>, AttemptError> {
        // 正文以借用方式交给同步后端；无正文保持其独立传输语义。
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

    /// 在剩余总预算允许时阻塞退避；不足以容纳退避时直接返回超时。
    fn wait_for_retry(
        &self,
        policy: &RetryPolicy,
        retry_number: u32,
        deadline: Instant,
        attempts: u32,
    ) -> Result<(), HttpError> {
        // 退避不能越过总 deadline；不把早到的 deadline 误报为次数耗尽。
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
        // 系统调度可能让实际等待超过期望延迟，醒来后重新检查总预算。
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

/// 将已准备模型转换为 ureq 请求，保留 header 顺序并脱敏本地构造错误。
fn build_ureq_request<S: ureq::AsSendBody>(
    method: &super::HttpMethod,
    url: &Url,
    headers: &HttpHeaders,
    body: S,
) -> Result<SyncRequest<S>, AttemptError> {
    // Custom 变体可被直接构造，因此在进入 provider 前仍校验方法 token。
    let method = SyncMethod::from_bytes(method.as_str().as_bytes())
        .map_err(|_| AttemptError::Local(HttpError::InvalidRequest { field: "method" }))?;
    let mut builder = SyncRequest::builder().method(method).uri(url.as_str());
    // provider 再次解析已验证 header，不允许错误文本穿过本库边界。
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

/// 收集受 header 和正文预算限制的响应，保留实际尝试次数。
fn read_sync_response(
    response: SyncResponse<SyncBody>,
    limit: usize,
    attempts: u32,
) -> Result<HttpResponse, AttemptError> {
    // 先检查 header 容量和可用的正文长度提示，过大响应无需读取正文。
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
    // ureq 的 BodyWithConfig::limit 位于解压前，会把小响应的压缩帧截断。
    // 在输出 reader 上限制到上限加一字节，使原始/解压响应都按返回字节判定超限。
    let mut reader = response
        .into_body()
        .into_reader()
        .take(limit.saturating_add(1) as u64);
    let mut body = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    loop {
        // 累计输出字节而非仅信任 Content-Length，覆盖 chunked 和解压后响应。
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

/// 按 ureq 的错误类型生成稳定分类，不读取 URL 或 provider 的错误文本。
fn map_ureq_error(error: &SyncError) -> HttpTransportErrorKind {
    match error {
        SyncError::Timeout(_) => HttpTransportErrorKind::Timeout,
        SyncError::Tls(_) | SyncError::Rustls(_) => HttpTransportErrorKind::Tls,
        SyncError::Protocol(_) => HttpTransportErrorKind::Protocol,
        // ureq 3.4 将 Rustls 证书/主机校验失败包装为 InvalidData I/O 错误。
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

/// 从正文 reader 的标准 I/O 包装中恢复 ureq 分类；未知包装保持 Other。
fn map_sync_body_error(error: &Error) -> HttpTransportErrorKind {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<SyncError>())
        .map(map_ureq_error)
        .unwrap_or(HttpTransportErrorKind::Other)
}
