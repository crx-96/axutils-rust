# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前项目最低支持 Rust 1.76。

当前提供 `TimeUtils` 和 `RegUtils`：前者不依赖第三方包，默认可用，用于获取当前 Unix
时间戳；后者的基础能力依赖第三方 `regex` crate，需要显式启用 `regex` feature，用于校验
电子邮箱地址和中国大陆手机号码。`RegUtils::is_phone` 还需要同时启用独立的
`libphonenumber` feature。

## 安装

在项目的 `Cargo.toml` 中添加：

```toml
[dependencies]
axutils = "0.1"
```

上面的依赖声明默认提供 `TimeUtils`。如果需要使用 `RegUtils`，请显式启用 `regex`
feature：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex"] }
```

如果需要校验带国家/地区前缀的国际手机号码，同时启用 `regex` 和 `libphonenumber`
features：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

## 使用 `TimeUtils`

```rust
use axutils::TimeUtils;

assert!(TimeUtils::timestamp_seconds() > 0);
assert!(TimeUtils::timestamp_milliseconds() > 0);
assert!(TimeUtils::timestamp_microseconds() > 0);
assert!(TimeUtils::timestamp_nanoseconds() > 0);
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

### `TimeUtils`

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

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。

默认 feature 为空，当前 crate 默认不会启用第三方 `regex` 或 `phonenumber` 依赖；
`TimeUtils` 直接从 crate 根模块导出，`RegUtils` 仅在启用 `regex` feature 后导出。
`RegUtils::is_phone` 必须显式同时启用 `regex` 和 `libphonenumber` features。
