# TimeUtils 使用文档

> 时间戳方法默认可用；日期格式化分别需要 `chrono`、`time` 或 `jiff` feature。三个后端互相
> 独立，单独启用一个后端时提供无后缀简写，多后端同时启用时只提供带后缀的方法。

## 导出内容

公开模块路径：

- `axutils::time_utils`；
- `axutils::utils::time_utils`。

`TimeUtils` 可从以下路径导入：

- 推荐：`axutils::TimeUtils`；
- `axutils::time_utils::TimeUtils`；
- `axutils::utils::TimeUtils`；
- `axutils::utils::time_utils::TimeUtils`。

`TimeError`、`TimeZoneOffset`、`TimeZoneOffsetError`、`TimeFormatError`、`TimeFormatToken` 和
`TimeValueKind` 由 crate 根直接导出：`axutils::TimeError`、`axutils::TimeZoneOffset` 等；
它们不从公开的 `time_utils` 或 `utils::time_utils` 路径导出。`src/time/` 是私有实现目录，
不是公共导入路径。

`TimeUtils` 是无字段工具结构体，无 `new` 方法，实现 `Debug`、`Clone`、`Copy`、`Default`。
时间戳方法不依赖第三方 feature。

`TimeZoneOffset` 的字段私有，实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`、`Hash`、
`Default`；`TimeZoneOffsetError` 实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`、
`Display` 和 `std::error::Error`。二者都没有 `#[non_exhaustive]`。

`TimeError` 实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`、`Display` 和
`std::error::Error`，变体为 `BeforeUnixEpoch`；它不回显路径或系统环境文本。

`TimeFormatToken` 有 `Year`、`Month`、`Day`、`Hour`、`Minute`、`Second`、`Millisecond`、
`Offset`、`UnknownAscii` 九个变体；`TimeValueKind` 有 `Date`、`DateTime`、
`DateTimeWithOffset` 三个变体。二者实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`，
且没有 `#[non_exhaustive]`。`TimeFormatError` 实现同样的派生 trait，以及 `Display` 和
`std::error::Error`，没有 `#[non_exhaustive]`，变体为：

- `UnclosedLiteral { position: usize }`；
- `UnsupportedToken { position: usize, token: TimeFormatToken }`；
- `TokenNotSupportedForValue { position: usize, token: TimeFormatToken, value_kind: TimeValueKind }`；
- `UnsupportedLeapSecond`（Chrono 闰秒内部值不受统一模板支持）。

前三个错误的 `position` 是模板中的 UTF-8 字节偏移，不是字符索引；错误不会回显完整模板或
日期值。

本模块没有公共自由函数、trait、类型别名、静态项或宏。

## 安装与启用

时间戳能力：

```toml
[dependencies]
axutils = "0.1"
```

日期后端按需选择：

```toml
[dependencies]
axutils = { version = "0.1", features = ["chrono"] }
chrono = { version = "0.4", default-features = false }
# 或 features = ["time"]
# 或 features = ["jiff"]
```

选择 `time` 或 `jiff` 时，也要在应用的 `[dependencies]` 中直接声明对应版本的
`time` 或 `jiff` crate；日期类型会出现在公开方法签名中，不能依赖 axutils 的传递依赖。

多个后端可以同时启用，但调用时使用 `format_*_chrono`、`format_*_time` 或
`format_*_jiff` 区分具体日期类型；无后缀简写只在恰好一个后端启用时存在。

## 时间戳方法

所有时间戳入口都默认可用，不需要第三方 feature。推荐使用下面五个 `try_timestamp*`
入口：它们将系统时钟早于 Unix 纪元这一可恢复环境错误返回为
`TimeError::BeforeUnixEpoch`，不会因该状态 panic。

### `TimeError::BeforeUnixEpoch`

