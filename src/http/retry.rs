//! HTTP 重试策略。

use std::time::Duration;

use super::error::HttpError;
use super::request::HttpMethod;

/// 包含首次发送在内允许配置的最大总网络尝试次数。
const MAX_ATTEMPTS: u32 = 16;
/// 单次退避允许的最大时长。
const MAX_DELAY: Duration = Duration::from_secs(60);

/// 请求重试策略。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetryPolicy {
    /// 包括首次发送的总尝试次数，默认 3，范围 1..=16。
    max_attempts: u32,
    /// 第一次重试之前的退避时间，后续指数增长。
    base_delay: Duration,
    /// 单次退避时长上限，最多 60 秒。
    max_delay: Duration,
    /// 升序且无重复的可重试 HTTP 状态码，供二分查询。
    statuses: Vec<u16>,
    /// 是否显式允许 GET/HEAD/OPTIONS 之外的方法自动重试，默认 false。
    allow_non_idempotent: bool,
}

impl Default for RetryPolicy {
    /// 默认对安全方法最多尝试三次，使用有上限的指数退避和常见临时状态集合。
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
        // 该历史方法名接收总次数；零次或超过上限均不能生成有效策略。
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
        // 保证初始延迟为正且不大于封顶值，后续计算可以安全饱和到 max_delay。
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
        // 保持排序且去重，启停同一状态是幂等的。
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

    /// 仅默认安全方法或显式允许的其他方法可以进入自动重试。
    pub(super) fn can_retry_method(&self, method: &HttpMethod) -> bool {
        self.allow_non_idempotent || method.is_idempotent_safe()
    }

    /// 使用排序后的配置集合判断状态是否可重试，不隐式添加 provider 策略。
    pub(super) fn should_retry_status(&self, status: u16) -> bool {
        self.statuses.binary_search(&status).is_ok()
    }

    /// 返回从 1 开始的重试序号所对应的指数退避，溢出时饱和到配置上限。
    pub(super) fn delay_for_retry(&self, retry_number: u32) -> Duration {
        // 序号和乘法都显式有界；最终延迟还受整个请求 deadline 约束。
        let exponent = retry_number.saturating_sub(1).min(16);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.base_delay
            .checked_mul(factor)
            .unwrap_or(self.max_delay)
            .min(self.max_delay)
    }
}
