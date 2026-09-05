# axutils 模块与 feature 定位

本文档是当前源码结构、公共路径和能力 feature 的定位清单。它描述“能力归属在哪里”，不复制每个
方法的完整签名；方法、错误和安全语义以 Rustdoc 与对应领域文档为准。

## 架构约束

`axutils` 保持单 crate，依赖方向固定为：

```text
axutils::utils façade -> 领域公开 API -> 领域私有实现 -> 第三方 crate
                                      -> 私有 telemetry
```

- `src/lib.rs` 只声明公开领域模块，不平铺重导出类型。
- Client、配置、错误和模型的规范路径是 `axutils::<domain>::Type`。
- 所有 `*Utils` 及其支持类型的规范路径是 `axutils::utils::Type`。
- `utils` 叶模块和领域实现模块不是公共 API；领域代码不得反向依赖 `utils`。
- 状态型 façade 只管理初始化、状态和实例访问，业务方法由返回的实例承担。
- 默认 feature 为空；无第三方依赖的基础能力默认可用。

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

不要创建 `prelude`，也不要重新引入 crate 根类型别名或公开 `utils::*_utils` 叶模块。

## 默认能力

以下入口不需要第三方依赖：

| 领域 | 公共入口 | 职责 |
| --- | --- | --- |
| `fs` | `axutils::fs::*`、`axutils::utils::FsUtils` | 同步文件与目录操作、受限读取、同步流式传输 |
| `time` | `axutils::time::*`、`axutils::utils::TimeUtils` | Unix 时间戳、格式模板和固定偏移支持类型 |
| `crypto` | `axutils::crypto::{CryptoError, TextEncoding}`、`axutils::utils::CryptoUtils` | Hex 与 UTF-8 文本编解码 |
| `convert` | `axutils::convert`、`axutils::utils::ConvertUtils` | feature 控制的数值/UUID 转换 façade |
| `utils` | `FormatUtils`、`PathUtils` | 持续时间/脱敏格式化与词法路径操作 |

对应文档：

- [文件系统](examples/fs.md)
- [时间](examples/time.md)
- [加密与编码](examples/crypto.md)
- [转换](examples/convert.md)
- [格式化](examples/format.md)
- [路径](examples/path.md)

## 能力 feature

### 基础与纯工具

| Feature | 开放能力 | 主要依赖 |
| --- | --- | --- |
| `itoa` | 整数格式化与 `IntegerBuffer` | `itoa` |
| `ryu` | `FloatFormat::Ryu` | `ryu` |
| `zmij` | `FloatFormat::Zmij` | `zmij` |
| `uuid` | UUID 解析、格式化与 `UuidBuffer` | `uuid` |
| `rand` | `RandomUtils`、`LetterCase`、`RandomRangeError` | `rand` |
| `regex` | 邮箱和中国大陆手机号校验 | `regex` |
| `phone-validation` | 国际手机号校验，同时包含 `regex` | `phonenumber` |
| `template-strfmt` | Strfmt 模板 | `serde`、`serde_json`、`strfmt` |
| `template-minijinja` | MiniJinja 模板 | `serde`、`minijinja` |
| `chrono` / `time` / `jiff` | 对应后端且名称稳定的时间格式化 API | 同名后端 |
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

详见 [邮件](examples/email.md)、[HTTP](examples/http.md)、[Redis](examples/redis.md) 和
[SQLx](examples/sqlx.md)。

### Runtime 与服务

| Feature | 契约 |
| --- | --- |
| `tokio` | 只开放 Tokio runtime、任务、channel、timeout 与 shutdown 工具 |
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
| `time` | `TimeError`、`TimeZoneOffset`、模板支持类型 | `TimeUtils` | 时间戳基线；后端按 feature |
| `config` | `ConfigLoader`、`ConfigFormat`、`ConfigValue`、`ConfigError` | `ConfigUtils` | `config` |
| `email` | `EmailClient`、配置、消息、错误 | `EmailUtils` | `email` |
| `http` | `HttpClient`、请求/响应、配置、策略、错误 | `HttpUtils` | `http` |
| `jwt` | `JwtCodec`、Key、配置、验证、错误 | `JwtUtils` | `jwt` |
| `redis` | `RedisClient`、配置、事务、锁、错误 | `RedisUtils` | `redis` |
| `sqlx` | `SqlxClient`、配置、row/result/transaction 别名、错误 | `SqlxUtils` | 任一 SQLx driver |
| `tokio` | `TokioConfig`、shutdown 类型；`TokioTaskGroup` 需 `task-group` | `TokioUtils` | `tokio`；任务组按 `task-group` |
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

这些全局对象成功初始化后不可 reset 或 replace。初始化失败不得占位；取得实例后，其关闭或失败语义
由领域实例决定。需要多配置、测试隔离或可控销毁时，应直接创建实例。

`ConfigUtils`、`FsUtils`、`ConvertUtils`、`FormatUtils`、`PathUtils`、`RandomUtils`、
`RegUtils`、`TimeUtils` 是无状态工具，不受上述生命周期收缩限制。

## 私有实现定位

- `src/<domain>/global.rs`：领域状态 façade 的私有实现。
- `src/<domain>/**`：client、config、transport、codec、policy、validation 等领域实现。
- `src/telemetry/**`：只在 `tracing` 下编译的私有事件适配；不形成 `axutils::tracing` 模块。
- `src/utils/*_utils.rs`：私有聚合叶；只由 `src/utils/mod.rs` 重导出。

跨模块调用应先导入有业务含义的模块限定符，例如：

```rust,ignore
use crate::telemetry::sqlx as sqlx_trace;
use crate::fs::transfer;

sqlx_trace::record_client_init(&result, started);
transfer::copy_file_with(source, destination, options, processor);
```

普通表达式与签名路径最多保留两个 segment；`execute`、`parse` 等通用函数不得裸导入。

## 新增或调整能力

新增能力时同时确认：

1. 领域归属和单向依赖是否明确；
2. canonical path 是否保留来源，且没有新增根级或公开叶模块别名；
3. 用户 feature 单独启用后是否存在可用 API，并只编译必要依赖；
4. sync/async、provider 和附加能力是否按语义 feature 分层；
5. 错误、资源上限、敏感数据和全局生命周期是否有正向及负向测试；
6. Rustdoc、本清单、对应领域文档和 CHANGELOG 是否同步。

实现模块可以继续拆分，但不要按行数机械切割；普通生产文件超过约 600 行时评估职责，超过 800 行
必须有清晰且不可再拆的理由。
