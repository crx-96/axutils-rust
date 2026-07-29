use crate::utils::TimeUtils;

use super::template::{
    render, Fields, TimeFormatError, TimeValueKind, DATETIME_TEMPLATE, DATE_TEMPLATE,
};
use super::TimeZoneOffset;

impl TimeUtils {
    /// 使用统一模板格式化 `time::Date`；`None` 使用 `yyyy-MM-dd`。
    ///
    /// 可用 token 为 `yyyy`、`MM`、`dd`；ASCII 字母字面量须使用单引号，`''` 表示单引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// assert_eq!(TimeUtils::format_date_time(date, None).unwrap(), "2024-02-29");
    /// ```
    pub fn format_date_time(
        value: ::time::Date,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATE_TEMPLATE),
            date_fields(value),
            TimeValueKind::Date,
            None,
        )
    }

    /// 格式化可选 `time::Date`；输入为 `None` 或模板错误均返回 `None`。
    ///
    /// `None` 使用 `yyyy-MM-dd`；仅可用 `yyyy`、`MM`、`dd`，ASCII 字母字面量须以单引号包围，
    /// `''` 表示单引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_date_time(None, None), None);
    /// ```
    pub fn format_option_date_time(
        value: Option<::time::Date>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_date_time(value, template).ok())
    }

    /// 使用统一模板格式化 `time::PrimitiveDateTime`。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`；`SSS` 截断纳秒，`XXX` 不可用。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// let value = date.with_hms(1, 2, 3).unwrap();
    /// assert_eq!(TimeUtils::format_datetime_time(value, None).unwrap(), "2024-02-29 01:02:03");
    /// ```
    pub fn format_datetime_time(
        value: ::time::PrimitiveDateTime,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATETIME_TEMPLATE),
            datetime_fields(value),
            TimeValueKind::DateTime,
            None,
        )
    }

    /// 格式化可选 `time::PrimitiveDateTime`；输入为 `None` 或模板错误均返回 `None`。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`；可用 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS`。
    /// `SSS` 截断纳秒，`XXX` 不可用；ASCII 字母字面量须以单引号包围。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_datetime_time(None, None), None);
    /// ```
    pub fn format_option_datetime_time(
        value: Option<::time::PrimitiveDateTime>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_datetime_time(value, template).ok())
    }

    /// 使用统一模板格式化 `time::PrimitiveDateTime` 及固定偏移。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`；`offset` 为 `None` 时使用默认
    /// `+08:00`。偏移不会转换原日期时间字段；需要输出偏移时，可显式在模板中使用 `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// let value = date.with_hms(1, 2, 3).unwrap();
    /// assert_eq!(
    ///     TimeUtils::format_datetime_with_offset_time(value, None, None).unwrap(),
    ///     "2024-02-29 01:02:03",
    /// );
    /// ```
    pub fn format_datetime_with_offset_time(
        value: ::time::PrimitiveDateTime,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATETIME_TEMPLATE),
            datetime_fields(value),
            TimeValueKind::DateTimeWithOffset,
            Some(offset.unwrap_or_default()),
        )
    }

    /// 格式化可选 `time::PrimitiveDateTime` 与固定偏移；输入为 `None` 或模板错误均返回 `None`。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`；`offset` 为 `None` 时使用 `+08:00`。
    /// 需要输出偏移时，可显式使用 `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(
    ///     TimeUtils::format_option_datetime_with_offset_time(None, None, None),
    ///     None,
    /// );
    /// ```
    pub fn format_option_datetime_with_offset_time(
        value: Option<::time::PrimitiveDateTime>,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_datetime_with_offset_time(value, offset, template).ok())
    }
}

#[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
impl TimeUtils {
    /// `time` 是唯一日期后端时 [`Self::format_date_time`] 的简写。
    ///
    /// `None` 使用 `yyyy-MM-dd`；仅可用 `yyyy`、`MM`、`dd`，ASCII 字母字面量须以单引号包围。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// assert_eq!(TimeUtils::format_date(date, None).unwrap(), "2024-02-29");
    /// ```
    pub fn format_date(
        value: ::time::Date,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        Self::format_date_time(value, template)
    }

    /// `time` 是唯一日期后端时 [`Self::format_option_date_time`] 的简写。
    ///
    /// `None` 使用 `yyyy-MM-dd`；仅可用 `yyyy`、`MM`、`dd`，ASCII 字母字面量须以单引号包围。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_date(None, None), None);
    /// ```
    pub fn format_option_date(
        value: Option<::time::Date>,
        template: Option<&str>,
    ) -> Option<String> {
        Self::format_option_date_time(value, template)
    }

