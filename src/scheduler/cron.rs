use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, LocalResult, NaiveDateTime, TimeDelta, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use croner::{
    parser::{CronParser, Seconds, Year},
    Cron,
};

use super::SchedulerError;

const MAX_CRON_BYTES: usize = 256;
const MAX_TIMEZONE_BYTES: usize = 128;
const MAX_OVERLAP_SECONDS: i64 = 172_800;
const MONTH_NAMES: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];
const WEEKDAY_NAMES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

pub(crate) struct CronSchedule {
    cron: Cron,
    timezone: Tz,
}

impl CronSchedule {
    pub(crate) fn parse(expression: &str, timezone: &str) -> Result<Self, SchedulerError> {
        if expression.len() > MAX_CRON_BYTES || !has_supported_fields(expression) {
            return Err(SchedulerError::InvalidCron);
        }
        if timezone.len() > MAX_TIMEZONE_BYTES {
            return Err(SchedulerError::InvalidTimezone);
        }
        let timezone = timezone
            .parse::<Tz>()
            .map_err(|_| SchedulerError::InvalidTimezone)?;
        let cron = CronParser::builder()
            .seconds(Seconds::Required)
            .year(Year::Disallowed)
            .alternative_weekdays(false)
            .dom_and_dow(false)
            .sloppy_ranges(true)
            .build()
            .parse(expression)
            .map_err(|_| SchedulerError::InvalidCron)?;
        Ok(Self { cron, timezone })
    }

    /// 从采样的墙钟时刻计算到下一次有效触发的正延迟，保留采样的亚秒精度。
    pub(crate) fn delay_from(&self, now: SystemTime) -> Result<Duration, SchedulerError> {
        let elapsed = now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SchedulerError::InvalidCron)?;
        let seconds = i64::try_from(elapsed.as_secs()).map_err(|_| SchedulerError::InvalidCron)?;
        let now_utc = DateTime::<Utc>::from_timestamp(seconds, elapsed.subsec_nanos())
            .ok_or(SchedulerError::InvalidCron)?;
        let search_utc = now_utc
            .with_nanosecond(0)
            .ok_or(SchedulerError::InvalidCron)?;
        let search_local = search_utc.with_timezone(&self.timezone);
        let mut next_local = self
            .cron
            .find_next_occurrence(&search_local, false)
            .map_err(|_| SchedulerError::InvalidCron)?;
        // Croner 也会返回回拨后第二次出现的间隔触发点，本库只保留第一次出现的触发点。
        let next_wall_time = next_local.naive_local();
        if matches!(
            self.timezone.from_local_datetime(&next_wall_time),
            LocalResult::Ambiguous(_, later) if next_local == later
        ) {
            next_local = self.next_after_repeated_range(next_wall_time)?;
        }
        if next_local.nanosecond() != 0 {
            return Err(SchedulerError::InvalidCron);
        }
        let next_utc = next_local.with_timezone(&Utc);
        let delay = next_utc
            .signed_duration_since(now_utc)
            .to_std()
            .map_err(|_| SchedulerError::InvalidCron)?;
        if delay.is_zero() {
            return Err(SchedulerError::InvalidCron);
        }
        Ok(delay)
    }

    /// 从已知的重复墙钟时间起，找到重复区间结束后的第一个匹配时刻。
    fn next_after_repeated_range(
        &self,
        repeated_wall_time: NaiveDateTime,
    ) -> Result<DateTime<Tz>, SchedulerError> {
        // 用指数步长找到区间外的探测点，再二分定位第一个不重复的秒，避免逐秒扫描。
        let mut upper = 1_i64;
        loop {
            let probe = repeated_wall_time
                .checked_add_signed(TimeDelta::seconds(upper))
                .ok_or(SchedulerError::InvalidCron)?;
            if !matches!(
                self.timezone.from_local_datetime(&probe),
                LocalResult::Ambiguous(_, _)
            ) {
                break;
            }
            if upper == MAX_OVERLAP_SECONDS {
                return Err(SchedulerError::InvalidCron);
            }
            upper = (upper * 2).min(MAX_OVERLAP_SECONDS);
        }

        let mut lower = 0_i64;
        while lower + 1 < upper {
            let middle = lower + (upper - lower) / 2;
            let probe = repeated_wall_time
                .checked_add_signed(TimeDelta::seconds(middle))
                .ok_or(SchedulerError::InvalidCron)?;
            if matches!(
                self.timezone.from_local_datetime(&probe),
                LocalResult::Ambiguous(_, _)
            ) {
                lower = middle;
            } else {
                upper = middle;
            }
        }

        let boundary_naive = repeated_wall_time
            .checked_add_signed(TimeDelta::seconds(upper))
            .ok_or(SchedulerError::InvalidCron)?;
        let boundary = self
            .timezone
            .from_local_datetime(&boundary_naive)
            .single()
            .ok_or(SchedulerError::InvalidCron)?;
        if self
            .cron
            .is_time_matching(&boundary)
            .map_err(|_| SchedulerError::InvalidCron)?
        {
            return Ok(boundary);
        }
        self.cron
            .find_next_occurrence(&boundary, false)
            .map_err(|_| SchedulerError::InvalidCron)
    }
}

