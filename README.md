# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前项目最低支持 Rust 1.88。这是因为配置文件读取能力的 YAML 后端使用 `serde-saphyr 1.0.0`，
该 crate 声明 `edition = "2024"` 并使用了 let-chains 语法，实测需要 Rust 1.88（let-chains
稳定化版本）才能编译；邮件能力使用的 `lettre 0.11.22` 只要求 Rust 1.85。因此依赖 `axutils`
新版本的 Rust 1.76—1.87 项目需要先升级工具链。

当前默认工具提供 `PathUtils`、`TimeUtils`、`FormatUtils`、`RegUtils` 和 `RandomUtils`：`PathUtils`、
`TimeUtils` 与 `FormatUtils::seconds_to_human` 不依赖第三方包，默认可用，分别用于路径处理、获取当前 Unix
时间戳和将秒数格式化为中文持续时间字符串；`FormatUtils` 还可按需启用模板后端；`RegUtils` 的基础能力依赖第三方 `regex` crate，
需要显式启用 `regex` feature，用于校验电子邮箱地址和中国大陆手机号码；`RandomUtils` 依赖
第三方 `rand` crate，需要显式启用 `rand` feature。`RegUtils::is_phone` 还需要同时启用独立的
`libphonenumber` feature。

邮件能力由 `lettre` feature 显式提供：同步发送只需要 `lettre`，异步发送必须同时启用
`lettre` 和 `tokio`。邮件使用 Rustls 强制 SMTPS/STARTTLS，不提供明文或机会式降级；真实账号
配置只应由调用方在本地安全管理，不能硬编码或提交到 Git。

日期格式化可按需启用 `chrono`、`time` 或 `jiff` 中的任一独立 feature。每个后端只接收
自身的日期类型；仅启用一个后端时可使用简写方法，同时启用多个后端时必须调用带后缀的方法。

配置文件读取能力（`ConfigLoader`/`ConfigUtils`）由 `serde` feature 显式提供，支持 JSON 和
自实现的 `.env`（dotenv）；YAML、TOML、INI 分别需要额外启用 `serde-saphyr`、`toml`、`rust-ini`
feature。所有格式共享同一套文件大小上限；JSON/TOML/YAML/INI 的无类型读取以及 YAML/INI 的
有类型读取使用配置的嵌套深度上限，JSON/TOML 有类型读取使用各自后端的递归保护，错误不回显
配置文件内容。

## 安装

在项目的 `Cargo.toml` 中添加：

```toml
[dependencies]
axutils = "0.1"
```

上面的依赖声明默认提供 `PathUtils`、`TimeUtils` 和 `FormatUtils`。如果需要使用 `RegUtils`，
请显式启用 `regex` feature：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex"] }
```

如果需要使用 `RandomUtils`，请显式启用 `rand` feature：

```toml
[dependencies]
axutils = { version = "0.1", features = ["rand"] }
```

如果需要同步 SMTP 邮件：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre"] }
```

如果需要异步 SMTP 邮件，必须显式同时启用两个 feature；调用方还需要自行提供 Tokio runtime：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

只启用 `tokio` 不会启用或导出任何邮件 API。`axutils` 的生产 Tokio 依赖只启用邮件异步传输所需
的最小能力，不使用 `full`；上面的 `macros` 和 `rt-multi-thread` 是应用自身 runtime 示例。
示例中的第三方依赖版本都是最低兼容版本，不是精确补丁锁定；Cargo 可以在兼容范围内解析后续
版本。项目 manifest 对 `lettre`、Tokio、`time` 等依赖也遵循相同规则。

如果需要使用 Chrono 日期格式化：

```toml
[dependencies]
axutils = { version = "0.1", features = ["chrono"] }
chrono = "0.4"
```

如果需要使用 `time` 日期格式化：

```toml
[dependencies]
axutils = { version = "0.1", features = ["time"] }
time = { version = "0.3.36", default-features = false }
```

如果需要使用 Jiff 日期格式化：

```toml
[dependencies]
axutils = { version = "0.1", features = ["jiff"] }
jiff = { version = "0.2.35", default-features = false }
```

