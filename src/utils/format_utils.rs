//! 将秒数格式化为中文持续时间字符串的工具。

#[cfg(all(feature = "minijinja", feature = "serde"))]
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
#[cfg(all(feature = "strfmt", feature = "serde"))]
use serde::Serialize;
#[cfg(all(feature = "strfmt", feature = "serde"))]
use std::collections::HashMap;

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

    /// 使用 `strfmt` 语法渲染扁平命名模板。
    ///
    /// 模板变量使用 `{name}` 形式。`context` 会被序列化为 JSON 对象，其顶层字段成为模板
    /// 变量：字符串保持原样；数字、布尔和 `null` 使用紧凑 JSON 文本；数组和对象同样使用
    /// 紧凑 JSON 文本。因此该方法不支持 `{profile.city}` 等嵌套访问。根上下文不是对象、
    /// 序列化失败、变量缺失或模板语法错误时，返回 `default` 的拥有副本；未提供默认值时
    /// 返回 `None`。成功渲染为空字符串时仍返回 `Some(String::new())`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FormatUtils;
    ///
    /// #[derive(serde::Serialize)]
    /// struct Greeting<'a> {
    ///     name: &'a str,
    ///     age: u8,
    /// }
    ///
    /// let context = Greeting { name: "小王", age: 18 };
    /// assert_eq!(
    ///     FormatUtils::template_strfmt("你好，{name}，今年 {age} 岁", &context, None),
    ///     Some("你好，小王，今年 18 岁".to_owned()),
    /// );
    /// ```
    #[cfg(all(feature = "strfmt", feature = "serde"))]
    pub fn template_strfmt<T: Serialize>(
        template: &str,
        context: &T,
        default: Option<&str>,
    ) -> Option<String> {
        strfmt_context(context)
            .and_then(|variables| strfmt::strfmt(template, &variables).ok())
            .or_else(|| default.map(str::to_owned))
    }

    /// 使用 MiniJinja 语法渲染模板。
    ///
    /// 模板变量使用 `{{ name }}` 形式，并支持嵌套字段、数组、条件和循环。未定义变量按严格
    /// 模式处理；模板语法错误、渲染错误或上下文序列化错误时，返回 `default` 的拥有副本；未
    /// 提供默认值时返回 `None`。环境明确关闭自动 HTML 转义，变量值不会被再次当作模板解析。
    /// 成功渲染为空字符串时仍返回 `Some(String::new())`。
    ///
    /// 运行时模板若来自不可信来源，调用方应限制模板长度、调用频率和数据规模，以控制完整
    /// 模板表达式语言带来的 CPU 与内存消耗。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::FormatUtils;
    ///
    /// #[derive(serde::Serialize)]
    /// struct Profile<'a> { city: &'a str }
    /// #[derive(serde::Serialize)]
    /// struct User<'a> { name: &'a str, profile: Profile<'a> }
    ///
    /// let user = User { name: "小王", profile: Profile { city: "杭州" } };
    /// assert_eq!(
    ///     FormatUtils::template_minijinja("你好，{{ name }}（{{ profile.city }}）", &user, None),
    ///     Some("你好，小王（杭州）".to_owned()),
    /// );
    /// ```
    #[cfg(all(feature = "minijinja", feature = "serde"))]
    pub fn template_minijinja<T: serde::Serialize>(
        template: &str,
        context: &T,
        default: Option<&str>,
    ) -> Option<String> {
        let mut environment = Environment::new();
        environment.set_undefined_behavior(UndefinedBehavior::Strict);
        environment.set_auto_escape_callback(|_| AutoEscape::None);

        environment
            .add_template("runtime-template.txt", template)
            .ok()
            .and_then(|()| {
                environment
                    .get_template("runtime-template.txt")
                    .ok()
                    .and_then(|added_template| added_template.render(context).ok())
            })
            .or_else(|| default.map(str::to_owned))
    }

    /// 使用当前唯一启用的模板后端渲染模板。
    ///
    /// 仅启用 `strfmt` feature 时采用 `{name}` 语法；仅启用 `minijinja` feature 时采用
    /// `{{ name }}` 语法。同时启用两个后端时不会导出此方法，以避免静默选择不同语义；请
    /// 改用 [`Self::template_strfmt`] 或 [`Self::template_minijinja`]。
    #[cfg(all(feature = "strfmt", feature = "serde", not(feature = "minijinja")))]
    pub fn template<T: Serialize>(
        template: &str,
        context: &T,
        default: Option<&str>,
    ) -> Option<String> {
        Self::template_strfmt(template, context, default)
    }

    /// 使用当前唯一启用的模板后端渲染模板。
    ///
    /// 仅启用 `minijinja` feature 时采用 `{{ name }}` 语法；仅启用 `strfmt` feature 时采用
    /// `{name}` 语法。同时启用两个后端时不会导出此方法，以避免静默选择不同语义；请改用
    /// [`Self::template_strfmt`] 或 [`Self::template_minijinja`]。
    #[cfg(all(feature = "minijinja", feature = "serde", not(feature = "strfmt")))]
    pub fn template<T: serde::Serialize>(
        template: &str,
        context: &T,
        default: Option<&str>,
    ) -> Option<String> {
        Self::template_minijinja(template, context, default)
    }
}

