//! Tower 全局并发预算与过载响应适配。

use super::super::{AxumError, AxumServerBuilder};

impl AxumServerBuilder {
    /// 安装 Tower fail-fast 全局并发限制；范围 1..=65,536，满载立即返回脱敏 503。
    ///
    /// 全 Router 的路径、HTTP 方法和 fallback 共享同一份许可；server clone 也不会复制配额。
    /// 许可覆盖 service future，返回响应头后释放，不覆盖后续流式响应 body 的传输时间。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum-tower")] {
    /// let _ = AxumApp::new()
    ///     .into_server_builder()
    ///     .with_concurrency_limit(1)
    ///     .unwrap();
    /// # }
    /// ```
    pub fn with_concurrency_limit(mut self, max: usize) -> Result<Self, AxumError> {
        use axum::{error_handling::HandleErrorLayer, http::StatusCode, BoxError};
        use tower::{
            limit::GlobalConcurrencyLimitLayer,
            load_shed::{error::Overloaded, LoadShedLayer},
            ServiceBuilder,
        };
        // 在安装 layer 前检查配额；错误不会返回已部分修改的 builder。
        if !(1..=65_536).contains(&max) {
            return Err(AxumError::InvalidConfig {
                field: "max_concurrency",
            });
        }
        // Router 会为多个 route 克隆 layer，必须使用共享 semaphore 的全局版本。
        // 外层把 LoadShed 的立即拒绝转换为 503，避免隐式等待或泄漏底层错误文本。
        let stack = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|error: BoxError| async move {
                if error.is::<Overloaded>() {
                    (StatusCode::SERVICE_UNAVAILABLE, "service overloaded")
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, "service error")
                }
            }))
            .layer(LoadShedLayer::new())
            .layer(GlobalConcurrencyLimitLayer::new(max));
        self.router = self.router.layer(stack);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    #[tokio::test]
    async fn regression_global_concurrency_limit_covers_routes_methods_and_fallback() {
        use crate::axum::AxumApp;
        use axum::{
            body::Body,
            http::{Request, StatusCode},
            routing::get,
            Router,
        };
        use tokio::sync::Notify;
        use tower::ServiceExt;

        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let router = Router::new()
            .route(
                "/held",
                get({
                    let entered = entered.clone();
                    let release = release.clone();
                    move || async move {
                        entered.notify_one();
                        release.notified().await;
                        "ok"
                    }
                })
                .post(|| async { "post" }),
            )
            .route("/other", get(|| async { "other" }))
            .fallback(|| async { "fallback" });
        let builder = AxumApp::from_router(router)
            .into_server_builder()
            .with_concurrency_limit(1)
            .unwrap();
        let first_router = builder.router.clone();
        let first = tokio::spawn(async move {
            first_router
                .oneshot(Request::builder().uri("/held").body(Body::empty()).unwrap())
                .await
                .unwrap()
        });
        entered.notified().await;
        let mut statuses = Vec::new();
        for (method, path) in [("GET", "/other"), ("POST", "/held"), ("GET", "/missing")] {
            let response = builder
                .router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            statuses.push(response.status());
        }
        release.notify_one();
        assert_eq!(first.await.unwrap().status(), StatusCode::OK);
        assert_eq!(statuses, [StatusCode::SERVICE_UNAVAILABLE; 3]);
        assert_eq!(
            builder
                .router
                .oneshot(
                    Request::builder()
                        .uri("/other")
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
}
