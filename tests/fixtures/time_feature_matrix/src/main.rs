#[cfg(feature = "chrono-only")]
fn main() {
    let date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_chrono(date, None).unwrap();
    let _ = axutils::TimeUtils::format_date(date, None).unwrap();
    let _ = axutils::TimeUtils::format_option_date;
    let _ = axutils::TimeUtils::format_datetime;
    let _ = axutils::TimeUtils::format_option_datetime;
    let _ = axutils::TimeUtils::format_datetime_with_offset;
    let _ = axutils::TimeUtils::format_option_datetime_with_offset;
}

#[cfg(feature = "time-only")]
fn main() {
    let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_time(date, None).unwrap();
    let _ = axutils::TimeUtils::format_date(date, None).unwrap();
    let _ = axutils::TimeUtils::format_option_date;
    let _ = axutils::TimeUtils::format_datetime;
    let _ = axutils::TimeUtils::format_option_datetime;
    let _ = axutils::TimeUtils::format_datetime_with_offset;
    let _ = axutils::TimeUtils::format_option_datetime_with_offset;
}

#[cfg(feature = "jiff-only")]
fn main() {
    let date = jiff::civil::Date::new(2024, 2, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_jiff(date, None).unwrap();
    let _ = axutils::TimeUtils::format_date(date, None).unwrap();
    let _ = axutils::TimeUtils::format_option_date;
    let _ = axutils::TimeUtils::format_datetime;
    let _ = axutils::TimeUtils::format_option_datetime;
    let _ = axutils::TimeUtils::format_datetime_with_offset;
    let _ = axutils::TimeUtils::format_option_datetime_with_offset;
}

#[cfg(feature = "chrono-time")]
fn main() {
    let chrono_date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let time_date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_chrono(chrono_date, None).unwrap();
    let _ = axutils::TimeUtils::format_date_time(time_date, None).unwrap();
}

#[cfg(feature = "chrono-jiff")]
fn main() {
    let chrono_date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let jiff_date = jiff::civil::Date::new(2024, 2, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_chrono(chrono_date, None).unwrap();
    let _ = axutils::TimeUtils::format_date_jiff(jiff_date, None).unwrap();
}

#[cfg(feature = "time-jiff")]
fn main() {
    let time_date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    let jiff_date = jiff::civil::Date::new(2024, 2, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_time(time_date, None).unwrap();
    let _ = axutils::TimeUtils::format_date_jiff(jiff_date, None).unwrap();
}

#[cfg(feature = "all")]
fn main() {
    let chrono_date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let time_date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
    let jiff_date = jiff::civil::Date::new(2024, 2, 29).unwrap();
    let _ = axutils::TimeUtils::format_date_chrono(chrono_date, None).unwrap();
    let _ = axutils::TimeUtils::format_date_time(time_date, None).unwrap();
    let _ = axutils::TimeUtils::format_date_jiff(jiff_date, None).unwrap();
}

#[cfg(any(
    feature = "negative-chrono-time-alias",
    feature = "negative-chrono-jiff-alias",
    feature = "negative-time-jiff-alias",
    feature = "negative-all-alias"
))]
fn main() {
    let _ = axutils::TimeUtils::format_date;
    let _ = axutils::TimeUtils::format_option_date;
    let _ = axutils::TimeUtils::format_datetime;
    let _ = axutils::TimeUtils::format_option_datetime;
    let _ = axutils::TimeUtils::format_datetime_with_offset;
    let _ = axutils::TimeUtils::format_option_datetime_with_offset;
}

#[cfg(feature = "none")]
fn main() {}

#[cfg(not(any(
    feature = "none",
    feature = "chrono-only",
    feature = "time-only",
    feature = "jiff-only",
    feature = "chrono-time",
    feature = "chrono-jiff",
    feature = "time-jiff",
    feature = "all",
    feature = "negative-chrono-time-alias",
    feature = "negative-chrono-jiff-alias",
    feature = "negative-time-jiff-alias",
    feature = "negative-all-alias"
)))]
fn main() {}