如果需要运行时模板渲染，必须显式同时启用 `serde` 和一个模板后端。`strfmt` 使用 `{name}`
语法，适用于扁平变量；`minijinja` 使用 `{{ name }}` 语法，支持嵌套字段、数组、条件和
循环。后端 feature 不会自动启用 `serde`，由调用方控制组合。调用
`FormatUtils::template` 时通过 `TemplateEngine` 参数显式选择后端，即使同时启用两个后端也
使用同一个入口：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "strfmt"] }
serde = { version = "1", features = ["derive"] }
```

```rust
use axutils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Greeting<'a> {
    name: &'a str,
}

let context = Greeting { name: "小王" };
assert_eq!(
    FormatUtils::template("你好，{name}", &context, None, TemplateEngine::Strfmt),
    Some("你好，小王".to_owned()),
);
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "minijinja"] }
serde = { version = "1", features = ["derive"] }
```

```rust
use axutils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Greeting<'a> {
    name: &'a str,
}

let context = Greeting { name: "小王" };
assert_eq!(
    FormatUtils::template("你好，{{ name }}", &context, None, TemplateEngine::MiniJinja),
    Some("你好，小王".to_owned()),
);
```

如果需要校验带国家/地区前缀的国际手机号码，同时启用 `regex` 和 `libphonenumber`
features：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

如果需要读取配置文件，启用 `serde` feature 即可读取 JSON 与 `.env`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde"] }
```

需要 YAML、TOML 或 INI 时，分别额外启用 `serde-saphyr`、`toml` 或 `rust-ini`；可以同时启用
多个后端：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "serde-saphyr", "toml", "rust-ini"] }
```

## 使用 `PathUtils`

`PathUtils` 提供路径判断、当前进程路径获取和多路径拼接：

- `is_absolute(path)`：按当前平台的路径语法判断是否为绝对路径，不检查路径是否存在；
- `current_dir()`：获取当前进程的工作目录，返回 `std::io::Result<PathBuf>`；
- `executable_path()`：获取当前进程可执行文件的路径，返回 `std::io::Result<PathBuf>`；
- `join(paths)`：按顺序拼接多个路径，并在词法层面处理 `.` 和 `..`；输入为空时返回当前目录的词法表示
  `.`。

`join` 不访问文件系统，不解析符号链接；如果后续路径片段是绝对路径，会按照
`PathBuf::push` 的平台规则替换此前的拼接结果。

```rust
use axutils::PathUtils;

let workspace = PathUtils::current_dir().expect("current directory should be available");
assert!(PathUtils::is_absolute(&workspace));

let source_file = PathUtils::join(["project", "src", "..", "README.md"]);
assert!(source_file.ends_with("README.md"));

let executable =
    PathUtils::executable_path().expect("the current executable should be available");
assert!(!executable.as_os_str().is_empty());
```

## 使用 `RegUtils`

启用 `regex` feature 后，可以使用正则校验工具：

```rust
use axutils::RegUtils;

assert!(RegUtils::is_email("user@example.com"));
assert!(!RegUtils::is_email("user@example"));
assert!(RegUtils::is_email_strict("user@example.com"));

assert!(RegUtils::is_phone_cn("13812345678"));
assert!(!RegUtils::is_phone_cn("12812345678"));
```

### `RegUtils::is_email`

使用以下正则表达式校验电子邮箱地址：

```text
^[^\s@.]+(?:\.[^\s@.]+)*@[^\s@.]+(?:\.[^\s@.]+)+$
```

该方法面向常见邮箱格式校验，并不试图完整覆盖 RFC 定义的所有邮箱地址形式。

### `RegUtils::is_email_strict`

`is_email_strict` 使用更严格的 ASCII 业务格式校验：要求 local-part 符合 `dot-atom`
规则，域名符合 DNS 主机名规则，并检查 local-part、域名标签及完整地址的长度限制。
该方法拒绝显示名、注释、引号 local-part、Unicode local-part、空白字符和数字顶级域名。
Unicode 域名应先转换为 ASCII punycode 形式。方法只校验格式，不验证邮箱是否真实存在。

```rust
use axutils::RegUtils;

assert!(RegUtils::is_email_strict("first.last+tag@example.co.uk"));
assert!(!RegUtils::is_email_strict("user@example"));
assert!(!RegUtils::is_email_strict("user name@example.com"));
```

### `RegUtils::is_phone_cn`

使用以下正则表达式校验中国大陆手机号码：

```text
^1[3-9][0-9]{9}$
```

方法要求输入为 11 位数字，且号段以 `13` 至 `19` 开头。

### `RegUtils::is_phone`

`is_phone` 需要同时启用 `regex` 和 `libphonenumber` features。输入必须是严格的 E.164
格式，即 `+` 加国家/地区码及号码，最多 15 位 ASCII 数字；不接受本地号码、空格、短横线、
括号或分机号。方法使用 `libphonenumber` 的国家码、号段和号码类型元数据，只接受类型为
`Mobile` 的号码，不验证号码是否已开通或当前可接通。

```rust
use axutils::RegUtils;

