//! Governor 的限流键、脱敏响应及服务运行期间的定期清理。

use futures_timer::Delay;
use tokio::{
    sync::watch::{channel as watch_channel, Sender as WatchSender},
    task::JoinHandle,
};

#[cfg(test)]
use super::super::AxumApp;
use super::super::{AxumError, AxumServerBuilder};
#[cfg(test)]
use axum::body;
use axum::{
    body::Body,
    http::{header::RETRY_AFTER, HeaderValue, Response, StatusCode},
};
use std::{num::NonZeroU32, sync::Arc, time::Duration};
use tower_governor::{
    errors::GovernorError, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
    GovernorLayer,
};

impl AxumServerBuilder {
    /// 按真实 TCP peer IP 安装限流 layer。
    ///
    /// 需要启用 `axum-governor` feature。`replenish_interval` 是补充
    /// 一个配额的周期，必须位于 1 毫秒到 1 小时（含）之间；`burst` 是允许的突发配额，
    /// 必须位于 1..=65,536。方法返回安装了 layer 的 builder；参数越界或 provider 无法生成
    /// 配置时返回 `AxumError::InvalidConfig`。
    ///
    /// `AxumServer::serve*` 会注入 `ConnectInfo<SocketAddr>`，限流键只取真实 TCP peer 地址；
    /// 缺少该扩展时响应脱敏的 500，超额时响应脱敏的 429 和 `Retry-After`。本方法本身不
    /// bind、不访问网络；处理请求时会维护内存限流状态。serve 每 60 秒清理 stale key，shutdown 时取消并等待任务；高基数来源仍应由部署边界限制。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// use axutils::axum::AxumApp;
    /// use std::{num::NonZeroU32, time::Duration};
    ///
    /// let server = AxumApp::new()
    ///     .into_server_builder()
    ///     .with_governor_peer(Duration::from_secs(1), NonZeroU32::new(10).unwrap())?
    ///     .build()?;
    /// # let _ = server;
    /// # Ok::<(), AxumError>(())
    /// ```
    pub fn with_governor_peer(
        mut self,
        replenish_interval: Duration,
        burst: NonZeroU32,
    ) -> Result<Self, AxumError> {
        // 先校验 refill 周期和突发数量，再构造真实 peer IP 的独立限流状态。
        validate(replenish_interval, burst)?;
        let mut builder = GovernorConfigBuilder::default();
        builder.period(replenish_interval).burst_size(burst.get());
        let config = Arc::new(
            builder
                .finish()
                .ok_or(AxumError::InvalidConfig { field: "governor" })?,
        );
        // 保存与 layer 共用的清理闭包；此时不启动任务，任务归 server 的运行阶段所有。
        let cleanup = config.clone();
        self.governor_cleanup.push(Arc::new(move || {
            cleanup.limiter().retain_recent();
            cleanup.limiter().shrink_to_fit();
        }));
        self.router = self
            .router
            .layer(GovernorLayer::new(config).error_handler(sanitized_error));
        Ok(self)
    }
    /// 按未经验证的转发 header 客户端 IP 安装限流 layer。
    ///
    /// 需要启用 `axum-governor` feature。`replenish_interval` 是补充
    /// 一个配额的周期，必须位于 1 毫秒到 1 小时（含）之间；`burst` 必须位于
    /// 1..=65,536。方法返回安装了 layer 的 builder；参数越界或 provider 无法生成配置时返回
    /// `AxumError::InvalidConfig`。
    ///
    /// 此模式无条件信任 `Forwarded`、`X-Forwarded-For` 和 `X-Real-IP`，不验证代理 CIDR；
    /// 只有可信入口代理会覆盖或清除客户端转发 header 时才可使用，否则客户端可伪造限流键。
    /// 无法提取键时响应脱敏的 500，超额时响应脱敏的 429 和 `Retry-After`。本方法本身不
    /// bind、不访问网络；处理请求时维护内存限流状态，serve 每 60 秒清理 stale key。
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use axutils::axum::*;
    /// use axutils::axum::AxumApp;
    /// use std::{num::NonZeroU32, time::Duration};
    ///
    /// let server = AxumApp::new()
    ///     .into_server_builder()
    ///     .with_governor_forwarded_headers_unchecked(
    ///         Duration::from_secs(1),
    ///         NonZeroU32::new(10).unwrap(),
    ///     )?
    ///     .build()?;
    /// # let _ = server;
    /// # Ok::<(), AxumError>(())
    /// ```
    pub fn with_governor_forwarded_headers_unchecked(
        mut self,
        replenish_interval: Duration,
        burst: NonZeroU32,
    ) -> Result<Self, AxumError> {
        // 显式选择信任转发头的 extractor；入口代理清理伪造头是调用方前提。
        validate(replenish_interval, burst)?;
        let mut builder = GovernorConfigBuilder::default();
        builder.period(replenish_interval).burst_size(burst.get());
        let mut builder = builder.key_extractor(SmartIpKeyExtractor);
        let config = Arc::new(
            builder
                .finish()
                .ok_or(AxumError::InvalidConfig { field: "governor" })?,
        );
        // layer 与后台清理共用同一个 limiter，避免清理无关实例。
        let cleanup = config.clone();
        self.governor_cleanup.push(Arc::new(move || {
            cleanup.limiter().retain_recent();
            cleanup.limiter().shrink_to_fit();
        }));
        self.router = self
            .router
            .layer(GovernorLayer::new(config).error_handler(sanitized_error));
        Ok(self)
    }
}
/// 校验 refill 周期和 burst 配额；错误只携带固定字段名。
fn validate(interval: Duration, burst: NonZeroU32) -> Result<(), AxumError> {
    if !(Duration::from_millis(1)..=Duration::from_secs(3600)).contains(&interval) {
        return Err(AxumError::InvalidConfig {
            field: "governor_replenish_interval",
        });
    }
    if burst.get() > 65_536 {
        return Err(AxumError::InvalidConfig {
            field: "governor_burst",
        });
    }
    Ok(())
}
/// 把限流器错误转换为固定响应，仅从超额错误提取 Retry-After 秒数。
fn sanitized_error(error: GovernorError) -> Response<Body> {
    // 不把 extractor/provider 的原始诊断或请求来源写进响应 body。
    let (status, body, retry) = match error {
        GovernorError::TooManyRequests { wait_time, .. } => (
            StatusCode::TOO_MANY_REQUESTS,
            "rate limit exceeded",
            Some(wait_time),
        ),
        GovernorError::UnableToExtractKey => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "rate limit integration error",
            None,
        ),
        GovernorError::Other { code, .. } => (
            if code.is_client_error() || code.is_server_error() {
                code
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            "rate limit error",
            None,
        ),
    };
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    // 只在确有等待时间时设置 Retry-After；无法生成合法 header 时保留既有脱敏响应。
    if let Some(seconds) = retry {
        if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
    }
    response
}