`TimeError` 从 crate 根导出：`axutils::TimeError`。当前稳定变体
`TimeError::BeforeUnixEpoch` 表示 `SystemTime` 早于 1970-01-01 00:00:00 UTC；它的
`Display`/`Debug` 只包含固定分类文本，不包含路径或系统环境文本，并实现
`std::error::Error`。该枚举当前不使用 `#[non_exhaustive]`；调用方可直接按变体匹配，或为
未来错误扩展保留 wildcard 分支。

### `TimeUtils::try_timestamp() -> Result<(u64, u128, u128, u128), TimeError>`

- **返回值**：按同一次 `SystemTime::now()` 采样依次返回秒、毫秒、微秒、纳秒 Unix 时间戳。
- **错误**：系统时钟早于 Unix 纪元时返回 `Err(TimeError::BeforeUnixEpoch)`。
- **示例**：

```rust
use axutils::TimeUtils;

let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::try_timestamp()?;
assert!(seconds > 0);
assert_eq!(milliseconds / 1_000, seconds as u128);
assert_eq!(microseconds / 1_000_000, seconds as u128);
assert_eq!(nanoseconds / 1_000_000_000, seconds as u128);
# Ok::<(), axutils::TimeError>(())
```

四个值基于同一次采样；系统时钟在调用之间可能调整，不能把不同调用的精度值当作同一次
采样。

### `TimeUtils::try_timestamp_seconds() -> Result<u64, TimeError>`

- **返回值**：当前 Unix 纪元以来的完整秒数；独立采样系统时间。
- **错误**：系统时钟早于 Unix 纪元时返回 `Err(TimeError::BeforeUnixEpoch)`。
- **示例**：

```rust
use axutils::TimeUtils;

assert!(TimeUtils::try_timestamp_seconds()?.checked_add(1).is_some());
# Ok::<(), axutils::TimeError>(())
```

### `TimeUtils::try_timestamp_milliseconds() -> Result<u128, TimeError>`

- **返回值**：当前 Unix 纪元以来的完整毫秒数；独立采样系统时间。
- **错误**：系统时钟早于 Unix 纪元时返回 `Err(TimeError::BeforeUnixEpoch)`。
- **示例**：

```rust
use axutils::TimeUtils;

assert!(TimeUtils::try_timestamp_milliseconds()? > 0);
# Ok::<(), axutils::TimeError>(())
```

### `TimeUtils::try_timestamp_microseconds() -> Result<u128, TimeError>`

- **返回值**：当前 Unix 纪元以来的完整微秒数；独立采样系统时间。
- **错误**：系统时钟早于 Unix 纪元时返回 `Err(TimeError::BeforeUnixEpoch)`。
- **示例**：

```rust
use axutils::TimeUtils;

assert!(TimeUtils::try_timestamp_microseconds()? > 0);
# Ok::<(), axutils::TimeError>(())
```

### `TimeUtils::try_timestamp_nanoseconds() -> Result<u128, TimeError>`

- **返回值**：当前 Unix 纪元以来的完整纳秒数；独立采样系统时间。
- **错误**：系统时钟早于 Unix 纪元时返回 `Err(TimeError::BeforeUnixEpoch)`。
- **示例**：

```rust
use axutils::TimeUtils;

assert!(TimeUtils::try_timestamp_nanoseconds()? > 0);
# Ok::<(), axutils::TimeError>(())
```

### 兼容入口 `TimeUtils::timestamp() -> (u64, u128, u128, u128)`

该入口保留原有返回签名和 panic 语义，现已标记 `deprecated`；系统时钟早于 Unix 纪元时
仍可能 panic。迁移到 [`TimeUtils::try_timestamp`]，不要把弃用入口用于不受控的系统时钟环境。

```rust
#![allow(deprecated)]

use axutils::TimeUtils;

let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
assert!(seconds > 0);
assert_eq!(milliseconds / 1_000, seconds as u128);
assert_eq!(microseconds / 1_000_000, seconds as u128);
assert_eq!(nanoseconds / 1_000_000_000, seconds as u128);
```

### 兼容入口 `TimeUtils::timestamp_seconds() -> u64`

