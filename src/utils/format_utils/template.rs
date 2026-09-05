#[cfg(feature = "template-minijinja")]
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
use serde::Serialize;
#[cfg(feature = "template-strfmt")]
use serde_json::Value as JsonValue;
#[cfg(feature = "template-strfmt")]
use std::collections::HashMap;

use super::FormatUtils;

#[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
/// 运行时模板渲染使用的引擎。
///
/// 未启用对应模板能力 feature 时，该变体不存在于枚举中，也不会出现在公共 API 里。枚举本身与
/// 统一入口使用相同的外层守卫，因此不会导出一个无法使用的空枚举或孤立类型。
///
/// # Examples
///
/// ```
/// use axutils::utils::TemplateEngine;
///
/// #[cfg(feature = "template-strfmt")]
/// let _ = TemplateEngine::Strfmt;
/// #[cfg(feature = "template-minijinja")]
/// let _ = TemplateEngine::MiniJinja;
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateEngine {
    /// `strfmt` 引擎；使用 `{name}` 语法，只支持扁平顶层变量。
    #[cfg(feature = "template-strfmt")]
    Strfmt,
    /// MiniJinja 引擎；使用 `{{ name }}` 语法，支持嵌套字段、数组、条件和循环。
    #[cfg(feature = "template-minijinja")]
    MiniJinja,
}

impl FormatUtils {
    /// 使用显式指定的引擎渲染运行时模板。
    ///
    /// `TemplateEngine::Strfmt` 使用 `{name}` 语法并只支持扁平顶层变量；`TemplateEngine::MiniJinja`
    /// 使用 `{{ name }}` 语法，支持嵌套字段、数组、条件和循环。模板解析、上下文序列化或渲染
    /// 失败时返回 `default` 的拥有副本；未提供默认值时返回 `None`。成功渲染为空字符串时仍
    /// 返回 `Some(String::new())`。MiniJinja 环境关闭自动 HTML 转义，变量值不会被再次当作模板
    /// 解析；对来自不可信来源的运行时模板，调用方应限制模板长度、调用频率和数据规模。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::utils::{FormatUtils, TemplateEngine};
    ///
    /// #[derive(serde::Serialize)]
    /// struct Greeting<'a> {
    ///     name: &'a str,
    /// }
    ///
    /// let context = Greeting { name: "小王" };
    /// #[cfg(feature = "template-strfmt")]
    /// assert_eq!(
    ///     FormatUtils::template("你好，{name}", &context, None, TemplateEngine::Strfmt),
    ///     Some("你好，小王".to_owned()),
    /// );
    /// #[cfg(feature = "template-minijinja")]
    /// assert_eq!(
    ///     FormatUtils::template("你好，{{ name }}", &context, None, TemplateEngine::MiniJinja),
    ///     Some("你好，小王".to_owned()),
    /// );
    /// ```
    #[cfg(any(feature = "template-strfmt", feature = "template-minijinja"))]
    pub fn template<T: Serialize>(
        template: &str,
        context: &T,
        default: Option<&str>,
        engine: TemplateEngine,
    ) -> Option<String> {
        match engine {
            #[cfg(feature = "template-strfmt")]
            TemplateEngine::Strfmt => strfmt_render(template, context, default),
            #[cfg(feature = "template-minijinja")]
            TemplateEngine::MiniJinja => minijinja_render(template, context, default),
        }
    }
}
#[cfg(feature = "template-strfmt")]
fn strfmt_render<T: Serialize>(
    template: &str,
    context: &T,
    default: Option<&str>,
) -> Option<String> {
    strfmt_context(context)
        .and_then(|variables| strfmt::strfmt(template, &variables).ok())
        .or_else(|| default.map(str::to_owned))
}

