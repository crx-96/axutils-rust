# ConvertUtils 使用文档

> `ConvertUtils` 始终可用；整数、浮点数和 UUID 转换分别需要 `itoa`、`ryu`/`zmij` 和
> `uuid` feature。推荐从 `axutils::ConvertUtils` 导入。本文覆盖当前全部公共类型、方法、
> feature 边界、解析错误和 buffer 生命周期。

## 导出内容

`ConvertUtils` 和 `convert` 模块始终公开；本次能力不创建 `axutils::convert_utils` 根模块。
实现子模块 `integer`、`float`、`uuid` 是私有的，不应从 `axutils::convert::integer` 等路径访问。

`ConvertUtils` 在可用时从以下四条路径指向同一个定义；`IntegerBuffer`/`IntegerValue`、
`FloatBuffer`/`FloatFormat`/`FloatValue` 和 `UuidBuffer` 属于 `convert` 领域实现，只从 crate 根
和 `axutils::convert` 导出，不从 `utils` 导出：

- 推荐的 crate 根路径：`axutils::ConvertUtils` 及 `axutils::{IntegerBuffer, IntegerValue}` 等重导出；
- `ConvertUtils` 的领域模块路径：`axutils::convert::ConvertUtils`；
- `ConvertUtils` 的 `utils` 兼容路径：`axutils::utils::ConvertUtils`；
- `ConvertUtils` 的 `utils` 子模块兼容路径：`axutils::utils::convert_utils::ConvertUtils`；
- 领域类型的路径：例如 `axutils::convert::{IntegerBuffer, IntegerValue}`、
  `axutils::convert::{FloatBuffer, FloatFormat, FloatValue}` 和 `axutils::convert::UuidBuffer`。

`ConvertUtils` 是 `#[derive(Debug, Clone, Copy, Default)]` 的零大小工具结构体，所有入口都是
关联函数，不需要实例状态。模块没有公共自由函数、常量、静态项、类型别名或宏。`IntegerValue`
和 `FloatValue` 是本 crate 定义的 sealed public dispatch trait，只为内建整数类型以及 `f32`/
`f64` 实现；外部类型不能实现它们。

## 安装与 feature

默认依赖只编译 `ConvertUtils` 空结构体和 `convert`/兼容模块。按需启用独立 feature：

```toml
[dependencies]
axutils = { version = "0.1.0", features = ["itoa", "ryu", "uuid"] }
uuid = { version = "1.24.0", default-features = false, features = ["std"] }
```

feature 与 API 的关系如下：

| feature | 提供的能力 | 直接依赖边界 |
| --- | --- | --- |
| `itoa` | `IntegerBuffer`、`IntegerValue` 和整数双向转换 | 只启用 `itoa`，整数解析仍是标准库 |
| `ryu` | 浮点转换和 `FloatFormat::Ryu` | 只启用 `ryu` |
| `zmij` | 浮点转换和 `FloatFormat::Zmij` | 只启用 `zmij` |
| `uuid` | `UuidBuffer` 和 UUID 双向转换 | 只启用 `uuid` 的 `std` feature |

`ryu` 与 `zmij` 可以同时启用。此时仍只有一套浮点关联函数，通过 `FloatFormat` 在调用点
显式选择后端；不存在无参数默认后端，也没有 `*_ryu`/`*_zmij` 后缀方法。`uuid::Uuid` 和
`uuid::Error` 出现在公开签名中，因此使用 UUID API 的应用必须像上方示例一样直接声明兼容
版本的 `uuid` 依赖；Rust 2018 不会把传递依赖自动作为应用的直接依赖。

## `ConvertUtils`

### `ConvertUtils` 工具类型

该类型没有公共字段或构造方法，直接使用关联函数即可。

```rust
use axutils::ConvertUtils;

let _utils = ConvertUtils;
```

## 整数转换（`itoa`）

### `IntegerBuffer`