/// 服务运行期间的 stale-key 清理任务集合；正常结束时通知并等待，取消时通知退出。
pub(in crate::axum) struct GovernorCleanupGuard {
    /// 广播停止状态，发送端关闭也会让任务退出。
    stop: WatchSender<bool>,
    /// 每个已安装 limiter 对应的任务句柄，正常关闭时逐个等待。
    handles: Vec<JoinHandle<()>>,
}
impl GovernorCleanupGuard {
    /// 为每个 limiter 启动清理任务，使用独立计时器以兼容无 Tokio time driver 的 runtime。
    pub(in crate::axum) fn start(jobs: &[Arc<dyn Fn() + Send + Sync>]) -> Self {
        // watch 保存状态而非瞬时通知，即使任务尚未首次 poll，也能观察到停止。
        let (stop, receiver) = watch_channel(false);
        let handles = jobs
            .iter()
            .cloned()
            .map(|job| {
                let mut receiver = receiver.clone();
                tokio::spawn(async move {
                    // 周期清理只影响不再活跃的 key；停止/发送端释放时终止循环。
                    loop {
                        tokio::select! {
                            changed = receiver.changed() => {
                                if changed.is_err() || *receiver.borrow() { break; }
                            }
                            _ = Delay::new(std::time::Duration::from_secs(60)) => job(),
                        }
                    }
                })
            })
            .collect();
        Self { stop, handles }
    }
    /// 请求停止并等待所有任务；任一异常退出统一返回不含 panic payload 的错误。
    pub(in crate::axum) async fn stop(mut self) -> Result<(), AxumError> {
        // 所有接收端已经退出时 send 失败也是已停止；仍需回收剩余 JoinHandle。
        let _ = self.stop.send(true);
        let mut failed = false;
        for handle in std::mem::take(&mut self.handles) {
            failed |= handle.await.is_err();
        }
        if failed {
            Err(AxumError::BackgroundTask)
        } else {
            Ok(())
        }
    }
}
impl Drop for GovernorCleanupGuard {
    /// 异常退出无法 await，只发出持久停止状态；任务在下次调度时退出。
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn peer_mode_without_connect_info_returns_sanitized_500() {
        let builder = AxumApp::from_router(Router::new().route("/", get(|| async { "ok" })))
            .into_server_builder()
            .with_governor_peer(Duration::from_secs(1), NonZeroU32::new(1).unwrap())
            .unwrap();
        let response = builder
            .router
            .oneshot(Request::new(Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = body::to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"rate limit integration error");
        assert!(!String::from_utf8_lossy(&body).contains("peer"));
    }
}
