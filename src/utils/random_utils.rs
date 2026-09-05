//! 生成普通随机字符串和数值的工具。
//!
//! 此模块需要启用 `rand` feature。

use std::collections::TryReserveError;
use std::fmt;
use std::ops::RangeInclusive;

use rand::distr::{Distribution, Uniform};

const DIGITS: &[u8] = b"0123456789";
const LOWERCASE_LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPERCASE_LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MIXED_LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const ALPHANUMERIC: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// 字母随机字符串的大小写模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterCase {
    /// 只生成小写 ASCII 字母 `a-z`。
    Lower,
    /// 只生成大写 ASCII 字母 `A-Z`。
    Upper,
    /// 生成大小写混合的 ASCII 字母 `a-zA-Z`。
    Mixed,
}

/// 随机数区间无效时返回的错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomRangeError {
    /// 区间起点大于终点，或者底层随机库无法构造对应的均匀分布。
    InvalidRange,
    /// 浮点区间包含 `NaN` 或正负无穷。
    NonFiniteFloat,
}

impl fmt::Display for RandomRangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => formatter.write_str("the random range is invalid"),
            Self::NonFiniteFloat => formatter.write_str("the float range must be finite"),
        }
    }
}

impl std::error::Error for RandomRangeError {}

/// 普通随机字符串和数值工具。
///
/// 该工具使用 `rand` 的线程本地随机生成器，适合测试数据、临时标识和一般业务中的
/// 随机取值。生成结果不承诺密码学安全，不应直接用于密码、Session Token、API 密钥或
/// 其他需要密码学安全随机数的场景。如果操作系统随机源不可用，底层随机生成器初始化
/// 可能会 panic。
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomUtils;

impl RandomUtils {
    /// 生成指定长度的纯数字 ASCII 字符串。
    ///
    /// 每一位都从 `0-9` 中独立均匀采样，因此结果可能以 `0` 开头。长度为 `0` 时返回
    /// 空字符串。方法会先尝试为结果预留所需容量；如果长度超出平台可分配范围，返回
    /// [`TryReserveError`]，而不是因为容量溢出直接 panic。时间复杂度为 `O(length)`，
    /// 额外空间复杂度为 `O(length)`。方法不会为 `length` 设置固定上限；对于来自不可信
    /// 输入的长度，调用方应先做业务上限校验，避免成功分配超大字符串带来的资源消耗。
    ///
    /// # Errors
    ///
    /// 当结果无法预留所需容量时返回 [`TryReserveError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")]
    /// # {
    /// use axutils::utils::RandomUtils;
    ///
    /// let value = RandomUtils::numeric_string(12).expect("the string should be allocatable");
    /// assert_eq!(value.len(), 12);
    /// assert!(value.bytes().all(|byte| byte.is_ascii_digit()));
    /// # }
    /// ```
    pub fn numeric_string(length: usize) -> Result<String, TryReserveError> {
        Self::from_alphabet(length, DIGITS)
    }

    /// 按指定大小写模式生成纯字母 ASCII 字符串。
    ///
    /// `LetterCase::Lower` 只生成 `a-z`，`LetterCase::Upper` 只生成 `A-Z`，
    /// `LetterCase::Mixed` 生成 `a-zA-Z`。长度为 `0` 时返回空字符串。方法会先尝试为
    /// 结果预留所需容量；如果长度超出平台可分配范围，返回 [`TryReserveError`]。
    /// 时间复杂度为 `O(length)`，额外空间复杂度为 `O(length)`。方法不会为 `length` 设置
    /// 固定上限；对于来自不可信输入的长度，调用方应先做业务上限校验，避免成功分配超大
    /// 字符串带来的资源消耗。
    ///
    /// # Errors
    ///
    /// 当结果无法预留所需容量时返回 [`TryReserveError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")]
    /// # {
    /// use axutils::utils::{LetterCase, RandomUtils};
    ///
    /// let value = RandomUtils::alphabetic_string(12, LetterCase::Upper)
    ///     .expect("the string should be allocatable");
    /// assert_eq!(value.len(), 12);
    /// assert!(value.bytes().all(|byte| byte.is_ascii_uppercase()));
    /// # }
    /// ```
    pub fn alphabetic_string(length: usize, case: LetterCase) -> Result<String, TryReserveError> {
        let alphabet = match case {
            LetterCase::Lower => LOWERCASE_LETTERS,
            LetterCase::Upper => UPPERCASE_LETTERS,
            LetterCase::Mixed => MIXED_LETTERS,
        };

        Self::from_alphabet(length, alphabet)
    }

