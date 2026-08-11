use std::{num::ParseFloatError, str::FromStr};

use super::ConvertUtils;

mod sealed {
    pub trait FloatSealed {}
}

/// 选择浮点格式化后端。
///
/// 只有当前启用的后端变体会出现在该枚举中。该枚举为非穷尽枚举，外部代码匹配时必须
/// 保留通配分支，以便未来增加后端时保持兼容。
///
/// # Examples
///
/// ```
/// use axutils::FloatFormat;
///
/// #[cfg(feature = "ryu")]
/// let format = FloatFormat::Ryu;
/// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
/// let format = FloatFormat::Zmij;
/// # #[cfg(any(feature = "ryu", feature = "zmij"))]
/// let _ = format;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FloatFormat {
    /// 使用 `ryu` 的最短十进制浮点格式。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "ryu")]
    /// {
    ///     use axutils::FloatFormat;
    ///     assert_eq!(FloatFormat::Ryu, FloatFormat::Ryu);
    /// }
    /// ```
    #[cfg(feature = "ryu")]
    Ryu,
    /// 使用 `zmij` 的最短十进制浮点格式。
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "zmij")]
    /// {
    ///     use axutils::FloatFormat;
    ///     assert_eq!(FloatFormat::Zmij, FloatFormat::Zmij);
    /// }
    /// ```
    #[cfg(feature = "zmij")]
    Zmij,
}

enum FloatBackend {
    #[cfg(feature = "ryu")]
    Ryu(::ryu::Buffer),
    #[cfg(feature = "zmij")]
    Zmij(::zmij::Buffer),
}

/// 调用方持有的浮点格式化 buffer。
///
/// buffer 在构造时固定一个 [`FloatFormat`]；本类型不实现 `Default`，因此不会在单后端和
/// 双后端 feature 组合之间隐式改变默认后端。`ConvertUtils::float_to_str` 返回的字符串切片
/// 借用该 buffer，并在下一次可变使用 buffer 前有效。
///
/// # Examples
///
/// ```
/// use axutils::{ConvertUtils, FloatBuffer, FloatFormat};
///
/// #[cfg(feature = "ryu")]
/// let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
/// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
/// let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
/// # #[cfg(any(feature = "ryu", feature = "zmij"))]
/// assert_eq!(ConvertUtils::float_to_str(1.5_f64, &mut buffer), "1.5");
/// ```
pub struct FloatBuffer {
    backend: FloatBackend,
}

impl FloatBuffer {
    /// 创建一个固定使用指定后端的浮点格式化 buffer。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, FloatBuffer, FloatFormat};
    ///
    /// #[cfg(feature = "ryu")]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    /// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
    /// # #[cfg(any(feature = "ryu", feature = "zmij"))]
    /// assert_eq!(ConvertUtils::float_to_str(0.25_f32, &mut buffer), "0.25");
    /// ```
    #[must_use]
    #[allow(unreachable_patterns)]
    pub fn new(format: FloatFormat) -> Self {
        let backend = match format {
            #[cfg(feature = "ryu")]
            FloatFormat::Ryu => FloatBackend::Ryu(::ryu::Buffer::new()),
            #[cfg(feature = "zmij")]
            FloatFormat::Zmij => FloatBackend::Zmij(::zmij::Buffer::new()),
            _ => unreachable!("FloatFormat has no matching enabled backend"),
        };
        Self { backend }
    }
}

/// `ConvertUtils` 使用的受限浮点格式化 dispatch trait。
///
/// 本 crate 只为 `f32` 和 `f64` 实现此 trait；sealed 约束防止外部类型把任意实现误当作
/// 浮点转换输入。普通调用方应优先使用 [`ConvertUtils::float_to_str`]。
///
/// # Examples
///
/// ```
/// use axutils::{FloatBuffer, FloatFormat, FloatValue};
///
/// #[cfg(feature = "ryu")]
/// let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
/// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
/// let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
/// # #[cfg(any(feature = "ryu", feature = "zmij"))]
/// assert_eq!(<f64 as FloatValue>::format_into(1.25, &mut buffer), "1.25");
/// ```
#[allow(private_bounds)]
#[allow(clippy::needless_lifetimes)]
pub trait FloatValue: sealed::FloatSealed + FromStr<Err = ParseFloatError> {
    /// 把一个浮点数写入调用方提供的 buffer，并返回借用 buffer 的文本切片。
    ///
    /// 返回值在 `buffer` 下一次可变使用前有效。特殊值的文本由所选后端的标准
    /// `Buffer::format` 语义决定。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{FloatBuffer, FloatFormat, FloatValue};
    ///
    /// #[cfg(feature = "ryu")]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    /// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
    /// # #[cfg(any(feature = "ryu", feature = "zmij"))]
    /// assert_eq!(<f32 as FloatValue>::format_into(2.5, &mut buffer), "2.5");
    /// ```
    fn format_into<'a>(value: Self, buffer: &'a mut FloatBuffer) -> &'a str;
}