`IntegerBuffer` 封装 `itoa::Buffer`，只保存栈内格式化状态，不拥有堆资源。

### `IntegerBuffer::new() -> IntegerBuffer`

创建一个新的整数格式化 buffer。

```rust
use axutils::{ConvertUtils, IntegerBuffer};

let mut buffer = IntegerBuffer::new();
assert_eq!(ConvertUtils::integer_to_str(-42_i64, &mut buffer), "-42");
```

### `IntegerBuffer` 的 `Default`

`Default::default()` 与 `IntegerBuffer::new()` 等价。

```rust
use axutils::{ConvertUtils, IntegerBuffer};

let mut buffer = IntegerBuffer::default();
assert_eq!(ConvertUtils::integer_to_str(0_u8, &mut buffer), "0");
```

### `IntegerValue`

签名为：

```text
pub trait IntegerValue: sealed::IntegerSealed + FromStr<Err = ParseIntError> {
    fn format_into<'a>(value: Self, buffer: &'a mut IntegerBuffer) -> &'a str;
}
```

它只由本 crate 为 `i8`、`i16`、`i32`、`i64`、`i128`、`isize`、`u8`、`u16`、`u32`、`u64`、
`u128` 和 `usize` 实现。`sealed::IntegerSealed` 是私有约束，外部类型不能实现该 trait。

### `IntegerValue::format_into(value, buffer) -> &str`

把一个内建整数写入给定 buffer，并返回借用 buffer 的结果。普通调用方优先使用
`ConvertUtils::integer_to_str`；此 dispatch 方法保留给需要显式 trait 调用的代码。

```rust
use axutils::{IntegerBuffer, IntegerValue};

let mut buffer = IntegerBuffer::new();
assert_eq!(<u128 as IntegerValue>::format_into(123_u128, &mut buffer), "123");
```

### `ConvertUtils::integer_to_str<'a, I>(value, buffer) -> &'a str`

- **feature**：`itoa`。
- **参数**：`value: I` 必须是已实现 `IntegerValue` 的内建整数；`buffer` 是调用方持有的
  可变 `IntegerBuffer`。
- **返回值**：借用 `buffer` 的整数文本；不创建结果堆分配。
- **生命周期**：返回值在 `buffer` 下一次可变使用前有效。

```rust
use axutils::{ConvertUtils, IntegerBuffer};

let mut buffer = IntegerBuffer::new();
let text = ConvertUtils::integer_to_str(i128::MIN, &mut buffer);
assert_eq!(text, "-170141183460469231731687303715884105728");
```

### `ConvertUtils::append_integer<I>(output, value)`

- **feature**：`itoa`。
- **参数**：`output` 是已有的 `String`，`value` 必须实现 `IntegerValue`。
- **返回值**：`()`。
- **分配语义**：使用局部栈 buffer，不创建中间 `String`；`output` 容量不足时允许自身扩容。

```rust
use axutils::ConvertUtils;

let mut output = String::from("count=");
ConvertUtils::append_integer(&mut output, 42_u64);
assert_eq!(output, "count=42");
```

### `ConvertUtils::integer_to_string<I>(value) -> String`

- **feature**：`itoa`。
- **参数**：`value` 必须实现 `IntegerValue`。
- **返回值**：独立拥有的整数文本。
- **分配语义**：承担拥有型 `String` 的分配和复制；返回值不依赖任何 `IntegerBuffer`。

```rust
use axutils::ConvertUtils;

let text = ConvertUtils::integer_to_string(-900_i32);
assert_eq!(text, "-900");
```

### `ConvertUtils::string_to_integer<T>(input) -> Result<T, ParseIntError>`

- **feature**：`itoa`。
- **参数**：`input` 只借用 `&str`，不自动裁剪空白。
- **返回值**：按目标类型 `T` 解析出的整数。
- **错误**：直接返回标准库的 `std::num::ParseIntError`；空输入、非法字符、符号不匹配和
  溢出遵守目标类型 `FromStr` 语义。