    /// 生成指定长度的数字字母 ASCII 字符串。
    ///
    /// 每一位都从 `a-zA-Z0-9` 中独立均匀采样。长度为 `0` 时返回空字符串。方法会先尝试
    /// 为结果预留所需容量；如果长度超出平台可分配范围，返回 [`TryReserveError`]。
    /// 时间复杂度为 `O(length)`，额外空间复杂度为 `O(length)`。方法不会为 `length` 设置
    /// 固定上限；对于来自不可信输入的长度，调用方应先做业务上限校验，避免成功分配超大
    /// 字符串带来的资源消耗。
    ///
    /// # Errors
    ///
    /// 当结果无法预留所需容量时返回 [`TryReserveError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")]
    /// # {
    /// use axutils::utils::RandomUtils;
    ///
    /// let value = RandomUtils::alphanumeric_string(16)
    ///     .expect("the string should be allocatable");
    /// assert_eq!(value.len(), 16);
    /// assert!(value.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    /// # }
    /// ```
    pub fn alphanumeric_string(length: usize) -> Result<String, TryReserveError> {
        Self::from_alphabet(length, ALPHANUMERIC)
    }

    /// 从闭区间中生成一个随机整数。
    ///
    /// `range` 的起点和终点都可能被返回。起点大于终点时返回
    /// [`RandomRangeError::InvalidRange`]；起点等于终点时返回该唯一值。
    ///
    /// # Errors
    ///
    /// 当区间反向，或底层随机库无法构造对应的均匀分布时返回
    /// [`RandomRangeError::InvalidRange`]。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")]
    /// # {
    /// use axutils::utils::RandomUtils;
    ///
    /// let value = RandomUtils::integer(1..=100).expect("the range should be valid");
    /// assert!((1..=100).contains(&value));
    /// assert_eq!(RandomUtils::integer(42..=42), Ok(42));
    /// # }
    /// ```
    pub fn integer(range: RangeInclusive<i64>) -> Result<i64, RandomRangeError> {
        if range.is_empty() {
            return Err(RandomRangeError::InvalidRange);
        }

        let distribution = Uniform::new_inclusive(*range.start(), *range.end())
            .map_err(|_| RandomRangeError::InvalidRange)?;
        let mut rng = rand::rng();

        Ok(distribution.sample(&mut rng))
    }

    /// 从可构造的有限闭区间中生成一个随机浮点数。
    ///
    /// `range` 的起点和终点都可能被返回。区间边界必须是有限值，不能包含 `NaN`、正无穷
    /// 或负无穷；不满足时返回 [`RandomRangeError::NonFiniteFloat`]。起点大于终点，或区间
    /// 跨度导致底层随机库无法构造均匀分布时返回 [`RandomRangeError::InvalidRange`]；起点
    /// 等于终点时返回该唯一值。
    ///
    /// # Errors
    ///
    /// - 边界包含 `NaN` 或正负无穷时返回 [`RandomRangeError::NonFiniteFloat`]；
    /// - 区间反向，或区间跨度无法由底层随机库表示时返回
    ///   [`RandomRangeError::InvalidRange`]。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "rand")]
    /// # {
    /// use axutils::utils::RandomUtils;
    ///
    /// let value = RandomUtils::float(-1.0..=1.0).expect("the range should be valid");
    /// assert!((-1.0..=1.0).contains(&value));
    /// assert_eq!(RandomUtils::float(2.5..=2.5), Ok(2.5));
    /// # }
    /// ```
    pub fn float(range: RangeInclusive<f64>) -> Result<f64, RandomRangeError> {
        let start = *range.start();
        let end = *range.end();

        if !start.is_finite() || !end.is_finite() {
            return Err(RandomRangeError::NonFiniteFloat);
        }

        if range.is_empty() {
            return Err(RandomRangeError::InvalidRange);
        }

        let distribution =
            Uniform::new_inclusive(start, end).map_err(|_| RandomRangeError::InvalidRange)?;
        let mut rng = rand::rng();

        Ok(distribution.sample(&mut rng))
    }