assert!(RegUtils::is_phone("+8613812345678"));
assert!(RegUtils::is_phone("+447911123456"));
assert!(!RegUtils::is_phone("13812345678"));
assert!(!RegUtils::is_phone("+86 13812345678"));
```

## 使用 `TimeUtils`

`TimeUtils` 提供五个获取当前 Unix 时间戳的方法：

- `timestamp()`：按秒、毫秒、微秒、纳秒顺序返回 `(u64, u128, u128, u128)`；
- `timestamp_seconds()`：秒，返回 `u64`；
- `timestamp_milliseconds()`：毫秒，返回 `u128`；
- `timestamp_microseconds()`：微秒，返回 `u128`；
- `timestamp_nanoseconds()`：纳秒，返回 `u128`。

```rust
use axutils::TimeUtils;

let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();

assert!(milliseconds / 1_000 >= seconds as u128);
assert!(microseconds / 1_000 >= milliseconds);
assert!(nanoseconds / 1_000 >= microseconds);
```

如果系统时间早于 Unix 纪元，这些方法会 panic。

### 日期格式化

`chrono`、`time` 和 `jiff` feature 彼此独立，且不启用时区数据库。三个后端共用以下小型
模板子集，而不是直接使用各库不兼容的原生格式语法：

| token | 含义 | 日期 | 无时区日期时间 | 固定偏移日期时间 |
| --- | --- | --- | --- | --- |
| `yyyy` | 带符号的完整公历年，绝对值至少四位 | 是 | 是 | 是 |
| `MM`、`dd` | 两位月、日 | 是 | 是 | 是 |
| `HH`、`mm`、`ss` | 两位时、分、秒 | 否 | 是 | 是 |
| `SSS` | 截断后的三位毫秒 | 否 | 是 | 是 |
| `XXX` | `Z`、`+HH:MM` 或 `+HH:MM:ss` 固定偏移 | 否 | 否 | 是 |

日期、无时区日期时间、固定偏移日期时间的默认模板分别为 `yyyy-MM-dd`、
`yyyy-MM-dd HH:mm:ss`、`yyyy-MM-dd HH:mm:ss`。非 ASCII 字符可直接作为字面量；ASCII
字母字面量须用单引号包围，`''` 表示一个单引号。无效模板或当前值不支持的 token 会由具体值方法
返回 `TimeFormatError`；`format_option_*` 会将输入 `None` 和全部格式化错误都折叠为 `None`，
需要错误详情时请使用返回 `Result` 的方法。

以下是 Chrono 后端的示例：

```rust
use axutils::TimeUtils;

let value = chrono::NaiveDate::from_ymd_opt(2024, 2, 29)
    .unwrap()
    .and_hms_nano_opt(1, 2, 3, 987_654_321)
    .unwrap();
assert_eq!(
    TimeUtils::format_datetime_with_offset_chrono(value, None, None).unwrap(),
    "2024-02-29 01:02:03",
);
assert_eq!(
    TimeUtils::format_datetime_chrono(value, Some("yyyy年MM月dd日 'at' HH:mm:ss.SSS")).unwrap(),
    "2024年02月29日 at 01:02:03.987",
);
```

带固定偏移的方法的 `offset` 参数为 `Option<TimeZoneOffset>`；传入 `None` 时使用默认的
`+08:00`。默认模板不输出偏移；如确有展示需求，可传入包含 `XXX` 的自定义模板。
`TimeZoneOffset` 仅代表 `-86_399..=86_399` 秒的固定 UTC 偏移，正值表示东区。它不会转换无时区
日期时间字段，也不表示 `Asia/Shanghai` 一类 IANA 时区，更不会查询 DST。`from_hours` 仅接收
`-23..=23` 的整小时偏移；半小时和秒级偏移请用 `from_seconds`。

仅启用一个日期后端时，可将同一组方法写为 `format_date`、`format_datetime`、
`format_datetime_with_offset` 及对应的 `format_option_*`。两个或三个日期后端同时启用时，这些
简写不会导出，调用方必须使用 `*_chrono`、`*_time` 或 `*_jiff`。

## 使用 `FormatUtils`

`FormatUtils` 提供 `seconds_to_human(seconds: u64) -> String`，将秒数格式化为中文
持续时间字符串，最大单位为天：

- 按天、小时、分钟、秒从高到低拆分，从最高的非零单位开始显示，直到秒；
- 更高位为零的单位（例如不足一天时的“天”）会被省略；
- 一旦某个单位非零，其后所有更低位单位即使为零也会显示；
- 不处理周、月、年等更大单位，也不处理小于一秒的部分；输入为 `0` 时返回 `"0秒"`。

```rust
use axutils::FormatUtils;

