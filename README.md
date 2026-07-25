# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前提供 `TimeUtils` 和 `RegUtils`：前者不依赖第三方包，默认可用，用于获取当前 Unix
时间戳；后者依赖第三方 `regex` crate，需要显式启用 `regex` feature，用于校验电子邮箱
地址和中国大陆手机号码。

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

### `RegUtils::is_phone_cn`

使用以下正则表达式校验中国大陆手机号码：

```text
^1[3-9][0-9]{9}$
```

方法要求输入为 11 位数字，且号段以 `13` 至 `19` 开头。

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

assert!(RegUtils::is_phone_cn("13812345678"));
assert!(!RegUtils::is_phone_cn("12812345678"));
```

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。

默认 feature 为空，当前 crate 默认不会启用第三方 `regex` 依赖；`TimeUtils` 直接从 crate
根模块导出，`RegUtils` 仅在启用 `regex` feature 后导出。
