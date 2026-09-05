use std::fmt;
#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
use std::fmt::Write as _;

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
use super::TimeZoneOffset;

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff"))]
pub(crate) const DATE_TEMPLATE: &str = "yyyy-MM-dd";
#[cfg(any(feature = "chrono", feature = "time", feature = "jiff"))]
pub(crate) const DATETIME_TEMPLATE: &str = "yyyy-MM-dd HH:mm:ss";

/// 统一日期模板中的 token 类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormatToken {
    /// `yyyy`：带符号且绝对值至少为四位的公历年。
    Year,
    /// `MM`：两位月。
    Month,
    /// `dd`：两位日。
    Day,
    /// `HH`：两位小时。
    Hour,
    /// `mm`：两位分钟。
    Minute,
    /// `ss`：两位秒。
    Second,
    /// `SSS`：截断后的三位毫秒。
    Millisecond,
    /// `XXX`：`Z` 或固定 UTC 偏移。
    Offset,
    /// 未被统一模板支持的 ASCII 字母序列。
    UnknownAscii,
}

/// 日期值的类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeValueKind {
    /// 仅包含年、月、日的日期。
    Date,
    /// 不含 UTC 偏移的日期时间。
    DateTime,
    /// 附加固定 UTC 偏移的日期时间。
    DateTimeWithOffset,
}

/// 统一日期模板的校验或渲染错误。
///
/// 错误只记录模板中的字节位置与紧凑 token 类别，不会回显模板或日期值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormatError {
    /// 单引号字面量没有对应的结束引号。
    UnclosedLiteral {
        /// 未闭合单引号在模板中的字节位置。
        position: usize,
    },
    /// 模板包含统一语法未定义的 token。
    UnsupportedToken {
        /// 无效 token 在模板中的字节位置。
        position: usize,
        /// 识别出的 token 类别。
        token: TimeFormatToken,
    },
    /// 模板 token 不适用于当前日期值类别。
    TokenNotSupportedForValue {
        /// 不适用 token 在模板中的字节位置。
        position: usize,
        /// 不适用的 token 类别。
        token: TimeFormatToken,
        /// 当前格式化值的类别。
        value_kind: TimeValueKind,
    },
    /// Chrono 的闰秒内部表示不受统一模板支持。
    UnsupportedLeapSecond,
}

impl fmt::Display for TimeFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclosedLiteral { .. } => {
                formatter.write_str("date template contains an unclosed literal")
            }
            Self::UnsupportedToken { .. } => {
                formatter.write_str("date template contains an unsupported token")
            }
            Self::TokenNotSupportedForValue { .. } => {
                formatter.write_str("date template token is unsupported for this value kind")
            }
            Self::UnsupportedLeapSecond => {
                formatter.write_str("Chrono leap-second values are unsupported")
            }
        }
    }
}

