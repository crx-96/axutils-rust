use std::fmt;

/// 获取 Unix 时间戳时的时间错误。
///
/// 该错误只表达系统时钟早于 Unix 纪元这一稳定分类，不保存系统路径、平台错误文本
/// 或其他环境信息。
/// 当前枚举不是 `#[non_exhaustive]`；调用方可以直接匹配现有变体，也可以保留 wildcard
/// 以便自行兼容未来扩展。
///
/// # Examples
///
/// ```
/// use axutils::TimeError;
///
/// assert!(matches!(TimeError::BeforeUnixEpoch, TimeError::BeforeUnixEpoch));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeError {
    /// 系统时钟早于 Unix 纪元（1970-01-01 00:00:00 UTC）。
    BeforeUnixEpoch,
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BeforeUnixEpoch => formatter.write_str("system time is before the Unix epoch"),
        }
    }
}

impl std::error::Error for TimeError {}

#[cfg(test)]
mod tests {
    use super::TimeError;

    #[test]
    fn before_unix_epoch_error_has_stable_redacted_text() {
        let error = TimeError::BeforeUnixEpoch;

        assert_eq!(error.to_string(), "system time is before the Unix epoch");
        assert_eq!(format!("{error:?}"), "BeforeUnixEpoch");
        assert!(!error.to_string().contains("path"));
        assert!(!error.to_string().contains("environment"));
    }
}
