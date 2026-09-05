use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};

use super::facade::TimeUtils;

use super::template::{
    render, Fields, TimeFormatError, TimeValueKind, DATETIME_TEMPLATE, DATE_TEMPLATE,
};
use super::TimeZoneOffset;

impl TimeUtils {
    /// 使用统一模板格式化 Chrono 日期。
    ///
    /// `None` 使用 `yyyy-MM-dd`。可用 token 为 `yyyy`、`MM`、`dd`；时间 token 和 `XXX`
    /// 会返回错误。ASCII 字母字面量须用单引号包围，`''` 表示单个引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use chrono::NaiveDate;
    ///
    /// let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    /// assert_eq!(TimeUtils::format_date_chrono(date, None).unwrap(), "2024-02-29");
    /// ```
    pub fn format_date_chrono(
        value: NaiveDate,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render(
            template.unwrap_or(DATE_TEMPLATE),
            date_fields(value),
            TimeValueKind::Date,
            None,
        )
    }

    /// 格式化可选 Chrono 日期；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_date_chrono`] 相同：`None` 使用 `yyyy-MM-dd`，仅可用
    /// `yyyy`、`MM`、`dd`，ASCII 字母字面量须以单引号包围，`''` 表示单引号。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use chrono::NaiveDate;
    ///
    /// let value = NaiveDate::from_ymd_opt(2024, 2, 29);
    /// assert_eq!(TimeUtils::format_option_date_chrono(value, None), Some("2024-02-29".to_owned()));
    /// ```
    pub fn format_option_date_chrono(
        value: Option<NaiveDate>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_date_chrono(value, template).ok())
    }

    /// 使用统一模板格式化 Chrono 无时区日期时间。
    ///
    /// `None` 使用 `yyyy-MM-dd HH:mm:ss`。可用 token 为 `yyyy`、`MM`、`dd`、`HH`、`mm`、
    /// `ss`、`SSS`；`SSS` 截断纳秒，`XXX` 会返回错误。Chrono 的闰秒内部表示会返回
    /// [`TimeFormatError::UnsupportedLeapSecond`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use chrono::NaiveDate;
    ///
    /// let value = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap().and_hms_opt(1, 2, 3).unwrap();
    /// assert_eq!(TimeUtils::format_datetime_chrono(value, Some("yyyy/MM/dd HH:mm:ss")).unwrap(), "2024/02/29 01:02:03");
    /// ```
    pub fn format_datetime_chrono(
        value: NaiveDateTime,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render_datetime(
            value,
            template.unwrap_or(DATETIME_TEMPLATE),
            TimeValueKind::DateTime,
            None,
        )
    }

    /// 格式化可选 Chrono 无时区日期时间；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_datetime_chrono`] 相同：`None` 使用
    /// `yyyy-MM-dd HH:mm:ss`，可用 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS`；
    /// `SSS` 截断纳秒，ASCII 字母字面量须以单引号包围，`XXX` 不可用。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// assert_eq!(TimeUtils::format_option_datetime_chrono(None, None), None);
    /// ```
    pub fn format_option_datetime_chrono(
        value: Option<NaiveDateTime>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| Self::format_datetime_chrono(value, template).ok())
    }

    /// 使用统一模板格式化 Chrono 无时区日期时间及固定 UTC 偏移。
    ///
    /// `template` 为 `None` 时使用 `yyyy-MM-dd HH:mm:ss`。`offset` 为 `None` 时使用默认
    /// `+08:00`；偏移仅附加到原字段，不执行时区转换。需要输出偏移时，可显式在模板中使用
    /// `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    /// use chrono::NaiveDate;
    ///
    /// let value = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap().and_hms_opt(1, 2, 3).unwrap();
    /// assert_eq!(TimeUtils::format_datetime_with_offset_chrono(value, None, None).unwrap(), "2024-02-29 01:02:03");
    /// ```
    pub fn format_datetime_with_offset_chrono(
        value: NaiveDateTime,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Result<String, TimeFormatError> {
        render_datetime(
            value,
            template.unwrap_or(DATETIME_TEMPLATE),
            TimeValueKind::DateTimeWithOffset,
            Some(offset.unwrap_or_default()),
        )
    }

    /// 格式化可选 Chrono 固定偏移日期时间；输入为 `None` 或格式化失败均返回 `None`。
    ///
    /// 模板规则与 [`Self::format_datetime_with_offset_chrono`] 相同：`template` 为 `None` 时使用
    /// `yyyy-MM-dd HH:mm:ss`，`offset` 为 `None` 时使用 `+08:00`。需要输出偏移时，可显式
    /// 使用 `XXX`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::TimeUtils;
    ///
    /// assert_eq!(
    ///     TimeUtils::format_option_datetime_with_offset_chrono(None, None, None),
    ///     None,
    /// );
    /// ```
    pub fn format_option_datetime_with_offset_chrono(
        value: Option<NaiveDateTime>,
        offset: Option<TimeZoneOffset>,
        template: Option<&str>,
    ) -> Option<String> {
        value.and_then(|value| {
            Self::format_datetime_with_offset_chrono(value, offset, template).ok()
        })
    }
}
fn date_fields(value: NaiveDate) -> Fields {
    Fields {
        year: value.year(),
        month: value.month() as u8,
        day: value.day() as u8,
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
    }
}