assert_eq!(FormatUtils::seconds_to_human(0), "0秒");
assert_eq!(FormatUtils::seconds_to_human(90), "1分钟30秒");
assert_eq!(FormatUtils::seconds_to_human(3600), "1小时0分钟0秒");
assert_eq!(FormatUtils::seconds_to_human(90_061), "1天1小时1分钟1秒");
```

### 模板渲染

显式同时启用 `serde` 和 `strfmt` 或 `minijinja` feature 后，模板方法接受任意
`serde::Serialize` 上下文，成功时返回 `Some(String)`，包括空字符串；模板语法错误、变量
缺失、序列化失败或后端渲染错误时，返回 `default` 的拥有副本，未提供默认值则返回 `None`。
通过 `TemplateEngine::Strfmt` 或 `TemplateEngine::MiniJinja` 参数显式选择后端；两个后端同时
启用时仍使用同一个 `FormatUtils::template` 入口。

`strfmt` 仅把序列化根对象的顶层字段映射为变量：字符串直接使用，数字、布尔、`null`、数组
和对象使用紧凑 JSON 文本。因此它支持 `{name}`，不支持 `{profile.city}`；根上下文为标量或
数组时会走回退路径。

```rust
use axutils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Greeting<'a> {
    name: &'a str,
    age: u8,
}

let context = Greeting { name: "小王", age: 18 };
assert_eq!(
    FormatUtils::template(
        "你好，{name}，今年 {age} 岁",
        &context,
        None,
        TemplateEngine::Strfmt,
    ),
    Some("你好，小王，今年 18 岁".to_owned()),
);
assert_eq!(
    FormatUtils::template(
        "你好，{missing}",
        &context,
        Some("匿名用户"),
        TemplateEngine::Strfmt,
    ),
    Some("匿名用户".to_owned()),
);
```

MiniJinja 使用严格未定义变量行为，并明确关闭自动 HTML 转义。变量值不会被再次解析为模板：

```rust
use axutils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Profile<'a> { city: &'a str }
#[derive(serde::Serialize)]
struct User<'a> { name: &'a str, profile: Profile<'a> }

let user = User { name: "小王", profile: Profile { city: "杭州" } };
assert_eq!(
    FormatUtils::template(
        "你好，{{ name }}（{{ profile.city }}）",
        &user,
        None,
        TemplateEngine::MiniJinja,
    ),
    Some("你好，小王（杭州）".to_owned()),
);
```

运行时模板如果来自不可信来源，调用方应限制模板长度、调用频率和数据规模，以控制 MiniJinja
完整表达式语言可能带来的 CPU 和内存消耗；这种场景优先选择功能更受限的 `strfmt`。

## 使用 `RandomUtils`

启用 `rand` feature 后，`RandomUtils` 可以生成普通 ASCII 随机字符串，以及从闭区间中
获取随机整数或浮点数：

```rust
use axutils::{LetterCase, RandomUtils};

let numeric = RandomUtils::numeric_string(8).expect("the string should be allocatable");
assert_eq!(numeric.len(), 8);
assert!(numeric.bytes().all(|byte| byte.is_ascii_digit()));

let lowercase = RandomUtils::alphabetic_string(8, LetterCase::Lower)
    .expect("the string should be allocatable");
assert!(lowercase.bytes().all(|byte| byte.is_ascii_lowercase()));

let alphanumeric =
    RandomUtils::alphanumeric_string(16).expect("the string should be allocatable");
assert!(alphanumeric.bytes().all(|byte| byte.is_ascii_alphanumeric()));

let integer = RandomUtils::integer(1..=100).expect("the range should be valid");
assert!((1..=100).contains(&integer));

