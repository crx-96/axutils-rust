#[cfg(feature = "format-default")]
fn main() {
    use axutils::format_utils::FormatUtils as FormatModuleUtils;
    use axutils::utils::format_utils::FormatUtils as UtilsModuleUtils;
    use axutils::utils::FormatUtils as UtilsFormatUtils;
    use axutils::FormatUtils;

    assert_eq!(
        FormatUtils::mask("13812345678", &[(3, 7)], None),
        Some("138****5678".to_owned())
    );
    assert_eq!(
        FormatModuleUtils::mask_email("alice@example.com", None),
        Some("ali****@example.com".to_owned())
    );
    assert_eq!(UtilsFormatUtils::seconds_to_human(60), "1分钟0秒");
    let _ = UtilsModuleUtils;
}

#[cfg(any(
    feature = "format-serde-strfmt",
    feature = "format-serde-minijinja",
    feature = "format-serde-all",
))]
#[derive(serde::Serialize)]
struct Context<'a> {
    name: &'a str,
}

#[cfg(feature = "format-serde-strfmt")]
fn main() {
    use axutils::format_utils::TemplateEngine as FormatTemplateEngine;
    use axutils::utils::TemplateEngine as UtilsTemplateEngine;
    use axutils::{FormatUtils, TemplateEngine};

    let context = Context { name: "小王" };
    let root_engine = TemplateEngine::Strfmt;
    let format_engine = FormatTemplateEngine::Strfmt;
    let utils_engine = UtilsTemplateEngine::Strfmt;
    let rendered = FormatUtils::template("你好，{name}", &context, None, root_engine)
        .expect("strfmt should render");
    assert_eq!(rendered, "你好，小王");
    let _ = (format_engine, utils_engine);
}

#[cfg(feature = "format-serde-minijinja")]
fn main() {
    use axutils::format_utils::TemplateEngine as FormatTemplateEngine;
    use axutils::utils::TemplateEngine as UtilsTemplateEngine;
    use axutils::{FormatUtils, TemplateEngine};

    let context = Context { name: "小王" };
    let root_engine = TemplateEngine::MiniJinja;
    let format_engine = FormatTemplateEngine::MiniJinja;
    let utils_engine = UtilsTemplateEngine::MiniJinja;
    let rendered = FormatUtils::template("你好，{{ name }}", &context, None, root_engine)
        .expect("minijinja should render");
    assert_eq!(rendered, "你好，小王");
    let _ = (format_engine, utils_engine);
}

#[cfg(feature = "format-serde-all")]
fn main() {
    use axutils::format_utils::TemplateEngine as FormatTemplateEngine;
    use axutils::utils::TemplateEngine as UtilsTemplateEngine;
    use axutils::{FormatUtils, TemplateEngine};

    let context = Context { name: "小王" };
    let strfmt = TemplateEngine::Strfmt;
    let minijinja = TemplateEngine::MiniJinja;
    let _ = (
        FormatTemplateEngine::Strfmt,
        FormatTemplateEngine::MiniJinja,
        UtilsTemplateEngine::Strfmt,
        UtilsTemplateEngine::MiniJinja,
    );
    assert_eq!(
        FormatUtils::template("你好，{name}", &context, None, strfmt),
        Some("你好，小王".to_owned())
    );
    assert_eq!(
        FormatUtils::template("你好，{{ name }}", &context, None, minijinja),
        Some("你好，小王".to_owned())
    );
}

#[cfg(any(
    feature = "negative-format-no-features",
    feature = "negative-format-serde-only",
    feature = "negative-format-strfmt-only",
    feature = "negative-format-minijinja-only",
))]
fn main() {
    let _ = axutils::TemplateEngine::Strfmt;
    let _ = axutils::FormatUtils::template;
}

#[cfg(feature = "negative-format-serde-strfmt-missing-minijinja")]
fn main() {
    let _ = axutils::TemplateEngine::MiniJinja;
}

#[cfg(feature = "negative-format-serde-minijinja-missing-strfmt")]
fn main() {
    let _ = axutils::TemplateEngine::Strfmt;
}

#[cfg(not(any(
    feature = "format-default",
    feature = "format-serde-strfmt",
    feature = "format-serde-minijinja",
    feature = "format-serde-all",
    feature = "negative-format-no-features",
    feature = "negative-format-serde-only",
    feature = "negative-format-strfmt-only",
    feature = "negative-format-minijinja-only",
    feature = "negative-format-serde-strfmt-missing-minijinja",
    feature = "negative-format-serde-minijinja-missing-strfmt",
)))]
fn main() {}
