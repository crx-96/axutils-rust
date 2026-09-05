# 时间

`TimeUtils` 从 `axutils::utils::TimeUtils` 导入；格式错误和固定偏移类型从 `axutils::time` 导入。
Unix 时间戳默认可用。日期格式化按 `chrono`、`time`、`jiff` 三个独立 feature 提供，并且**始终**使用
后缀 API，因此多后端同时启用时路径和方法名不会变化。

## Unix 时间戳

优先使用 `try_timestamp*` 系列。它们在系统时钟早于 Unix 纪元时返回
`axutils::time::TimeError::BeforeUnixEpoch`；不建议使用保留的旧 panic 风格入口。

```rust
use axutils::{time::TimeError, utils::TimeUtils};

let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::try_timestamp()?;
assert!(milliseconds >= u128::from(seconds) * 1_000);
assert!(microseconds >= milliseconds * 1_000);
assert!(nanoseconds >= microseconds * 1_000);
# Ok::<(), TimeError>(())
```

## 启用日期后端

日期类型位于公开签名中，因此应用必须直接声明自己使用的日期 crate：

```toml
[dependencies]
axutils = { version = "1.0", features = ["chrono", "time", "jiff"] }
chrono = { version = "0.4", default-features = false }
time = { version = "0.3", default-features = false }
jiff = { version = "0.2", default-features = false }
```

统一模板 token 为 `yyyy`、`MM`、`dd`、`HH`、`mm`、`ss`、`SSS` 和偏移 token `XXX`。日期不支持
时间 token；无偏移日期时间不支持 `XXX`。模板字母字面量需以单引号包围，`''` 表示一个单引号。
返回 `Result` 的方法保留 `TimeFormatError`，`format_option_*` 则把空值或格式错误折叠为 `None`。

## Chrono

```rust
use axutils::{
    time::TimeZoneOffset,
    utils::TimeUtils,
};
use chrono::NaiveDate;

let value = NaiveDate::from_ymd_opt(2024, 2, 29)
    .unwrap()
    .and_hms_opt(1, 2, 3)
    .unwrap();
assert_eq!(
    TimeUtils::format_datetime_with_offset_chrono(
        value,
        Some(TimeZoneOffset::UTC),
        Some("yyyy-MM-dd HH:mm:ss XXX"),
    )
    .unwrap(),
    "2024-02-29 01:02:03 Z",
);
```

## `time`

```rust
use axutils::utils::TimeUtils;
use time::{Date, Month};

let date = Date::from_calendar_date(2024, Month::February, 29).unwrap();
let value = date.with_hms(1, 2, 3).unwrap();
assert_eq!(
    TimeUtils::format_datetime_time(value, Some("yyyy/MM/dd HH:mm:ss")).unwrap(),
    "2024/02/29 01:02:03",
);
```

## Jiff

```rust
use axutils::utils::TimeUtils;
use jiff::civil::DateTime;

let value = DateTime::new(2024, 2, 29, 1, 2, 3, 0).unwrap();
assert_eq!(TimeUtils::format_datetime_jiff(value, None).unwrap(), "2024-02-29 01:02:03");
assert_eq!(TimeUtils::format_option_datetime_jiff(None, None), None);
```

## 固定偏移与边界

`TimeZoneOffset::UTC` 是 `Z`，`TimeZoneOffset::DEFAULT` 是 `+08:00`；可用
`TimeZoneOffset::from_hours` 或 `from_seconds` 校验构造。带偏移格式化只附加固定偏移文本，**不会**
转换日期字段、查询 IANA 时区或处理 DST。需要日历运算、时区数据库或本地化显示时，应由应用选择
相应日期库。

模板来自不可信输入时，调用方应限制其长度和调用频率。`TimeFormatError` 仅给出 token 分类或 UTF-8
字节位置，不回显整个模板或日期值。
