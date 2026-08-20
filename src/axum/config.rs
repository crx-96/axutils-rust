use super::AxumError;
use std::time::Duration;

/// Axum 服务的有限边界配置；middleware 默认不自动安装。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxumConfig {
    service_timeout: Duration,
    max_body_bytes: usize,
    max_concurrency: usize,
}
impl Default for AxumConfig {
    fn default() -> Self {
        Self {
            service_timeout: Duration::from_secs(30),
            max_body_bytes: 1024 * 1024,
            max_concurrency: 1024,
        }
    }
}
impl AxumConfig {
    /// 创建默认边界值，不 bind 或安装 middleware。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert_eq!(axutils::AxumConfig::new().max_body_bytes(),1024*1024); }
    /// ```
    pub fn new() -> Self {
        Self::default()
    }
    /// 设置 service future 预算，范围 1 毫秒..=10 分钟；不是连接/header/drain timeout。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert!(axutils::AxumConfig::new().with_service_timeout(std::time::Duration::ZERO).is_err()); }
    /// ```
    pub fn with_service_timeout(mut self, value: Duration) -> Result<Self, AxumError> {
        if !(Duration::from_millis(1)..=Duration::from_secs(600)).contains(&value) {
            return Err(AxumError::InvalidConfig {
                field: "service_timeout",
            });
        }
        self.service_timeout = value;
        Ok(self)
    }
    /// 设置请求体边界值，范围 1 字节..=64 MiB；需显式安装 body-limit provider 才生效。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert!(axutils::AxumConfig::new().with_max_body_bytes(0).is_err()); }
    /// ```
    pub fn with_max_body_bytes(mut self, value: usize) -> Result<Self, AxumError> {
        if !(1..=64 * 1024 * 1024).contains(&value) {
            return Err(AxumError::InvalidConfig {
                field: "max_body_bytes",
            });
        }
        self.max_body_bytes = value;
        Ok(self)
    }
    /// 设置并发边界值，范围 1..=65,536；需显式 tower provider 才生效。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert!(axutils::AxumConfig::new().with_max_concurrency(65_537).is_err()); }
    /// ```
    pub fn with_max_concurrency(mut self, value: usize) -> Result<Self, AxumError> {
        if !(1..=65_536).contains(&value) {
            return Err(AxumError::InvalidConfig {
                field: "max_concurrency",
            });
        }
        self.max_concurrency = value;
        Ok(self)
    }
    /// 返回 service future 边界值。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert_eq!(axutils::AxumConfig::new().service_timeout(),std::time::Duration::from_secs(30)); }
    /// ```
    pub fn service_timeout(&self) -> Duration {
        self.service_timeout
    }
    /// 返回请求体边界值。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { let _=axutils::AxumConfig::new().max_body_bytes(); }
    /// ```
    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
    /// 返回并发边界值。
    /// # Examples
    /// ```rust
    /// # #[cfg(all(feature="axum",feature="tokio"))] { assert_eq!(axutils::AxumConfig::new().max_concurrency(),1024); }
    /// ```
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }
}