```rust
use axutils::ConvertUtils;

let value: i32 = ConvertUtils::string_to_integer("-42").unwrap();
assert_eq!(value, -42);
assert!(ConvertUtils::string_to_integer::<u8>("256").is_err());
assert!(ConvertUtils::string_to_integer::<i32>(" 42").is_err());
```

## 浮点转换（`ryu` 或 `zmij`）

### `FloatFormat`

`FloatFormat` 是 `#[non_exhaustive]` 的 `Copy`/`Eq` 枚举。启用 `ryu` 时有 `Ryu` 变体，启用
`zmij` 时有 `Zmij` 变体；两个 feature 同时启用时两者都存在。外部 `match` 必须保留 `_`
分支。

```rust
#[cfg(any(feature = "ryu", feature = "zmij"))]
{
    use axutils::FloatFormat;

    #[cfg(feature = "ryu")]
    let format = FloatFormat::Ryu;
    #[cfg(all(not(feature = "ryu"), feature = "zmij"))]
    let format = FloatFormat::Zmij;
    let label = match format {
        #[cfg(feature = "ryu")]
        FloatFormat::Ryu => "ryu",
        #[cfg(feature = "zmij")]
        FloatFormat::Zmij => "zmij",
        _ => "future-backend",
    };
    assert_eq!(label, if cfg!(feature = "ryu") { "ryu" } else { "zmij" });
}
```

### `FloatFormat::Ryu`

仅在 `ryu` feature 下存在，选择 `ryu::Buffer::format` 的最短十进制表示。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::FloatFormat;

    let _format = FloatFormat::Ryu;
}
```

### `FloatFormat::Zmij`

仅在 `zmij` feature 下存在，选择 `zmij::Buffer::format` 的最短十进制表示。

```rust
#[cfg(feature = "zmij")]
{
    use axutils::FloatFormat;

    let _format = FloatFormat::Zmij;
}
```

### `FloatBuffer`

`FloatBuffer` 在构造时固定一个 `FloatFormat`，不实现 `Default`，避免 feature 组合改变隐式
后端选择。它封装启用后端的栈内 buffer，不把 `ryu::Buffer` 或 `zmij::Buffer` 暴露到公共签名。

### `FloatBuffer::new(format) -> FloatBuffer`

按显式后端创建 buffer。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::{ConvertUtils, FloatBuffer, FloatFormat};

    let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    assert_eq!(ConvertUtils::float_to_str(1.5_f64, &mut buffer), "1.5");
}
```

### `FloatValue`

签名为：

```text
pub trait FloatValue: sealed::FloatSealed + FromStr<Err = ParseFloatError> {
    fn format_into<'a>(value: Self, buffer: &'a mut FloatBuffer) -> &'a str;
}
```

它只由本 crate 为 `f32` 和 `f64` 实现。sealed 约束不允许外部类型实现该 trait，公共签名也
不会暴露 `ryu::Float` 或 `zmij::Float` trait bound。

### `FloatValue::format_into(value, buffer) -> &str`

使用 `buffer` 构造时固定的后端格式化浮点数，并返回借用结果。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::{FloatBuffer, FloatFormat, FloatValue};

    let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    assert_eq!(<f64 as FloatValue>::format_into(2.5, &mut buffer), "2.5");
}
```

### `ConvertUtils::float_to_str<'a, T>(value, buffer) -> &'a str`

- **feature**：至少一个 `ryu`/`zmij`。
- **参数**：`value` 只能是 `f32` 或 `f64`；`buffer` 是调用方按 `FloatFormat` 创建的可变
  `FloatBuffer`。
- **返回值**：借用 buffer 的最短十进制文本；不创建结果堆分配。
- **特殊值**：使用选定后端 `Buffer::format` 的 `NaN`、无穷和负零语义。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::{ConvertUtils, FloatBuffer, FloatFormat};

    let mut buffer = FloatBuffer::new(FloatFormat::Ryu);
    assert_eq!(ConvertUtils::float_to_str(-0.0_f64, &mut buffer), "-0.0");
}
```