let float = RandomUtils::float(0.0..=1.0).expect("the range should be valid");
assert!((0.0..=1.0).contains(&float));
```

`alphabetic_string` 支持 `LetterCase::Lower`、`LetterCase::Upper` 和
`LetterCase::Mixed` 三种模式，分别生成小写、大写和大小写混合的 ASCII 字母。
字符串长度为 `0` 时返回空字符串；如果长度超出平台可分配范围，会返回
`std::collections::TryReserveError`。这些方法不会为长度设置固定上限；对于来自不可信输入的
长度，调用方应先做业务上限校验，避免成功分配超大字符串带来的资源消耗。整数和浮点数的
区间都是闭区间，起点大于终点时返回 `RandomRangeError::InvalidRange`，浮点区间还会拒绝
`NaN` 和正负无穷；如果有限浮点区间的跨度导致底层均匀分布无法构造，也会返回
`RandomRangeError::InvalidRange`。

该工具用于普通随机数据、测试数据和一般业务随机取值，不承诺密码学安全，不应直接用于
密码、Session Token、API 密钥或其他安全敏感数据。

## 使用 SMTP 邮件能力

启用 `lettre` feature 后，可以创建多个互不覆盖的 `EmailClient`。配置只接受 DNS 主机名，
不接受 SMTP URL 或 IP 字面量；安全模式只有强制 SMTPS（`ImplicitTls`）和强制 STARTTLS
（`StartTls`）。构造客户端不会连接网络，`send` 才会执行阻塞 SMTP I/O：

```rust
use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

fn main() -> Result<(), axutils::EmailError> {
    let config = EmailConfig::new(
        "smtp.example.com",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "application-password",
        "sender@example.com",
    )?;
    let client = EmailClient::new(config)?;
    let message = EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "A test message",
        "Hello from axutils.",
    )?;
    client.send(message)?;
    Ok(())
}
```

异步发送只在 `lettre,tokio` 组合下提供。客户端构造不会要求 runtime；异步连接池会在首次
`send_async` 时于调用方的 Tokio runtime 中初始化。它不会创建 runtime 或调用 `block_on`，
应用必须先运行自己的 Tokio runtime：

```rust,no_run
use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

#[tokio::main]
async fn main() -> Result<(), axutils::EmailError> {
    let client = EmailClient::new(EmailConfig::new(
        "smtp.example.com",
        587,
        EmailSecurity::StartTls,
        "sender@example.com",
        "application-password",
        "sender@example.com",
    )?)?;
    let message = EmailMessage::html(
        vec!["receiver@example.com".to_owned()],
        "An HTML message",
        "<p>Hello from <strong>axutils</strong>.</p>",
    )?;
    client.send_async(message).await?;
    Ok(())
}
```

`EmailMessage::text` 和 `EmailMessage::html` 都立即校验输入。每封邮件至少需要一个、最多
100 个收件人；单个收件人最多 4 KiB，主题最多 16 KiB，正文最多 10 MiB。空主题和空正文允许
发送。主题、显示名和地址中的控制字符会被拒绝以防止邮件头注入；HTML 不会自动净化或生成
纯文本 fallback。上限是本 crate 的资源保护边界，不是服务商配额或 SMTP 传输行长度保证。

每个 `EmailClient` 的同步池和（启用异步时）异步池分别最多 10 条连接，连接空闲 60 秒回收。
多个账号应使用多个 `EmailClient`，调用方需要限制实例数量和并发发送量。同步方法会阻塞当前
线程，Tokio 服务应使用异步方法；本 crate 不提供重试、队列、附件、抄送、密送、OAuth2 或
邮件接收能力。

单默认账号可以使用一次初始化、不可重置的全局入口：

```rust
use axutils::{EmailConfig, EmailMessage, EmailSecurity, EmailUtils};

