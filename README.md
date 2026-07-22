# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前提供 `RegUtils` 和 `TimeUtils`：前者用于校验电子邮箱地址和中国大陆手机号码，
后者用于获取当前 Unix 时间戳。

## 安装

在项目的 `Cargo.toml` 中添加：

```toml
[dependencies]
axutils = "0.1"
```

`regex` 是默认 feature，用于启用依赖第三方 `regex` crate 的正则工具。如果需要显式声明 feature，或关闭默认 feature 后按需启用，可以写成：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["regex"] }
```

## 使用

```rust
use axutils::RegUtils;
use axutils::TimeUtils;

assert!(RegUtils::is_email("user@example.com"));
assert!(!RegUtils::is_email("user@example"));

assert!(RegUtils::is_phone_cn("13812345678"));
assert!(!RegUtils::is_phone_cn("12812345678"));

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

### `RegUtils::is_phone_cn`

使用以下正则表达式校验中国大陆手机号码：

```text
^1[3-9][0-9]{9}$
```

方法要求输入为 11 位数字，且号段以 `13` 至 `19` 开头。

### `TimeUtils`

`TimeUtils` 提供四个获取当前 Unix 时间戳的方法：

- `timestamp_seconds()`：秒，返回 `u64`；
- `timestamp_milliseconds()`：毫秒，返回 `u128`；
- `timestamp_microseconds()`：微秒，返回 `u128`；
- `timestamp_nanoseconds()`：纳秒，返回 `u128`。

```rust
use axutils::TimeUtils;

let seconds = TimeUtils::timestamp_seconds();
let milliseconds = TimeUtils::timestamp_milliseconds();
let microseconds = TimeUtils::timestamp_microseconds();
let nanoseconds = TimeUtils::timestamp_nanoseconds();

assert!(milliseconds / 1_000 >= seconds as u128);
assert!(microseconds / 1_000 >= milliseconds);
assert!(nanoseconds / 1_000 >= microseconds);
```

如果系统时间早于 Unix 纪元，这些方法会 panic。

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。

关闭 `regex` feature 后，当前 crate 不会启用第三方 `regex` 依赖，`RegUtils` 也不会导出。
未来不依赖第三方包的方法将直接属于默认能力，不添加 feature 守卫，并从 crate 根模块默认导出。
