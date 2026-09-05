use std::{num::ParseIntError, str::FromStr};

use itoa::Buffer as ItoaBuffer;

use super::facade::ConvertUtils;

mod sealed {
    pub trait IntegerSealed {}
}

/// 调用方持有的整数格式化 buffer。
///
/// buffer 只包含栈内状态，不拥有堆资源。`ConvertUtils::integer_to_str` 返回的字符串切片
/// 借用该 buffer，并在下一次可变使用 buffer 前有效。
///
/// # Examples
///
/// ```
/// use axutils::{convert::IntegerBuffer, utils::ConvertUtils};
///
/// let mut buffer = IntegerBuffer::new();
/// assert_eq!(ConvertUtils::integer_to_str(42_i32, &mut buffer), "42");
/// ```
pub struct IntegerBuffer {
    inner: ItoaBuffer,
}

impl IntegerBuffer {
    /// 创建一个新的整数格式化 buffer。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{convert::IntegerBuffer, utils::ConvertUtils};
    ///
    /// let mut buffer = IntegerBuffer::new();
    /// let text = ConvertUtils::integer_to_str(-7_i64, &mut buffer);
    /// assert_eq!(text, "-7");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ItoaBuffer::new(),
        }
    }
}

impl Default for IntegerBuffer {
    /// 创建一个等价于 [`IntegerBuffer::new`] 的 buffer。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{convert::IntegerBuffer, utils::ConvertUtils};
    ///
    /// let mut buffer = IntegerBuffer::default();
    /// assert_eq!(ConvertUtils::integer_to_str(0_u8, &mut buffer), "0");
    /// ```
    fn default() -> Self {
        Self::new()
    }
}

/// `ConvertUtils` 使用的受限整数格式化 dispatch trait。
///
/// 本 crate 只为 Rust 内建的 12 种有符号和无符号整数实现此 trait；trait 的 sealed 约束
/// 防止外部类型把任意 `Display` 实现误当作整数转换输入。普通调用方应优先使用
/// [`ConvertUtils::integer_to_str`]。
///
/// # Examples
///
/// ```
/// use axutils::convert::{IntegerBuffer, IntegerValue};
///
/// let mut buffer = IntegerBuffer::new();
/// let text = <i32 as IntegerValue>::format_into(123_i32, &mut buffer);
/// assert_eq!(text, "123");
/// ```
#[allow(private_bounds)]
#[allow(clippy::needless_lifetimes)]
pub trait IntegerValue: sealed::IntegerSealed + FromStr<Err = ParseIntError> {
    /// 把一个整数写入调用方提供的 buffer，并返回借用 buffer 的文本切片。
    ///
    /// 返回值在 `buffer` 下一次可变使用前有效。该方法是受限的公共 dispatch 入口，通常
    /// 应调用 [`ConvertUtils::integer_to_str`]，以便让输入类型从关联函数参数自然推断。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::convert::{IntegerBuffer, IntegerValue};
    ///
    /// let mut buffer = IntegerBuffer::new();
    /// assert_eq!(<u128 as IntegerValue>::format_into(123_u128, &mut buffer), "123");
    /// ```
    fn format_into<'a>(value: Self, buffer: &'a mut IntegerBuffer) -> &'a str;
}

macro_rules! impl_integer_value {
    ($($type:ty),+ $(,)?) => {
        $(
            impl sealed::IntegerSealed for $type {}

            impl IntegerValue for $type {
                #[allow(clippy::needless_lifetimes)]
                #[inline]
                fn format_into<'a>(
                    value: Self,
                    buffer: &'a mut IntegerBuffer,
                ) -> &'a str {
                    buffer.inner.format(value)
                }
            }
        )+
    };
}

