# JWT 使用文档

## 能力与 feature

JWT 能力需要显式启用独立的 `jwt` feature：

```toml
[dependencies]
axutils = { version = "0.1", features = ["jwt"] }
serde = { version = "1", features = ["derive"] }
```

该 feature 直接启用 `jsonwebtoken 11.0.0`、`serde` 和 `serde_json`，并为
`jsonwebtoken` 固定 `rust_crypto` 与 `use_pem`。它不会启用 axutils 的 `serde`、配置、邮件、
AES 或其他项目 feature，因此只启用 `jwt` 时不存在 `axutils::config` 和 `ConfigLoader`。

当前支持的算法是 HS256/384/512、RS256/384/512、PS256/384/512、ES256/384，以及 API 名称为
`Ed25519`、底层 Header 名称为 `EdDSA` 的 Ed25519。首期不支持 `none`、ES512/P-521 和 Ed448。
JWT 在本模块中表示 JWS：签名提供完整性和来源认证，不会加密 payload。需要 payload 保密时应
另行使用 JWE 或其他经过审计的加密协议，不能把 JWT 当作加密格式。

## 导出路径

所有下列路径都需要 `jwt` feature：

| 项目 | 公共路径 |
| --- | --- |
| 领域模块 | `axutils::jwt` |
| `JwtAlgorithm` | `axutils::JwtAlgorithm`、`axutils::jwt::JwtAlgorithm` |
| `JwtSigningKey` | `axutils::JwtSigningKey`、`axutils::jwt::JwtSigningKey` |
| `JwtVerificationKey` | `axutils::JwtVerificationKey`、`axutils::jwt::JwtVerificationKey` |
| `JwtConfig` | `axutils::JwtConfig`、`axutils::jwt::JwtConfig` |
| `JwtValidation` | `axutils::JwtValidation`、`axutils::jwt::JwtValidation` |
| `JwtError` | `axutils::JwtError`、`axutils::jwt::JwtError` |
| `JwtUtils` | `axutils::JwtUtils`、`axutils::utils::JwtUtils`、`axutils::utils::jwt_utils::JwtUtils` |

推荐从 crate 根导入领域类型和 `JwtUtils`。`axutils::jwt` 是公开领域模块；本期不提供
`axutils::jwt_utils` 根级模块别名，也不暴露第三方 `EncodingKey`、`DecodingKey`、错误类型或
可轮换的 `JwtClient`。

`serde` 是调用方为自己的 claims 类型添加 `Serialize`/`Deserialize` derive 的直接依赖；axutils
通过 `jwt` feature 启用它供本 crate API 使用，但不会把传递依赖变成应用 crate 的直接依赖。

```rust
use axutils::{
    JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtUtils, JwtValidation,
    JwtVerificationKey,
};
use axutils::jwt::{JwtAlgorithm as DomainAlgorithm, JwtConfig as DomainConfig};

let _: DomainAlgorithm = JwtAlgorithm::Hs256;
let _: fn() -> bool = JwtUtils::is_initialized;
let _: Option<JwtError> = None;
let _: Option<JwtSigningKey> = None;
let _: Option<JwtVerificationKey> = None;
let _: Option<DomainConfig> = None;
let _: Option<JwtValidation> = None;
```

## API 签名速查

以下是本 crate 定义的全部公开 inherent/associated 方法签名；每个方法后续小节都有独立说明和调用示例。

```text
JwtSigningKey::from_hmac_secret(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_rsa_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_rsa_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_ec_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_ec_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_ed_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtSigningKey::from_ed_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>

JwtVerificationKey::from_hmac_secret(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_rsa_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_rsa_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_ec_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_ec_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_ed_pem(input: impl AsRef<[u8]>) -> Result<Self, JwtError>
JwtVerificationKey::from_ed_der(input: impl AsRef<[u8]>) -> Result<Self, JwtError>

JwtValidation::new() -> Self
JwtValidation::with_validate_exp(self, validate: bool) -> Self
JwtValidation::with_require_exp(self, require: bool) -> Self
JwtValidation::with_validate_nbf(self, validate: bool) -> Self
JwtValidation::with_require_nbf(self, require: bool) -> Self
JwtValidation::with_require_aud(self, require: bool) -> Self
JwtValidation::with_require_iss(self, require: bool) -> Self
JwtValidation::with_require_sub(self, require: bool) -> Self
JwtValidation::with_audience(self, value: impl AsRef<str>) -> Result<Self, JwtError>
JwtValidation::with_audiences<I, S>(self, values: I) -> Result<Self, JwtError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>;
JwtValidation::with_issuer(self, value: impl AsRef<str>) -> Result<Self, JwtError>
JwtValidation::with_issuers<I, S>(self, values: I) -> Result<Self, JwtError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>;
JwtValidation::with_subject(self, value: impl AsRef<str>) -> Result<Self, JwtError>
JwtValidation::with_leeway(self, leeway: u64) -> Result<Self, JwtError>

JwtConfig::new(
    algorithm: JwtAlgorithm,
    signing_key: Option<JwtSigningKey>,
    verification_key: Option<JwtVerificationKey>,
    validation: JwtValidation,
) -> Result<Self, JwtError>

JwtUtils::init(config: JwtConfig) -> Result<(), JwtError>
JwtUtils::is_initialized() -> bool
JwtUtils::encode<T: serde::Serialize>(claims: &T) -> Result<String, JwtError>
JwtUtils::decode<T: serde::de::DeserializeOwned>(token: &str) -> Result<T, JwtError>
```

