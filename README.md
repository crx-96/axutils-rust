# axutils

`axutils` 是一个单 crate、按能力 feature 组合的 Rust 工具库。默认 feature 为空，默认构建不引入
第三方正常依赖；网络、异步、数据库、配置后端、模板和加密后端均按需启用。

- MSRV：Rust 1.95
- 当前 crate 版本：`1.0.0`
- Edition：2021
- 默认 feature：`[]`

## API 组织

公共 API 保留来源信息：

- Client、配置、错误和领域模型：`axutils::<domain>::Type`
- 工具 façade：`axutils::utils::XxxUtils`
- crate 根不重导出类型
- `utils::*_utils` 叶模块是私有实现路径

```rust
use axutils::{
    config::{ConfigFormat, ConfigLoader, ConfigValue},
    redis::{RedisClient, RedisConfig},
    utils::{ConfigUtils, RedisUtils},
};
# let _ = (
#     ConfigFormat::Json,
#     ConfigLoader::new(),
#     ConfigValue::Null,
#     RedisClient::new,
#     RedisConfig::single("redis://127.0.0.1:6379/0"),
#     ConfigUtils::loader(),
#     RedisUtils::is_initialized(),
# );
```

不提供 `prelude`。这种结构让 `ConfigLoader`、`RedisClient` 或 `JwtCodec` 的领域归属在调用点保持
清晰。

## 安装

只使用默认的标准库能力：

```toml
[dependencies]
axutils = { version = "1.0", default-features = false }
```

按能力组合：

```toml
[dependencies]
axutils = {
    version = "1.0",
    default-features = false,
    features = ["config-yaml", "http-async", "http-json", "redis-async"]
}
```

异步示例使用 `#[tokio::main]` 时，应用仍需直接依赖 Tokio 并启用宏/runtime feature。库的领域
feature 只保证其自身所需的最小 runtime 能力，不替应用选择执行器配置。

## 默认可用能力

无第三方依赖时可使用：

- `PathUtils`：词法路径组合、当前目录与可执行文件路径；
- `FsUtils`：同步文件/目录、受限读取与同步流式传输；
- `TimeUtils`：Unix 时间戳；
- `FormatUtils`：持续时间与字符串脱敏；
- `CryptoUtils`：Hex；
- `TextEncoding::Utf8`；
- `ConvertUtils` 类型本身（具体转换方法由 feature 开放）。

```rust
use axutils::utils::{CryptoUtils, FormatUtils, PathUtils, TimeUtils};

let path = PathUtils::join(["var", "data", "app.json"]);
let masked = FormatUtils::mask_email("user@example.com", None);
let hex = CryptoUtils::hex_encode(b"axutils")?;
let seconds = TimeUtils::try_timestamp_seconds()?;

assert!(path.ends_with("app.json"));
assert!(masked.is_some());
assert_eq!(hex, "61787574696c73");
assert!(seconds > 0);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Feature 一览

### 基础工具

| Feature | 能力 |
| --- | --- |
| `itoa` | 整数高性能格式化 |
| `ryu` / `zmij` | 两种显式选择的浮点格式化后端 |
| `uuid` | UUID 解析与格式化 |
| `rand` | ASCII 字符串和有界数值随机能力 |
| `regex` | 邮箱和中国大陆手机号校验 |
| `phone-validation` | 国际手机号校验；包含 `regex` |
| `template-strfmt` | Strfmt 模板 |
| `template-minijinja` | MiniJinja 模板 |
| `chrono` / `time` / `jiff` | 对应后端的带后缀时间 API |
| `base64` / `md5` / `aes` | 编码、摘要或 AES |
| `encoding_rs` | legacy 文本编码 |
| `jwt` | JWS、公开 `JwtCodec` 与全局生命周期入口 |
| `tracing` | 脱敏结构化事件 |
| `logging` | subscriber 初始化；包含 `tracing` |

### 文件、配置和外部服务

| Feature | 能力 |
| --- | --- |
| `fs-async` | 异步 FS |
| `fs-temp` | 同步临时资源 |
| `fs-temp-async` | 异步临时资源，不开放完整异步 FS |
| `config` | JSON、`.env`、typed/untyped 配置 |
| `config-yaml` / `config-toml` / `config-ini` | 对应格式并包含 `config` |
| `config-async` | 异步配置文件读取并包含 `config` |
| `email` / `email-async` | 同步邮件 / 显式异步邮件 |
| `http` | 同步 `ureq + url`，不包含 `reqwest` |
| `http-async` | 异步 HTTP 并包含 `http` |
| `http-json` | JSON/query API 并包含 `http` |
| `redis` | 单机同步、MessagePack、事务与锁 |
| `redis-cluster` | 同步 Cluster |
| `redis-async` | 异步单机 |
| `redis-cluster-async` | 异步 Cluster |
| `sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite` | SQLx Any + 单 driver |
| `sqlx` | 聚合三个 SQLx driver |

异步 JSON HTTP 组合为 `http-async + http-json`。Redis 四层 feature 互不偷带能力：
`redis-cluster + redis-async` 同时具有同步 Cluster 和异步单机 API，但只有
`redis-cluster-async` 开放异步 Cluster 后端。

### Runtime 与服务

| Feature | 能力 |
| --- | --- |
| `tokio` | Tokio 工具本身 |
| `task-group` | Tokio 任务组 |
| `scheduler` | 完整调度器（单 feature 即可使用） |
| `axum` | 基础 Axum server |
| `axum-tower` | concurrency limit / load shed |
| `axum-tower-http` | CORS、request-id、timeout、body-limit、panic |
| `axum-governor` | 限流 |

单独启用 `tokio` 不会开放 FS、Config、Email、HTTP、Redis 或 SQLx 的异步 API。

完整映射见仓库中的 [模块与 feature 定位](https://github.com/crx-96/axutils-rust/blob/main/docs/module-map.md)。

## 实例优先，全局入口保持薄

HTTP、Redis、Email、JWT、SQLx、Scheduler 和 Axum 都提供实例 API。全局 `*Utils` 只负责一次
初始化、状态查询和取得实例：

```rust,no_run
use axutils::{
    http::{HttpConfig, HttpError, HttpMethod, HttpRequest},
    utils::HttpUtils,
};