#[cfg(feature = "template-minijinja")]
fn minijinja_render<T: Serialize>(
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

#[cfg(feature = "template-strfmt")]
fn strfmt_context<T: Serialize>(context: &T) -> Option<HashMap<String, String>> {
    let JsonValue::Object(object) = serde_json::to_value(context).ok()? else {
        return None;
    };

    Some(
        object
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    JsonValue::String(value) => value,
                    value => value.to_string(),
                };
                (name, value)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "template-strfmt")]
    mod strfmt_tests {
        use super::super::{FormatUtils, TemplateEngine};
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
                FormatUtils::template(
                    "{name}|{age}|{enabled}|{empty}|{tags}",
                    &context,
                    None,
                    TemplateEngine::Strfmt,
                ),
                Some("小王|18|true|null|[\"rust\",\"模板\"]".to_owned()),
            );
        }

        #[test]
        fn renders_hash_map_and_escaped_braces() {
            let context = HashMap::from([("name", "小王")]);
            assert_eq!(
                FormatUtils::template("{{{name}}}", &context, None, TemplateEngine::Strfmt),
                Some("{小王}".to_owned()),
            );
        }

        #[test]
        fn successful_empty_output_is_not_replaced_by_default() {
            #[derive(Serialize)]
            struct Name<'a> {
                name: &'a str,
            }
            let context = Name { name: "小王" };
            assert_eq!(
                FormatUtils::template("", &context, Some("失败"), TemplateEngine::Strfmt),
                Some(String::new()),
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
                FormatUtils::template(
                    "{missing}",
                    &context,
                    Some("匿名用户"),
                    TemplateEngine::Strfmt,
                ),
                Some("匿名用户".to_owned()),
            );
            assert_eq!(
                FormatUtils::template("{missing}", &context, None, TemplateEngine::Strfmt),
                None
            );
            assert_eq!(
                FormatUtils::template("{name", &context, Some("失败"), TemplateEngine::Strfmt),
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
                FormatUtils::template("{name}", &42, Some("失败"), TemplateEngine::Strfmt),
                Some("失败".to_owned()),
            );
            assert_eq!(
                FormatUtils::template("{name}", &Failing, None, TemplateEngine::Strfmt),
                None
            );
        }
    }

    #[cfg(feature = "template-minijinja")]
    mod minijinja_tests {
        use super::super::{FormatUtils, TemplateEngine};
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
                FormatUtils::template(
                    "你好，{{ name }}（{{ profile.city }}）{% for tag in tags %}[{{ tag }}]{% endfor %}",
                    &user,
                    None,
                    TemplateEngine::MiniJinja,
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
                FormatUtils::template(
                    "{{ missing }}",
                    &user,
                    Some("匿名用户"),
                    TemplateEngine::MiniJinja,
                ),
                Some("匿名用户".to_owned()),
            );
            assert_eq!(
                FormatUtils::template("{{ missing }}", &user, None, TemplateEngine::MiniJinja),
                None
            );
            assert_eq!(
                FormatUtils::template("{% if %}", &user, Some("失败"), TemplateEngine::MiniJinja),
                Some("失败".to_owned()),
            );
            assert_eq!(
                FormatUtils::template(
                    "{% if false %}x{% endif %}",
                    &user,
                    None,
                    TemplateEngine::MiniJinja,
                ),
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
                FormatUtils::template("{{ value }}", &context, None, TemplateEngine::MiniJinja),
                Some("{{ secret }}".to_owned()),
            );
        }

        #[test]
        fn preserves_boolean_and_optional_context_values() {
            #[derive(Serialize)]
            struct Context {
                enabled: bool,
                optional: Option<&'static str>,
            }

            let context = Context {
                enabled: true,
                optional: None,
            };
            assert_eq!(
                FormatUtils::template("{{ enabled }}", &context, None, TemplateEngine::MiniJinja),
                Some("True".to_owned()),
            );
            assert_eq!(
                FormatUtils::template(
                    "{% if optional %}present{% else %}absent{% endif %}",
                    &context,
                    None,
                    TemplateEngine::MiniJinja,
                ),
                Some("absent".to_owned()),
            );
        }
    }

    #[cfg(all(feature = "template-strfmt", feature = "template-minijinja"))]
    mod both_backend_tests {
        use super::super::{FormatUtils, TemplateEngine};
        use serde::Serialize;

        #[derive(Serialize)]
        struct Context<'a> {
            name: &'a str,
        }

        #[test]
        fn explicit_engine_selects_the_requested_backend() {
            let context = Context { name: "小王" };
            assert_eq!(
                FormatUtils::template("你好，{name}", &context, None, TemplateEngine::Strfmt),
                Some("你好，小王".to_owned()),
            );
            assert_eq!(
                FormatUtils::template(
                    "你好，{{ name }}",
                    &context,
                    None,
                    TemplateEngine::MiniJinja,
                ),
                Some("你好，小王".to_owned()),
            );
        }
    }
}
