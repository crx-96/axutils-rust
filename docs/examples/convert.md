# 转换

`ConvertUtils` 是无状态的字符串转换入口，始终从
`axutils::utils::ConvertUtils` 导入。格式化 buffer、格式枚举和 sealed trait 属于
`axutils::convert` 领域模块；实现叶模块不是公共 API。

## 启用

按所需数据类型启用独立能力：

```toml
[dependencies]
axutils = { version = "1.0", features = ["itoa", "ryu", "uuid"] }
uuid = { version = "1", default-features = false, features = ["std"] }
```

| feature | 公共能力 |
| --- | --- |
| `itoa` | 整数格式化与解析；`IntegerBuffer`、`IntegerValue` |
| `ryu` | `FloatFormat::Ryu` 浮点格式化 |
| `zmij` | `FloatFormat::Zmij` 浮点格式化 |
| `uuid` | UUID 字符串转换与 `UuidBuffer` |

`ryu` 与 `zmij` 可同时启用，调用点必须用 `FloatFormat` 明确选择后端。UUID 类型出现在
签名中，应用需要直接声明兼容的 `uuid` 依赖，不能依赖传递依赖。

## 整数

启用 `itoa` 后，借用型格式化会复用调用方 buffer；追加型写入已有 `String`；拥有型方法分配
新字符串。解析不 trim 输入，也不会包装标准库的 `ParseIntError`。

```rust
use axutils::{convert::IntegerBuffer, utils::ConvertUtils};

let mut buffer = IntegerBuffer::new();
assert_eq!(ConvertUtils::integer_to_str(-42_i64, &mut buffer), "-42");

let mut output = String::from("id=");
ConvertUtils::append_integer(&mut output, 7_u32);
assert_eq!(output, "id=7");
assert_eq!(ConvertUtils::string_to_integer::<u16>("42"), Ok(42));
assert!(ConvertUtils::string_to_integer::<u8>(" 42").is_err());
```

`IntegerValue` 只为 Rust 内建整数实现，外部类型不能实现它；通常无需直接调用该 trait。

## 浮点

启用 `ryu` 或 `zmij` 后，`FloatBuffer` 与 `FloatFormat` 让调用者显式控制格式化后端。解析仍使用
标准库 `FromStr` 语义：不接受首尾空白，极大指数可能解析为无穷大或零。

```rust
use axutils::{
    convert::{FloatBuffer, FloatFormat},
    utils::ConvertUtils,
};

let format = FloatFormat::Ryu;
let mut buffer = FloatBuffer::new(format);
assert_eq!(ConvertUtils::float_to_str(2.5_f64, &mut buffer), "2.5");
assert_eq!(ConvertUtils::float_to_string(-0.0_f64, format), "-0.0");
assert_eq!(ConvertUtils::string_to_float::<f64>("1.25"), Ok(1.25));
```

把上例中的 `FloatFormat::Ryu` 换为 `FloatFormat::Zmij` 即可使用 `zmij` 后端。浮点格式化不提供
隐式默认后端。

## UUID

启用 `uuid` 后，支持 canonical、无连字符、URN 和花括号等由 `uuid` crate 接受的输入形式；格式化
始终产出小写 canonical 字符串，解析错误直接返回 `uuid::Error`。

```rust
use axutils::{convert::UuidBuffer, utils::ConvertUtils};

let value = ConvertUtils::string_to_uuid("550e8400-e29b-41d4-a716-446655440000").unwrap();
let mut buffer = UuidBuffer::new();
assert_eq!(ConvertUtils::uuid_to_str(&value, &mut buffer), "550e8400-e29b-41d4-a716-446655440000");
assert!(ConvertUtils::string_to_uuid("not-a-uuid").is_err());
```

## 错误与边界

- 这些方法只负责整数、`f32`/`f64` 与 UUID；不处理本地化、日期、JSON、布尔值或 UUID 生成。
- `*_to_str` 返回的切片借用传入 buffer；下一次使用同一 buffer 会覆盖其内容。
- `append_*` 避免额外结果字符串，但目标 `String` 仍可能因扩容分配。
- 对不可信的大输入，调用方应限制输入长度与输出累计量。
