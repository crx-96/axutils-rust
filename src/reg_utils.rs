//! 基于正则表达式的常用格式校验工具。

use std::sync::OnceLock;

use regex::Regex;

const EMAIL_PATTERN: &str = r"^[^\s@.]+(?:\.[^\s@.]+)*@[^\s@.]+(?:\.[^\s@.]+)+$";
const PHONE_CN_PATTERN: &str = r"^1[3-9][0-9]{9}$";

static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
static PHONE_CN_REGEX: OnceLock<Regex> = OnceLock::new();

/// 正则表达式格式校验工具。
#[derive(Debug, Clone, Copy, Default)]
pub struct RegUtils;

impl RegUtils {
    /// 校验字符串是否符合常见电子邮箱地址格式。
    ///
    /// 校验使用的正则表达式为
    /// `r"^[^\s@.]+(?:\.[^\s@.]+)*@[^\s@.]+(?:\.[^\s@.]+)+$"`。
    /// 该方法只返回格式校验结果，不会验证邮箱是否真实存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RegUtils;
    ///
    /// assert!(RegUtils::is_email("user@example.com"));
    /// assert!(RegUtils::is_email("first.last+tag@example.co.uk"));
    /// assert!(!RegUtils::is_email("user@example"));
    /// assert!(!RegUtils::is_email("user @example.com"));
    /// ```
    pub fn is_email(value: &str) -> bool {
        EMAIL_REGEX
            .get_or_init(|| Regex::new(EMAIL_PATTERN).expect("the email pattern must be valid"))
            .is_match(value)
    }

    /// 校验字符串是否符合中国大陆手机号码格式。
    ///
    /// 校验使用的正则表达式为 `r"^1[3-9][0-9]{9}$"`，要求输入为 11 位数字，
    /// 且第二位为 `3` 至 `9`。该方法只进行格式校验，不会验证号码是否真实存在。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RegUtils;
    ///
    /// assert!(RegUtils::is_phone_cn("13812345678"));
    /// assert!(RegUtils::is_phone_cn("19900000000"));
    /// assert!(!RegUtils::is_phone_cn("12812345678"));
    /// assert!(!RegUtils::is_phone_cn("1381234567"));
    /// ```
    pub fn is_phone_cn(value: &str) -> bool {
        PHONE_CN_REGEX
            .get_or_init(|| {
                Regex::new(PHONE_CN_PATTERN)
                    .expect("the mainland China phone pattern must be valid")
            })
            .is_match(value)
    }
}

#[cfg(test)]
mod tests {
    use super::RegUtils;

    #[test]
    fn accepts_common_email_addresses() {
        let valid_values = [
            "user@example.com",
            "first.last+tag@example.co.uk",
            "a1@sub.example.cn",
            "name_1@example.travel",
        ];

        for value in valid_values {
            assert!(RegUtils::is_email(value), "expected a valid email: {value}");
        }
    }

    #[test]
    fn rejects_invalid_email_addresses() {
        let invalid_values = [
            "user@example",
            "user @example.com",
            "@example.com",
            "user@example..com",
            ".user@example.com",
            "user.@example.com",
            "user..name@example.com",
        ];

        for value in invalid_values {
            assert!(
                !RegUtils::is_email(value),
                "expected an invalid email: {value}"
            );
        }
    }

    #[test]
    fn accepts_valid_mainland_china_mobile_numbers() {
        let valid_values = ["13812345678", "15000000000", "16612345678", "19900000000"];

        for value in valid_values {
            assert!(
                RegUtils::is_phone_cn(value),
                "expected a valid mainland China mobile number: {value}"
            );
        }
    }

    #[test]
    fn rejects_invalid_mainland_china_mobile_numbers() {
        let invalid_values = [
            "12812345678",
            "1381234567",
            "138123456789",
            "1381234567a",
            "+8613812345678",
            "138 1234 5678",
        ];

        for value in invalid_values {
            assert!(
                !RegUtils::is_phone_cn(value),
                "expected an invalid mainland China mobile number: {value}"
            );
        }
    }
}
