use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, LocalResult, TimeDelta, TimeZone, Timelike, Utc};
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
            .build()
            .parse(expression)
            .map_err(|_| SchedulerError::InvalidCron)?;
        Ok(Self { cron, timezone })
    }

    pub(crate) fn delay_from_now(&self) -> Result<Duration, SchedulerError> {
        self.delay_from(SystemTime::now())
    }

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
        if next_local.with_timezone(&Utc) <= now_utc {
            next_local = self.next_after_repeated_hour(&search_local, &now_utc)?;
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

    fn next_after_repeated_hour(
        &self,
        now_local: &DateTime<Tz>,
        now_utc: &DateTime<Utc>,
    ) -> Result<DateTime<Tz>, SchedulerError> {
        let naive_now = now_local.naive_local();
        let LocalResult::Ambiguous(_, later) = self.timezone.from_local_datetime(&naive_now) else {
            return Err(SchedulerError::InvalidCron);
        };
        if later.with_timezone(&Utc) > *now_utc {
            return Err(SchedulerError::InvalidCron);
        }

        // Croner searches on naive wall-clock values and resolves ambiguous values to
        // the earlier offset. Once the later copy of a repeated hour has begun, skip
        // the remainder of that repeated wall-clock range so it cannot be replayed.
        let mut upper = 1_i64;
        loop {
            let probe = naive_now
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
            let probe = naive_now
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

        let boundary_naive = naive_now
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

fn has_supported_fields(expression: &str) -> bool {
    let fields = expression.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 6 {
        return false;
    }
    fields.iter().enumerate().all(|(index, field)| {
        if field
            .chars()
            .any(|value| matches!(value, '#' | '+' | '?' | '@'))
        {
            return false;
        }
        let mut normalized = field.to_ascii_uppercase();
        let names: &[&str] = match index {
            4 => &MONTH_NAMES,
            5 => &WEEKDAY_NAMES,
            _ => &[],
        };
        for name in names {
            normalized = normalized.replace(name, "");
        }
        !normalized.chars().any(|value| value.is_ascii_alphabetic())
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

        let overlap_boundary = CronSchedule::parse("0 */10 * * * *", "America/New_York").unwrap();
        assert_eq!(
            overlap_boundary
                .delay_from(system_time(second_hour.timestamp()))
                .unwrap(),
            Duration::from_secs(50 * 60)
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