## `JwtAlgorithm`

`JwtAlgorithm` 是 `#[non_exhaustive]` 枚举，不暴露后端的 `jsonwebtoken::Algorithm`。变体如下：

- `Hs256`、`Hs384`、`Hs512`：HMAC。
- `Rs256`、`Rs384`、`Rs512`：RSA PKCS#1 v1.5。
- `Ps256`、`Ps384`、`Ps512`：RSA-PSS。
- `Es256`、`Es384`：ECDSA P-256/P-384。
- `Ed25519`：Ed25519 EdDSA，生成的 Header `alg` 为 `EdDSA`。

```rust
use axutils::JwtAlgorithm;

let algorithms = [
    JwtAlgorithm::Hs256,
    JwtAlgorithm::Hs384,
    JwtAlgorithm::Hs512,
    JwtAlgorithm::Rs256,
    JwtAlgorithm::Rs384,
    JwtAlgorithm::Rs512,
    JwtAlgorithm::Ps256,
    JwtAlgorithm::Ps384,
    JwtAlgorithm::Ps512,
    JwtAlgorithm::Es256,
    JwtAlgorithm::Es384,
    JwtAlgorithm::Ed25519,
];
assert_eq!(algorithms.len(), 12);
```

算法在 `JwtConfig::new` 时固定。decode 不会根据未验证 Header 动态选择算法或 key；Header 的
`alg` 必须与配置完全相同，`typ` 必须是 `JWT`，除 `alg`/`typ` 外的字段和重复字段都会拒绝。

## `JwtSigningKey`

`JwtSigningKey` 是拥有型签名 key，不实现 `Clone`，也不提供读取、序列化或原始内容的 API。
`Display` 和 `Debug` 只显示 key family，不显示 secret、PEM、DER 或私钥。

### `from_hmac_secret`

```rust
use axutils::JwtSigningKey;

let _key = JwtSigningKey::from_hmac_secret([0x11; 32])?;
# Ok::<(), axutils::JwtError>(())
```

输入是原始 secret bytes，不是 Base64 文本。secret 不能为空且最多 4096 字节；配置 HS256、
HS384、HS512 时分别至少需要 32、48、64 字节。

### `from_rsa_pem`

```rust,no_run
use axutils::JwtSigningKey;

let pem = std::fs::read("rsa-private.pem")?;
let _key = JwtSigningKey::from_rsa_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

接受 RSA PKCS#1 `RSA PRIVATE KEY` 或 RSA PKCS#8 `PRIVATE KEY`，最多 128 KiB。公钥、证书、
其他算法族和 RSA modulus 小于 2048 bit 的 key 不可用于配置。

### `from_rsa_der`

```rust
use axutils::JwtSigningKey;

let _key = JwtSigningKey::from_rsa_der([0x01, 0x02, 0x03]);
```

输入语义是 RSA PKCS#1 私钥 DER，最多 128 KiB。该构造器只承诺大小和算法族边界；opaque DER
的 ASN.1 结构无法在构造期证明时，会在 encode 阶段返回 `UnsupportedKeyFormat` 或 `InvalidKey`。

### `from_ec_pem`

```rust,no_run
use axutils::JwtSigningKey;

let pem = std::fs::read("ec-private-pkcs8.pem")?;
let _key = JwtSigningKey::from_ec_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

只接受 ECDSA PKCS#8 `PRIVATE KEY`。SEC1 `EC PRIVATE KEY`、公钥、证书和其他算法族不在承诺范围内。