fn render_datetime(
    value: NaiveDateTime,
    template: &str,
    kind: TimeValueKind,
    offset: Option<TimeZoneOffset>,
) -> Result<String, TimeFormatError> {
    if value.nanosecond() >= 1_000_000_000 {
        return Err(TimeFormatError::UnsupportedLeapSecond);
    }
    render(
        template,
        Fields {
            year: value.year(),
            month: value.month() as u8,
            day: value.day() as u8,
            hour: value.hour() as u8,
            minute: value.minute() as u8,
            second: value.second() as u8,
            nanosecond: value.nanosecond(),
        },
        kind,
        offset,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_option_contracts() {
        let date = NaiveDate::from_ymd_opt(12_000, 2, 29).unwrap();
        let value = date.and_hms_nano_opt(0, 0, 0, 987_654_321).unwrap();
        assert_eq!(
            TimeUtils::format_date_chrono(date, None).unwrap(),
            "12000-02-29"
        );
        assert_eq!(
            TimeUtils::format_datetime_chrono(value, Some("yyyy-MM-dd HH:mm:ss.SSS")).unwrap(),
            "12000-02-29 00:00:00.987"
        );
        assert_eq!(TimeUtils::format_option_date_chrono(None, None), None);
        assert_eq!(
            TimeUtils::format_option_date_chrono(Some(date), Some("'")),
            None
        );
        assert_eq!(
            TimeUtils::format_option_date_chrono(Some(date), Some("")),
            Some(String::new())
        );
        assert_eq!(
            TimeUtils::format_option_datetime_chrono(Some(value), Some("XXX")),
            None
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_chrono(value, None, None).unwrap(),
            "12000-02-29 00:00:00"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset_chrono(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime_with_offset_chrono(
                value,
                Some(TimeZoneOffset::from_seconds(-19_800).unwrap()),
                Some("yyyy-MM-dd HH:mm:ss XXX")
            )
            .unwrap(),
            "12000-02-29 00:00:00 -05:30"
        );
    }

    #[test]
    fn rejects_chrono_leap_second() {
        let leap = NaiveDate::from_ymd_opt(2016, 12, 31)
            .unwrap()
            .and_hms_nano_opt(23, 59, 59, 1_000_000_000)
            .unwrap();
        assert_eq!(
            TimeUtils::format_datetime_chrono(leap, None),
            Err(TimeFormatError::UnsupportedLeapSecond)
        );
        assert_eq!(
            TimeUtils::format_option_datetime_chrono(Some(leap), None),
            None
        );
    }

    #[cfg(not(any(feature = "time", feature = "jiff")))]
    #[test]
    fn chrono_entries_format_dates_and_datetimes() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        let value = date.and_hms_opt(1, 2, 3).unwrap();
        assert_eq!(
            TimeUtils::format_date_chrono(date, None).unwrap(),
            "2024-02-29"
        );
        assert_eq!(
            TimeUtils::format_option_date_chrono(Some(date), None),
            Some("2024-02-29".to_owned())
        );
        assert_eq!(
            TimeUtils::format_datetime_chrono(value, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(TimeUtils::format_option_datetime_chrono(None, None), None);
        assert_eq!(
            TimeUtils::format_datetime_with_offset_chrono(value, None, None).unwrap(),
            "2024-02-29 01:02:03"
        );
        assert_eq!(
            TimeUtils::format_option_datetime_with_offset_chrono(Some(value), None, Some("XXX")),
            Some("+08:00".to_owned())
        );
    }
}
