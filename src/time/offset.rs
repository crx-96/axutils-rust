use std::fmt;

/// 已验证的固定 UTC 偏移。
///
/// 正值表示东区，例如 `TimeZoneOffset::from_hours(8)` 为 `+08:00`。它不是 IANA
/// 时区，既不查询夏令时规则，也不执行时区转换。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeZoneOffset(i32);

/// 构造 [`TimeZoneOffset`] 时发生的范围错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZoneOffsetError {
    /// 整小时偏移不在 `-23..=23` 内。
    HoursOutOfRange { hours: i32 },
    /// 秒级偏移不在 `-86_399..=86_399` 内。
    SecondsOutOfRange { seconds: i32 },
}

impl TimeZoneOffset {
    /// UTC（零偏移）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeZoneOffset;
    ///
    /// assert_eq!(TimeZoneOffset::UTC.as_seconds(), 0);
    /// ```
    pub const UTC: Self = Self(0);

    /// 默认固定偏移（`+08:00`）。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeZoneOffset;
    ///
    /// assert_eq!(TimeZoneOffset::DEFAULT.as_seconds(), 28_800);
    /// ```
    pub const DEFAULT: Self = Self(8 * 3_600);

    /// 从整小时构造固定偏移。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeZoneOffset;
    ///
    /// assert_eq!(TimeZoneOffset::from_hours(8).unwrap().as_seconds(), 28_800);
    /// ```
    pub fn from_hours(hours: i32) -> Result<Self, TimeZoneOffsetError> {
        if !(-23..=23).contains(&hours) {
            return Err(TimeZoneOffsetError::HoursOutOfRange { hours });
        }
        Self::from_seconds(hours * 3_600)
    }

    /// 从秒数构造固定偏移。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeZoneOffset;
    ///
    /// assert_eq!(TimeZoneOffset::from_seconds(-19_815).unwrap().as_seconds(), -19_815);
    /// ```
    pub fn from_seconds(seconds: i32) -> Result<Self, TimeZoneOffsetError> {
        if !(-86_399..=86_399).contains(&seconds) {
            return Err(TimeZoneOffsetError::SecondsOutOfRange { seconds });
        }
        Ok(Self(seconds))
    }

    /// 返回与 UTC 相差的秒数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeZoneOffset;
    ///
    /// assert_eq!(TimeZoneOffset::UTC.as_seconds(), 0);
    /// ```
    pub const fn as_seconds(self) -> i32 {
        self.0
    }
}

impl Default for TimeZoneOffset {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for TimeZoneOffsetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HoursOutOfRange { .. } => {
                formatter.write_str("fixed UTC offset hours are out of range")
            }
            Self::SecondsOutOfRange { .. } => {
                formatter.write_str("fixed UTC offset seconds are out of range")
            }
        }
    }
}

impl std::error::Error for TimeZoneOffsetError {}
