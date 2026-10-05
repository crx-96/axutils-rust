//! 默认可用的 HTML 字符转义和字面标记替换，不依赖模板引擎。

use super::FormatUtils;

impl FormatUtils {
    /// 转义 HTML 文本及单引号或双引号包围的属性值中的五类特殊字符。
    ///
    /// `&`、`<`、`>`、`"` 和 `'` 分别转换为 `&amp;`、`&lt;`、`&gt;`、`&quot;` 和 `&#39;`，
    /// 其他 Unicode 字符保持原样。输入被视为尚未转义的文本，已有实体中的 `&` 也会转义；
    /// 空输入返回空字符串。本方法默认可用，返回拥有的字符串。
    ///
    /// 这不是 HTML 清洗器，不验证标签、属性或协议，也不处理 URL、JavaScript、CSS 或没有
    /// 引号包围的属性上下文。调用方负责选择正确的输出上下文并限制输入及结果规模。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::FormatUtils;
    ///
    /// assert_eq!(FormatUtils::escape_html("中文<&>\"'"), "中文&lt;&amp;&gt;&quot;&#39;");
    /// ```
    pub fn escape_html(value: &str) -> String {
        let mut output = String::with_capacity(value.len());

        // 只扫描原文的 Unicode 标量值；实体直接写入输出，不会被第二次转义。
        for character in value.chars() {
            match character {
                '&' => output.push_str("&amp;"),
                '<' => output.push_str("&lt;"),
                '>' => output.push_str("&gt;"),
                '"' => output.push_str("&quot;"),
                '\'' => output.push_str("&#39;"),
                other => output.push(other),
            }
        }
        output
    }

    /// 按借用的 `(字面标记, 替换值)` 表对原文执行一次从左到右的非递归替换。
    ///
    /// 总是先处理原文中最早匹配的位置；多个非空标记在同一位置匹配时，使用表中较早的一项，
    /// 不优先选择更长的标记。匹配成功后跳过该标记，因此已消费区域中的重叠匹配不再处理。
    /// 空标记被忽略，空替换值会删除匹配文本；没有注册的标记、花括号和其他 Unicode 原样保留。
    ///
    /// 替换值只写入结果，永远不会再次作为原文扫描。本方法默认可用，不解释模板语法、不自动
    /// 执行 HTML 转义，也不改变 [`FormatUtils`] 的可选模板引擎行为。空输入返回空字符串；
    /// 空标记表返回原文的拥有副本。调用方负责限制原文长度、标记数量和替换值长度。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::FormatUtils;
    ///
    /// assert_eq!(
    ///     FormatUtils::replace_placeholders(
    ///         "你好，{name}！{unknown}",
    ///         &[("{name}", "{other}"), ("{other}", "不递归展开")],
    ///     ),
    ///     "你好，{other}！{unknown}",
    /// );
    /// ```
    pub fn replace_placeholders(template: &str, replacements: &[(&str, &str)]) -> String {
        let mut output = String::with_capacity(template.len());
        let mut rest = template;

        // 沿原文的字符边界前进，从而先发现最早位置；find 保留相同位置的表内优先级。
        while !rest.is_empty() {
            if let Some(&(marker, value)) = replacements
                .iter()
                .find(|(marker, _)| !marker.is_empty() && rest.starts_with(*marker))
            {
                // 只跳过原标记，替换值直接进入结果，防止注入的标记被递归展开。
                output.push_str(value);
                rest = &rest[marker.len()..];
                continue;
            }

            // 当前边界没有匹配时保留一个完整 Unicode 字符，并继续处理剩余原文。
            let mut characters = rest.chars();
            if let Some(character) = characters.next() {
                output.push(character);
            }
            rest = characters.as_str();
        }
        output
    }
}