    /// `time` 是唯一日期后端时 [`Self::format_datetime_time`] 的简写。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`；可用 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// let value = date.with_hms(1, 2, 3).unwrap();
    /// assert_eq!(TimeUtils::format_datetime(value, None).unwrap(), "2024-02-29 01:02:03");
    /// ```
    pub fn format_datetime(
        value: ::time::PrimitiveDateTime,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        Self::format_datetime_time(value, template)
    }

    /// `time` 是唯一日期后端时 [`Self::format_option_datetime_time`] 的简写。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`；可用 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_datetime(None, None), None);
    /// ```
    pub fn format_option_datetime(
        value: Option<::time::PrimitiveDateTime>,
        template: Option<&str>,
    ) -> Option<String> {
        Self::format_option_datetime_time(value, template)
    }

    /// `time` 是唯一日期后端时 [`Self::format_datetime_with_offset_time`] 的简写。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`；`offset` 为 `None` 时使用 `+08:00`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    /// let value = date.with_hms(1, 2, 3).unwrap();
    /// assert_eq!(
    ///     TimeUtils::format_datetime_with_offset(value, None, None).unwrap(),
    ///     "2024-02-29 01:02:03",
    /// );
    /// ```
    pub fn format_datetime_with_offset(
        value: ::time::PrimitiveDateTime,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        Self::format_datetime_with_offset_time(value, offset, template)
    }

    /// `time` 是唯一日期后端时 [`Self::format_option_datetime_with_offset_time`] 的简写。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`；`offset` 为 `None` 时使用 `+08:00`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::TimeUtils;
    ///
    /// assert_eq!(
    ///     TimeUtils::format_option_datetime_with_offset(None, None, None),
    ///     None,
    /// );
    /// ```
    pub fn format_option_datetime_with_offset(
        value: Option<::time::PrimitiveDateTime>,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Option<String> {
        Self::format_option_datetime_with_offset_time(value, offset, template)
    }
}

fn date_fields(value: ::time::Date) -> Fields {
    Fields {
        year: value.year(),
        month: value.month() as u8,
        day: value.day(),
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
    }
}
fn datetime_fields(value: ::time::PrimitiveDateTime) -> Fields {
    Fields {
        year: value.year(),
        month: value.month() as u8,
        day: value.day(),
        hour: value.hour(),
        minute: value.minute(),
        second: value.second(),
        nanosecond: value.nanosecond(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::time::Month;

    #[test]
    fn formats_time_values_and_option_contracts() {
        let date = ::time::Date::from_calendar_date(-4, Month::February, 29).unwrap();
        let value = date.with_hms_nano(0, 3, 4, 987_654_321).unwrap();
        assert_eq!(
            TimeUtils::format_date_time(date, None).unwrap(),
            "-0004-02-29"
        );
        assert_eq!(
            TimeUtils::format_datetime_time(value, Some("yyyy-MM-dd HH:mm:ss.SSS")).unwrap(),
            "-0004-02-29 00:03:04.987"
        );
        assert_eq!(
            TimeUtils::format_option_date_time(Some(date), None),
            Some("-0004-02-29".to_owned())
        );
        assert_eq!(TimeUtils::format_option_datetime_time(None, None), None);
        assert_eq!(
            TimeUtils::format_option_datetime_time(Some(value), Some("XXX")),
            None
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_time(value, None, None).unwrap(),
            "-0004-02-29 00:03:04"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset_time(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_time(
                value,
                Some(TimeZoneOffset::UTC),
                Some("yyyy-MM-dd HH:mm:ss XXX")
            )
            .unwrap(),
            "-0004-02-29 00:03:04 Z"
        );
    }

    #[cfg(not(any(feature = "chrono", feature = "jiff")))]
    #[test]
    fn single_backend_aliases_forward_to_time() {
        let date = ::time::Date::from_calendar_date(2024, Month::February, 29).unwrap();
        let value = date.with_hms(1, 2, 3).unwrap();
        assert_eq!(TimeUtils::format_date(date, None).unwrap(), "2024-02-29");
        assert_eq!(
            TimeUtils::format_option_date(Some(date), None),
            Some("2024-02-29".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime(value, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(TimeUtils::format_option_datetime(None, None), None);
        assert_eq!(
            TimeUtils::format_datetime_with_offset(value, None, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
    }
}