#[cfg(all(feature = "strfmt", feature = "serde"))]
fn strfmt_context<T: Serialize>(context: &T) -> Option<HashMap<String, String>> {
    let serde_json::Value::Object(object) = serde_json::to_value(context).ok()? else {
        return None;
    };

    Some(
        object
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    serde_json::Value::String(value) => value,
                    value => value.to_string(),
                };
                (name, value)
            })
            .collect(),
    )
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

    #[cfg(all(feature = "strfmt", feature = "serde"))]
    mod strfmt_tests {
        use super::FormatUtils;
        use serde::ser::Error as _;
        use serde::{Serialize, Serializer};
        use std::collections::HashMap;

        #[derive(Serialize)]
        struct Context<'a> {
            name: &'a str,
            age: u8,
            enabled: bool,
            empty: Option<&'a str>,
            tags: [&'a str; 2],
        }

        #[test]
        fn renders_struct_fields_and_json_normalized_values() {
            let context = Context {
                name: "小王",
                age: 18,
                enabled: true,
                empty: None,
                tags: ["rust", "模板"],
            };
            assert_eq!(
                FormatUtils::template_strfmt(
                    "{name}|{age}|{enabled}|{empty}|{tags}",
                    &context,
                    None,
                ),
                Some("小王|18|true|null|[\"rust\",\"模板\"]".to_owned()),
            );
        }

        #[test]
        fn renders_hash_map_and_escaped_braces() {
            let context = HashMap::from([("name", "小王")]);
            assert_eq!(
                FormatUtils::template_strfmt("{{{name}}}", &context, None),
                Some("{小王}".to_owned()),
            );
        }

        #[test]
        fn errors_use_owned_default_or_none() {
            #[derive(Serialize)]
            struct Name<'a> {
                name: &'a str,
            }
            let context = Name { name: "小王" };
            assert_eq!(
                FormatUtils::template_strfmt("{missing}", &context, Some("匿名用户")),
                Some("匿名用户".to_owned()),
            );
            assert_eq!(
                FormatUtils::template_strfmt("{missing}", &context, None),
                None
            );
            assert_eq!(
                FormatUtils::template_strfmt("{name", &context, Some("失败")),
                Some("失败".to_owned()),
            );
        }

        #[test]
        fn scalar_and_serialization_failure_use_default() {
            struct Failing;
            impl Serialize for Failing {
                fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    Err(S::Error::custom("intentional failure"))
                }
            }

            assert_eq!(
                FormatUtils::template_strfmt("{name}", &42, Some("失败")),
                Some("失败".to_owned()),
            );
            assert_eq!(FormatUtils::template_strfmt("{name}", &Failing, None), None);
        }

        #[test]
        fn single_backend_alias_uses_strfmt() {
            #[cfg(not(feature = "minijinja"))]
            {
                let context = HashMap::from([("name", "小王")]);
                assert_eq!(
                    FormatUtils::template("你好，{name}", &context, None),
                    Some("你好，小王".to_owned()),
                );
            }
        }
    }

    #[cfg(all(feature = "minijinja", feature = "serde"))]
    mod minijinja_tests {
        use super::FormatUtils;
        use serde::Serialize;

        #[derive(Serialize)]
        struct Profile<'a> {
            city: &'a str,
        }
        #[derive(Serialize)]
        struct User<'a> {
            name: &'a str,
            profile: Profile<'a>,
            tags: [&'a str; 2],
        }

        #[test]
        fn renders_nested_fields_arrays_and_unicode() {
            let user = User {
                name: "小王",
                profile: Profile { city: "杭州" },
                tags: ["Rust", "模板"],
            };
            assert_eq!(
                FormatUtils::template_minijinja(
                    "你好，{{ name }}（{{ profile.city }}）{% for tag in tags %}[{{ tag }}]{% endfor %}",
                    &user,
                    None,
                ),
                Some("你好，小王（杭州）[Rust][模板]".to_owned()),
            );
        }

        #[test]
        fn errors_are_strict_and_empty_output_is_successful() {
            let user = User {
                name: "小王",
                profile: Profile { city: "杭州" },
                tags: ["Rust", "模板"],
            };
            assert_eq!(
                FormatUtils::template_minijinja("{{ missing }}", &user, Some("匿名用户")),
                Some("匿名用户".to_owned()),
            );
            assert_eq!(
                FormatUtils::template_minijinja("{{ missing }}", &user, None),
                None
            );
            assert_eq!(
                FormatUtils::template_minijinja("{% if %}", &user, Some("失败")),
                Some("失败".to_owned()),
            );
            assert_eq!(
                FormatUtils::template_minijinja("{% if false %}x{% endif %}", &user, None),
                Some(String::new()),
            );
        }

        #[test]
        fn values_are_not_recursively_rendered() {
            #[derive(Serialize)]
            struct Context<'a> {
                value: &'a str,
            }
            let context = Context {
                value: "{{ secret }}",
            };
            assert_eq!(
                FormatUtils::template_minijinja("{{ value }}", &context, None),
                Some("{{ secret }}".to_owned()),
            );
        }

        #[test]
        fn single_backend_alias_uses_minijinja() {
            #[cfg(not(feature = "strfmt"))]
            {
                let context = User {
                    name: "小王",
                    profile: Profile { city: "杭州" },
                    tags: ["Rust", "模板"],
                };
                assert_eq!(
                    FormatUtils::template("你好，{{ name }}", &context, None),
                    Some("你好，小王".to_owned()),
                );
            }
        }
    }
}