    fn from_alphabet(length: usize, alphabet: &[u8]) -> Result<String, TryReserveError> {
        let mut value = String::new();
        value.try_reserve(length)?;

        let distribution =
            Uniform::new(0, alphabet.len()).expect("random string alphabet must not be empty");
        let mut rng = rand::rng();
        for _ in 0..length {
            let index = distribution.sample(&mut rng);
            value.push(alphabet[index] as char);
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{LetterCase, RandomRangeError, RandomUtils};

    const DIGITS: &[u8] = b"0123456789";
    const LOWERCASE_LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const UPPERCASE_LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const MIXED_LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const ALPHANUMERIC: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    fn assert_contains_only(value: &str, alphabet: &[u8]) {
        assert!(value.bytes().all(|byte| alphabet.contains(&byte)));
    }

    #[test]
    fn generates_expected_string_alphabets() {
        let numeric = RandomUtils::numeric_string(64).expect("numeric string should fit");
        let lowercase = RandomUtils::alphabetic_string(64, LetterCase::Lower)
            .expect("lowercase string should fit");
        let uppercase = RandomUtils::alphabetic_string(64, LetterCase::Upper)
            .expect("uppercase string should fit");
        let mixed = RandomUtils::alphabetic_string(64, LetterCase::Mixed)
            .expect("mixed-case string should fit");
        let alphanumeric =
            RandomUtils::alphanumeric_string(64).expect("alphanumeric string should fit");

        assert_eq!(numeric.len(), 64);
        assert_eq!(lowercase.len(), 64);
        assert_eq!(uppercase.len(), 64);
        assert_eq!(mixed.len(), 64);
        assert_eq!(alphanumeric.len(), 64);

        assert_contains_only(&numeric, DIGITS);
        assert_contains_only(&lowercase, LOWERCASE_LETTERS);
        assert_contains_only(&uppercase, UPPERCASE_LETTERS);
        assert_contains_only(&mixed, MIXED_LETTERS);
        assert_contains_only(&alphanumeric, ALPHANUMERIC);
    }

    #[test]
    fn generates_empty_strings_for_zero_length() {
        assert_eq!(RandomUtils::numeric_string(0).unwrap(), "");
        assert_eq!(
            RandomUtils::alphabetic_string(0, LetterCase::Mixed).unwrap(),
            ""
        );
        assert_eq!(RandomUtils::alphanumeric_string(0).unwrap(), "");
    }

    #[test]
    fn reports_string_capacity_overflow() {
        assert!(RandomUtils::numeric_string(usize::MAX).is_err());
    }

    #[test]
    fn generates_integers_within_an_inclusive_range() {
        for _ in 0..64 {
            let value = RandomUtils::integer(-10..=10).expect("the range should be valid");
            assert!((-10..=10).contains(&value));
        }
    }

    #[test]
    fn supports_single_value_integer_ranges_and_full_integer_ranges() {
        assert_eq!(RandomUtils::integer(42..=42), Ok(42));

        let value =
            RandomUtils::integer(i64::MIN..=i64::MAX).expect("the full i64 range should be valid");
        assert!((i64::MIN..=i64::MAX).contains(&value));
    }

    #[test]
    fn rejects_reversed_integer_ranges() {
        let start = 10;
        let end = 1;

        assert_eq!(
            RandomUtils::integer(start..=end),
            Err(RandomRangeError::InvalidRange)
        );
    }

    #[test]
    fn generates_floats_within_an_inclusive_range() {
        for _ in 0..64 {
            let value = RandomUtils::float(-10.0..=10.0).expect("the range should be valid");
            assert!((-10.0..=10.0).contains(&value));
        }
    }

    #[test]
    fn supports_single_value_float_ranges() {
        assert_eq!(RandomUtils::float(2.5..=2.5), Ok(2.5));
    }

    #[test]
    fn rejects_invalid_float_ranges() {
        assert_eq!(
            RandomUtils::float(10.0..=1.0),
            Err(RandomRangeError::InvalidRange)
        );
        assert_eq!(
            RandomUtils::float(f64::NAN..=1.0),
            Err(RandomRangeError::NonFiniteFloat)
        );
        assert_eq!(
            RandomUtils::float(0.0..=f64::INFINITY),
            Err(RandomRangeError::NonFiniteFloat)
        );
        assert_eq!(
            RandomUtils::float(f64::NEG_INFINITY..=0.0),
            Err(RandomRangeError::NonFiniteFloat)
        );
        assert_eq!(
            RandomUtils::float(f64::MIN..=f64::MAX),
            Err(RandomRangeError::InvalidRange)
        );
    }
}