该入口保留原有返回签名和 panic 语义，现已标记 `deprecated`；系统时钟早于 Unix 纪元时
仍可能 panic。迁移到 [`TimeUtils::try_timestamp_seconds`]。

```rust
#![allow(deprecated)]

use axutils::TimeUtils;

assert!(TimeUtils::timestamp_seconds() > 0);
```

### 兼容入口 `TimeUtils::timestamp_milliseconds() -> u128`

该入口保留原有返回签名和 panic 语义，现已标记 `deprecated`；系统时钟早于 Unix 纪元时
仍可能 panic。迁移到 [`TimeUtils::try_timestamp_milliseconds`]。

```rust
#![allow(deprecated)]

use axutils::TimeUtils;

assert!(TimeUtils::timestamp_milliseconds() > 0);
```

### 兼容入口 `TimeUtils::timestamp_microseconds() -> u128`

该入口保留原有返回签名和 panic 语义，现已标记 `deprecated`；系统时钟早于 Unix 纪元时
仍可能 panic。迁移到 [`TimeUtils::try_timestamp_microseconds`]。

```rust
#![allow(deprecated)]

use axutils::TimeUtils;

assert!(TimeUtils::timestamp_microseconds() > 0);
```

### 兼容入口 `TimeUtils::timestamp_nanoseconds() -> u128`

该入口保留原有返回签名和 panic 语义，现已标记 `deprecated`；系统时钟早于 Unix 纪元时
仍可能 panic。迁移到 [`TimeUtils::try_timestamp_nanoseconds`]。

```rust
#![allow(deprecated)]

use axutils::TimeUtils;

assert!(TimeUtils::timestamp_nanoseconds() > 0);
```

## `TimeZoneOffset` 方法与常量

固定偏移不是 IANA 时区：它不查询夏令时、不转换日期字段，只在带偏移格式化方法中附加到
原始日期时间。偏移可以是 `-23:59:59` 到 `+23:59:59` 的固定秒数。

### `TimeZoneOffset::UTC`

零秒固定偏移常量，适合明确输出 UTC：

```rust
use axutils::TimeZoneOffset;

assert_eq!(TimeZoneOffset::UTC.as_seconds(), 0);
```

### `TimeZoneOffset::DEFAULT`

默认固定偏移常量，值为 `+08:00`：

```rust
use axutils::TimeZoneOffset;

assert_eq!(TimeZoneOffset::DEFAULT.as_seconds(), 8 * 3_600);
assert_eq!(TimeZoneOffset::default(), TimeZoneOffset::DEFAULT);
```

### `TimeZoneOffset::from_hours(hours: i32) -> Result<Self, TimeZoneOffsetError>`

- **参数**：整小时偏移，允许 `-23..=23`。
- **返回值**：成功时返回固定偏移；超出范围返回
  `TimeZoneOffsetError::HoursOutOfRange { hours }`。
- **示例**：

```rust
use axutils::{TimeZoneOffset, TimeZoneOffsetError};

assert_eq!(TimeZoneOffset::from_hours(8).unwrap().as_seconds(), 28_800);
assert_eq!(
    TimeZoneOffset::from_hours(24),
    Err(TimeZoneOffsetError::HoursOutOfRange { hours: 24 })
);
```

### `TimeZoneOffset::from_seconds(seconds: i32) -> Result<Self, TimeZoneOffsetError>`

- **参数**：秒级固定偏移，允许 `-86_399..=86_399`。
- **返回值**：成功时返回固定偏移；超出范围返回
  `TimeZoneOffsetError::SecondsOutOfRange { seconds }`。
- **示例**：

```rust
use axutils::{TimeZoneOffset, TimeZoneOffsetError};

assert_eq!(
    TimeZoneOffset::from_seconds(19_815).unwrap().as_seconds(),
    19_815
);
assert_eq!(
    TimeZoneOffset::from_seconds(86_400),
    Err(TimeZoneOffsetError::SecondsOutOfRange { seconds: 86_400 })
);
```

