//! HTTP 重试策略。

use std::time::Duration;

use super::error::HttpError;
use super::request::HttpMethod;

const MAX_ATTEMPTS: u32 = 16;
const MAX_DELAY: Duration = Duration::from_secs(60);

/// 请求重试策略。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetryPolicy {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    statuses: Vec<u16>,
    allow_non_idempotent: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            statuses: vec![408, 425, 429, 500, 502, 503, 504],
            allow_non_idempotent: false,
        }
    }
}

impl RetryPolicy {
    /// 创建默认策略。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置一次调用允许的最大总网络尝试次数，包括首次请求。
    ///
    /// `1` 表示只发送首次请求并禁用自动重试；默认值为 `3`，不是三次额外重试。
    pub fn with_max_retries(mut self, max_attempts: u32) -> Result<Self, HttpError> {
        if !(1..=MAX_ATTEMPTS).contains(&max_attempts) {
            return Err(HttpError::InvalidConfig {
                field: "max_retries",
            });
        }
        self.max_attempts = max_attempts;
        Ok(self)
    }

    /// 设置指数退避的初始和最大延迟。
    pub fn with_backoff(
        mut self,
        base_delay: Duration,
        max_delay: Duration,
    ) -> Result<Self, HttpError> {
        if base_delay.is_zero()
            || base_delay > max_delay
            || max_delay > MAX_DELAY
            || max_delay.is_zero()
        {
            return Err(HttpError::InvalidConfig { field: "backoff" });
        }
        self.base_delay = base_delay;
        self.max_delay = max_delay;
        Ok(self)
    }

    /// 启用或禁用某个可重试响应状态。
    pub fn with_retry_status(mut self, status: u16, enabled: bool) -> Result<Self, HttpError> {
        if !(100..=599).contains(&status) {
            return Err(HttpError::InvalidConfig {
                field: "retry_status",
            });
        }
        match (enabled, self.statuses.binary_search(&status)) {
            (true, Err(index)) => self.statuses.insert(index, status),
            (false, Ok(index)) => {
                self.statuses.remove(index);
            }
            _ => {}
        }
        Ok(self)
    }

    /// 允许对非幂等方法重试。默认关闭。
    pub fn with_allow_non_idempotent(mut self, allow: bool) -> Self {
        self.allow_non_idempotent = allow;
        self
    }

    /// 返回一次调用允许的最大总网络尝试次数，包括首次请求。
    ///
    /// 方法名沿用 `max_retries` 以保持现有 API 路径；返回值不是额外重试次数。
    pub fn max_retries(&self) -> u32 {
        self.max_attempts
    }

    /// 返回初始退避时间。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new();
    /// assert_eq!(policy.base_delay(), Duration::from_millis(100));
    /// ```
    pub fn base_delay(&self) -> Duration {
        self.base_delay
    }

    /// 返回最大退避时间。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::RetryPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = RetryPolicy::new();
    /// assert_eq!(policy.max_delay(), Duration::from_secs(2));
    /// ```
    pub fn max_delay(&self) -> Duration {
        self.max_delay
    }

    /// 返回是否允许非幂等方法重试。
    pub fn allows_non_idempotent(&self) -> bool {
        self.allow_non_idempotent
    }

    /// 返回当前配置的重试状态码。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::RetryPolicy;
    ///
    /// let policy = RetryPolicy::new();
    /// assert!(policy.retry_statuses().any(|status| *status == 503));
    /// ```
    pub fn retry_statuses(&self) -> impl Iterator<Item = &u16> {
        self.statuses.iter()
    }

    pub(crate) fn can_retry_method(&self, method: &HttpMethod) -> bool {
        self.allow_non_idempotent || method.is_idempotent_safe()
    }

    pub(crate) fn should_retry_status(&self, status: u16) -> bool {
        self.statuses.binary_search(&status).is_ok()
    }

    pub(crate) fn delay_for_retry(&self, retry_number: u32) -> Duration {
        let exponent = retry_number.saturating_sub(1).min(16);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.base_delay
            .checked_mul(factor)
            .unwrap_or(self.max_delay)
            .min(self.max_delay)
    }
}
