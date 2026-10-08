//! 请求合并模式和有界完成缓存策略。

use std::time::Duration;

use super::HttpError;

/// single-flight 的合并模式。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeduplicationMode {
    /// 不合并请求。
    Disabled,
    /// 只合并当前正在执行的相同请求。
    InFlight,
    /// 合并正在执行的请求，并在成功响应上保留显式 TTL 缓存。
    WithCompletedTtl,
}

/// HTTP 请求去重和短期完成缓存策略。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DeduplicationPolicy {
    /// 禁用、仅在途合并或含完成缓存的策略模式。
    mode: DeduplicationMode,
    /// 成功响应的完成缓存寿命；仅 WithCompletedTtl 生效，最大 1 小时。
    ttl: Duration,
    /// 最多追踪的不同在途请求键；达到上限的新键直接独立执行。
    max_inflight_keys: usize,
    /// 完成缓存允许的最大条目数，不超过 1,024。
    max_completed_entries: usize,
    /// 完成缓存内响应体字节总预算，不包括键和 header 的有界开销。
    max_cached_body_bytes: usize,
}

impl Default for DeduplicationPolicy {
    /// 默认只合并正在执行的安全请求，不启用完成缓存。
    fn default() -> Self {
        Self {
            mode: DeduplicationMode::InFlight,
            ttl: Duration::ZERO,
            max_inflight_keys: 1024,
            max_completed_entries: 128,
            max_cached_body_bytes: 8 * 1024 * 1024,
        }
    }
}

impl DeduplicationPolicy {
    /// 禁用请求去重。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    ///
    /// let policy = DeduplicationPolicy::disabled();
    /// assert!(!policy.is_enabled());
    /// ```
    pub fn disabled() -> Self {
        Self {
            mode: DeduplicationMode::Disabled,
            ..Self::default()
        }
    }

    /// 创建只合并 in-flight 请求的策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    ///
    /// let policy = DeduplicationPolicy::in_flight(16).unwrap();
    /// assert_eq!(policy.max_inflight_keys(), 16);
    /// ```
    pub fn in_flight(max_inflight_keys: usize) -> Result<Self, HttpError> {
        // 校验追踪容量；满载时执行层会绕过去重，不会拒绝或无限等待新键。
        validate_key_limit(max_inflight_keys)?;
        Ok(Self {
            mode: DeduplicationMode::InFlight,
            max_inflight_keys,
            ..Self::default()
        })
    }

    /// 创建带完成缓存 TTL 的策略。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = DeduplicationPolicy::with_completed_ttl(
    ///     Duration::from_secs(5),
    ///     16,
    ///     8,
    ///     1024,
    /// )
    /// .unwrap();
    /// assert!(policy.cache_enabled());
    /// ```
    pub fn with_completed_ttl(
        ttl: Duration,
        max_inflight_keys: usize,
        max_completed_entries: usize,
        max_cached_body_bytes: usize,
    ) -> Result<Self, HttpError> {
        // TTL、条目与正文总预算共同限制完成缓存，不使用无限期或无界默认缓存。
        validate_key_limit(max_inflight_keys)?;
        if ttl.is_zero() || ttl > Duration::from_secs(60 * 60) {
            return Err(HttpError::InvalidConfig {
                field: "deduplication_ttl",
            });
        }
        if !(1..=1024).contains(&max_completed_entries) {
            return Err(HttpError::InvalidConfig {
                field: "max_completed_entries",
            });
        }
        if !(1..=64 * 1024 * 1024).contains(&max_cached_body_bytes) {
            return Err(HttpError::InvalidConfig {
                field: "max_cached_body_bytes",
            });
        }
        Ok(Self {
            mode: DeduplicationMode::WithCompletedTtl,
            ttl,
            max_inflight_keys,
            max_completed_entries,
            max_cached_body_bytes,
        })
    }

    /// 返回去重模式。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::{DeduplicationMode, DeduplicationPolicy};
    ///
    /// let policy = DeduplicationPolicy::disabled();
    /// assert_eq!(policy.mode(), DeduplicationMode::Disabled);
    /// ```
    pub fn mode(&self) -> DeduplicationMode {
        self.mode
    }

    /// 返回完成缓存 TTL；仅 `WithCompletedTtl` 模式生效。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = DeduplicationPolicy::with_completed_ttl(
    ///     Duration::from_secs(5),
    ///     16,
    ///     8,
    ///     1024,
    /// )
    /// .unwrap();
    /// assert_eq!(policy.ttl(), Duration::from_secs(5));
    /// ```
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// 返回允许同时追踪的 in-flight key 数量。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    ///
    /// let policy = DeduplicationPolicy::in_flight(16).unwrap();
    /// assert_eq!(policy.max_inflight_keys(), 16);
    /// ```
    pub fn max_inflight_keys(&self) -> usize {
        self.max_inflight_keys
    }

    /// 返回完成缓存最大条目数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = DeduplicationPolicy::with_completed_ttl(
    ///     Duration::from_secs(5),
    ///     16,
    ///     8,
    ///     1024,
    /// )
    /// .unwrap();
    /// assert_eq!(policy.max_completed_entries(), 8);
    /// ```
    pub fn max_completed_entries(&self) -> usize {
        self.max_completed_entries
    }

    /// 返回完成缓存允许占用的响应体总字节数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = DeduplicationPolicy::with_completed_ttl(
    ///     Duration::from_secs(5),
    ///     16,
    ///     8,
    ///     1024,
    /// )
    /// .unwrap();
    /// assert_eq!(policy.max_cached_body_bytes(), 1024);
    /// ```
    pub fn max_cached_body_bytes(&self) -> usize {
        self.max_cached_body_bytes
    }

    /// 返回是否开启请求去重。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    ///
    /// assert!(DeduplicationPolicy::in_flight(16).unwrap().is_enabled());
    /// assert!(!DeduplicationPolicy::disabled().is_enabled());
    /// ```
    pub fn is_enabled(&self) -> bool {
        self.mode != DeduplicationMode::Disabled
    }

    /// 返回是否开启成功响应缓存。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::http::DeduplicationPolicy;
    /// use std::time::Duration;
    ///
    /// let policy = DeduplicationPolicy::with_completed_ttl(
    ///     Duration::from_secs(5),
    ///     16,
    ///     8,
    ///     1024,
    /// )
    /// .unwrap();
    /// assert!(policy.cache_enabled());
    /// ```
    pub fn cache_enabled(&self) -> bool {
        self.mode == DeduplicationMode::WithCompletedTtl && !self.ttl.is_zero()
    }
}

/// 将在途键容量限制在 1..=4,096，避免无界保存请求和等待者状态。
fn validate_key_limit(value: usize) -> Result<(), HttpError> {
    if !(1..=4096).contains(&value) {
        return Err(HttpError::InvalidConfig {
            field: "max_inflight_keys",
        });
    }
    Ok(())
}