### `from_ec_der`

```rust
use axutils::JwtSigningKey;

let _key = JwtSigningKey::from_ec_der([0x01, 0x02, 0x03]);
```

输入语义是 ECDSA PKCS#8 私钥 DER，最多 128 KiB；未证明的结构错误延迟到 encode 阶段。

### `from_ed_pem`

```rust,no_run
use axutils::JwtSigningKey;

let pem = std::fs::read("ed25519-private-pkcs8.pem")?;
let _key = JwtSigningKey::from_ed_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

只接受 Ed25519 PKCS#8 `PRIVATE KEY`，不支持 Ed448、公钥、SEC1 私钥或证书。

### `from_ed_der`

```rust
use axutils::JwtSigningKey;

let _key = JwtSigningKey::from_ed_der([0x01, 0x02, 0x03]);
```

输入语义是 Ed25519 PKCS#8 私钥 DER，最多 128 KiB；结构错误延迟到 encode 阶段。

## `JwtVerificationKey`

`JwtVerificationKey` 同样是拥有型且不实现 `Clone`，但验证服务可以只配置公钥而不持有私钥。
`Display` 和 `Debug` 只显示 key family。RSA 私钥 PEM、ECDSA/Ed25519 私钥 PEM 和错误角色都会
在构造期拒绝；Ed25519 raw 公钥会在后端调用前严格限制为 32 bytes，以避免短 key 触发后端 panic。

### `from_hmac_secret`

```rust
use axutils::JwtVerificationKey;

let _key = JwtVerificationKey::from_hmac_secret([0x11; 32])?;
# Ok::<(), axutils::JwtError>(())
```

输入是与签名端相同的原始 HMAC secret；不能为空、最多 4096 字节，并在配置阶段按算法检查最小长度。

### `from_rsa_pem`

```rust,no_run
use axutils::JwtVerificationKey;

let pem = std::fs::read("rsa-public.pem")?;
let _key = JwtVerificationKey::from_rsa_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

接受 RSA PKCS#1 `RSA PUBLIC KEY` 或 SubjectPublicKeyInfo `PUBLIC KEY`，最多 128 KiB。wrapper
会按 PEM label 拒绝 RSA 私钥，即使底层构造函数能够暂时接受它；RSA modulus 至少 2048 bit。

### `from_rsa_der`

```rust
use axutils::JwtVerificationKey;

let _key = JwtVerificationKey::from_rsa_der([0x01, 0x02, 0x03]);
```

输入语义是 RSA PKCS#1 公钥 DER，最多 128 KiB。opaque DER 可暂存，但结构错误或私钥 DER
不能完成验证，decode 时返回稳定的 key-format 错误。

### `from_ec_pem`

```rust,no_run
use axutils::JwtVerificationKey;

let pem = std::fs::read("ec-public.pem")?;
let _key = JwtVerificationKey::from_ec_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

只接受 ECDSA PKCS#8 `PUBLIC KEY`。不接受私钥或 SEC1 私钥 label。

### `from_ec_der`

```rust
use axutils::JwtVerificationKey;

let _key = JwtVerificationKey::from_ec_der([0x04, 0x01, 0x02]);
```

该方法名称与后端一致，但输入语义固定为 SEC1 encoded public-point bytes；SubjectPublicKeyInfo
DER 只有经过独立 probe 证明后才能加入承诺范围。结构错误在 decode 阶段返回 key 错误。

### `from_ed_pem`

```rust,no_run
use axutils::JwtVerificationKey;

let pem = std::fs::read("ed25519-public.pem")?;
let _key = JwtVerificationKey::from_ed_pem(pem)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

只接受 Ed25519 `PUBLIC KEY`，不支持 Ed448、私钥或证书。

### `from_ed_der`

```rust
use axutils::JwtVerificationKey;

let _key = JwtVerificationKey::from_ed_der([0x33; 32])?;
# Ok::<(), axutils::JwtError>(())
```

只接受恰好 32-byte 的 Ed25519 raw 公钥。0、31、33 或其他不超过 128 KiB 的长度在构造期返回
`UnsupportedKeyFormat`；超过 128 KiB 先返回 `InvalidConfig`，不会把不安全长度交给后端 verifier。

## `JwtValidation`

### `new`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new();
```

默认值为：`validate_exp=true`、`require_exp=true`、`validate_nbf=false`、
`require_nbf=false`、`require_aud=false`、`require_iss=false`、`require_sub=false`，allowlist
未配置，`leeway=60`。`Default::default()` 与 `new()` 等价。`exp`/`nbf` 即使关闭时间比较，
存在时仍必须是非负整数秒。

### `with_validate_exp`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_validate_exp(false);
```

