# 随机值

启用 `rand` 后，从 `axutils::utils` 导入 `RandomUtils`、`LetterCase` 和 `RandomRangeError`。这些
接口适合测试数据、展示标识和普通抽样，**不是密码学安全随机源**；不得生成密码、认证 token、密钥
或安全 nonce。

## 启用与导入

```toml
[dependencies]
axutils = { version = "1.0", features = ["rand"] }
```

```rust
use axutils::utils::{LetterCase, RandomUtils};

let numeric = RandomUtils::numeric_string(8).unwrap();
assert_eq!(numeric.len(), 8);
assert!(numeric.bytes().all(|byte| byte.is_ascii_digit()));

let letters = RandomUtils::alphabetic_string(12, LetterCase::Mixed).unwrap();
assert_eq!(letters.len(), 12);

let value = RandomUtils::integer(-10..=10).unwrap();
assert!((-10..=10).contains(&value));
```

## 范围与错误

整数和浮点范围均为闭区间，端点相同会返回该唯一值。反向范围或无法构造均匀分布时返回
`RandomRangeError::InvalidRange`；浮点端点为 `NaN` 或无穷大时返回
`RandomRangeError::NonFiniteFloat`。

```rust
use axutils::utils::{RandomRangeError, RandomUtils};

assert_eq!(RandomUtils::integer(7..=7), Ok(7));
assert_eq!(RandomUtils::float(2.5..=2.5), Ok(2.5));
assert_eq!(RandomUtils::integer(2..=1), Err(RandomRangeError::InvalidRange));
assert_eq!(
    RandomUtils::float(f64::NAN..=1.0),
    Err(RandomRangeError::NonFiniteFloat),
);
```

字符串 API 可能为所请求长度分配内存，并在无法预留容量时返回 `TryReserveError`。应用应在进入这些
API 前限制不可信长度；本模块不提供可复现 seed 或随机序列控制。
