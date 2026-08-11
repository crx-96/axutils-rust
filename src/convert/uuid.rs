use super::ConvertUtils;

/// 调用方持有的 UUID 标准小写连字符格式 buffer。
///
/// buffer 内部固定保存 36 字节，不拥有堆资源。`ConvertUtils::uuid_to_str` 返回的字符串
/// 切片借用该 buffer，并在下一次可变使用 buffer 前有效。
///
/// # Examples
///
/// ```
/// use axutils::{ConvertUtils, UuidBuffer};
/// use uuid::Uuid;
///
/// let uuid = Uuid::try_parse("550e8400-e29b-41d4-a716-446655440000").unwrap();
/// let mut buffer = UuidBuffer::new();
/// assert_eq!(
///     ConvertUtils::uuid_to_str(&uuid, &mut buffer),
///     "550e8400-e29b-41d4-a716-446655440000"
/// );
/// ```
pub struct UuidBuffer {
    bytes: [u8; 36],
}

impl UuidBuffer {
    /// 创建一个新的 UUID 格式化 buffer。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, UuidBuffer};
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// let mut buffer = UuidBuffer::new();
    /// assert_eq!(ConvertUtils::uuid_to_str(&uuid, &mut buffer).len(), 36);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self { bytes: [0; 36] }
    }
}

impl Default for UuidBuffer {
    /// 创建一个等价于 [`UuidBuffer::new`] 的 buffer。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, UuidBuffer};
    /// use uuid::Uuid;
    ///
    /// let mut buffer = UuidBuffer::default();
    /// assert_eq!(ConvertUtils::uuid_to_str(&Uuid::nil(), &mut buffer), "00000000-0000-0000-0000-000000000000");
    /// ```
    fn default() -> Self {
        Self::new()
    }
}

impl ConvertUtils {
    /// 把 UUID 编码为标准小写连字符文本，并返回借用调用方 buffer 的字符串切片。
    ///
    /// 该方法使用 `uuid` crate 提供的安全 buffer 编码 API，不为结果创建堆分配。返回值只
    /// 在 `buffer` 下一次可变使用前有效；需要独立保存结果时使用
    /// [`ConvertUtils::uuid_to_string`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{ConvertUtils, UuidBuffer};
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// let mut buffer = UuidBuffer::new();
    /// assert_eq!(ConvertUtils::uuid_to_str(&uuid, &mut buffer), "00000000-0000-0000-0000-000000000000");
    /// ```
    #[inline]
    pub fn uuid_to_str<'a>(uuid: &::uuid::Uuid, buffer: &'a mut UuidBuffer) -> &'a str {
        uuid.hyphenated().encode_lower(&mut buffer.bytes)
    }

    /// 把 UUID 的标准小写连字符形式直接追加到已有字符串中。
    ///
    /// 方法使用局部的 36 字节 buffer，不创建中间 `String`。当 `output` 容量不足时，目标
    /// 字符串可以按自身规则扩容。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConvertUtils;
    /// use uuid::Uuid;
    ///
    /// let mut output = String::from("id=");
    /// ConvertUtils::append_uuid(&mut output, &Uuid::nil());
    /// assert_eq!(output, "id=00000000-0000-0000-0000-000000000000");
    /// ```
    #[inline]
    pub fn append_uuid(output: &mut String, uuid: &::uuid::Uuid) {
        let mut buffer = UuidBuffer::new();
        output.push_str(Self::uuid_to_str(uuid, &mut buffer));
    }

    /// 把 UUID 编码为独立拥有的标准小写连字符 `String`。
    ///
    /// 该方法为结果承担拥有型字符串所需的分配和复制；它适合需要跨越 buffer 生命周期保存
    /// 文本的调用方。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConvertUtils;
    /// use uuid::Uuid;
    ///
    /// assert_eq!(
    ///     ConvertUtils::uuid_to_string(&Uuid::nil()),
    ///     "00000000-0000-0000-0000-000000000000"
    /// );
    /// ```
    #[must_use]
    pub fn uuid_to_string(uuid: &::uuid::Uuid) -> String {
        let mut output = String::with_capacity(36);
        Self::append_uuid(&mut output, uuid);
        output
    }

    /// 使用 `Uuid::try_parse` 把字符串解析为 UUID。
    ///
    /// 输入不自动裁剪空白，接受的语法由当前 `uuid` crate 版本的 `try_parse` 契约决定；本
    /// crate 文档和测试只承诺标准 simple、连字符、URN 和 Microsoft GUID 形式。
    ///
    /// # Errors
    ///
    /// 输入为空、长度不正确、分隔符不正确或包含非法十六进制字符时，返回原生
    /// [`::uuid::Error`]。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::ConvertUtils;
    ///
    /// let uuid = ConvertUtils::string_to_uuid("550e8400-e29b-41d4-a716-446655440000").unwrap();
    /// assert_eq!(ConvertUtils::uuid_to_string(&uuid), "550e8400-e29b-41d4-a716-446655440000");
    /// assert!(ConvertUtils::string_to_uuid("not-a-uuid").is_err());
    /// ```
    pub fn string_to_uuid(input: &str) -> Result<::uuid::Uuid, ::uuid::Error> {
        ::uuid::Uuid::try_parse(input)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConvertUtils, UuidBuffer};
    use ::uuid::Uuid;

    const CANONICAL: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn formats_as_lowercase_hyphenated_uuid_through_all_layers() {
        let uuid = Uuid::try_parse(CANONICAL).unwrap();
        let mut buffer = UuidBuffer::new();
        assert_eq!(ConvertUtils::uuid_to_str(&uuid, &mut buffer), CANONICAL);

        let mut output = String::with_capacity(36);
        ConvertUtils::append_uuid(&mut output, &uuid);
        assert_eq!(output, CANONICAL);
        assert_eq!(ConvertUtils::uuid_to_string(&uuid), CANONICAL);
    }

    #[test]
    fn parses_documented_uuid_forms_and_rejects_invalid_input() {
        for input in [
            CANONICAL,
            "550e8400e29b41d4a716446655440000",
            "urn:uuid:550e8400-e29b-41d4-a716-446655440000",
            "{550e8400-e29b-41d4-a716-446655440000}",
            "550E8400-E29B-41D4-A716-446655440000",
        ] {
            assert_eq!(
                ConvertUtils::string_to_uuid(input).unwrap(),
                Uuid::try_parse(CANONICAL).unwrap()
            );
        }

        for input in [
            "",
            "550e8400-e29b-41d4-a716-44665544000",
            "550e8400-e29b-41d4-a716-44665544000g",
            "550e8400_e29b-41d4-a716-446655440000",
            " 550e8400-e29b-41d4-a716-446655440000",
        ] {
            assert!(
                ConvertUtils::string_to_uuid(input).is_err(),
                "input should be rejected: {input:?}"
            );
        }
    }
}
