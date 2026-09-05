# JWT

启用 `jwt` 后，JWT 的配置、密钥、错误与实例 codec 均位于 `axutils::jwt`。codec 不依赖
进程级状态，适合多租户或需要明确生命周期的服务。

```toml
[dependencies]
axutils = { version = "1.0", features = ["jwt"] }
serde = { version = "1", features = ["derive"] }
```

## 实例 codec

`JwtConfig::new` 会校验算法、签名/验证密钥角色与 claims 规则；`JwtCodec::new` 消费已校验
配置。密钥内容不会经 codec 的 `Debug` 或错误输出暴露。

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
```

`encode` 接受可序列化的 claims，`decode` 仅在固定算法签名和标准 claims 校验成功后才反序列化。
token 与 claims 都有大小、结构和资源预算；token 不是加密内容，不应写入日志、错误或指标标签。

```rust
use axutils::jwt::{JwtCodec, JwtError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    sub: String,
    exp: u64,
}

fn round_trip(codec: &JwtCodec) -> Result<Claims, JwtError> {
    let token = codec.encode(&Claims {
        sub: "user-42".to_owned(),
        exp: 2_000_000_000,
    })?;
    codec.decode(&token)
}
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
```
