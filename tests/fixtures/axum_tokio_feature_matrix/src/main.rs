#[cfg(feature = "core")]
fn core_builder() -> axutils::AxumServerBuilder {
    axutils::AxumApp::new().into_server_builder()
}
fn main() {
    #[cfg(feature = "none")]
    let _ = axutils::PathUtils::is_absolute("/");
    #[cfg(feature = "tokio-only")]
    {
        let _ = axutils::TokioConfig::new();
        let _ = std::any::type_name::<axutils::TokioConfig>();
        let _ = std::any::type_name::<axutils::TokioError>();
        let _ = std::any::type_name::<axutils::TokioRuntimeFlavor>();
        let _ = std::any::type_name::<axutils::TokioShutdownReason>();
        let _ = std::any::type_name::<axutils::tokio::TokioConfig>();
        let _ = std::any::type_name::<axutils::tokio::TokioError>();
        let _ = std::any::type_name::<axutils::tokio::TokioRuntimeFlavor>();
        let _ = std::any::type_name::<axutils::tokio::TokioShutdownReason>();
        let _ = std::any::type_name::<axutils::TokioUtils>();
        let _ = std::any::type_name::<axutils::utils::TokioUtils>();
        let _wait = axutils::tokio::wait_for_shutdown;
        let _ = std::any::type_name::<axutils::utils::tokio_utils::TokioUtils>();
    }
    #[cfg(feature = "core")]
    {
        let _ = core_builder().build();
        let _: axum::Router = axutils::AxumApp::create_router();
        let _: axum::Router<String> = axutils::AxumApp::<String>::create_router();
        let _: axum::Router = axutils::AxumUtils::create_router();
        let _: axum::Router<String> = axutils::AxumUtils::create_router();
        let _: axutils::AxumApp = axutils::AxumUtils::create_app();
        let _: axum::Router = axutils::utils::AxumUtils::create_router();
        let _: axum::Router<String> = axutils::utils::AxumUtils::create_router();
        let _: axutils::AxumApp = axutils::utils::axum_utils::AxumUtils::create_app();
        let _ = std::any::type_name::<axutils::AxumApp>();
        let _ = std::any::type_name::<axutils::AxumServerBuilder>();
        let _ = std::any::type_name::<axutils::AxumServer>();
        let _ = std::any::type_name::<axutils::AxumConfig>();
        let _ = std::any::type_name::<axutils::AxumError>();
        let _ = std::any::type_name::<axutils::AxumShutdownHandle>();
        let _ = std::any::type_name::<axutils::AxumShutdownReason>();
        let _ = std::any::type_name::<axutils::AxumServeOutcome>();
        let _ = std::any::type_name::<axutils::axum::AxumApp>();
        let _ = std::any::type_name::<axutils::axum::AxumServerBuilder>();
        let _ = std::any::type_name::<axutils::axum::AxumServer>();
        let _ = std::any::type_name::<axutils::axum::AxumConfig>();
        let _ = std::any::type_name::<axutils::axum::AxumError>();
        let _ = std::any::type_name::<axutils::axum::AxumShutdownHandle>();
        let _ = std::any::type_name::<axutils::axum::AxumShutdownReason>();
        let _ = std::any::type_name::<axutils::axum::AxumServeOutcome>();
        let _ = std::any::type_name::<axutils::AxumUtils>();
        let _ = std::any::type_name::<axutils::utils::AxumUtils>();
        let _ = std::any::type_name::<axutils::utils::axum_utils::AxumUtils>();
    }
    #[cfg(feature = "task-group")]
    {
        let _ = axutils::TokioTaskGroup::new();
        let _ = std::any::type_name::<axutils::tokio::TokioTaskGroup>();
    }
    #[cfg(any(feature = "provider-tower-only", feature = "provider-tower-http-only", feature = "provider-governor-only", feature = "provider-tokio-util-only"))]
    let _ = axutils::PathUtils::is_absolute("/");
    #[cfg(feature = "tower")]
    let _ = core_builder().with_concurrency_limit(1);
    #[cfg(feature = "tower-http")]
    {
        let _ = core_builder().with_body_limit(1024);
        let _ = axutils::AxumTimeoutStatus::RequestTimeout;
        let _ = std::any::type_name::<axutils::AxumCorsConfig>();
        let _ = std::any::type_name::<axutils::AxumCorsOrigin>();
        let _ = std::any::type_name::<axutils::axum::AxumTimeoutStatus>();
        let _ = std::any::type_name::<axutils::axum::AxumCorsConfig>();
        let _ = std::any::type_name::<axutils::axum::AxumCorsOrigin>();
    }
    #[cfg(feature = "governor")]
    let _ = core_builder().with_governor_peer(
        std::time::Duration::from_secs(1),
        std::num::NonZeroU32::new(1).unwrap(),
    );
    #[cfg(feature = "tracing-positive")]
    let _ = core_builder().with_http_trace();
    #[cfg(feature = "negative-no-tokio-root")]
    let _ = std::any::type_name::<axutils::TokioConfig>();
    #[cfg(feature = "negative-no-tokio-module")]
    let _ = std::any::type_name::<axutils::tokio::TokioConfig>();
    #[cfg(feature = "negative-no-tokio-utils")]
    let _ = std::any::type_name::<axutils::utils::TokioUtils>();
    #[cfg(feature = "negative-no-tokio-utils-module")]
    let _ = std::any::type_name::<axutils::utils::tokio_utils::TokioUtils>();
    #[cfg(feature = "negative-no-tokio-axum-module")]
    let _ = std::any::type_name::<axutils::axum::AxumServer>();
    #[cfg(feature = "negative-no-tokio-axum-utils")]
    let _ = std::any::type_name::<axutils::utils::AxumUtils>();
    #[cfg(feature = "negative-no-tokio-axum-utils-module")]
    let _ = std::any::type_name::<axutils::utils::axum_utils::AxumUtils>();
    #[cfg(feature = "negative-no-tokio-axum-server")]
    let _ = std::any::type_name::<axutils::AxumServer>();
    #[cfg(feature = "negative-no-axum-server")]
    let _ = std::any::type_name::<axutils::AxumServer>();
    #[cfg(feature = "negative-no-task-group")]
    let _ = axutils::TokioTaskGroup::new();
    #[cfg(feature = "negative-no-tower-method")]
    let _ = axutils::AxumApp::new().into_server_builder().with_concurrency_limit(1);
    #[cfg(feature = "negative-no-tower-http-method")]
    let _ = axutils::AxumApp::new().into_server_builder().with_body_limit(1024);
    #[cfg(feature = "negative-no-tracing-method")]
    let _ = axutils::AxumApp::new().into_server_builder().with_http_trace();
    #[cfg(feature = "negative-no-governor-method")]
    let _ = axutils::AxumApp::new().into_server_builder().with_governor_peer(
        std::time::Duration::from_secs(1),
        std::num::NonZeroU32::new(1).unwrap(),
    );
}
