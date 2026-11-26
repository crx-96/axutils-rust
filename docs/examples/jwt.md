# JWT

启用 `jwt` 后，JWT 的配置、密钥、错误与实例 codec 均位于 `axutils::jwt`。每个 codec 独立持有
密钥和验证规则。`axutils` 3.0 使用 `jsonwebtoken` v12 的进程级加密 provider。

```toml
[dependencies]
axutils = { version = "3.0", features = ["jwt"] }
serde = { version = "1", features = ["derive"] }
```

`axutils` 的 `jwt` feature 启用 `jsonwebtoken/rust_crypto`。仅启用这个 provider 时，签发和验签会自动使用它。
如果下游也启用了同一版本的 `jsonwebtoken/aws_lc_rs`，宿主应在启动时显式选择一个 provider，例如调用
`jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER.install_default()` 并处理其返回值。每个进程只能成功安装一次。
选择前仍可构造密钥、配置和 codec。此时用合法输入签发或验签会得到 `JwtError::InvalidConfig { field: "crypto_provider" }`。
宿主完成选择后，可以用原 codec 重试。

## 实例 codec

`JwtConfig::new` 会校验算法、签名/验证密钥角色与 claims 规则；`JwtCodec::new` 消费已校验
配置。密钥内容不会经 codec 的 `Debug` 或错误输出暴露。

`JwtVerificationKey::from_ec_der` 接收 SEC1 公钥点字节。token 的 Header 和 claims 通过预检后，
`decode` 遇到无法按配置曲线解析的公钥点会返回 `JwtError::UnsupportedKeyFormat { kind: "verification_key" }`。
使用有效公钥验证不匹配的签名则返回 `JwtError::InvalidToken { segment: "token" }`。RustCrypto 和 AWS-LC
遵循相同分类。

```rust
use axutils::jwt::{
    JwtAlgorithm, JwtCodec, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};

fn codec() -> Result<JwtCodec, JwtError> {
    let signing = JwtSigningKey::from_hmac_secret([0x11; 32])?;
    let verification = JwtVerificationKey::from_hmac_secret([0x11; 32])?;
    let config = JwtConfig::new(
        JwtAlgorithm::Hs256,
        Some(signing),
        Some(verification),
        JwtValidation::new(),
    )?;
    Ok(JwtCodec::new(config))
}

let _codec = codec()?;
# Ok::<(), JwtError>(())
```

`encode` 对调用方的 claims 序列化一次，签发这份 JSON 原文。`decode` 在固定算法签名和标准
claims 校验成功后从原文读取目标类型，因此业务字段可以用 `u128` 精确保留超过 `u64::MAX` 的整数。
token 与 claims 都有大小、结构和资源预算；token 不是加密内容，不应写入日志、错误或指标标签。

```rust
use axutils::jwt::{
    JwtAlgorithm, JwtCodec, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Claims {
    sub: String,
    exp: u64,
}

let config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    Some(JwtVerificationKey::from_hmac_secret([0x11; 32])?),
    JwtValidation::new(),
)?;
let codec = JwtCodec::new(config);
let claims = Claims {
    sub: "user-42".to_owned(),
    exp: 2_000_000_000,
};
let token = codec.encode(&claims)?;
assert_eq!(codec.decode::<Claims>(&token)?, claims);
assert!(codec.decode::<Claims>("invalid-token").is_err());
# Ok::<(), JwtError>(())
```

验证启用 `exp` 或 `nbf` 时使用系统 Unix 时钟；应用应明确选择 audience、issuer、subject 和
leeway 规则，并将无效 token 视为不可信输入。

## 进程级入口

`JwtUtils` 只提供一次初始化、状态和 codec 访问。初始化后不能 reset 或 replace；需要多套 key、
轮换策略或请求级选择时，直接持有 `JwtCodec`。

```rust
use axutils::{
    jwt::{JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation},
    utils::JwtUtils,
};

fn initialize() -> Result<(), JwtError> {
    let key = JwtSigningKey::from_hmac_secret([0x22; 32])?;
    let config = JwtConfig::new(JwtAlgorithm::Hs256, Some(key), None, JwtValidation::new())?;
    JwtUtils::init(config)?;
    let _codec = JwtUtils::codec()?;
    Ok(())
}

initialize()?;
assert!(JwtUtils::is_initialized());
# Ok::<(), JwtError>(())
```
