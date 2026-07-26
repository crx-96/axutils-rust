//! 将秒数格式化为中文持续时间字符串的工具。

/// 秒数格式化工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct FormatUtils;

impl FormatUtils {
    /// 将秒数格式化为中文持续时间字符串，最大单位为天。
    ///
    /// 按天、小时、分钟、秒从高到低拆分：从最高的非零单位开始显示，直到秒；
    /// 更高位为零的单位（例如不足一天时的“天”）会被省略，但一旦某个单位非零，
    /// 其后所有更低位单位即使为零也会显示。不处理周、月、年等更大单位，
    /// 也不处理小于一秒的部分；输入为 `0` 时返回 `"0秒"`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FormatUtils;
    ///
    /// assert_eq!(FormatUtils::seconds_to_human(0), "0秒");
    /// assert_eq!(FormatUtils::seconds_to_human(45), "45秒");
    /// assert_eq!(FormatUtils::seconds_to_human(90), "1分钟30秒");
    /// assert_eq!(FormatUtils::seconds_to_human(3600), "1小时0分钟0秒");
    /// assert_eq!(FormatUtils::seconds_to_human(90_061), "1天1小时1分钟1秒");
    /// ```
    pub fn seconds_to_human(seconds: u64) -> String {
        if seconds == 0 {
            return "0秒".to_string();
        }

        let days = seconds / 86_400;
        let hours = seconds / 3_600 % 24;
        let minutes = seconds / 60 % 60;
        let secs = seconds % 60;

        if days > 0 {
            format!("{days}天{hours}小时{minutes}分钟{secs}秒")
        } else if hours > 0 {
            format!("{hours}小时{minutes}分钟{secs}秒")
        } else if minutes > 0 {
            format!("{minutes}分钟{secs}秒")
        } else {
            format!("{secs}秒")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FormatUtils;

    #[test]
    fn zero_seconds_formats_as_zero_seconds() {
        assert_eq!(FormatUtils::seconds_to_human(0), "0秒");
    }

    #[test]
    fn seconds_only_omits_higher_units() {
        assert_eq!(FormatUtils::seconds_to_human(45), "45秒");
    }

    #[test]
    fn minute_boundary_includes_zero_seconds() {
        assert_eq!(FormatUtils::seconds_to_human(60), "1分钟0秒");
    }

    #[test]
    fn minutes_and_seconds_omit_higher_units() {
        assert_eq!(FormatUtils::seconds_to_human(125), "2分钟5秒");
    }

    #[test]
    fn hour_boundary_includes_zero_minutes_and_seconds() {
        assert_eq!(FormatUtils::seconds_to_human(3600), "1小时0分钟0秒");
    }

    #[test]
    fn hours_minutes_and_seconds_omit_days() {
        assert_eq!(FormatUtils::seconds_to_human(7325), "2小时2分钟5秒");
    }

    #[test]
    fn day_boundary_includes_all_lower_units_as_zero() {
        assert_eq!(FormatUtils::seconds_to_human(86_400), "1天0小时0分钟0秒");
    }

    #[test]
    fn days_hours_minutes_and_seconds_all_present() {
        assert_eq!(FormatUtils::seconds_to_human(90_061), "1天1小时1分钟1秒");
    }

    #[test]
    fn formats_u64_max_exactly_without_overflow() {
        assert_eq!(
            FormatUtils::seconds_to_human(u64::MAX),
            "213503982334601天7小时0分钟15秒"
        );
    }
}
