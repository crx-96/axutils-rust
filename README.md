# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前项目最低支持 Rust 1.76。

当前提供 `PathUtils`、`TimeUtils`、`FormatUtils`、`RegUtils` 和 `RandomUtils`：`PathUtils`、
`TimeUtils` 与 `FormatUtils` 不依赖第三方包，默认可用，分别用于路径处理、获取当前 Unix
时间戳和将秒数格式化为中文持续时间字符串；`RegUtils` 的基础能力依赖第三方 `regex` crate，
需要显式启用 `regex` feature，用于校验电子邮箱地址和中国大陆手机号码；`RandomUtils` 依赖
第三方 `rand` crate，需要显式启用 `rand` feature。`RegUtils::is_phone` 还需要同时启用独立的
`libphonenumber` feature。

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

如果需要校验带国家/地区前缀的国际手机号码，同时启用 `regex` 和 `libphonenumber`
features：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
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

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。

默认 feature 为空，当前 crate 默认不会启用第三方 `rand`、`regex` 或 `phonenumber` 依赖；
`PathUtils`、`TimeUtils` 和 `FormatUtils` 直接从 crate 根模块导出，`RandomUtils` 及其相关类型仅在启用
`rand` feature 后导出，`RegUtils` 仅在启用 `regex` feature 后导出。`RegUtils::is_phone`
必须显式同时启用 `regex` 和 `libphonenumber` features。