### `TimeZoneOffset::as_seconds(self) -> i32`

- **feature**：默认可用；这是 `const fn`。
- **返回值**：固定偏移相对于 UTC 的秒数。
- **示例**：

```rust
use axutils::TimeZoneOffset;

const OFFSET_SECONDS: i32 = TimeZoneOffset::UTC.as_seconds();
assert_eq!(OFFSET_SECONDS, 0);
```

## 统一日期模板

日期后端共享下表中的 token：

| token | 含义 | 可用于 |
| --- | --- | --- |
| `yyyy` | 带符号、绝对值至少四位的公历年 | 日期、日期时间、带偏移日期时间 |
| `MM` | 两位月份 | 全部 |
| `dd` | 两位日期 | 全部 |
| `HH` | 两位小时 | 日期时间、带偏移日期时间 |
| `mm` | 两位分钟 | 日期时间、带偏移日期时间 |
| `ss` | 两位秒 | 日期时间、带偏移日期时间 |
| `SSS` | 纳秒截断为三位毫秒 | 日期时间、带偏移日期时间 |
| `XXX` | `Z` 或固定 UTC 偏移 | 仅带偏移日期时间 |

`None` 模板使用日期默认模板 `yyyy-MM-dd`，日期时间默认模板
`yyyy-MM-dd HH:mm:ss`。ASCII 字母必须属于上表 token，或者放在单引号字面量中；`''` 表示
一个单引号。未知 ASCII 字母返回 `UnsupportedToken`，未闭合字面量返回 `UnclosedLiteral`，
把时间 token 用在日期上或把 `XXX` 用在无偏移值上返回 `TokenNotSupportedForValue`。

带偏移方法的 `offset: None` 使用 `TimeZoneOffset::DEFAULT`；偏移只附加到原始字段，不做
日期时区转换。`XXX` 在零偏移时输出 `Z`，整分钟非零偏移输出 `+HH:MM`，带秒的偏移可能
输出 `+HH:MM:SS`。`format_option_*` 的输入为 `None` 时返回 `None`，格式化错误也折叠为
`None`，不会把错误暴露给调用方。

## Chrono 后端（`chrono` feature）

以下方法只能在启用 `chrono` 时使用；返回错误时使用 `TimeFormatError`。

### `TimeUtils::format_date_chrono(value: chrono::NaiveDate, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 Chrono 无时区日期；`template` 为 `None` 使用日期默认模板，
  成功返回字符串，模板不适用时返回 `TimeFormatError`。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::TimeUtils;

let date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
assert_eq!(TimeUtils::format_date_chrono(date, None).unwrap(), "2024-02-29");
assert!(TimeUtils::format_date_chrono(date, Some("HH")).is_err());
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

### `TimeUtils::format_option_date_chrono(value: Option<chrono::NaiveDate>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误均返回 `None`；有值时规则与
  `format_date_chrono` 相同。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::TimeUtils;

let date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29);
assert_eq!(TimeUtils::format_option_date_chrono(date, None), Some("2024-02-29".to_owned()));
assert_eq!(TimeUtils::format_option_date_chrono(None, None), None);
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

### `TimeUtils::format_datetime_chrono(value: chrono::NaiveDateTime, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 Chrono 无时区日期时间；可使用日期、时间和毫秒 token，
  `XXX` 不适用；成功返回格式化字符串。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::TimeUtils;

