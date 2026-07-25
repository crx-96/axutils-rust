//! 常用格式校验工具。

use std::sync::OnceLock;

use regex::Regex;

#[cfg(all(feature = "regex", feature = "libphonenumber"))]
use libphonenumber::Type;

const EMAIL_PATTERN: &str = r"^[^\s@.]+(?:\.[^\s@.]+)*@[^\s@.]+(?:\.[^\s@.]+)+$";
const EMAIL_STRICT_LOCAL_PATTERN: &str =
    r"\A[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+)*\z";
const PHONE_CN_PATTERN: &str = r"^1[3-9][0-9]{9}$";
const EMAIL_MAX_LENGTH: usize = 254;
const EMAIL_LOCAL_MAX_LENGTH: usize = 64;
const EMAIL_DOMAIN_MAX_LENGTH: usize = 255;

static EMAIL_REGEX: OnceLock<Regex> = OnceLock::new();
static EMAIL_STRICT_LOCAL_REGEX: OnceLock<Regex> = OnceLock::new();
static PHONE_CN_REGEX: OnceLock<Regex> = OnceLock::new();

/// 电子邮箱和手机号码格式校验工具。
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

    /// 校验字符串是否符合严格的 ASCII 电子邮箱地址格式。
    ///
    /// 该方法要求地址由符合 `dot-atom` 规则的 local-part 和 DNS 主机名格式的域名组成，
    /// 并检查 local-part、域名标签以及完整地址的长度限制。它不接受显示名、注释、引号
    /// local-part、Unicode local-part 或空白字符，也不会验证邮箱是否真实存在。
    /// Unicode 域名需要先转换为 ASCII 形式（例如 `xn--` punycode）后再传入。
    ///
    /// 该方法是严格的业务格式校验，不承诺接受 RFC 5322 中所有历史兼容语法。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RegUtils;
    ///
    /// assert!(RegUtils::is_email_strict("user@example.com"));
    /// assert!(RegUtils::is_email_strict("first.last+tag@example.co.uk"));
    /// assert!(!RegUtils::is_email_strict("user@example"));
    /// assert!(!RegUtils::is_email_strict("user name@example.com"));
    /// ```
    pub fn is_email_strict(value: &str) -> bool {
        if value.is_empty() || value.len() > EMAIL_MAX_LENGTH || !value.is_ascii() {
            return false;
        }

        if value.matches('@').count() != 1 {
            return false;
        }

        let Some((local, domain)) = value.split_once('@') else {
            return false;
        };

        if local.is_empty()
            || domain.is_empty()
            || local.len() > EMAIL_LOCAL_MAX_LENGTH
            || domain.len() > EMAIL_DOMAIN_MAX_LENGTH
        {
            return false;
        }

        let local_is_valid = EMAIL_STRICT_LOCAL_REGEX
            .get_or_init(|| {
                Regex::new(EMAIL_STRICT_LOCAL_PATTERN)
                    .expect("the strict email local-part pattern must be valid")
            })
            .is_match(local);

        local_is_valid && is_strict_email_domain(domain)
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

    /// 校验字符串是否为有效的国际手机号码。
    ///
    /// 输入必须是严格的 E.164 形式：以 `+` 开头，后接 1 至 15 位 ASCII 数字，
    /// 不接受空格、短横线、括号、分机号或依赖默认国家/地区的本地号码。校验会使用
    /// `libphonenumber` 的国家码、号段和号码类型元数据，并且只接受类型为 `Mobile`
    /// 的号码；不会验证号码是否已开通或当前可接通。
    ///
    /// 由于部分国家/地区无法仅凭号段区分固定电话和手机号码，元数据标记为
    /// `FixedLineOrMobile` 的号码不会被此严格方法接受。
    ///
    /// 此方法需要同时启用 `regex` 和 `libphonenumber` features。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::RegUtils;
    ///
    /// assert!(RegUtils::is_phone("+8613812345678"));
    /// assert!(RegUtils::is_phone("+447911123456"));
    /// assert!(!RegUtils::is_phone("13812345678"));
    /// assert!(!RegUtils::is_phone("+86 13812345678"));
    /// ```
    #[cfg(all(feature = "regex", feature = "libphonenumber"))]
    pub fn is_phone(value: &str) -> bool {
        let Some(digits) = value.strip_prefix('+') else {
            return false;
        };

        if digits.is_empty()
            || digits.len() > 15
            || !digits.bytes().all(|byte| byte.is_ascii_digit())
        {
            return false;
        }

        let Ok(number) = libphonenumber::parse(None, value) else {
            return false;
        };

        number.is_valid() && number.number_type(&libphonenumber::metadata::DATABASE) == Type::Mobile
    }
}