HttpUtils::init(HttpConfig::default())?;
let request = HttpRequest::new(HttpMethod::Get, "https://example.com/health")?;
let response = HttpUtils::client()?.execute(request)?;
assert!(HttpUtils::is_initialized());
# Ok::<(), HttpError>(())
```

需要多个账号、多个连接池、测试隔离或可控销毁时，应直接创建领域实例，而不是使用全局入口。全局
初始化成功后不可 reset 或 replace；初始化失败不得占用位置。

JWT 也可完全绕过全局状态：

```rust
use axutils::jwt::{
    JwtAlgorithm, JwtCodec, JwtConfig, JwtError, JwtSigningKey, JwtValidation,
};

let config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    None,
    JwtValidation::new(),
)?;
let codec = JwtCodec::new(config);
let _ = codec;
# Ok::<(), JwtError>(())
```

日志业务事件使用标准 `tracing` 宏；`LogUtils` 只初始化 subscriber 并报告状态。

## 安全与运行时边界

- 错误、`Debug` 和 telemetry 不应回显 URL 凭据、Header、正文、SQL/bind、Redis key/value/token、
  邮件账号、配置原文或密钥。
- HTTPS 与 SMTP TLS 不提供跳过证书/hostname 校验或自动降级；HTTP 仍可显式使用 `http://`，
  Redis 首版只支持明文 `redis://`，SQLx 首版不配置 TLS。后两者必须部署在可信网络或受控隧道中，
  且不得在不受信链路传输敏感值。真实凭据由下游应用安全管理，不得写入仓库、命令或日志。
- 异步领域 API 使用调用方 runtime，不隐式 `block_on`，也不创建隐藏 runtime。
- 文件写入、传输、临时资源和取消语义以对应领域文档为准；库不提供安全根或事务式文件系统。
- Redis 锁不是 Redlock 或 fencing token；SQLx 不代替原生 bind/transaction 语义。
- AES-CBC 不提供完整性认证；MD5 不适用于对抗性安全用途。
- library 不注册全局 allocator。下游 binary 如需 `mimalloc`、`rpmalloc` 或其他 allocator，应自行
  声明唯一的 `#[global_allocator]`。

## 领域文档

- [Convert](docs/examples/convert.md)
- [Crypto](docs/examples/crypto.md)
- [Format](docs/examples/format.md)
- [Path](docs/examples/path.md)
- [Random](docs/examples/random.md)
- [Reg](docs/examples/reg.md)
- [Time](docs/examples/time.md)
- [FS](docs/examples/fs.md)
- [Config](docs/examples/config.md)
- [Email](docs/examples/email.md)
- [HTTP](docs/examples/http.md)
- [JWT](docs/examples/jwt.md)
- [Redis](docs/examples/redis.md)
- [SQLx](docs/examples/sqlx.md)
- [Tokio](docs/examples/tokio.md)
- [Scheduler](docs/examples/scheduler.md)
- [Axum](docs/examples/axum.md)
- [Logging](docs/examples/log.md)

## 开发与验证

常用快速门禁：

```bash
cargo fmt --all -- --check
cargo test --no-default-features
cargo clippy --no-default-features --all-targets -- -D warnings
```

完整非 live 门禁、feature matrix、文档示例 harness 与发布包检查见
[开发与验收](docs/develop.md)。真实 SMTP、Redis 和 Redis Cluster 测试默认 ignored，不能在没有
显式授权和受控服务的情况下运行。

## 从旧公共路径迁移

| 旧形式 | 当前形式 |
| --- | --- |
| `axutils::RedisClient` | `axutils::redis::RedisClient` |
| `axutils::ConfigUtils` | `axutils::utils::ConfigUtils` |
| `axutils::utils::redis_utils::RedisUtils` | `axutils::utils::RedisUtils` |
| `RedisUtils::get(...)` | `RedisUtils::client()?.get(...)` |
| `SqlxUtils::init(...)` | `SqlxUtils::init_async(...)` |
| 多个 provider feature 手工拼装 Scheduler | `scheduler` |
| `http + tokio` | `http-async` |
| `redis + tokio` | `redis-async` |

旧根路径、公开叶模块、provider-only feature、状态 façade 业务转发、单后端时间无后缀 API 和
library allocator feature 已删除。

## License

MIT，见 [LICENSE](LICENSE)。