macro_rules! impl_float_value {
    ($($type:ty),+ $(,)?) => {
        $(
            impl sealed::FloatSealed for $type {}

            impl FloatValue for $type {
                #[allow(clippy::needless_lifetimes)]
                #[inline]
                fn format_into<'a>(
                    value: Self,
                    buffer: &'a mut FloatBuffer,
                ) -> &'a str {
                    match &mut buffer.backend {
                        #[cfg(feature = "ryu")]
                        FloatBackend::Ryu(inner) => inner.format(value),
                        #[cfg(feature = "zmij")]
                        FloatBackend::Zmij(inner) => inner.format(value),
                    }
                }
            }
        )+
    };
}

impl_float_value!(f32, f64);

impl ConvertUtils {
    /// 把一个 `f32` 或 `f64` 格式化到调用方持有的 buffer 中，并返回借用 buffer 的字符串切片。
    ///
    /// 该方法不为结果创建堆分配。返回值只在 `buffer` 下一次可变使用前有效；需要独立保存
    /// 结果时使用 [`ConvertUtils::float_to_string`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, FloatBuffer, FloatFormat};
    ///
    /// #[cfg(feature = "ryu")]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    /// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    /// let mut buffer = FloatBuffer::new(FloatFormat::Zmij);
    /// # #[cfg(any(feature = "ryu", feature = "zmij"))]
    /// assert_eq!(ConvertUtils::float_to_str(-0.5_f64, &mut buffer), "-0.5");
    /// ```
    #[allow(clippy::needless_lifetimes)]
    #[inline]
    pub fn float_to_str<'a, T>(value: T, buffer: &'a mut FloatBuffer) -> &'a str
    where
        T: FloatValue,
    {
        T::format_into(value, buffer)
    }

    /// 把一个 `f32` 或 `f64` 按显式后端直接追加到已有字符串中。
    ///
    /// 方法使用局部栈 buffer，不创建中间 `String`。当 `output` 容量不足时，目标字符串
    /// 可以按自身规则扩容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, FloatFormat};
    ///
    /// let mut output = String::from("value=");
    /// #[cfg(feature = "ryu")]
    /// ConvertUtils::append_float(&mut output, 1.25_f64, FloatFormat::Ryu);
    /// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    /// ConvertUtils::append_float(&mut output, 1.25_f64, FloatFormat::Zmij);
    /// assert_eq!(output, "value=1.25");
    /// ```
    #[inline]
    pub fn append_float<T>(output: &mut String, value: T, format: FloatFormat)
    where
        T: FloatValue,
    {
        let mut buffer = FloatBuffer::new(format);
        output.push_str(Self::float_to_str(value, &mut buffer));
    }

    /// 按显式后端把一个 `f32` 或 `f64` 格式化为独立拥有的 `String`。
    ///
    /// 该方法为结果承担拥有型字符串所需的分配和复制；它适合需要跨越 buffer 生命周期保存
    /// 文本的调用方。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, FloatFormat};
    ///
    /// #[cfg(feature = "ryu")]
    /// let text = ConvertUtils::float_to_string(1.5_f64, FloatFormat::Ryu);
    /// #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    /// let text = ConvertUtils::float_to_string(1.5_f64, FloatFormat::Zmij);
    /// # #[cfg(any(feature = "ryu", feature = "zmij"))]
    /// assert_eq!(text, "1.5");
    /// ```
    #[must_use]
    pub fn float_to_string<T>(value: T, format: FloatFormat) -> String
    where
        T: FloatValue,
    {
        let mut output = String::new();
        Self::append_float(&mut output, value, format);
        output
    }

    /// 使用目标浮点类型的标准库解析器把字符串解析为 `f32` 或 `f64`。
    ///
    /// 方法不自动裁剪空白、不提供默认值，也不重写 [`ParseFloatError`]。特殊值、溢出和
    /// 下溢行为遵守目标类型 `FromStr` 的标准语义。
    ///
    /// # Errors
    ///
    /// 返回目标浮点类型原生的 [`ParseFloatError`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConvertUtils;
    ///
    /// let value: f64 = ConvertUtils::string_to_float("-1.25e2").unwrap();
    /// assert_eq!(value, -125.0);
    /// assert!(ConvertUtils::string_to_float::<f64>(" 1.0").is_err());
    /// assert!(ConvertUtils::string_to_float::<f32>("not-a-number").is_err());
    /// ```
    pub fn string_to_float<T>(input: &str) -> Result<T, ParseFloatError>
    where
        T: FloatValue,
    {
        input.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::{ConvertUtils, FloatBuffer, FloatFormat, FloatValue};

    #[allow(clippy::vec_init_then_push)]
    fn formats() -> Vec<FloatFormat> {
        let mut formats = Vec::new();
        #[cfg(feature = "ryu")]
        formats.push(FloatFormat::Ryu);
        #[cfg(feature = "zmij")]
        formats.push(FloatFormat::Zmij);
        formats
    }

    #[test]
    fn finite_values_round_trip_through_each_backend() {
        for format in formats() {
            for value in [
                0.0_f64,
                -0.0,
                1.25,
                -12345.5,
                1.0e-30,
                1.0e30,
                f64::MIN_POSITIVE,
                f64::MAX,
                -f64::MAX,
            ] {
                let text = ConvertUtils::float_to_string(value, format);
                let parsed: f64 = ConvertUtils::string_to_float(&text).unwrap();
                assert_eq!(parsed.to_bits(), value.to_bits(), "backend text: {text}");
            }

            for value in [
                0.0_f32,
                -0.0,
                1.25,
                -12345.5,
                1.0e-20,
                1.0e20,
                f32::MIN_POSITIVE,
                f32::MAX,
                -f32::MAX,
            ] {
                let text = ConvertUtils::float_to_string(value, format);
                let parsed: f32 = ConvertUtils::string_to_float(&text).unwrap();
                assert_eq!(parsed.to_bits(), value.to_bits(), "backend text: {text}");
            }
        }
    }

    #[test]
    fn special_values_use_backend_format_and_standard_parser_semantics() {
        for format in formats() {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let text = ConvertUtils::float_to_string(value, format);
                let parsed: f64 = ConvertUtils::string_to_float(&text).unwrap();
                if value.is_nan() {
                    assert!(parsed.is_nan(), "backend text: {text}");
                } else {
                    assert_eq!(parsed, value, "backend text: {text}");
                }
            }
            assert_eq!(ConvertUtils::float_to_string(-0.0_f64, format), "-0.0");
        }

        assert!(ConvertUtils::string_to_float::<f64>("").is_err());
        assert!(ConvertUtils::string_to_float::<f64>(" 1.0").is_err());
        assert!(ConvertUtils::string_to_float::<f64>("1.0 ").is_err());
        assert!(ConvertUtils::string_to_float::<f64>("1.0x").is_err());
        assert_eq!(
            ConvertUtils::string_to_float::<f64>("1e9999").unwrap(),
            f64::INFINITY
        );
        assert_eq!(
            ConvertUtils::string_to_float::<f64>("1e-9999").unwrap(),
            0.0
        );
    }

    #[test]
    fn borrowed_and_appended_float_results_have_expected_content() {
        let format = formats()[0];
        let mut buffer = FloatBuffer::new(format);
        assert_eq!(ConvertUtils::float_to_str(2.5_f32, &mut buffer), "2.5");
        assert_eq!(<f64 as FloatValue>::format_into(2.5, &mut buffer), "2.5");

        let mut output = String::with_capacity(16);
        ConvertUtils::append_float(&mut output, -0.5_f64, format);
        assert_eq!(output, "-0.5");
    }
}
