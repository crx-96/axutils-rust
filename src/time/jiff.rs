use jiff::civil::{Date, DateTime};

use super::facade::TimeUtils;

use super::template::{
    render, Fields, TimeFormatError, TimeValueKind, DATETIME_TEMPLATE, DATE_TEMPLATE,
};
use super::TimeZoneOffset;

impl TimeUtils {
    /// 使用统一模板格式化 Jiff civil 日期。
    ///
    /// `None` 使用 `yyyy-MM-dd`。可用 token 为 `yyyy`、`MM`、`dd`；ASCII 字母字面量须用
    /// 单引号包围，`''` 表示单个引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use jiff::civil::Date;
    ///
    /// let date = Date::new(2024, 2, 29).unwrap();
    /// assert_eq!(TimeUtils::format_date_jiff(date, None).unwrap(), "2024-02-29");
    /// ```
    pub fn format_date_jiff(
        value: Date,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATE_TEMPLATE),
            date_fields(value),
            TimeValueKind::Date,
            None,
        )
    }

    /// 格式化可选 Jiff 日期；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_date_jiff`] 相同：`None` 使用 `yyyy-MM-dd`，仅可用
    /// `yyyy`、`MM`、`dd`，ASCII 字母字面量须以单引号包围，`''` 表示单引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use jiff::civil::Date;
    ///
    /// let value = Date::new(2024, 2, 29).ok();
    /// assert_eq!(TimeUtils::format_option_date_jiff(value, None), Some("2024-02-29".to_owned()));
    /// ```
    pub fn format_option_date_jiff(value: Option<Date>, template: Option<&str>) -> Option<String> {
        value.and_then(|value| Self::format_date_jiff(value, template).ok())
    }

    /// 使用统一模板格式化 Jiff civil 日期时间。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`。可用 token 为 `yyyy`、`MM`、`dd`、`HH`、`mm`、
    /// `ss`、`SSS`；`SSS` 截断纳秒，`XXX` 会返回错误。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use jiff::civil::DateTime;
    ///
    /// let value = DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
    /// assert_eq!(TimeUtils::format_datetime_jiff(value, Some("yyyy/MM/dd HH:mm:ss")).unwrap(), "2024/02/29 01:02:03");
    /// ```
    pub fn format_datetime_jiff(
        value: DateTime,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATETIME_TEMPLATE),
            datetime_fields(value),
            TimeValueKind::DateTime,
            None,
        )
    }

    /// 格式化可选 Jiff civil 日期时间；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_datetime_jiff`] 相同：`None` 使用
    /// `yyyy-MM-dd HH:mm:ss`，可用 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS`；
    /// `SSS` 截断纳秒，ASCII 字母字面量须以单引号包围，`XXX` 不可用。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_datetime_jiff(None, None), None);
    /// ```
    pub fn format_option_datetime_jiff(
        value: Option<DateTime>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_datetime_jiff(value, template).ok())
    }

    /// 使用统一模板格式化 Jiff civil 日期时间及固定 UTC 偏移。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`。`offset` 为 `None` 时使用默认
    /// `+08:00`；偏移只附加到原字段，不执行时区转换。需要输出偏移时，可显式在模板中使用
    /// `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use jiff::civil::DateTime;
    ///
    /// let value = DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
    /// assert_eq!(TimeUtils::format_datetime_with_offset_jiff(value, None, None).unwrap(), "2024-02-29 01:02:03");
    /// ```
    pub fn format_datetime_with_offset_jiff(
        value: DateTime,
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

    /// 格式化可选 Jiff civil 日期时间与固定偏移；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_datetime_with_offset_jiff`] 相同：`template` 为 `None` 时使用
    /// `yyyy-MM-dd HH:mm:ss`，`offset` 为 `None` 时使用 `+08:00`。需要输出偏移时，可显式
    /// 使用 `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// assert_eq!(
    ///     TimeUtils::format_option_datetime_with_offset_jiff(None, None, None),
    ///     None,
    /// );
    /// ```
    pub fn format_option_datetime_with_offset_jiff(
        value: Option<DateTime>,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_datetime_with_offset_jiff(value, offset, template).ok())
    }
}
fn date_fields(value: Date) -> Fields {
    Fields {
        year: value.year().into(),
        month: value.month() as u8,
        day: value.day() as u8,
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
    }
}
fn datetime_fields(value: DateTime) -> Fields {
    Fields {
        year: value.year().into(),
        month: value.month() as u8,
        day: value.day() as u8,
        hour: value.hour() as u8,
        minute: value.minute() as u8,
        second: value.second() as u8,
        nanosecond: value.subsec_nanosecond() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_jiff_values_and_option_contracts() {
        let date = Date::new(-4, 2, 29).unwrap();
        let value = DateTime::new(-4, 2, 29, 0, 3, 4, 987_654_321).unwrap();
        assert_eq!(
            TimeUtils::format_date_jiff(date, None).unwrap(),
            "-0004-02-29"
        );
        assert_eq!(
            TimeUtils::format_datetime_jiff(value, Some("yyyy-MM-dd HH:mm:ss.SSS")).unwrap(),
            "-0004-02-29 00:03:04.987"
        );
        assert_eq!(TimeUtils::format_option_date_jiff(None, None), None);
        assert_eq!(
            TimeUtils::format_option_date_jiff(Some(date), None),
            Some("-0004-02-29".to_owned())
        );
        assert_eq!(
            TimeUtils::format_option_datetime_jiff(Some(value), Some("XXX")),
            None
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_jiff(value, None, None).unwrap(),
            "-0004-02-29 00:03:04"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset_jiff(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_jiff(
                value,
                Some(TimeZoneOffset::from_seconds(19_815).unwrap()),
                Some("yyyy-MM-dd HH:mm:ss XXX")
            )
            .unwrap(),
            "-0004-02-29 00:03:04 +05:30:15"
        );
    }

    #[cfg(not(any(feature = "chrono", feature = "time")))]
    #[test]
    fn jiff_entries_format_dates_and_datetimes() {
        let date = Date::new(2024, 2, 29).unwrap();
        let value = DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
        assert_eq!(
            TimeUtils::format_date_jiff(date, None).unwrap(),
            "2024-02-29"
        );
        assert_eq!(
            TimeUtils::format_option_date_jiff(Some(date), None),
            Some("2024-02-29".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime_jiff(value, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(TimeUtils::format_option_datetime_jiff(None, None), None);
        assert_eq!(
            TimeUtils::format_datetime_with_offset_jiff(value, None, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset_jiff(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
    }
}
