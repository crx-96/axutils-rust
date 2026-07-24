//! 获取 Unix 时间戳的工具。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 当前时间戳工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeUtils;

impl TimeUtils {
    /// 获取当前 Unix 时间戳，依次返回秒、毫秒、微秒和纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整时间，
    /// 四个精度的值均基于同一次系统时间采样计算。如果系统时间早于 Unix 纪元，
    /// 该方法会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
    /// assert!(seconds > 0);
    /// assert!(milliseconds > 0);
    /// assert!(microseconds > 0);
    /// assert!(nanoseconds > 0);
    /// ```
    pub fn timestamp() -> (u64, u128, u128, u128) {
        let duration = Self::unix_duration();

        (
            duration.as_secs(),
            duration.as_millis(),
            duration.as_micros(),
            duration.as_nanos(),
        )
    }

    /// 获取当前 Unix 时间戳，单位为秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_seconds();
    /// assert!(timestamp > 0);
    /// ```
    pub fn timestamp_seconds() -> u64 {
        Self::unix_duration().as_secs()
    }

    /// 获取当前 Unix 时间戳，单位为毫秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的毫秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_milliseconds();
    /// assert!(timestamp > 0);
    /// ```
    pub fn timestamp_milliseconds() -> u128 {
        Self::unix_duration().as_millis()
    }

    /// 获取当前 Unix 时间戳，单位为微秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的微秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_microseconds();
    /// assert!(timestamp > 0);
    /// ```
    pub fn timestamp_microseconds() -> u128 {
        Self::unix_duration().as_micros()
    }

    /// 获取当前 Unix 时间戳，单位为纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的纳秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_nanoseconds();
    /// assert!(timestamp > 0);
    /// ```
    pub fn timestamp_nanoseconds() -> u128 {
        Self::unix_duration().as_nanos()
    }

    fn unix_duration() -> Duration {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch")
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::TimeUtils;

    #[test]
    fn timestamps_are_current() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch");

        let seconds = TimeUtils::timestamp_seconds();
        let milliseconds = TimeUtils::timestamp_milliseconds();
        let microseconds = TimeUtils::timestamp_microseconds();
        let nanoseconds = TimeUtils::timestamp_nanoseconds();

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch");

        assert!(
            (before.as_secs()..=after.as_secs()).contains(&seconds),
            "seconds timestamp is outside the current range"
        );
        assert!(
            (before.as_millis()..=after.as_millis()).contains(&milliseconds),
            "milliseconds timestamp is outside the current range"
        );
        assert!(
            (before.as_micros()..=after.as_micros()).contains(&microseconds),
            "microseconds timestamp is outside the current range"
        );
        assert!(
            (before.as_nanos()..=after.as_nanos()).contains(&nanoseconds),
            "nanoseconds timestamp is outside the current range"
        );
    }

    #[test]
    fn timestamp_returns_current_values() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch");

        let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch");

        assert!((before.as_secs()..=after.as_secs()).contains(&seconds));
        assert!((before.as_millis()..=after.as_millis()).contains(&milliseconds));
        assert!((before.as_micros()..=after.as_micros()).contains(&microseconds));
        assert!((before.as_nanos()..=after.as_nanos()).contains(&nanoseconds));
    }

    #[test]
    fn timestamp_components_have_expected_unit_boundaries() {
        let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
        let seconds = seconds as u128;

        assert_eq!(milliseconds / 1_000, seconds);
        assert_eq!(microseconds / 1_000_000, seconds);
        assert_eq!(nanoseconds / 1_000_000_000, seconds);
    }

    #[test]
    fn timestamps_have_expected_unit_boundaries() {
        let seconds = TimeUtils::timestamp_seconds() as u128;
        let milliseconds = TimeUtils::timestamp_milliseconds();
        let microseconds = TimeUtils::timestamp_microseconds();
        let nanoseconds = TimeUtils::timestamp_nanoseconds();

        assert!(milliseconds / 1_000 >= seconds);
        assert!(microseconds / 1_000 >= milliseconds);
        assert!(nanoseconds / 1_000 >= microseconds);
    }

    #[test]
    fn unix_epoch_duration_has_zero_for_each_unit() {
        let duration = UNIX_EPOCH
            .duration_since(UNIX_EPOCH)
            .expect("the Unix epoch must not precede itself");

        assert_eq!(duration.as_secs(), 0);
        assert_eq!(duration.as_millis(), 0);
        assert_eq!(duration.as_micros(), 0);
        assert_eq!(duration.as_nanos(), 0);
    }
}
