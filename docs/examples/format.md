# FormatUtils 使用文档

> `FormatUtils::seconds_to_human` 默认可用；运行时模板需要 `serde` 与至少一个模板后端
> (`strfmt` 或 `minijinja`)。两个后端同时启用时，通过 `TemplateEngine` 显式选择语法。

## 导出内容

公开模块路径：`axutils::format_utils` 和 `axutils::utils::format_utils`。

`FormatUtils` 可从以下路径导入：

- 推荐：`axutils::FormatUtils`；
- `axutils::format_utils::FormatUtils`；
- `axutils::utils::FormatUtils`；
- `axutils::utils::format_utils::FormatUtils`。

它是无字段工具结构体，无 `new` 方法，实现 `Debug`、`Clone`、`Copy`、`Default`。

`TemplateEngine` 只在 `serde` 且至少启用一个模板后端时导出，支持同样的四类路径：

- `axutils::TemplateEngine`；
- `axutils::format_utils::TemplateEngine`；
- `axutils::utils::TemplateEngine`；
- `axutils::utils::format_utils::TemplateEngine`。

它实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`，不实现 `Default`，并标记为
`#[non_exhaustive]`。变体按 feature 存在：

- `TemplateEngine::Strfmt`：需要 `serde` + `strfmt`，使用扁平 `{name}` 语法；
- `TemplateEngine::MiniJinja`：需要 `serde` + `minijinja`，使用 `{{ name }}` 语法并支持
  嵌套字段、数组、条件和循环。

本模块没有公共自由函数、trait、类型别名、常量、静态项或宏。

## 安装与启用

持续时间格式化无需额外 feature：

```toml
[dependencies]
axutils = "0.1"
```

启用 `strfmt`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "strfmt"] }
serde = { version = "1", features = ["derive"] }
```

启用 `minijinja`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "minijinja"] }
serde = { version = "1", features = ["derive"] }
```

后端 feature 不会自动启用 `serde`；两个后端都启用时使用
`features = ["serde", "strfmt", "minijinja"]`，并在调用时显式传入引擎。

## 函数与方法详解

### `FormatUtils::seconds_to_human(seconds: u64) -> String`

- **feature**：默认可用。
- **参数**：整数秒数，不处理小数秒。
- **返回值**：按天、小时、分钟、秒拆分后的中文持续时间字符串。
- **示例**：

```rust
use axutils::FormatUtils;

assert_eq!(FormatUtils::seconds_to_human(0), "0秒");
assert_eq!(FormatUtils::seconds_to_human(90), "1分钟30秒");
assert_eq!(FormatUtils::seconds_to_human(3_600), "1小时0分钟0秒");
assert_eq!(FormatUtils::seconds_to_human(90_061), "1天1小时1分钟1秒");
```

从最高的非零单位开始显示到秒；更高位为零时省略，例如 `45` 只显示 `45秒`。方法不处理
周、月、年或小数秒，且使用整数除法，不会因 `u64::MAX` 溢出：

```rust
use axutils::FormatUtils;

assert_eq!(FormatUtils::seconds_to_human(45), "45秒");
assert_eq!(
    FormatUtils::seconds_to_human(u64::MAX),
    "213503982334601天7小时0分钟15秒"
);
```

**注意**：返回字符串会分配与输出长度相称的内存；调用方应在展示或日志场景中自行控制输入
范围和调用频率。

### `FormatUtils::template<T: Serialize>(template: &str, context: &T, default: Option<&str>, engine: TemplateEngine) -> Option<String>`

- **feature**：`serde` + `strfmt` 或 `serde` + `minijinja`；`TemplateEngine` 的对应变体还
  必须存在。
- **参数**：`template` 是运行时模板；`context` 是可序列化上下文；`default` 是失败时复制
  成拥有字符串的回退值；`engine` 显式选择后端。
- **返回值**：渲染成功返回 `Some(String)`，即使结果为空字符串也是 `Some(String::new())`；
  模板解析、上下文序列化或渲染失败时返回 `default.map(str::to_owned)`，未提供回退则为
  `None`。

`strfmt` 只把上下文序列化为顶层对象，使用 `{name}`；顶层标量或序列化失败会走回退，嵌套
值会按 JSON 字符串表示参与替换：

```rust
# #[cfg(all(feature = "serde", feature = "strfmt"))]
# fn main() {
use axutils::{FormatUtils, TemplateEngine};
use serde::Serialize;

#[derive(Serialize)]
struct Greeting<'a> {
    name: &'a str,
}

let context = Greeting { name: "小王" };
assert_eq!(
    FormatUtils::template("你好，{name}", &context, None, TemplateEngine::Strfmt),
    Some("你好，小王".to_owned())
);
assert_eq!(
    FormatUtils::template("{missing}", &context, Some("匿名用户"), TemplateEngine::Strfmt),
    Some("匿名用户".to_owned())
);
# }
# #[cfg(not(all(feature = "serde", feature = "strfmt")))]
# fn main() {}
```

`minijinja` 使用 `{{ name }}`，严格处理未定义变量，支持嵌套字段、数组、条件和循环，并
关闭自动 HTML 转义：

```rust
# #[cfg(all(feature = "serde", feature = "minijinja"))]
# fn main() {
use axutils::{FormatUtils, TemplateEngine};
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
    Some("你好，小王（杭州）[Rust][模板]".to_owned())
);
assert_eq!(
    FormatUtils::template("{{ missing }}", &user, Some("匿名用户"), TemplateEngine::MiniJinja),
    Some("匿名用户".to_owned())
);
# }
# #[cfg(not(all(feature = "serde", feature = "minijinja")))]
# fn main() {}
```

空输出属于成功，不会被回退值替换；上下文值也不会被再次当作模板解析：

```rust
# #[cfg(all(feature = "serde", feature = "minijinja"))]
# fn main() {
use axutils::{FormatUtils, TemplateEngine};
use serde::Serialize;

#[derive(Serialize)]
struct Context<'a> {
    value: &'a str,
}

let context = Context { value: "{{ secret }}" };
assert_eq!(
    FormatUtils::template(
        "{% if false %}ignored{% endif %}",
        &context,
        Some("fallback"),
        TemplateEngine::MiniJinja,
    ),
    Some(String::new())
);
assert_eq!(
    FormatUtils::template("{{ value }}", &context, None, TemplateEngine::MiniJinja),
    Some("{{ secret }}".to_owned())
);
# }
# #[cfg(not(all(feature = "serde", feature = "minijinja")))]
# fn main() {}
```

**注意**：这是运行时模板 API，调用方必须限制模板长度、调用频率和上下文数据规模。MiniJinja
关闭自动转义，HTML 安全与输出编码由调用方负责；模板错误不会把模板或上下文写入返回错误，
只返回回退值或 `None`。当 `TemplateEngine` 标记为 `#[non_exhaustive]` 时，跨版本匹配应保留
`_` 通配分支。

## 使用场景与限制

`seconds_to_human` 适合人类可读的简单持续时间展示；`template` 适合受控模板和小型上下文。
本模块不提供模板缓存、沙箱、国际化时间单位、HTML 清理、自动转义、模板文件读取或密码学
秘密处理。对不可信模板和上下文，调用方需要自行限制资源并决定是否进行输出净化。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