只关闭 `exp` 的当前时间比较，不关闭字段类型检查，也不改变 `require_exp`。

### `with_require_exp`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_require_exp(false);
```

只控制 `exp` 是否必须存在；缺失时返回 `MissingRequiredClaim`，类型错误仍返回 `InvalidClaim`。

### `with_validate_nbf`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_validate_nbf(true);
```

开启 `nbf <= now + leeway` 比较；`nbf` 存在时始终要求非负整数秒。

### `with_require_nbf`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_require_nbf(true);
```

只控制 `nbf` 是否必须存在，不自动开启时间比较。

### `with_require_aud`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_require_aud(true);
```

要求 `aud` 存在且为非空字符串或非空字符串数组；配置 allowlist 后才进行成员交集匹配。

### `with_require_iss`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_require_iss(true);
```

要求 `iss` 存在且为非空字符串。`iss` 数组始终是 `InvalidClaim`。

### `with_require_sub`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_require_sub(true);
```

要求 `sub` 存在且为非空字符串；配置 expected subject 后再做精确匹配。

### `with_audience`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_audience("api.example.com")?;
# Ok::<(), axutils::JwtError>(())
```

设置单个 expected audience，等价于单元素 allowlist。空字符串、控制字符、超过 4 KiB 返回
`InvalidConfig`。

### `with_audiences`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_audiences(["api", "worker"])?;
# Ok::<(), axutils::JwtError>(())
```

集合必须非空，最多 32 项；每项非空、最多 4 KiB、不能包含控制字符，且不能重复。token 的
`aud` 可以是字符串或数组，只要与 allowlist 有一个成员交集即可。

### `with_issuer`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_issuer("issuer.example.com")?;
# Ok::<(), axutils::JwtError>(())
```

设置单个 expected issuer；token `iss` 必须是单个字符串并精确匹配。

### `with_issuers`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_issuers(["issuer-a", "issuer-b"])?;
# Ok::<(), axutils::JwtError>(())
```

设置 issuer allowlist，集合限制与 `with_audiences` 相同。token `iss` 不接受数组形式。

### `with_subject`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_subject("user-42")?;
# Ok::<(), axutils::JwtError>(())
```

设置 expected subject，token 中的 `sub` 必须是非空字符串并精确匹配；不做大小写折叠或前缀匹配。

### `with_leeway`

```rust
use axutils::JwtValidation;

let _validation = JwtValidation::new().with_leeway(120)?;
# Ok::<(), axutils::JwtError>(())
```

允许范围为 0 到 86,400 秒。`exp + leeway` 与 `now + leeway` 使用 checked arithmetic，溢出
返回 `InvalidClaim`，不会饱和计算。

## `JwtConfig`

### `new`

```rust
use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtValidation, JwtVerificationKey};

let signing = JwtSigningKey::from_hmac_secret([0x11; 32])?;
let verifying = JwtVerificationKey::from_hmac_secret([0x11; 32])?;
let _config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(signing),
    Some(verifying),
    JwtValidation::new(),
)?;
# Ok::<(), axutils::JwtError>(())
```

`JwtConfig` 是一次初始化的配置对象，不提供 encode/decode 方法，也不实现 `Clone`。至少需要
一类 key；只配置 signing key 可签发，只有 verification key 可验证。构造阶段检查算法/key
family、HMAC 最小强度、RSA modulus 最小位数、key 资源上限和 validation allowlist。失败配置不会
占用全局单例。

## `JwtError`

`JwtError` 是 `#[non_exhaustive]` 的脱敏分类，且 `Error::source()` 永远为 `None`。当前变体为：

- `InvalidConfig { field }`：配置字段、allowlist 或资源参数无效。
- `InvalidKey { kind }`：key 角色、算法族或 key 参数不匹配。
- `UnsupportedKeyFormat { kind }`：当前 backend 不承诺或无法解析的 key 格式。
- `MissingSigningKey`、`MissingVerificationKey`：对应操作缺少 key。
- `NotInitialized`、`AlreadyInitialized`：全局入口状态错误。
- `TokenTooLarge { length, limit }`：token 超过 64 KiB。
- `ClaimsTooLarge { length, limit }`：encode 的 claims JSON 超过 32 KiB。
- `InvalidHeader { field }`：Header segment、三段结构、重复字段、未知字段、`typ` 或 `alg` 无效。
- `InvalidClaim { claim }`：标准 claims 类型、时间或 allowlist 不满足规则。
- `MissingRequiredClaim { claim }`：要求存在但字段完全缺失。
- `InvalidToken { segment }`：payload Base64/JSON、签名或调用方类型反序列化失败。