fn main() -> Result<(), axutils::EmailError> {
    EmailUtils::init(EmailConfig::new(
        "smtp.example.com",
        465,
        EmailSecurity::ImplicitTls,
        "sender@example.com",
        "application-password",
        "sender@example.com",
    )?)?;
    EmailUtils::send(EmailMessage::text(
        vec!["receiver@example.com".to_owned()],
        "A test message",
        "Hello from axutils.",
    )?)?;
    Ok(())
}
```

`EmailUtils::init` 成功后再次初始化会返回 `AlreadyInitialized`，未初始化发送会返回
`NotInitialized`；需要热切换账号、独立生命周期或多账号时请使用实例 API。密码不提供 getter，
也不应写入源码、日志、SMTP URL、命令行参数或版本库。

### 邮件 feature 矩阵

| feature | 邮件类型/同步 API | 异步 API |
| --- | --- | --- |
| 无 | 不导出 | 不导出 |
| `tokio` | 不导出 | 不导出 |
| `lettre` | 导出 `EmailBody`、`EmailClient`、`EmailConfig`、`EmailError`、`EmailMessage`、`EmailSecurity`、`EmailTransportErrorKind` 和 `EmailUtils`，支持同步 | 不导出 |
| `lettre,tokio` | 同上 | 支持 `EmailClient::send_async` 和 `EmailUtils::send_async` |

邮件传输固定使用 Rustls、`ring` 和 `webpki-roots`。常见 Linux 构建不需要为该功能安装
OpenSSL、`pkg-config`、CMake、Go 或系统 CA 包；`webpki-roots` 不读取企业私有 CA，因此自签名
或私有 CA relay 不在首期支持范围内，也不能通过关闭证书校验绕过。

## 使用配置文件读取能力

启用 `serde` feature 后，`ConfigUtils` 和 `ConfigLoader` 提供统一的配置文件读取能力，支持
JSON、`.env`、YAML（`serde-saphyr`）、TOML（`toml`）、INI（`rust-ini`）五种格式。每种格式都有
两条读取路径：无类型的 `ConfigValue`（支持 `"server.tls.port"` 点号路径访问）和有类型的
`serde::Deserialize`。文件大小上限由加载器统一执行；JSON/TOML/YAML/INI 的无类型读取以及
YAML/INI 的有类型读取还使用加载器配置的嵌套深度上限，JSON/TOML 有类型读取使用各自后端的
递归保护。格式默认按文件扩展名推断，也可以显式指定：

```rust
use axutils::{ConfigFormat, ConfigUtils};
use serde::Deserialize;

#[derive(Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Deserialize)]
struct AppConfig {
    server: ServerConfig,
}

