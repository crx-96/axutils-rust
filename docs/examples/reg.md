# RegUtils 使用文档

> 基础校验需要 `regex` feature；国际手机号码校验还需要同时启用 `libphonenumber` feature。
> 本模块只做格式、号段和号码类型校验，不验证地址或号码真实存在、是否已开通或当前可接通。

## 导出内容

模块仅在 `regex` feature 下公开，路径为：

- `axutils::reg_utils`；
- `axutils::utils::reg_utils`。

`RegUtils` 是无字段、无 `new` 方法的工具结构体，支持以下完整导入路径：

- 推荐：`axutils::RegUtils`；
- `axutils::reg_utils::RegUtils`；
- `axutils::utils::RegUtils`；
- `axutils::utils::reg_utils::RegUtils`。

它实现 `Debug`、`Clone`、`Copy`、`Default`，方法均为静态关联方法。模块没有公共自由函数、
trait、类型别名、常量、静态项或宏。

## 安装与启用

基础邮箱和中国大陆手机号码校验：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex"] }
```

国际号码校验：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

`libphonenumber` 是独立 feature，不会自动启用 `regex`；只启用 `libphonenumber` 时
`RegUtils` 和 `axutils::reg_utils` 都不存在。

## 函数与方法详解

### `RegUtils::is_email(value: &str) -> bool`

- **feature**：`regex`。
- **参数**：待检查的字符串切片。
- **返回值**：是否匹配常见邮箱格式正则
  `^[^\s@.]+(?:\.[^\s@.]+)*@[^\s@.]+(?:\.[^\s@.]+)+$`。
- **示例**：

```rust
use axutils::RegUtils;

assert!(RegUtils::is_email("user@example.com"));
assert!(RegUtils::is_email("first.last+tag@example.co.uk"));
assert!(!RegUtils::is_email("user@example"));
assert!(!RegUtils::is_email("user @example.com"));
```

**注意**：这是较宽松的常见格式检查，不等同于完整 RFC 5322 解析，也不验证域名 DNS、邮箱
所有权或地址可投递性。

### `RegUtils::is_email_strict(value: &str) -> bool`

- **feature**：`regex`。
- **参数**：需要严格检查的 ASCII 邮箱字符串。
- **返回值**：是否同时满足 ASCII dot-atom local-part、DNS label 和总长度规则。
- **示例**：

```rust
use axutils::RegUtils;

assert!(RegUtils::is_email_strict("first.last+tag@example.co.uk"));
assert!(RegUtils::is_email_strict("customer/department=shipping@example.com"));
assert!(!RegUtils::is_email_strict("user name@example.com"));
assert!(!RegUtils::is_email_strict("\"user\"@example.com"));
```

严格规则包括：完整地址最多 254 字节，local-part 最多 64 字节，域名最多 255 字节；local-part
不接受 Unicode、空白、显示名、注释或引号语法；域名至少两个 label，每个 label 非空且最多
63 字节，只允许 ASCII 字母、数字和连字符，首尾不能是连字符；顶级 label 至少两个字符，
必须全为 ASCII 字母，或以 `xn--` 开头且前缀后仍有内容。`xn--` 分支只检查 ASCII 前缀和
label 语法，不验证后续内容是否是可解码的 Punycode。

```rust
use axutils::RegUtils;

let local_at_limit = "a".repeat(64);
let address = format!("{local_at_limit}@example.com");
assert!(RegUtils::is_email_strict(&address));
assert!(!RegUtils::is_email_strict("user@example.123"));
assert!(!RegUtils::is_email_strict("user@example.c"));
```

**注意**：严格校验仍然只是业务格式规则；调用方需要自行做 DNS、邮箱验证和投递策略。

### `RegUtils::is_phone_cn(value: &str) -> bool`

- **feature**：`regex`。
- **参数**：中国大陆手机号码字符串，不带国家码、空格或分隔符。
- **返回值**：是否匹配 `^1[3-9][0-9]{9}$`，也就是 11 位数字且第二位为 `3` 到 `9`。
- **示例**：

```rust
use axutils::RegUtils;

assert!(RegUtils::is_phone_cn("13812345678"));
assert!(RegUtils::is_phone_cn("19900000000"));
assert!(!RegUtils::is_phone_cn("12812345678"));
assert!(!RegUtils::is_phone_cn("+8613812345678"));
```

**注意**：只做号码形状检查，不查询运营商、不验证号码真实存在或可接通。

### `RegUtils::is_phone(value: &str) -> bool`

- **feature**：`regex` + `libphonenumber`。
- **参数**：严格 E.164 字符串，必须以 `+` 开头，后接 1 到 15 位 ASCII 数字；不接受空格、
  短横线、括号、分机号或依赖默认国家/地区的本地号码。
- **返回值**：经过 `libphonenumber` 国家码、号段、有效性和号码类型元数据检查，且类型必须
  是 `Mobile` 时返回 `true`。`FixedLineOrMobile` 不接受。
- **示例**：以下代码块只在两个 feature 都启用时编译；只启用 `regex` 时该方法不可用。

```rust
# #[cfg(all(feature = "regex", feature = "libphonenumber"))]
# fn main() {
use axutils::RegUtils;

assert!(RegUtils::is_phone("+8613812345678"));
assert!(RegUtils::is_phone("+447911123456"));
assert!(!RegUtils::is_phone("13812345678"));
assert!(!RegUtils::is_phone("+86 13812345678"));
# }
# #[cfg(not(all(feature = "regex", feature = "libphonenumber")))]
# fn main() {}
```

**注意**：号码库的元数据可能随依赖版本变化；即使返回 `true`，也不代表号码已开通或当前
可接通。方法不会替调用方发送短信、拨号或查询实时运营商状态。

## 使用场景与限制

适合在业务入口做低成本格式预校验。`regex` 的 `OnceLock` 正则在进程内延迟初始化并复用，
但本模块不提供邮箱验证邮件、DNS 查询、手机短信验证或隐私脱敏策略。对不可信输入仍应
由调用方限制字符串长度、校验频率和日志内容；不要把原始邮箱或电话号码无必要地写入日志。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