```rust
use axutils::JwtError;

fn classify(error: JwtError) -> &'static str {
    match error {
        JwtError::NotInitialized => "state",
        JwtError::AlreadyInitialized => "state",
        JwtError::MissingRequiredClaim { .. } => "claims",
        _ => "other",
    }
}

let _ = classify(JwtError::NotInitialized);
```

错误的 `Display`/`Debug` 只包含固定字段名、算法、长度、位置或限制值；不会包含完整 token、
claims、secret、私钥、PEM 内容或 `jsonwebtoken`/serde 原始错误文本。

## `JwtUtils`

### `init`

```rust,no_run
use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtUtils, JwtValidation};

let config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    None,
    JwtValidation::new(),
)?;
JwtUtils::init(config)?;
# Ok::<(), axutils::JwtError>(())
```

全局入口只允许第一个完整配置成功初始化。后续调用统一返回 `AlreadyInitialized`，不会替换 key、
算法或规则；没有 reset、replace 或运行时轮换。示例使用 `no_run`，因为 doctest 进程中的全局
单例不能重置。

### `is_initialized`

```rust
use axutils::JwtUtils;

let _initialized = JwtUtils::is_initialized();
```

它只报告是否有一次初始化成功，不代表外部 provider、时钟同步或 key 的业务可用性。

### `encode`

```rust
use axutils::{JwtError, JwtUtils};

#[derive(serde::Serialize)]
struct Claims {
    exp: u64,
}

let _encode: fn(&Claims) -> Result<String, JwtError> = JwtUtils::encode::<Claims>;
```

调用必须先初始化。`T` 只序列化一次为 JSON bytes，先进行 root object、重复键、深度、成员数、
数组元素数和 32 KiB claims 检查，再把同一个 `serde_json::Value` 交给固定算法签名。不会自动
注入 `iat`、`exp`、`nbf`、`aud` 或 `iss`；最终 token 也不能超过 64 KiB。

### `decode`

```rust
use axutils::{JwtError, JwtUtils};

#[derive(serde::Deserialize)]
struct Claims {
    exp: u64,
}

let _decode: fn(&str) -> Result<Claims, JwtError> = JwtUtils::decode::<Claims>;
```

调用必须先初始化。固定顺序为：token 长度和三段结构检查、受限 Header JSON/重复键/允许字段/
`typ`/固定 `alg` 检查、payload Base64URL 解码、claims preflight、固定 key 签名验证、标准 claims
检查、同一 `Value` 二次资源计数，最后反序列化为 `T`。未验证 payload 不会交给调用方类型或业务代码。

完整的 `no_run` 调用形态如下：

```rust,no_run
use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtUtils, JwtValidation};

#[derive(serde::Deserialize)]
struct Claims {
    exp: u64,
}

let config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    None,
    JwtValidation::new(),
)?;
JwtUtils::init(config)?;
let _claims: Claims = JwtUtils::decode("token-created-by-the-same-fixed-config")?;
# Ok::<(), axutils::JwtError>(())
```

## 生命周期与安全边界

- 全局 `OnceLock` 中的 codec/key 与进程同寿命，不支持热轮换；正常退出前不承诺触发第三方 key
  内部数据的可观察清零。
- 本期不提供实例 codec、JWKS、远程 key 获取、`kid` 路由、多 key 轮换或自动密钥发现。
- 本期不提供撤销、黑名单、重放保护、密钥托管或时钟同步；调用方仍需设计这些业务边界。
- Header 不接受 `kid`、`jku`、`jwk`、`x5u`、`x5c`、`crit`、`zip` 等未支持字段，也不会根据它们
  选择 key 或访问网络。
- 在本 crate 依赖树只有 `rust_crypto` 且外部代码未预先安装 provider 时，`jsonwebtoken` 使用其
  `rust_crypto::DEFAULT_PROVIDER`。如果同一进程的其他依赖通过 Cargo feature unification 同时
  启用 `aws_lc_rs`，且没有预先安装 provider，首次 encode/decode 可能触发底层 provider panic；
  本 crate 只验证自身没有启用 `aws_lc_rs`，不承诺隔离该外部组合。