fn main() -> Result<(), axutils::ConfigError> {
    // 无类型读取：按扩展名推断格式。
    let value = ConfigUtils::parse_value(r#"{"server": {"port": 8080}}"#, ConfigFormat::Json)?;
    assert_eq!(value.get("server.port").and_then(|v| v.as_i64()), Some(8080));

    // 有类型读取：直接反序列化为调用方结构体。
    let config: AppConfig = ConfigUtils::parse(
        r#"{"server": {"host": "localhost", "port": 8080}}"#,
        ConfigFormat::Json,
    )?;
    assert_eq!(config.server.port, 8080);
    Ok(())
}
```

从磁盘读取文件使用 `ConfigUtils::load_value`/`load`（按扩展名推断）或
`ConfigUtils::load_value_as`/`load_as`（显式指定格式）。需要自定义文件大小/嵌套深度上限，或
关闭 `.env` 的进程环境变量回退时，使用 `ConfigUtils::loader()` 获取一个可配置的
`ConfigLoader`：

```rust,no_run
use axutils::ConfigUtils;

fn main() -> Result<(), axutils::ConfigError> {
    let loader = ConfigUtils::loader()
        .with_max_bytes(64 * 1024)?
        .with_max_depth(16)?
        .with_env_substitution(false);
    let value = loader.load_value("app.toml")?;
    let _ = value;
    Ok(())
}
```

### `.env`（dotenv）语法

`.env` 解析器为本 crate 自实现，不依赖第三方包，随 `serde` feature 直接可用。文件名为
`.env` 或以 `.env.` 开头（例如 `.env.local`）时按文件名识别，其余情况按 `env` 扩展名识别。

| 语法元素 | 规则 |
| --- | --- |
| `KEY=VALUE` | 等号两侧允许空白；允许可选的 `export ` 前缀 |
| 注释 | 空行忽略；`#` 开头的整行为注释；未加引号的值中 ` #` 之后视为行尾注释 |
| 键名 | 限定为 `[A-Za-z_][A-Za-z0-9_]*`，不做大小写转换 |
| 无引号值 | 去掉首尾空白，不处理转义，不跨行 |
| 单引号值 | 原样保留，不做插值、不处理转义，可跨行直到配对的单引号 |
| 双引号值 | 处理 `\n`、`\r`、`\t`、`\\`、`\"`、`\$` 转义，做 `${VAR}` 插值，可跨行 |
| 重复键 | 返回 `DuplicateKey` 错误，不静默覆盖 |

解析结果是扁平表（`ConfigValue::Table`，值全部为字符串），不按 `__` 等分隔符拆分为嵌套结构；
需要嵌套配置请使用 TOML/YAML/JSON。

`${VAR}` 插值规则：

1. 先在当前文件中已解析的键里查找（文件优先）；
2. 找不到时，若 `with_env_substitution(true)`（默认）则回退到进程环境变量；
3. 两者都没有时返回 `UndefinedVariable`，**不会静默替换为空字符串**；
4. 只支持 `${VAR}` 形式，不支持 `$VAR`、`${VAR:-default}` 或命令替换；
5. 只能引用同一文件中出现在自身之前的键，不存在循环引用。

与 `dotenv`/`dotenvy` 的已知差异：插值优先级相反（本 crate 文件优先，`dotenvy` 环境变量
优先）、未定义变量报错而非空串、不支持 `$VAR` 无花括号形式、重复键报错而非静默覆盖。本 crate
不声称与其完全兼容。

```rust
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value(
    "HOST=localhost\nGREETING=\"hello, ${HOST}\"\n",
    ConfigFormat::Env,
)
.unwrap();
assert_eq!(value.get("GREETING").and_then(|v| v.as_str()), Some("hello, localhost"));
```

### 安全与资源上限

- **文件大小**：默认上限 1 MiB，可调 1 KiB–16 MiB；采用 `File::open` + `Read::take` 流式截断，
  不依赖 `fs::metadata` 报告的大小，避免 TOCTOU 及命名管道、`/proc` 等特殊文件耗尽内存。
- **嵌套深度**：默认上限 64，可调 1–256；JSON/TOML/INI 由本 crate 在构建 `ConfigValue` 时精确
  计数，YAML 由 `serde-saphyr` 自身的 `Budget::max_depth` 精确强制；JSON/TOML 有类型读取路径
  依赖各后端自身的递归限制，YAML/INI 有类型读取路径使用上述配置。
- **YAML 别名炸弹**：显式设置 `serde-saphyr` 的 `Budget`（含 `max_depth`）与
  `AliasLimits`：总回放事件最多 1,000,000 次、单个 anchor 最多展开 10,000 次、回放栈深度
  不超过嵌套深度上限，并关闭错误的源码片段渲染（`with_snippet: false`），防御 billion-laughs
  一类别名展开攻击。
- **重复键**：TOML 语法本身禁止；YAML 显式配置为拒绝；INI 与 `.env` 由本 crate 检测并拒绝；
  `serde_json` 的“后者覆盖”语义无法配置——五种格式在该点上行为不完全一致。
- **整数范围**：超出 `i64` 可表示范围的整数返回 `ValueOutOfRange`，不会静默转换为浮点数丢失
  精度（JSON 后端对不含小数点/指数、且超过 `u64::MAX` 的纯整数字面量存在已知限制，见
  `ConfigValue::Integer` 的 API doc）。
- **编码**：只接受 UTF-8，自动跳过前导 BOM；非 UTF-8 返回 `NotUtf8`，不做编码猜测。
- **错误脱敏**：错误不回显配置文件的原始内容、解析出的值或原始出错行文本；键名、格式名、文件
  路径和资源上限可能出现在错误中，配置值本身永远不会。
- **无隐式副作用**：不写入进程环境变量、不创建或修改任何文件、不解析 `include` 指令、不访问
  网络、不执行模板或表达式；配置文件常含凭据，调用方仍需自行限制文件权限，并避免把整棵
  `ConfigValue` 或反序列化结果打日志。

### 配置 feature 矩阵

| 启用组合 | `config` 模块/`ConfigUtils` | 支持的格式 |
| --- | --- | --- |
| 无 | 不导出 | 无 |
| 仅 `toml` / `serde-saphyr` / `rust-ini` | 不导出 | 无 |
| `serde` | 导出 | JSON、`.env` |
| `serde` + `toml` | 导出 | JSON、`.env`、TOML |
| `serde` + `serde-saphyr` | 导出 | JSON、`.env`、YAML |
| `serde` + `rust-ini` | 导出 | JSON、`.env`、INI |
| `serde` + 全部三个后端 | 导出 | 五种全部 |

只启用某个后端 feature 而不启用 `serde` 时，不会导出任何配置 API；未启用对应后端的格式，其
`ConfigFormat` 变体和解析函数都不存在于公共 API 中，而不是运行时报错。

## 构建与部署依赖

`axutils` 是 library crate，本身不生成可部署的二进制或容器。下表是消费方应用已有 Rust
工具链时的常见基础构建包；邮件功能本身不增加 TLS 系统包。应用的数据库、图像、压缩或其他
native 依赖仍需按其自身文档安装对应包。

| 环境 | 消费方源码构建的基础包 | 邮件功能额外 TLS 包 |
| --- | --- | --- |
| Debian/Ubuntu（含 slim） | `build-essential` | 无 |
| Alpine Linux | `build-base` | 无 |
| Fedora/RHEL | `gcc`、`make`、`glibc-devel`、按需 `binutils` | 无 |
| Windows MSVC | Visual Studio Build Tools 的 C++ workload 和 Windows SDK | 无 OpenSSL |
| macOS | Xcode Command Line Tools | 无 OpenSSL |

例如，已安装 Rust 但系统较精简时，消费方可以自行安装：

```bash
# Debian / Ubuntu
sudo apt-get update
sudo apt-get install -y --no-install-recommends build-essential

# Alpine Linux
sudo apk add --no-cache build-base

# Fedora / RHEL
sudo dnf install -y gcc make glibc-devel binutils

# macOS
xcode-select --install
```

这些命令是消费方主动配置开发环境的示例。Docker builder
还需要按镜像和取包方式提供 HTTPS 下载所需的工具/CA；那是构建期依赖，与 SMTP 运行时的
`webpki-roots` 不同。

### Debian/Ubuntu 多阶段 Docker 说明性模板

以下模板只适用于消费方自行替换后的应用，不是本仓库提供或验证过的 Dockerfile。请替换
`<pinned-version>`、`<binary-name>`、workspace 清单和 `src` 路径，并按应用的其他依赖调整：

```dockerfile
ARG RUST_VERSION
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/<binary-name> /usr/local/bin/app
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/app"]
```

`RUST_VERSION` 应由消费方替换为不低于 1.88 的固定版本，正式部署还应按供应链策略固定
基础镜像 digest。runtime 阶段不需要为 axutils 邮件功能安装 OpenSSL 或 CA 包；应用其他
功能需要的动态库、时区数据或健康检查工具须单独评估。

### Alpine/musl 多阶段 Docker 说明性模板

原生构建当前容器架构时，官方 Alpine Rust builder 通常直接执行 `cargo build`，不要无条件
写死 `x86_64-unknown-linux-musl`；Buildx 同时构建 ARM64 时尤其要避免把 AMD64 target 写死：

```dockerfile
ARG RUST_VERSION
FROM rust:${RUST_VERSION}-alpine AS builder

RUN apk add --no-cache build-base
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM alpine:<pinned-version> AS runtime
RUN addgroup -S app && adduser -S -G app app
WORKDIR /app
COPY --from=builder /app/target/release/<binary-name> /usr/local/bin/app
USER app
ENTRYPOINT ["/usr/local/bin/app"]
```

Alpine runtime 不需要为邮件功能安装 `openssl`、`libssl3` 或 `ca-certificates`；`curl` 也只有
在应用健康检查或其他功能实际需要时才添加。Alpine/musl、ARM64、WASM、Android、iOS 和
FreeBSD 不属于本 crate 首期已承诺的验证目标；消费方必须自行验证其完整应用和镜像。

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。

默认 feature 为空，当前 crate 默认不会启用第三方 `rand`、`regex`、`phonenumber`、`serde`、
`serde_json`、`strfmt`、`minijinja`、`chrono`、`time`、`jiff`、`lettre`、`tokio`、
`serde-saphyr`、`toml` 或 `rust-ini` 依赖；`PathUtils`、`TimeUtils` 和 `FormatUtils` 直接从
crate 根模块导出，`RandomUtils` 及其相关类型仅在启用 `rand` feature 后导出，`RegUtils` 仅在
启用 `regex` feature 后导出，邮件类型仅在启用 `lettre` feature 后导出，配置读取类型
（`ConfigLoader`、`ConfigUtils`、`ConfigFormat`、`ConfigValue`、`ConfigError`）仅在启用
`serde` feature 后导出，其 YAML/TOML/INI 变体和后端还分别需要额外启用
`serde-saphyr`/`toml`/`rust-ini` feature。`RegUtils::is_phone` 必须显式同时启用 `regex` 和
`libphonenumber` features；异步邮件必须显式同时启用 `lettre` 和 `tokio`。
