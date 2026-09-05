# 格式化与脱敏

`FormatUtils` 是无状态工具，从 `axutils::utils::FormatUtils` 导入。持续时间格式化、位置脱敏和
邮箱本地部分脱敏默认可用；模板渲染使用显式引擎。

## 启用模板

模板能力使用语义 feature，而不是 provider feature：

```toml
[dependencies]
axutils = { version = "1.0", features = ["template-minijinja"] }
serde = { version = "1", features = ["derive"] }
```

| feature | `TemplateEngine` 变体与语法 |
| --- | --- |
| `template-strfmt` | `Strfmt`，扁平变量，如 `{name}` |
| `template-minijinja` | `MiniJinja`，如 `{{ name }}`，支持嵌套、条件和循环 |

两个 feature 可以同时启用；调用处必须显式传入 `TemplateEngine`。

## 持续时间与脱敏

`seconds_to_human` 以天为最大单位。`mask` 的范围是零基、左闭右开的 Unicode 字符位置，必须按
升序且不重叠；错误范围或分配失败返回 `None`。`mask_email` 不验证邮件真实性，只安全地拆分本地
部分和域名。

```rust
use axutils::utils::FormatUtils;

assert_eq!(FormatUtils::seconds_to_human(90_061), "1天1小时1分钟1秒");
assert_eq!(
    FormatUtils::mask("甲乙丙丁戊", &[(1, 3)], Some("#")),
    Some("甲#丁戊".to_owned()),
);
assert_eq!(
    FormatUtils::mask_email("alice@example.com", None),
    Some("ali****@example.com".to_owned()),
);
assert_eq!(FormatUtils::mask("abc", &[(2, 1)], None), None);
```

位置型脱敏不会清除输入或其余副本，也不理解字段语义。对不可信输入，调用方应限制文本长度、范围
数量和 replacement 长度。

## 模板

渲染成功可返回空字符串；模板解析、上下文序列化或渲染失败时，返回 `default` 的拥有副本，未提供
默认值时返回 `None`。不要把不可信模板当成安全策略：调用方应限制模板长度、渲染频率和上下文规模。

```rust
use axutils::utils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Context<'a> {
    name: &'a str,
}

let context = Context { name: "小王" };
let rendered = FormatUtils::template(
    "你好，{{ name }}",
    &context,
    Some("匿名用户"),
    TemplateEngine::MiniJinja,
);
assert_eq!(rendered, Some("你好，小王".to_owned()));
```

MiniJinja 采用严格未定义变量处理，变量值不会再次作为模板执行。`Strfmt` 只处理顶层变量；嵌套
数据会按 JSON 值文本处理。
