//! 获取 Unix 时间戳的工具。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::TimeError;

/// 当前时间戳工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct TimeUtils;

impl TimeUtils {
    /// 获取当前 Unix 时间戳，依次返回秒、毫秒、微秒和纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整时间，
    /// 四个精度的值均基于同一次系统时间采样计算。如果系统时间早于 Unix 纪元，
    /// 该方法会 panic；需要显式处理此环境错误时使用 [`Self::try_timestamp`]。
    ///
    /// # Examples
    ///
    /// ```
    /// #![allow(deprecated)]
    ///
    /// use axutils::utils::TimeUtils;
    ///
    /// let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
    /// assert!(seconds > 0);
    /// assert!(milliseconds > 0);
    /// assert!(microseconds > 0);
    /// assert!(nanoseconds > 0);
    /// ```
    #[deprecated(
        note = "use TimeUtils::try_timestamp instead; this method may panic before the Unix epoch"
    )]
    pub fn timestamp() -> (u64, u128, u128, u128) {
        let duration = Self::unix_duration_or_panic();

        (
            duration.as_secs(),
            duration.as_millis(),
            duration.as_micros(),
            duration.as_nanos(),
        )
    }

    /// 获取当前 Unix 时间戳，依次返回秒、毫秒、微秒和纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整时间，四个精度的值均
    /// 基于同一次系统时间采样计算。如果系统时间早于 Unix 纪元，返回
    /// [`TimeError::BeforeUnixEpoch`]，不会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// let result = TimeUtils::try_timestamp();
    /// assert!(result.is_ok());
    /// ```
    pub fn try_timestamp() -> Result<(u64, u128, u128, u128), TimeError> {
        let duration = Self::try_unix_duration()?;

        Ok((
            duration.as_secs(),
            duration.as_millis(),
            duration.as_micros(),
            duration.as_nanos(),
        ))
    }

    /// 获取当前 Unix 时间戳，单位为秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic；需要显式处理此环境错误时使用
    /// [`Self::try_timestamp_seconds`]。
    ///
    /// # Examples
    ///
    /// ```
    /// #![allow(deprecated)]
    ///
    /// use axutils::utils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_seconds();
    /// assert!(timestamp > 0);
    /// ```
    #[deprecated(
        note = "use TimeUtils::try_timestamp_seconds instead; this method may panic before the Unix epoch"
    )]
    pub fn timestamp_seconds() -> u64 {
        Self::unix_duration_or_panic().as_secs()
    }

    /// 获取当前 Unix 时间戳，单位为秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整秒数。如果系统时间早于
    /// Unix 纪元，返回 [`TimeError::BeforeUnixEpoch`]，不会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// let result = TimeUtils::try_timestamp_seconds();
    /// assert!(result.is_ok());
    /// ```
    pub fn try_timestamp_seconds() -> Result<u64, TimeError> {
        Ok(Self::try_unix_duration()?.as_secs())
    }

    /// 获取当前 Unix 时间戳，单位为毫秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的毫秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic；需要显式处理此环境错误时使用
    /// [`Self::try_timestamp_milliseconds`]。
    ///
    /// # Examples
    ///
    /// ```
    /// #![allow(deprecated)]
    ///
    /// use axutils::utils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_milliseconds();
    /// assert!(timestamp > 0);
    /// ```
    #[deprecated(
        note = "use TimeUtils::try_timestamp_milliseconds instead; this method may panic before the Unix epoch"
    )]
    pub fn timestamp_milliseconds() -> u128 {
        Self::unix_duration_or_panic().as_millis()
    }

    /// 获取当前 Unix 时间戳，单位为毫秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整毫秒数。如果系统时间早于
    /// Unix 纪元，返回 [`TimeError::BeforeUnixEpoch`]，不会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// let result = TimeUtils::try_timestamp_milliseconds();
    /// assert!(result.is_ok());
    /// ```
    pub fn try_timestamp_milliseconds() -> Result<u128, TimeError> {
        Ok(Self::try_unix_duration()?.as_millis())
    }

    /// 获取当前 Unix 时间戳，单位为微秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的微秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic；需要显式处理此环境错误时使用
    /// [`Self::try_timestamp_microseconds`]。
    ///
    /// # Examples
    ///
    /// ```
    /// #![allow(deprecated)]
    ///
    /// use axutils::utils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_microseconds();
    /// assert!(timestamp > 0);
    /// ```
    #[deprecated(
        note = "use TimeUtils::try_timestamp_microseconds instead; this method may panic before the Unix epoch"
    )]
    pub fn timestamp_microseconds() -> u128 {
        Self::unix_duration_or_panic().as_micros()
    }

    /// 获取当前 Unix 时间戳，单位为微秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整微秒数。如果系统时间早于
    /// Unix 纪元，返回 [`TimeError::BeforeUnixEpoch`]，不会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// let result = TimeUtils::try_timestamp_microseconds();
    /// assert!(result.is_ok());
    /// ```
    pub fn try_timestamp_microseconds() -> Result<u128, TimeError> {
        Ok(Self::try_unix_duration()?.as_micros())
    }

    /// 获取当前 Unix 时间戳，单位为纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的纳秒数。
    /// 如果系统时间早于 Unix 纪元，该方法会 panic；需要显式处理此环境错误时使用
    /// [`Self::try_timestamp_nanoseconds`]。
    ///
    /// # Examples
    ///
    /// ```
    /// #![allow(deprecated)]
    ///
    /// use axutils::utils::TimeUtils;
    ///
    /// let timestamp = TimeUtils::timestamp_nanoseconds();
    /// assert!(timestamp > 0);
    /// ```
    #[deprecated(
        note = "use TimeUtils::try_timestamp_nanoseconds instead; this method may panic before the Unix epoch"
    )]
    pub fn timestamp_nanoseconds() -> u128 {
        Self::unix_duration_or_panic().as_nanos()
    }

    /// 获取当前 Unix 时间戳，单位为纳秒。
    ///
    /// 返回值表示从 1970-01-01 00:00:00 UTC 到当前时间经过的完整纳秒数。如果系统时间早于
    /// Unix 纪元，返回 [`TimeError::BeforeUnixEpoch`]，不会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// let result = TimeUtils::try_timestamp_nanoseconds();
    /// assert!(result.is_ok());
    /// ```
    pub fn try_timestamp_nanoseconds() -> Result<u128, TimeError> {
        Ok(Self::try_unix_duration()?.as_nanos())
    }

    fn try_unix_duration() -> Result<Duration, TimeError> {
        Self::unix_duration_at(SystemTime::now())
    }

    fn unix_duration_at(time: SystemTime) -> Result<Duration, TimeError> {
        time.duration_since(UNIX_EPOCH)
            .map_err(|_| TimeError::BeforeUnixEpoch)
    }

    fn unix_duration_or_panic() -> Duration {
        Self::try_unix_duration().expect("system time must not be earlier than the Unix epoch")
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{TimeError, TimeUtils};

    #[test]
    fn try_timestamp_entries_are_current() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must not be earlier than the Unix epoch");

        let (combined_seconds, combined_milliseconds, combined_microseconds, combined_nanoseconds) =
            TimeUtils::try_timestamp().expect("current system time should be after the epoch");
        let seconds = TimeUtils::try_timestamp_seconds()
            .expect("current system time should be after the epoch");
        let milliseconds = TimeUtils::try_timestamp_milliseconds()
            .expect("current system time should be after the epoch");
        let microseconds = TimeUtils::try_timestamp_microseconds()
            .expect("current system time should be after the epoch");
        let nanoseconds = TimeUtils::try_timestamp_nanoseconds()
            .expect("current system time should be after the epoch");

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
        assert!(combined_seconds > 0);
        assert!(combined_milliseconds > 0);
        assert!(combined_microseconds > 0);
        assert!(combined_nanoseconds > 0);
    }

    #[test]
    #[allow(deprecated)]
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
    #[allow(deprecated)]
    fn timestamp_components_have_expected_unit_boundaries() {
        let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
        let seconds = seconds as u128;

        assert_eq!(milliseconds / 1_000, seconds);
        assert_eq!(microseconds / 1_000_000, seconds);
        assert_eq!(nanoseconds / 1_000_000_000, seconds);
    }

    #[test]
    #[allow(deprecated)]
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
    fn injected_system_times_classify_before_epoch_epoch_and_after_epoch() {
        let before_epoch = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(
            TimeUtils::unix_duration_at(before_epoch),
            Err(TimeError::BeforeUnixEpoch)
        );

        let at_epoch = TimeUtils::unix_duration_at(UNIX_EPOCH)
            .expect("the Unix epoch must not precede itself");
        assert_eq!(at_epoch, Duration::ZERO);

        let after_epoch = TimeUtils::unix_duration_at(UNIX_EPOCH + Duration::from_secs(1))
            .expect("a time after the Unix epoch should convert");
        assert_eq!(after_epoch, Duration::from_secs(1));
    }
}