### `ConvertUtils::append_float<T>(output, value, format)`

- **feature**：至少一个 `ryu`/`zmij`。
- **参数**：`value` 是 `f32` 或 `f64`；`format` 显式选择后端；`output` 是已有 `String`。
- **返回值**：`()`。
- **分配语义**：使用局部后端 buffer，不创建中间 `String`；`output` 容量不足时允许自身扩容。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::{ConvertUtils, FloatFormat};

    let mut output = String::from("value=");
    ConvertUtils::append_float(&mut output, 1.25_f64, FloatFormat::Ryu);
    assert_eq!(output, "value=1.25");
}
```

### `ConvertUtils::float_to_string<T>(value, format) -> String`

- **feature**：至少一个 `ryu`/`zmij`。
- **参数**：`value` 是 `f32` 或 `f64`；`format` 显式选择后端。
- **返回值**：独立拥有的浮点文本。
- **分配语义**：承担拥有型 `String` 的分配和复制；不依赖调用方 buffer。

```rust
#[cfg(feature = "ryu")]
{
    use axutils::{ConvertUtils, FloatFormat};

    let text = ConvertUtils::float_to_string(1.5_f64, FloatFormat::Ryu);
    assert_eq!(text, "1.5");
}
```

### `ConvertUtils::string_to_float<T>(input) -> Result<T, ParseFloatError>`

- **feature**：至少一个 `ryu`/`zmij`。
- **参数**：`input` 只借用 `&str`，不自动裁剪空白。
- **返回值**：按目标类型 `T` 解析出的 `f32` 或 `f64`。
- **错误**：直接返回标准库的 `std::num::ParseFloatError`。特殊值、溢出和下溢遵守目标类型
  `FromStr` 语义；例如当前标准库把 `1e9999` 解析为正无穷，而不是返回错误。

```rust
use axutils::ConvertUtils;

let value: f64 = ConvertUtils::string_to_float("-1.25e2").unwrap();
assert_eq!(value, -125.0);
assert!(ConvertUtils::string_to_float::<f64>(" 1.0").is_err());
assert_eq!(ConvertUtils::string_to_float::<f64>("1e9999").unwrap(), f64::INFINITY);
```

有限值只承诺能被各后端的最短十进制文本解析回相同浮点值，不承诺 `ryu` 与 `zmij` 对每个
有限输入产生完全相同的文本。`NaN` 应使用 `is_nan()` 检查，`-0.0` 如需区分符号应使用
`to_bits()`。

## UUID 转换（`uuid`）

### `UuidBuffer`

`UuidBuffer` 封装固定的 `[u8; 36]`，用于标准小写连字符格式，不使用 `unsafe` 或中间格式
字符串。

### `UuidBuffer::new() -> UuidBuffer`

创建 UUID 格式化 buffer。

```rust
use axutils::{ConvertUtils, UuidBuffer};
use uuid::Uuid;

let mut buffer = UuidBuffer::new();
assert_eq!(ConvertUtils::uuid_to_str(&Uuid::nil(), &mut buffer).len(), 36);
```

### `UuidBuffer` 的 `Default`

`Default::default()` 与 `UuidBuffer::new()` 等价。

```rust
use axutils::{ConvertUtils, UuidBuffer};
use uuid::Uuid;

let mut buffer = UuidBuffer::default();
assert_eq!(ConvertUtils::uuid_to_str(&Uuid::nil(), &mut buffer), "00000000-0000-0000-0000-000000000000");
```

### `ConvertUtils::uuid_to_str<'a>(uuid, buffer) -> &'a str`

- **feature**：`uuid`。
- **参数**：借用 `&uuid::Uuid` 和调用方持有的 `UuidBuffer`。
- **返回值**：36 字节小写连字符文本，借用 `buffer`，不创建结果堆分配。

```rust
use axutils::{ConvertUtils, UuidBuffer};
use uuid::Uuid;