let value = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap().and_hms_opt(1, 2, 3).unwrap();
assert_eq!(
    TimeUtils::format_datetime_chrono(value, Some("yyyy/MM/dd HH:mm:ss")).unwrap(),
    "2024/02/29 01:02:03"
);
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_chrono(value: Option<chrono::NaiveDateTime>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；有值时规则与
  `format_datetime_chrono` 相同。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::TimeUtils;

assert_eq!(TimeUtils::format_option_datetime_chrono(None, None), None);
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

### `TimeUtils::format_datetime_with_offset_chrono(value: chrono::NaiveDateTime, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 Chrono 无时区日期时间；`offset` 为 `None` 时使用 `+08:00`；
  `template` 需要 `XXX` 才会输出偏移；成功返回字符串。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

let value = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap().and_hms_opt(1, 2, 3).unwrap();
assert_eq!(
    TimeUtils::format_datetime_with_offset_chrono(
        value,
        Some(TimeZoneOffset::UTC),
        Some("yyyy-MM-dd HH:mm:ss XXX")
    ).unwrap(),
    "2024-02-29 01:02:03 Z"
);
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_with_offset_chrono(value: Option<chrono::NaiveDateTime>, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；其他规则与带偏移 Chrono 方法相同。
- **示例**：

```rust
# #[cfg(feature = "chrono")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

assert_eq!(
    TimeUtils::format_option_datetime_with_offset_chrono(
        None,
        Some(TimeZoneOffset::UTC),
        Some("XXX")
    ),
    None
);
# }
# #[cfg(not(feature = "chrono"))]
# fn main() {}
```

## `time` 后端（`time` feature）

以下方法只能在启用 `time` 时使用。日期参数为 `time::Date`，日期时间参数为
`time::PrimitiveDateTime`。

### `TimeUtils::format_date_time(value: time::Date, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 `time::Date`；`None` 模板使用 `yyyy-MM-dd`，日期上使用时间
  token 会返回错误。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::TimeUtils;

let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
assert_eq!(TimeUtils::format_date_time(date, None).unwrap(), "2024-02-29");
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

### `TimeUtils::format_option_date_time(value: Option<time::Date>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；有值时规则与 `format_date_time` 相同。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::TimeUtils;

assert_eq!(TimeUtils::format_option_date_time(None, None), None);
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

### `TimeUtils::format_datetime_time(value: time::PrimitiveDateTime, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 `time::PrimitiveDateTime`；可使用日期、时间和毫秒 token，
  不支持 `XXX`。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::TimeUtils;

let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
let value = date.with_hms(1, 2, 3).unwrap();
assert_eq!(TimeUtils::format_datetime_time(value, None).unwrap(), "2024-02-29 01:02:03");
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_time(value: Option<time::PrimitiveDateTime>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；有值时规则与 `format_datetime_time` 相同。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::TimeUtils;

assert_eq!(TimeUtils::format_option_datetime_time(None, None), None);
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

### `TimeUtils::format_datetime_with_offset_time(value: time::PrimitiveDateTime, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`offset: None` 使用 `+08:00`；只有在模板包含 `XXX` 时才输出偏移，且
  偏移只附加、不转换日期时间字段。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
let value = date.with_hms(1, 2, 3).unwrap();
assert_eq!(
    TimeUtils::format_datetime_with_offset_time(value, Some(TimeZoneOffset::UTC), Some("XXX")).unwrap(),
    "Z"
);
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_with_offset_time(value: Option<time::PrimitiveDateTime>, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；其他规则与带偏移 `time` 方法相同。
- **示例**：

```rust
# #[cfg(feature = "time")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

assert_eq!(
    TimeUtils::format_option_datetime_with_offset_time(None, Some(TimeZoneOffset::UTC), Some("XXX")),
    None
);
# }
# #[cfg(not(feature = "time"))]
# fn main() {}
```

## Jiff 后端（`jiff` feature）

以下方法只能在启用 `jiff` 时使用。日期参数为 `jiff::civil::Date`，日期时间参数为
`jiff::civil::DateTime`。

### `TimeUtils::format_date_jiff(value: jiff::civil::Date, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 Jiff civil 日期；`None` 模板使用 `yyyy-MM-dd`。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::TimeUtils;

let date = jiff::civil::Date::new(2024, 2, 29).unwrap();
assert_eq!(TimeUtils::format_date_jiff(date, None).unwrap(), "2024-02-29");
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

### `TimeUtils::format_option_date_jiff(value: Option<jiff::civil::Date>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；有值时规则与 `format_date_jiff` 相同。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::TimeUtils;

let date = jiff::civil::Date::new(2024, 2, 29).ok();
assert_eq!(TimeUtils::format_option_date_jiff(date, None), Some("2024-02-29".to_owned()));
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

### `TimeUtils::format_datetime_jiff(value: jiff::civil::DateTime, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`value` 是 Jiff civil 日期时间；可使用日期、时间和毫秒 token，不能使用
  `XXX`。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::TimeUtils;

let value = jiff::civil::DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
assert_eq!(TimeUtils::format_datetime_jiff(value, None).unwrap(), "2024-02-29 01:02:03");
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_jiff(value: Option<jiff::civil::DateTime>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；有值时规则与 `format_datetime_jiff` 相同。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::TimeUtils;

assert_eq!(TimeUtils::format_option_datetime_jiff(None, None), None);
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

### `TimeUtils::format_datetime_with_offset_jiff(value: jiff::civil::DateTime, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Result<String, TimeFormatError>`

- **参数/返回值**：`offset: None` 使用 `+08:00`；需要输出偏移时在模板中使用 `XXX`；偏移
  只附加到 Jiff 字段。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

let value = jiff::civil::DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
assert_eq!(
    TimeUtils::format_datetime_with_offset_jiff(value, Some(TimeZoneOffset::UTC), Some("XXX")).unwrap(),
    "Z"
);
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_with_offset_jiff(value: Option<jiff::civil::DateTime>, offset: Option<TimeZoneOffset>, template: Option<&str>) -> Option<String>`

- **参数/返回值**：`None` 输入或格式错误返回 `None`；其他规则与带偏移 Jiff 方法相同。
- **示例**：

```rust
# #[cfg(feature = "jiff")]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};

assert_eq!(
    TimeUtils::format_option_datetime_with_offset_jiff(None, Some(TimeZoneOffset::UTC), Some("XXX")),
    None
);
# }
# #[cfg(not(feature = "jiff"))]
# fn main() {}
```

## 单后端无后缀简写

以下 6 个方法只在对应后端是唯一启用的后端时存在；`chrono,time`、`time,jiff`、
`chrono,jiff` 或三者同时启用时，无后缀方法全部不可用。

### `TimeUtils::format_date(value, template)`

在唯一后端下，它等价于对应的 `format_date_chrono`、`format_date_time` 或
`format_date_jiff`，参数类型分别为 `chrono::NaiveDate`、`time::Date` 或
`jiff::civil::Date`，返回 `Result<String, TimeFormatError>`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let value = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
assert_eq!(TimeUtils::format_date(value, None).unwrap(), "2024-02-29");
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let value = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
assert_eq!(TimeUtils::format_date(value, None).unwrap(), "2024-02-29");
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::TimeUtils;
let value = jiff::civil::Date::new(2024, 2, 29).unwrap();
assert_eq!(TimeUtils::format_date(value, None).unwrap(), "2024-02-29");
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

### `TimeUtils::format_option_date(value, template)`

唯一后端下，它等价于对应的 `format_option_date_*`，参数为对应日期类型的 `Option`，返回
`Option<String>`；空输入或格式错误折叠为 `None`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let value = chrono::NaiveDate::from_ymd_opt(2024, 2, 29);
assert_eq!(TimeUtils::format_option_date(value, None), Some("2024-02-29".to_owned()));
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let value = time::Date::from_calendar_date(2024, time::Month::February, 29).ok();
assert_eq!(TimeUtils::format_option_date(value, None), Some("2024-02-29".to_owned()));
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::TimeUtils;
let value = jiff::civil::Date::new(2024, 2, 29).ok();
assert_eq!(TimeUtils::format_option_date(value, None), Some("2024-02-29".to_owned()));
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

### `TimeUtils::format_datetime(value, template)`

唯一后端下，它等价于对应的 `format_datetime_*`，参数为对应无偏移日期时间类型，返回
`Result<String, TimeFormatError>`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
assert_eq!(TimeUtils::format_datetime(date.and_hms_opt(1, 2, 3).unwrap(), None).unwrap(), "2024-02-29 01:02:03");
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
assert_eq!(TimeUtils::format_datetime(date.with_hms(1, 2, 3).unwrap(), None).unwrap(), "2024-02-29 01:02:03");
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::TimeUtils;
let value = jiff::civil::DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
assert_eq!(TimeUtils::format_datetime(value, None).unwrap(), "2024-02-29 01:02:03");
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

### `TimeUtils::format_option_datetime(value, template)`

唯一后端下，它等价于对应的 `format_option_datetime_*`，参数为对应日期时间类型的
`Option`，返回 `Option<String>`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
assert_eq!(TimeUtils::format_option_datetime(None, None), None);
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::TimeUtils;
assert_eq!(TimeUtils::format_option_datetime(None, None), None);
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::TimeUtils;
assert_eq!(TimeUtils::format_option_datetime(None, None), None);
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

### `TimeUtils::format_datetime_with_offset(value, offset, template)`

唯一后端下，它等价于对应的 `format_datetime_with_offset_*`，参数为对应无偏移日期时间、
`Option<TimeZoneOffset>` 和模板，返回 `Result<String, TimeFormatError>`。`offset: None`
使用 `+08:00`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
let date = chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
let value = date.and_hms_opt(1, 2, 3).unwrap();
assert_eq!(TimeUtils::format_datetime_with_offset(value, Some(TimeZoneOffset::UTC), Some("XXX")).unwrap(), "Z");
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
let date = time::Date::from_calendar_date(2024, time::Month::February, 29).unwrap();
let value = date.with_hms(1, 2, 3).unwrap();
assert_eq!(TimeUtils::format_datetime_with_offset(value, Some(TimeZoneOffset::UTC), Some("XXX")).unwrap(), "Z");
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
let value = jiff::civil::DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
assert_eq!(TimeUtils::format_datetime_with_offset(value, Some(TimeZoneOffset::UTC), Some("XXX")).unwrap(), "Z");
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

### `TimeUtils::format_option_datetime_with_offset(value, offset, template)`

唯一后端下，它等价于对应的 `format_option_datetime_with_offset_*`，参数为对应日期时间的
`Option`、偏移和模板，返回 `Option<String>`；空输入或格式错误返回 `None`。

```rust
# #[cfg(all(feature = "chrono", not(any(feature = "time", feature = "jiff"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
assert_eq!(TimeUtils::format_option_datetime_with_offset(None, Some(TimeZoneOffset::UTC), Some("XXX")), None);
# }
# #[cfg(all(feature = "time", not(any(feature = "chrono", feature = "jiff"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
assert_eq!(TimeUtils::format_option_datetime_with_offset(None, Some(TimeZoneOffset::UTC), Some("XXX")), None);
# }
# #[cfg(all(feature = "jiff", not(any(feature = "chrono", feature = "time"))))]
# fn main() {
use axutils::{TimeUtils, TimeZoneOffset};
assert_eq!(TimeUtils::format_option_datetime_with_offset(None, Some(TimeZoneOffset::UTC), Some("XXX")), None);
# }
# #[cfg(any(
#     not(any(feature = "chrono", feature = "time", feature = "jiff")),
#     all(feature = "chrono", feature = "time"),
#     all(feature = "chrono", feature = "jiff"),
#     all(feature = "time", feature = "jiff")
# ))]
# fn main() {}
```

## 使用场景与限制

时间戳方法适合读取当前 Unix 时间；日期方法适合在已经选定日期类型后用统一模板输出。该
模块不提供 IANA 时区数据库、夏令时转换、日期算术、系统时钟校准或本地化格式。格式化模板
和日期输入来自不可信来源时，调用方应限制字符串长度与调用频率；`format_option_*` 将所有
格式错误折叠成 `None`，需要诊断时应调用返回 `Result` 的对应方法。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