impl_integer_value!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl ConvertUtils {
    /// 把一个内建整数格式化到调用方持有的 buffer 中，并返回借用 buffer 的字符串切片。
    ///
    /// 该方法不为结果创建堆分配。返回值只在 `buffer` 下一次可变使用前有效；需要独立保存
    /// 结果时使用 [`ConvertUtils::integer_to_string`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{convert::IntegerBuffer, utils::ConvertUtils};
    ///
    /// let mut buffer = IntegerBuffer::new();
    /// let text = ConvertUtils::integer_to_str(i128::MIN, &mut buffer);
    /// assert_eq!(text, "-170141183460469231731687303715884105728");
    /// ```
    #[allow(clippy::needless_lifetimes)]
    #[inline]
    pub fn integer_to_str<'a, I>(value: I, buffer: &'a mut IntegerBuffer) -> &'a str
    where
        I: IntegerValue,
    {
        I::format_into(value, buffer)
    }

    /// 把一个内建整数直接追加到已有字符串中。
    ///
    /// 方法使用局部栈 buffer，不创建中间 `String`。当 `output` 容量不足时，目标字符串
    /// 可以按自身规则扩容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::ConvertUtils;
    ///
    /// let mut output = String::from("id=");
    /// ConvertUtils::append_integer(&mut output, 42_u64);
    /// assert_eq!(output, "id=42");
    /// ```
    #[inline]
    pub fn append_integer<I>(output: &mut String, value: I)
    where
        I: IntegerValue,
    {
        let mut buffer = IntegerBuffer::new();
        output.push_str(Self::integer_to_str(value, &mut buffer));
    }

    /// 把一个内建整数格式化为独立拥有的 `String`。
    ///
    /// 与借用型和追加型接口不同，该方法为结果承担拥有型字符串所需的分配和复制；它适合
    /// 需要跨越 buffer 生命周期保存文本的调用方。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::ConvertUtils;
    ///
    /// let text = ConvertUtils::integer_to_string(-900_i32);
    /// assert_eq!(text, "-900");
    /// ```
    #[must_use]
    pub fn integer_to_string<I>(value: I) -> String
    where
        I: IntegerValue,
    {
        let mut output = String::with_capacity(40);
        Self::append_integer(&mut output, value);
        output
    }

    /// 使用目标整数类型的标准库解析器把字符串解析为整数。
    ///
    /// 方法不自动裁剪空白、不提供默认值，也不重写 [`ParseIntError`]。非法字符、空输入、
    /// 符号不匹配和溢出均遵守目标类型 `FromStr` 的标准语义。
    ///
    /// # Errors
    ///
    /// 返回目标整数类型原生的 [`ParseIntError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::ConvertUtils;
    ///
    /// let value: i32 = ConvertUtils::string_to_integer("-42").unwrap();
    /// assert_eq!(value, -42);
    /// assert!(ConvertUtils::string_to_integer::<u8>("256").is_err());
    /// assert!(ConvertUtils::string_to_integer::<i32>(" 42").is_err());
    /// ```
    pub fn string_to_integer<T>(input: &str) -> Result<T, ParseIntError>
    where
        T: IntegerValue,
    {
        input.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConvertUtils, IntegerBuffer, IntegerValue};

    #[test]
    fn formats_and_parses_all_integer_types() {
        macro_rules! assert_round_trip {
            ($($type:ty => [$($value:expr),+ $(,)?]),+ $(,)?) => {
                $(
                    $(
                        let value: $type = $value;
                        let text = ConvertUtils::integer_to_string(value);
                        let parsed: $type = ConvertUtils::string_to_integer(&text).unwrap();
                        assert_eq!(parsed, value);
                    )+
                )+
            };
        }

        assert_round_trip!(
            i8 => [i8::MIN, -1, 0, i8::MAX],
            i16 => [i16::MIN, -1, 0, i16::MAX],
            i32 => [i32::MIN, -1, 0, i32::MAX],
            i64 => [i64::MIN, -1, 0, i64::MAX],
            i128 => [i128::MIN, -1, 0, i128::MAX],
            isize => [isize::MIN, -1, 0, isize::MAX],
            u8 => [0, 1, u8::MAX],
            u16 => [0, 1, u16::MAX],
            u32 => [0, 1, u32::MAX],
            u64 => [0, 1, u64::MAX],
            u128 => [0, 1, u128::MAX],
            usize => [0, 1, usize::MAX],
        );
    }

    #[test]
    fn rejects_invalid_integer_input_without_trimming() {
        assert!(ConvertUtils::string_to_integer::<i32>("").is_err());
        assert!(ConvertUtils::string_to_integer::<i32>(" 1").is_err());
        assert!(ConvertUtils::string_to_integer::<i32>("1 ").is_err());
        assert!(ConvertUtils::string_to_integer::<u32>("-1").is_err());
        assert!(ConvertUtils::string_to_integer::<i8>("-129").is_err());
        assert!(ConvertUtils::string_to_integer::<i8>("128").is_err());
        assert!(ConvertUtils::string_to_integer::<u8>("256").is_err());
        assert!(ConvertUtils::string_to_integer::<i32>("1x").is_err());
    }

    #[test]
    fn borrowed_and_appended_integer_results_have_expected_lifetimes_and_content() {
        let mut buffer = IntegerBuffer::default();
        let borrowed = ConvertUtils::integer_to_str(123_i32, &mut buffer);
        assert_eq!(borrowed, "123");

        let mut output = String::with_capacity(16);
        ConvertUtils::append_integer(&mut output, i64::MIN);
        assert_eq!(output, i64::MIN.to_string());

        assert_eq!(
            <u16 as IntegerValue>::format_into(65535, &mut buffer),
            "65535"
        );
    }
}
