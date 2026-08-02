# RandomUtils 使用文档

> 需要显式启用 `rand` feature；使用 `rand` 的线程本地生成器生成普通 ASCII 字符串和闭区间
> 数值。该能力不是密码学安全随机数。

## 导出内容

公开模块路径：`axutils::random_utils` 和 `axutils::utils::random_utils`，模块与类型均只在
`rand` feature 下存在。

以下类型均支持四类公共路径，推荐 crate 根路径：

| 类型 | 推荐路径 | 其他可访问路径 |
| --- | --- | --- |
| `RandomUtils` | `axutils::RandomUtils` | `axutils::random_utils::RandomUtils`、`axutils::utils::RandomUtils`、`axutils::utils::random_utils::RandomUtils` |
| `LetterCase` | `axutils::LetterCase` | `axutils::random_utils::LetterCase`、`axutils::utils::LetterCase`、`axutils::utils::random_utils::LetterCase` |
| `RandomRangeError` | `axutils::RandomRangeError` | `axutils::random_utils::RandomRangeError`、`axutils::utils::RandomRangeError`、`axutils::utils::random_utils::RandomRangeError` |

`RandomUtils` 是无字段工具结构体，无 `new` 方法，提供静态关联方法，实现 `Debug`、`Clone`、
`Copy`、`Default`。`LetterCase` 实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`，可穷举匹配，
变体为：

- `LetterCase::Lower`：只生成 `a-z`；
- `LetterCase::Upper`：只生成 `A-Z`；
- `LetterCase::Mixed`：生成 `a-zA-Z`。

`RandomRangeError` 实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`、`Display` 和
`std::error::Error`，可穷举匹配，变体为：

- `InvalidRange`：整数或浮点区间反向，或底层均匀分布无法构造；
- `NonFiniteFloat`：浮点区间包含 `NaN` 或正负无穷。

本模块没有公共自由函数、trait、类型别名、常量、静态项或宏。

## 安装与启用

```toml
[dependencies]
axutils = { version = "0.1", features = ["rand"] }
```

## 函数与方法详解

### `RandomUtils::numeric_string(length: usize) -> Result<String, TryReserveError>`

- **参数**：`length` 是输出的 ASCII 字节数。
- **返回值**：成功时每一位均为 `0-9`，`length == 0` 返回空字符串；容量无法预留时返回
  标准库 `TryReserveError`。
- **示例**：

```rust
use axutils::RandomUtils;

let value = RandomUtils::numeric_string(12).expect("the string should be allocatable");
assert_eq!(value.len(), 12);
assert!(value.bytes().all(|byte| byte.is_ascii_digit()));
```

边界输入不会生成固定前导字符，零长度明确返回空串：

```rust
use axutils::RandomUtils;

assert_eq!(RandomUtils::numeric_string(0).unwrap(), "");
assert!(RandomUtils::numeric_string(usize::MAX).is_err());
```

**注意**：时间和额外空间复杂度为 `O(length)`；方法没有固定长度上限，调用方必须先限制不
可信的长度，避免超大分配。

### `RandomUtils::alphabetic_string(length: usize, case: LetterCase) -> Result<String, TryReserveError>`

- **参数**：`length` 是输出长度；`case` 选择 `Lower`、`Upper` 或 `Mixed` 字母表。
- **返回值**：成功时返回符合所选 ASCII 字母表的字符串；容量无法预留时返回
  `TryReserveError`；零长度返回空串。
- **示例**：

```rust
use axutils::{LetterCase, RandomUtils};

let lower = RandomUtils::alphabetic_string(8, LetterCase::Lower).unwrap();
let upper = RandomUtils::alphabetic_string(8, LetterCase::Upper).unwrap();
let mixed = RandomUtils::alphabetic_string(8, LetterCase::Mixed).unwrap();
assert!(lower.bytes().all(|byte| byte.is_ascii_lowercase()));
assert!(upper.bytes().all(|byte| byte.is_ascii_uppercase()));
assert!(mixed.bytes().all(|byte| byte.is_ascii_alphabetic()));
```

```rust
use axutils::{LetterCase, RandomUtils};

assert_eq!(
    RandomUtils::alphabetic_string(0, LetterCase::Mixed).unwrap(),
    ""
);
```

**注意**：`LetterCase` 只描述 ASCII 字母表，不会生成 Unicode 字母；资源边界与
`numeric_string` 相同。

### `RandomUtils::alphanumeric_string(length: usize) -> Result<String, TryReserveError>`

- **参数**：`length` 是输出的 ASCII 字节数。
- **返回值**：成功时每一位均为 `a-zA-Z0-9`；容量无法预留时返回 `TryReserveError`；零
  长度返回空串。
- **示例**：

```rust
use axutils::RandomUtils;

let value = RandomUtils::alphanumeric_string(16).unwrap();
assert_eq!(value.len(), 16);
assert!(value.bytes().all(|byte| byte.is_ascii_alphanumeric()));
```

**注意**：没有固定长度上限，调用方应限制不可信输入；生成器不是密码学安全生成器。

### `RandomUtils::integer(range: RangeInclusive<i64>) -> Result<i64, RandomRangeError>`

- **参数**：`range` 是包含起点和终点的 `i64` 闭区间。
- **返回值**：成功时返回区间内的值，起点等于终点时返回唯一值；空区间或底层均匀分布
  无法构造时返回 `RandomRangeError::InvalidRange`。
- **示例**：

```rust
use axutils::RandomUtils;

let value = RandomUtils::integer(1..=100).unwrap();
assert!((1..=100).contains(&value));
assert_eq!(RandomUtils::integer(42..=42), Ok(42));
```

```rust
use axutils::{RandomRangeError, RandomUtils};

assert_eq!(
    RandomUtils::integer(10..=1),
    Err(RandomRangeError::InvalidRange)
);
```

**注意**：区间是闭区间，不要把它当作 Rust `Range` 的半开区间；底层分布构造失败也归类为
`InvalidRange`，调用方不应依赖更细的原因。

### `RandomUtils::float(range: RangeInclusive<f64>) -> Result<f64, RandomRangeError>`

- **参数**：`range` 是包含起点和终点的有限 `f64` 闭区间。
- **返回值**：成功时返回区间内的值，单值区间返回该值；边界不是有限值时返回
  `NonFiniteFloat`；区间反向或底层分布无法构造时返回 `InvalidRange`。
- **示例**：

```rust
use axutils::RandomUtils;

let value = RandomUtils::float(-1.0..=1.0).unwrap();
assert!((-1.0..=1.0).contains(&value));
assert_eq!(RandomUtils::float(2.5..=2.5), Ok(2.5));
```

```rust
use axutils::{RandomRangeError, RandomUtils};

assert_eq!(
    RandomUtils::float(f64::NAN..=1.0),
    Err(RandomRangeError::NonFiniteFloat)
);
assert_eq!(
    RandomUtils::float(10.0..=1.0),
    Err(RandomRangeError::InvalidRange)
);
```

**注意**：`NaN`、正无穷和负无穷在区间检查中先被拒绝；极大但有限的跨度也可能无法由
底层均匀分布表示。

## 使用场景与限制

适合测试数据、临时标识和一般业务随机取值。底层使用 `rand` 的线程本地生成器；如果操作
系统随机源不可用，生成器初始化可能 panic。生成结果不承诺密码学安全，不能用于密码、
Session token、API key、授权码或其他需要抗预测性的秘密；需要密码学安全随机源时应使用
专门的密码学随机库。调用方还要限制字符串长度、调用频率和并发量。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
