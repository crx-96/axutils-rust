//! 服务声明配置的有限取值；对应执行限制由调用方显式安装 middleware。

use super::AxumError;
use std::time::Duration;

/// Axum 服务的有限边界配置；middleware 默认不自动安装。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxumConfig {
    /// 声明的 service future 预算，默认 30 秒，范围 1 毫秒至 10 分钟。
    service_timeout: Duration,
    /// 声明的请求体字节预算，默认 1 MiB，范围 1 字节至 64 MiB。
    max_body_bytes: usize,
    /// 声明的并发请求预算，默认 1,024，范围 1 至 65,536。
    max_concurrency: usize,
}
impl Default for AxumConfig {
    /// 使用有限默认声明值，不自动向 Router 安装 provider layer。
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
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert_eq!(AxumConfig::new().max_body_bytes(), 1024 * 1024);
    /// # }
    /// ```
    pub fn new() -> Self {
        Self::default()
    }
    /// 设置 service future 预算，范围 1 毫秒..=10 分钟；不是连接/header/drain timeout。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert!(AxumConfig::new()
    ///     .with_service_timeout(std::time::Duration::ZERO)
    ///     .is_err());
    /// # }
    /// ```
    pub fn with_service_timeout(mut self, value: Duration) -> Result<Self, AxumError> {
        // 只接受可用于服务预算的有限时长；具体 timeout layer 必须显式安装。
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
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert!(AxumConfig::new().with_max_body_bytes(0).is_err());
    /// # }
    /// ```
    pub fn with_max_body_bytes(mut self, value: usize) -> Result<Self, AxumError> {
        // 在保存声明前校验范围，防止后续按配置安装 layer 时得到无界预算。
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
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert!(AxumConfig::new().with_max_concurrency(65_537).is_err());
    /// # }
    /// ```
    pub fn with_max_concurrency(mut self, value: usize) -> Result<Self, AxumError> {
        // 零配额与超大配额都作为配置错误处理，不修改已有声明值。
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
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert_eq!(
    ///     AxumConfig::new().service_timeout(),
    ///     std::time::Duration::from_secs(30)
    /// );
    /// # }
    /// ```
    pub fn service_timeout(&self) -> Duration {
        self.service_timeout
    }
    /// 返回请求体边界值。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// let _ = AxumConfig::new().max_body_bytes();
    /// # }
    /// ```
    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
    /// 返回并发边界值。
    /// # Examples
    /// ```rust
    /// # use axutils::axum::*;
    /// # #[cfg(feature = "axum")] {
    /// assert_eq!(AxumConfig::new().max_concurrency(), 1024);
    /// # }
    /// ```
    pub fn max_concurrency(&self) -> usize {
        self.max_concurrency
    }
}
