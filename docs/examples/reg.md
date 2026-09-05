# 格式校验

启用 `regex` 后，从 `axutils::utils::RegUtils` 导入本地邮箱和手机号格式校验。它不联网、不查询账号
状态，也不证明邮箱或号码真实存在。

## 启用

```toml
[dependencies]
axutils = { version = "1.0", features = ["regex", "phone-validation"] }
```

`phone-validation` 包含 `regex`，并增加国际 E.164 mobile provider 校验；不需要也不应单独启用
其底层 provider。

## 邮箱与中国大陆手机号

`is_email` 是常见格式检查。`is_email_strict` 仅接受 ASCII dot-atom local-part 与 DNS 形式域名，
不接受显示名、注释、引号 local-part、Unicode local-part 或空白；Unicode 域名须先转为 ASCII
punycode。

```rust
use axutils::utils::RegUtils;

assert!(RegUtils::is_email("first.last+tag@example.co.uk"));
assert!(RegUtils::is_email_strict("user@example.com"));
assert!(!RegUtils::is_email_strict("用户@example.com"));

assert!(RegUtils::is_phone_cn("13812345678"));
assert!(!RegUtils::is_phone_cn("138 1234 5678"));
```

## 国际手机号

启用 `phone-validation` 后，`is_phone` 只接受以 `+` 开头、1 到 15 位 ASCII 数字组成的 E.164
输入；它使用号码元数据并且只接受 `Mobile` 类型。空格、短横线、分机、本地号码和
`FixedLineOrMobile` 均不接受。

```rust
use axutils::utils::RegUtils;

assert!(RegUtils::is_phone("+8613812345678"));
assert!(!RegUtils::is_phone("13812345678"));
assert!(!RegUtils::is_phone("+86 13812345678"));
```

格式校验不是身份验证、地址验证或号码可达性验证。将结果用于安全决策前，仍需完成业务级验证与
相应的所有权证明。