fn is_strict_email_domain(value: &str) -> bool {
    let mut label_count = 0;
    let mut top_level_label = "";

    for label in value.split('.') {
        if !is_strict_email_domain_label(label) {
            return false;
        }

        label_count += 1;
        top_level_label = label;
    }

    label_count >= 2
        && (top_level_label.len() >= 2
            && (top_level_label
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic())
                || (top_level_label.starts_with("xn--") && top_level_label.len() > 4)))
}

fn is_strict_email_domain_label(label: &str) -> bool {
    let bytes = label.as_bytes();

    if bytes.is_empty() || bytes.len() > 63 {
        return false;
    }

    if !bytes[0].is_ascii_alphanumeric() || !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return false;
    }

    bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::{RegUtils, EMAIL_MAX_LENGTH};

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
    fn accepts_strict_email_addresses() {
        let valid_values = [
            "user@example.com",
            "first.last+tag@example.co.uk",
            "customer/department=shipping@example.com",
            "user@sub.example.xn--fiqs8s",
        ];

        for value in valid_values {
            assert!(
                RegUtils::is_email_strict(value),
                "expected a strict valid email: {value}"
            );
        }
    }

    #[test]
    fn rejects_invalid_strict_email_addresses() {
        let invalid_values = [
            "user@example",
            "user @example.com",
            "user@example..com",
            ".user@example.com",
            "user.@example.com",
            "user..name@example.com",
            "\"user\"@example.com",
            "user@-example.com",
            "user@example-.com",
            "user@exam_ple.com",
            "user@example.c",
            "user@example.123",
            "用户@example.com",
            "user@example.com\n",
            "user\n@example.com",
        ];

        for value in invalid_values {
            assert!(
                !RegUtils::is_email_strict(value),
                "expected a strict invalid email: {value:?}"
            );
        }
    }

    #[test]
    fn enforces_strict_email_length_limits() {
        let max_length_domain = format!(
            "{}.{}.{}.com",
            "b".repeat(61),
            "c".repeat(61),
            "d".repeat(61)
        );
        let max_length_email = format!("{}@{max_length_domain}", "a".repeat(64));

        assert_eq!(max_length_email.len(), EMAIL_MAX_LENGTH);
        assert!(RegUtils::is_email_strict(&max_length_email));

        let too_long_local = format!("{}@example.com", "a".repeat(65));
        let too_long_label = format!("user@{}.com", "a".repeat(64));
        let too_long_address = format!("{}@{max_length_domain}", "a".repeat(65));

        assert!(!RegUtils::is_email_strict(&too_long_local));
        assert!(!RegUtils::is_email_strict(&too_long_label));
        assert!(!RegUtils::is_email_strict(&too_long_address));
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

    #[cfg(all(feature = "regex", feature = "libphonenumber"))]
    #[test]
    fn accepts_valid_international_mobile_numbers() {
        let valid_values = ["+8613812345678", "+447911123456", "+919876543210"];

        for value in valid_values {
            assert!(
                RegUtils::is_phone(value),
                "expected a valid international mobile number: {value}"
            );
        }
    }

    #[cfg(all(feature = "regex", feature = "libphonenumber"))]
    #[test]
    fn rejects_invalid_international_mobile_numbers() {
        let invalid_values = [
            "13812345678",
            "+861381234567",
            "+8612812345678",
            "+86 13812345678",
            "+999123456789",
            "+86138123456789",
            "+441234567890",
            "+14155552671",
        ];

        for value in invalid_values {
            assert!(
                !RegUtils::is_phone(value),
                "expected an invalid international mobile number: {value}"
            );
        }
    }
}