impl std::error::Error for TimeFormatError {}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
#[derive(Clone, Copy)]
pub(crate) struct Fields {
    pub(crate) year: i32,
    pub(crate) month: u8,
    pub(crate) day: u8,
    pub(crate) hour: u8,
    pub(crate) minute: u8,
    pub(crate) second: u8,
    pub(crate) nanosecond: u32,
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
pub(crate) fn render(
    template: &str,
    fields: Fields,
    value_kind: TimeValueKind,
    offset: Option<TimeZoneOffset>,
) -> Result<String, TimeFormatError> {
    let mut output = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut index = 0;
    let mut literal_start = None;
    while index < bytes.len() {
        if bytes[index] == b'\'' {
            if index + 1 < bytes.len() && bytes[index + 1] == b'\'' {
                output.push('\'');
                index += 2;
            } else if literal_start.is_some() {
                literal_start = None;
                index += 1;
            } else {
                literal_start = Some(index);
                index += 1;
            }
            continue;
        }
        if literal_start.is_some() {
            let character = template[index..]
                .chars()
                .next()
                .expect("index is a UTF-8 boundary");
            output.push(character);
            index += character.len_utf8();
            continue;
        }
        if bytes[index].is_ascii_alphabetic() {
            let (token, length) =
                token_at(&bytes[index..]).ok_or(TimeFormatError::UnsupportedToken {
                    position: index,
                    token: TimeFormatToken::UnknownAscii,
                })?;
            render_token(&mut output, token, fields, value_kind, offset, index)?;
            index += length;
        } else {
            let character = template[index..]
                .chars()
                .next()
                .expect("index is a UTF-8 boundary");
            output.push(character);
            index += character.len_utf8();
        }
    }
    if let Some(position) = literal_start {
        return Err(TimeFormatError::UnclosedLiteral { position });
    }
    Ok(output)
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
fn token_at(input: &[u8]) -> Option<(TimeFormatToken, usize)> {
    const TOKENS: [(&[u8], TimeFormatToken); 8] = [
        (b"yyyy", TimeFormatToken::Year),
        (b"SSS", TimeFormatToken::Millisecond),
        (b"XXX", TimeFormatToken::Offset),
        (b"MM", TimeFormatToken::Month),
        (b"dd", TimeFormatToken::Day),
        (b"HH", TimeFormatToken::Hour),
        (b"mm", TimeFormatToken::Minute),
        (b"ss", TimeFormatToken::Second),
    ];
    TOKENS
        .into_iter()
        .find_map(|(text, token)| input.starts_with(text).then_some((token, text.len())))
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
fn render_token(
    output: &mut String,
    token: TimeFormatToken,
    fields: Fields,
    value_kind: TimeValueKind,
    offset: Option<TimeZoneOffset>,
    position: usize,
) -> Result<(), TimeFormatError> {
    let supports = match token {
        TimeFormatToken::Year | TimeFormatToken::Month | TimeFormatToken::Day => true,
        TimeFormatToken::Hour
        | TimeFormatToken::Minute
        | TimeFormatToken::Second
        | TimeFormatToken::Millisecond => value_kind != TimeValueKind::Date,
        TimeFormatToken::Offset => value_kind == TimeValueKind::DateTimeWithOffset,
        TimeFormatToken::UnknownAscii => false,
    };
    if !supports {
        return Err(TimeFormatError::TokenNotSupportedForValue {
            position,
            token,
            value_kind,
        });
    }
    match token {
        TimeFormatToken::Year => write_year(output, fields.year),
        TimeFormatToken::Month => write!(output, "{:02}", fields.month),
        TimeFormatToken::Day => write!(output, "{:02}", fields.day),
        TimeFormatToken::Hour => write!(output, "{:02}", fields.hour),
        TimeFormatToken::Minute => write!(output, "{:02}", fields.minute),
        TimeFormatToken::Second => write!(output, "{:02}", fields.second),
        TimeFormatToken::Millisecond => write!(output, "{:03}", fields.nanosecond / 1_000_000),
        TimeFormatToken::Offset => write_offset(
            output,
            offset.expect("offset value kind always has an offset"),
        ),
        TimeFormatToken::UnknownAscii => unreachable!("unknown token is rejected before rendering"),
    }
    .expect("writing to String cannot fail");
    Ok(())
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
fn write_year(output: &mut String, year: i32) -> fmt::Result {
    if year < 0 {
        write!(output, "-{:04}", -(i64::from(year)))
    } else {
        write!(output, "{year:04}")
    }
}

#[cfg(any(feature = "chrono", feature = "time", feature = "jiff", test))]
fn write_offset(output: &mut String, offset: TimeZoneOffset) -> fmt::Result {
    let seconds = offset.as_seconds();
    if seconds == 0 {
        return output.write_char('Z');
    }
    let sign = if seconds < 0 { '-' } else { '+' };
    let absolute = seconds.unsigned_abs();
    let hours = absolute / 3_600;
    let minutes = absolute % 3_600 / 60;
    let secs = absolute % 60;
    write!(output, "{sign}{hours:02}:{minutes:02}")?;
    if secs != 0 {
        write!(output, ":{secs:02}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimeZoneOffsetError;

    const FIELDS: Fields = Fields {
        year: -1,
        month: 2,
        day: 29,
        hour: 0,
        minute: 3,
        second: 4,
        nanosecond: 987_654_321,
    };

    #[test]
    fn renders_tokens_literals_and_offset_precision() {
        assert_eq!(
            render(
                "yyyy年MM月dd日 'at' HH:mm:ss.SSS XXX",
                FIELDS,
                TimeValueKind::DateTimeWithOffset,
                Some(TimeZoneOffset::from_seconds(19_815).unwrap())
            )
            .unwrap(),
            "-0001年02月29日 at 00:03:04.987 +05:30:15"
        );
        assert_eq!(
            render(
                "XXX",
                FIELDS,
                TimeValueKind::DateTimeWithOffset,
                Some(TimeZoneOffset::UTC)
            )
            .unwrap(),
            "Z"
        );
        assert_eq!(
            render("''", FIELDS, TimeValueKind::Date, None).unwrap(),
            "'"
        );
    }

    #[test]
    fn reports_template_errors_without_echoing_input() {
        assert_eq!(
            render("yyyy 'text", FIELDS, TimeValueKind::Date, None),
            Err(TimeFormatError::UnclosedLiteral { position: 5 })
        );
        assert_eq!(
            render("yyyy-Q", FIELDS, TimeValueKind::Date, None),
            Err(TimeFormatError::UnsupportedToken {
                position: 5,
                token: TimeFormatToken::UnknownAscii
            })
        );
        assert_eq!(
            render("HH", FIELDS, TimeValueKind::Date, None),
            Err(TimeFormatError::TokenNotSupportedForValue {
                position: 0,
                token: TimeFormatToken::Hour,
                value_kind: TimeValueKind::Date
            })
        );
    }

    #[test]
    fn validates_offset_bounds_before_multiplication() {
        assert_eq!(TimeZoneOffset::default(), TimeZoneOffset::DEFAULT);
        assert_eq!(TimeZoneOffset::default().as_seconds(), 28_800);
        assert_eq!(TimeZoneOffset::from_hours(23).unwrap().as_seconds(), 82_800);
        assert_eq!(
            TimeZoneOffset::from_hours(-23).unwrap().as_seconds(),
            -82_800
        );
        assert_eq!(
            TimeZoneOffset::from_seconds(-86_399).unwrap().as_seconds(),
            -86_399
        );
        assert_eq!(
            TimeZoneOffset::from_seconds(86_399).unwrap().as_seconds(),
            86_399
        );
        assert_eq!(
            TimeZoneOffset::from_hours(-24),
            Err(TimeZoneOffsetError::HoursOutOfRange { hours: -24 })
        );
        assert_eq!(
            TimeZoneOffset::from_hours(24),
            Err(TimeZoneOffsetError::HoursOutOfRange { hours: 24 })
        );
        assert_eq!(
            TimeZoneOffset::from_seconds(-86_400),
            Err(TimeZoneOffsetError::SecondsOutOfRange { seconds: -86_400 })
        );
        assert_eq!(
            TimeZoneOffset::from_seconds(86_400),
            Err(TimeZoneOffsetError::SecondsOutOfRange { seconds: 86_400 })
        );
    }
}