/// 在 Croner 替换名称前校验六段表达式及允许的名称，数值范围和步进由 Croner 解析。
fn has_supported_fields(expression: &str) -> bool {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 6 {
        return false;
    }
    fields.iter().enumerate().all(|(index, field)| {
        // 单值步进保留历史写法 `5/10`，省略起点的 `/10` 仍属于无效输入。
        if field.split(',').any(|item| item.starts_with('/'))
            || field
                .chars()
                .any(|value| matches!(value, '#' | '+' | '?' | '@'))
        {
            return false;
        }
        let names: &[&str] = match index {
            4 => &MONTH_NAMES,
            5 => &WEEKDAY_NAMES,
            _ => &[],
        };
        // 列表、范围和步进分隔符界定完整名称，避免 JANFEB 被上游连续替换成 12。
        field.split([',', '-', '/']).all(|token| {
            !token.bytes().any(|byte| byte.is_ascii_alphabetic())
                || names.iter().any(|name| token.eq_ignore_ascii_case(name))
        })
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use chrono::{TimeZone, Utc};

    use super::CronSchedule;

    fn system_time(timestamp: i64) -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_secs(u64::try_from(timestamp).unwrap())
    }

    #[test]
    fn dst_gap_moves_to_first_valid_instant_and_overlap_is_not_replayed() {
        let gap = CronSchedule::parse("0 30 2 * * *", "America/New_York").unwrap();
        let gap_start = Utc.with_ymd_and_hms(2025, 3, 9, 6, 59, 59).unwrap();
        assert_eq!(
            gap.delay_from(system_time(gap_start.timestamp())).unwrap(),
            Duration::from_secs(1)
        );

        let overlap = CronSchedule::parse("0 30 1 * * *", "America/New_York").unwrap();
        let before_overlap = Utc.with_ymd_and_hms(2025, 11, 2, 4, 59, 59).unwrap();
        assert_eq!(
            overlap
                .delay_from(system_time(before_overlap.timestamp()))
                .unwrap(),
            Duration::from_secs(30 * 60 + 1)
        );
        let first_occurrence = Utc.with_ymd_and_hms(2025, 11, 2, 5, 30, 0).unwrap();
        assert!(
            overlap
                .delay_from(system_time(first_occurrence.timestamp()))
                .unwrap()
                > Duration::from_secs(23 * 60 * 60),
            "public next-occurrence API must not replay the second 01:30"
        );
        let second_hour = Utc.with_ymd_and_hms(2025, 11, 2, 6, 10, 0).unwrap();
        assert_eq!(
            overlap
                .delay_from(system_time(second_hour.timestamp()))
                .unwrap(),
            Duration::from_secs(24 * 60 * 60 + 20 * 60)
        );

        let overlap_interval = CronSchedule::parse("0 */10 * * * *", "America/New_York").unwrap();
        for (hour, minute, expected_delay_minutes) in [(5, 10, 10), (5, 50, 70), (6, 10, 50)] {
            let now = Utc.with_ymd_and_hms(2025, 11, 2, hour, minute, 0).unwrap();
            assert_eq!(
                overlap_interval
                    .delay_from(system_time(now.timestamp()))
                    .unwrap(),
                Duration::from_secs(expected_delay_minutes * 60),
                "unexpected delay from {now}"
            );
        }
    }

    #[test]
    fn dst_overlap_skips_a_half_hour_fold_and_resumes_on_the_next_match() {
        let schedule = CronSchedule::parse("0 5-59/10 * * * *", "Australia/Lord_Howe").unwrap();
        let before_fold = Utc.with_ymd_and_hms(2025, 4, 5, 14, 55, 0).unwrap();
        let during_fold = Utc.with_ymd_and_hms(2025, 4, 5, 15, 15, 0).unwrap();
        assert_eq!(
            schedule
                .delay_from(system_time(before_fold.timestamp()))
                .unwrap(),
            Duration::from_secs(40 * 60)
        );
        assert_eq!(
            schedule
                .delay_from(system_time(during_fold.timestamp()))
                .unwrap(),
            Duration::from_secs(20 * 60)
        );
    }

    #[test]
    fn single_value_steps_keep_the_legacy_range_and_require_a_start() {
        let schedule = CronSchedule::parse("0 5/10 * * * *", "UTC").unwrap();
        let now = Utc.with_ymd_and_hms(2025, 1, 1, 0, 5, 0).unwrap();
        assert_eq!(
            schedule.delay_from(system_time(now.timestamp())).unwrap(),
            Duration::from_secs(10 * 60)
        );
        assert!(CronSchedule::parse("0 /10 * * * *", "UTC").is_err());
        assert!(CronSchedule::parse("0 5,/10 * * * *", "UTC").is_err());
    }

    #[test]
    fn repeated_sunday_range_only_matches_sunday() {
        let schedule = CronSchedule::parse("0 0 0 * * SUN-SUN", "UTC").unwrap();
        let monday = Utc.with_ymd_and_hms(2025, 1, 6, 0, 0, 0).unwrap();
        assert_eq!(
            schedule
                .delay_from(system_time(monday.timestamp()))
                .unwrap(),
            Duration::from_secs(6 * 24 * 60 * 60)
        );
    }

    #[test]
    fn posix_dom_and_dow_use_or_semantics() {
        let schedule = CronSchedule::parse("0 0 0 15 * FRI", "UTC").unwrap();
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(
            schedule.delay_from(system_time(start.timestamp())).unwrap(),
            Duration::from_secs(2 * 24 * 60 * 60)
        );
    }

    #[test]
    fn named_ranges_lists_and_steps_match_the_expected_dates() {
        for (expression, start, next) in [
            ("0 0 0 * jAn-MaY/2,SEP *", (1, 31), (3, 1)),
            ("0 0 0 * jAn-MaY/2,SEP *", (5, 31), (9, 1)),
            ("0 0 0 * JAN/2 *", (1, 31), (3, 1)),
            ("0 0 0 * * MON/2", (1, 6), (1, 8)),
            ("0 0 0 * * mOn-fRi/2,SUN", (1, 6), (1, 8)),
            ("0 0 0 * * mOn-fRi/2,SUN", (1, 10), (1, 12)),
        ] {
            let schedule = CronSchedule::parse(expression, "UTC").unwrap();
            let midnight = |(month, day)| {
                let date = Utc.with_ymd_and_hms(2025, month, day, 0, 0, 0).unwrap();
                system_time(date.timestamp())
            };
            let start = midnight(start);
            assert_eq!(start + schedule.delay_from(start).unwrap(), midnight(next));
        }
    }

    #[test]
    fn names_must_be_complete_month_or_weekday_tokens() {
        for expression in [
            "0 0 0 * JANFEB *",
            "0 0 0 * jan2 *",
            "0 0 0 * 0JAN *",
            "0 0 0 * JAN-MARAPR/2 *",
            "0 0 0 * * SUNMON",
            "0 0 0 * * sun1,FRI",
            "0 0 0 * * 0MON",
            "0 0 0 * MON *",
            "0 0 0 * * JAN",
        ] {
            assert!(
                matches!(
                    CronSchedule::parse(expression, "UTC"),
                    Err(super::SchedulerError::InvalidCron)
                ),
                "{expression}"
            );
        }
    }

    #[test]
    fn standard_names_are_allowed_and_croner_extensions_are_rejected_case_insensitively() {
        assert!(CronSchedule::parse("0 0 0 * JUL WED", "UTC").is_ok());
        for expression in [
            "0 0 0 15W * *",
            "0 0 0 15w * *",
            "0 0 0 L * *",
            "0 0 0 l * *",
            "0 0 0 * * FRI#1",
            "0 0 0 1+2 * *",
            "0 0 0 ? * *",
        ] {
            assert!(
                CronSchedule::parse(expression, "UTC").is_err(),
                "{expression}"
            );
        }
    }

    #[test]
    fn search_is_strictly_later_than_actual_subsecond_time() {
        let schedule = CronSchedule::parse("* * * * * *", "UTC").unwrap();
        let exact = UNIX_EPOCH + Duration::from_secs(1_735_689_600);
        assert_eq!(schedule.delay_from(exact).unwrap(), Duration::from_secs(1));
        assert_eq!(
            schedule
                .delay_from(exact + Duration::from_millis(500))
                .unwrap(),
            Duration::from_millis(500)
        );
        assert!(schedule
            .delay_from(UNIX_EPOCH - Duration::from_secs(1))
            .is_err());
    }

    #[test]
    fn shanghai_midnight_uses_wall_clock_timezone() {
        let schedule = CronSchedule::parse("0 0 0 * * *", "Asia/Shanghai").unwrap();
        let before_midnight = Utc.with_ymd_and_hms(2025, 1, 1, 15, 59, 59).unwrap();
        assert_eq!(
            schedule
                .delay_from(system_time(before_midnight.timestamp()))
                .unwrap(),
            Duration::from_secs(1)
        );
    }
}