let uuid = Uuid::try_parse("550e8400-e29b-41d4-a716-446655440000").unwrap();
let mut buffer = UuidBuffer::new();
assert_eq!(ConvertUtils::uuid_to_str(&uuid, &mut buffer), "550e8400-e29b-41d4-a716-446655440000");
```

### `ConvertUtils::append_uuid(output, uuid)`

- **feature**：`uuid`。
- **参数**：已有 `String` 和借用 `&uuid::Uuid`。
- **返回值**：`()`。
- **分配语义**：使用局部 36 字节 buffer，不创建中间 `String`；目标容量不足时允许自身扩容。

```rust
use axutils::ConvertUtils;
use uuid::Uuid;

let mut output = String::from("id=");
ConvertUtils::append_uuid(&mut output, &Uuid::nil());
assert_eq!(output, "id=00000000-0000-0000-0000-000000000000");
```

### `ConvertUtils::uuid_to_string(uuid) -> String`

- **feature**：`uuid`。
- **参数**：借用 `&uuid::Uuid`。
- **返回值**：独立拥有的标准小写连字符文本。
- **分配语义**：承担拥有型 `String` 的分配和复制。

```rust
use axutils::ConvertUtils;
use uuid::Uuid;

assert_eq!(
    ConvertUtils::uuid_to_string(&Uuid::nil()),
    "00000000-0000-0000-0000-000000000000"
);
```

### `ConvertUtils::string_to_uuid(input) -> Result<uuid::Uuid, uuid::Error>`

- **feature**：`uuid`。
- **参数**：只借用 `&str`，不自动裁剪空白。
- **返回值**：`uuid::Uuid`。
- **错误**：直接返回 `uuid::Error`。当前文档和测试只承诺标准 simple、连字符、URN 和
  Microsoft GUID 形式；空输入、错误长度、错误分隔符、非法十六进制字符和前后空白被拒绝。

```rust
use axutils::ConvertUtils;

let uuid = ConvertUtils::string_to_uuid("550e8400-e29b-41d4-a716-446655440000").unwrap();
assert_eq!(ConvertUtils::uuid_to_string(&uuid), "550e8400-e29b-41d4-a716-446655440000");
assert!(ConvertUtils::string_to_uuid("not-a-uuid").is_err());
```

## 生命周期、分配和输入边界

- `*_to_str` 的返回值借用调用方 buffer；在下一次可变使用同一 buffer 前必须消费或复制该切片。
- `append_*` 不创建临时结果 `String`，但目标 `String` 扩容仍是允许的；如果调用方已经知道
  结果规模，可以先 `reserve` 或使用足够容量的字符串。
- `*_to_string` 明确返回拥有型结果，会承担结果分配和复制，适合跨越 buffer 生命周期保存。
- 所有解析入口只借用 `&str`，不自动 `trim`、不默认回退、不静默吞错，也不重写标准库或
  `uuid` crate 的错误类型。
- 格式化和解析按输入/输出长度线性处理；本 crate 不擅自设置数字或 UUID 输入上限。网络、
  文件或消息边界接收不可信文本时，应由上层先设置请求大小限制。
- 数字格式化使用 `itoa`、`ryu` 或 `zmij` 的安全 buffer API；UUID 使用 `Uuid` 的安全
  `encode_lower` API，不包含全局状态、线程本地 buffer、`unsafe` 或运行时后端配置。

## 使用场景与限制

该能力适合高频、明确类型的基础文本转换，不负责布尔值、字符、日期时间、字节数组、JSON、
Serde 类型、数字格式化模板、千分位、固定小数位、进制转换、UUID 生成/版本校验或本地化。
需要这些语义时，应在业务层选择对应的专用 API，并单独定义输入边界和错误策略。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短概览](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
