# axutils 模块与 feature 定位

本文档是当前源码结构、公共路径和能力 feature 的定位清单。目录、依赖和拆分约定集中在
[架构基线](architecture.md)；方法、错误和安全语义见 Rustdoc 与对应领域文档。

## 架构与公共路径

领域类型从 `axutils::<domain>` 导入，工具入口从 `axutils::utils` 导入。类型归属与重导出
边界见 [固定边界](architecture.md#固定边界)，调用关系见 [入口与依赖方向](architecture.md#入口与依赖方向)。

推荐导入：

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

## 默认能力

以下入口不需要第三方依赖：

| 领域 | 公共入口 | 职责 |
| --- | --- | --- |
| `fs` | `axutils::fs::*`、`axutils::utils::FsUtils` | 同步文件与目录操作、受限读取、同步流式传输 |
| `time` | `axutils::time::*`、`axutils::utils::TimeUtils` | Unix 时间戳、格式模板和固定偏移支持类型 |
| `crypto` | `axutils::crypto::{CryptoError, TextEncoding}`、`axutils::utils::CryptoUtils` | Hex 与 UTF-8 文本编解码 |
| `convert` | `axutils::convert`、`axutils::utils::ConvertUtils` | feature 控制的数值/UUID 转换 façade |
| `utils` | `FormatUtils`、`PathUtils` | 持续时间/脱敏格式化、HTML 转义、字面标记替换与词法路径操作 |
| `tree` | `build_forest`、`TreeNode`、`TreeBuildError` | 泛型 ID/排序森林构建、全节点校验与可失败后序转换 |
| `concurrency` | `KeyedAdmission`、`KeyedPermit`、`AdmissionError` | 进程内按键互斥准入与总容量约束 |

对应文档：

- [文件系统](examples/fs.md)
- [时间](examples/time.md)
- [加密与编码](examples/crypto.md)
- [转换](examples/convert.md)
- [格式化](examples/format.md)
- [路径](examples/path.md)
- [树与森林](examples/tree.md)
- [按键并发准入](examples/concurrency.md)

## 能力 feature

### 基础与纯工具

| Feature | 开放能力 | 主要依赖 |
| --- | --- | --- |
| `itoa` | 整数格式化与 `IntegerBuffer` | `itoa` |
| `ryu` | `FloatFormat::Ryu` | `ryu` |
| `zmij` | `FloatFormat::Zmij` | `zmij` |
| `uuid` | UUID 解析、格式化与 `UuidBuffer` | `uuid` |
| `rand` | `RandomUtils`、`LetterCase`、`RandomRangeError` | `rand` |
| `secure-random` | `CryptoUtils::secure_random_bytes/digits/hex` | `getrandom` |
| `regex` | 邮箱和中国大陆手机号校验 | `regex` |
| `phone-validation` | 国际手机号校验，同时包含 `regex` | `phonenumber` |
| `template-strfmt` | Strfmt 模板 | `serde`、`serde_json`、`strfmt` |
| `template-minijinja` | MiniJinja 模板 | `serde`、`minijinja` |
| `chrono` / `time` / `jiff` | 带明确后端后缀的时间格式化 API，名称不随后端组合改变 | 同名后端 |
| `base64` / `md5` / `aes` | 对应编码、摘要或 AES 实例/全局 cipher | 对应加密后端 |
| `encoding_rs` | `TextEncoding` 的 legacy 编码变体 | `encoding_rs` |
| `jwt` | JWS 配置、Key、公开 `JwtCodec` 与全局生命周期入口 | `jsonwebtoken`、Serde |
| `tracing` | 私有 telemetry 发出的脱敏结构化事件 | `tracing` |
| `logging` | 日志 subscriber 生命周期入口，同时包含 `tracing` | `tracing-subscriber`、`tracing-appender` |

详见 [随机数](examples/random.md)、[正则校验](examples/reg.md)、[JWT](examples/jwt.md) 和
[日志](examples/log.md)。

### 文件系统与配置

| Feature | 契约 |
| --- | --- |
| `fs-async` | 异步文件读写与流式传输 |
| `fs-temp` | 同步临时文件/目录 |
| `fs-temp-async` | 异步临时文件/目录；不开放完整异步 FS API |
| `config` | JSON、`.env`、typed/untyped 配置 |
| `config-yaml` / `config-toml` / `config-ini` | 包含 `config` 并增加单一格式后端 |
| `config-async` | 包含 `config` 并增加异步文件读取 |

单独启用 `tokio` 不会开放 FS 或 Config 的异步 API。详见 [配置](examples/config.md)。

### 外部服务

| Feature | 契约 |
| --- | --- |
| `email` | 同步 SMTP client |
| `email-async` | 包含 `email` 并增加异步发送 |
| `http` | 同步 `ureq + url`；依赖树不包含 `reqwest` |
| `http-async` | 包含 `http`，增加 `reqwest` 与异步 transport |
| `http-json` | 包含 `http`，增加同步 JSON/query API；异步 JSON 需再启用 `http-async` |
| `redis` | 单机同步、r2d2、MessagePack、事务与租约锁 |
| `redis-cluster` | 包含 `redis`，增加同步 Cluster |
| `redis-async` | 包含 `redis`，增加异步单机连接管理 |
| `redis-cluster-async` | 包含 `redis-cluster + redis-async`，增加异步 Cluster |
| `sqlx-postgres` / `sqlx-mysql` / `sqlx-sqlite` | SQLx Any、Tokio runtime 与一个 driver |
| `sqlx` | 聚合三个 SQLx driver |

任一 SQLx driver 提供 `SqlxError::is_infrastructure_unavailable`；仅 `sqlx-postgres` 开放
`is_postgres_transaction_conflict`，分类本身不执行重试，也不代表业务可安全重试。

详见 [邮件](examples/email.md)、[HTTP](examples/http.md)、[Redis](examples/redis.md) 和
[SQLx](examples/sqlx.md)。

### Runtime 与服务

| Feature | 契约 |
| --- | --- |
| `tokio` | 只开放 Tokio runtime、任务、channel、timeout 与 shutdown 工具，含 Drop 请求 abort 的 `TokioTaskGuard` |
| `task-group` | 包含 `tokio`，增加基于 `tokio-util` 的任务组 |
| `scheduler` | 一次启用 Tokio、Chrono、IANA 时区和 Croner 的完整调度能力 |
| `axum` | 基础 Axum HTTP/1 server 与最小 runtime |
| `axum-tower` | limit/load-shed 等 Tower 能力 |
| `axum-tower-http` | CORS、request-id、timeout、body-limit、panic 等能力 |
| `axum-governor` | Governor 限流能力 |

详见 [Tokio](examples/tokio.md)、[调度器](examples/scheduler.md) 和 [Axum](examples/axum.md)。

## 公开领域模块

| 模块 | 主要领域类型 | `utils` 入口 | 可用条件 |
| --- | --- | --- | --- |
| `convert` | `IntegerBuffer`、`FloatBuffer`、`FloatFormat`、`UuidBuffer` | `ConvertUtils` | 模块默认；方法按转换 feature |
| `crypto` | `CryptoError`、`TextEncoding`、`Base64Options`、`AesKey`、`AesMode`、`AesCipher` | `CryptoUtils` | 基线 + 对应后端 feature |
| `fs` | `FsError`、传输类型、临时资源类型 | `FsUtils` | 同步基线；异步/临时按 feature |
| `tree` | `TreeNode`、`TreeBuildError` 与 `build_forest` | 无 | 默认 |
| `concurrency` | `KeyedAdmission`、`KeyedPermit`、`AdmissionError` | 无 | 默认 |
| `time` | `TimeError`、`TimeZoneOffset`、模板支持类型 | `TimeUtils` | 时间戳基线；后端按 feature |
| `config` | `ConfigLoader`、`ConfigFormat`、`ConfigValue`、`ConfigError` | `ConfigUtils` | `config` |
| `email` | `EmailClient`、配置、消息、错误 | `EmailUtils` | `email` |
| `http` | `HttpClient`、请求/响应、配置、策略、错误 | `HttpUtils` | `http` |
| `jwt` | `JwtCodec`、Key、配置、验证、错误 | `JwtUtils` | `jwt` |
| `redis` | `RedisClient`、配置、事务、锁、错误 | `RedisUtils` | `redis` |
| `sqlx` | `SqlxClient`、配置、row/result/transaction 别名、错误 | `SqlxUtils` | 任一 SQLx driver |
| `tokio` | `TokioConfig`、`TokioTaskGuard`、shutdown 类型；`TokioTaskGroup` 需 `task-group` | `TokioUtils` | `tokio`；任务组按 `task-group` |
| `scheduler` | `Scheduler`、配置、Schedule、TaskId、错误 | `SchedulerUtils` | `scheduler` |
| `axum` | `AxumApp`、Server/Builder、配置与关闭类型；中间件类型按扩展 feature | `AxumUtils` | 基础 `axum`；扩展按 `axum-*` |
| `logging` | `LogConfig`、level、file/rotation、错误 | `LogUtils` | `logging` |

## 状态型 façade

| Façade | 保留入口 | 实例业务入口 |
| --- | --- | --- |
| `EmailUtils` | `init`、`is_initialized`、`client` | `EmailClient` |
| `HttpUtils` | `init`、`is_initialized`、`client` | `HttpClient` |
| `JwtUtils` | `init`、`is_initialized`、`codec` | `JwtCodec` |
| `RedisUtils` | `init`、`init_async`、`is_initialized`、`client` | `RedisClient` |
| `SqlxUtils` | `init_async`、`is_initialized`、`client` | `SqlxClient` |
| `SchedulerUtils` | `init`、`is_initialized`、`scheduler` | `Scheduler` |
| `AxumUtils` | `init`、`is_initialized`、`server` | `AxumServer` |
| `CryptoUtils`（AES） | `aes_init`、`aes_init_from_bytes`、`aes_is_initialized`、`cipher` | `AesCipher` |
| `LogUtils` | `init`、`is_initialized` | 标准 `tracing` 宏 |

上述入口的初始化、关闭和多实例原则见 [状态与能力隔离](architecture.md#状态与能力隔离)。

`ConfigUtils`、`FsUtils`、`ConvertUtils`、`FormatUtils`、`PathUtils`、`RandomUtils`、
`RegUtils`、`TimeUtils` 是无状态工具，不受上述生命周期收缩限制。

## 私有实现定位

- Client、config、transport、codec、policy、validation 等实现位于 `src/<domain>/`。
- Email/HTTP/JWT/Redis/SQLx/Scheduler/Axum 的状态入口位于各领域 `global.rs`；
  Crypto/Logging 使用 `facade.rs`，其他无状态领域工具也在各自 `facade.rs`。
- `src/utils/*_utils.rs` 聚合对应领域工具；Format/Path/Random/Reg 在 `utils` 内实现。
- 事件适配位于 `src/telemetry/<domain>.rs`；与 Logging 的职责边界见架构文档。

跨模块调用可导入有业务含义的模块限定符，例如：

```rust,ignore
use crate::telemetry::sqlx as sqlx_trace;
use crate::fs::transfer;

sqlx_trace::record_client_init(&result, started);
transfer::copy_file_with(source, destination, options, processor);
```

路径风格及现有 lint 的适用方式见
[项目 Skill 的路径与命名](skills/review-rust-library-change/references/api-and-features.md#路径与命名)。

## 新增或调整能力

本清单随模块职责、公共路径、feature 或文档映射的变化更新。领域归属与职责拆分遵循
[架构基线](architecture.md)，库级设计和验收按 [项目 Skill](skills/review-rust-library-change/SKILL.md)
选用相关主题。
