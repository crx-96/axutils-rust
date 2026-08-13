# 日志与 tracing

`axutils` 使用两个分层 feature：

- `tracing` 只启用库内各领域的结构化事件；
- `logging` 依赖 `tracing`，并额外启用 `LogUtils`、同步 `fmt` formatter、可配置的
  `EnvFilter` target directive 与 `tracing-appender` 文件轮转 writer。

默认 feature 为空。库不会因为加载、构造客户端或调用任意业务 API 而自动安装全局
subscriber；只想接收库内事件时，应用仍需启用 `tracing`，然后自行安装 subscriber。

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["logging"] }
```

## 公共路径

推荐从 crate 根导入：

```rust,no_run
use axutils::{LogConfig, LogError, LogFileConfig, LogLevel, LogRotation, LogUtils};
```

兼容路径为 `axutils::utils::{LogUtils, LogConfig, LogLevel, LogFileConfig, LogRotation,
LogError}` 和 `axutils::utils::log_utils::*`。不存在 `axutils::log_utils` 根模块别名。

跨领域事件的非公开辅助实现位于 `src/tracing/`；应用不需要、也不能导入该内部模块。

## `LogUtils::init`

`LogUtils` 是无状态类型，提供一次性初始化和状态查询。初始化成功后，
全局 subscriber、writer 和文件句柄持续到进程结束，不能 reset、replace、关闭或重新配置。
调用方必须明确选择一次初始化时机，并在整个进程内协调其他库是否已经安装了 global subscriber。

```text
pub fn init(config: LogConfig) -> Result<(), LogError>
```

```rust,no_run
use axutils::{LogConfig, LogError, LogUtils};

fn main() -> Result<(), LogError> {
    LogUtils::init(LogConfig::default())?;
    assert!(LogUtils::is_initialized());
    Ok(())
}
```

如果进程中已经安装其他全局 subscriber，`init` 返回
`LogError::GlobalSubscriberAlreadySet`，且 `is_initialized()` 仍为 `false`。如果本 crate
已经成功初始化，重复调用返回 `LogError::AlreadyInitialized`。应用不能通过再次调用来
替换第一个配置。

过滤器由 `LogConfig::with_level` 与 `LogConfig::with_directives` 共同构造。例如应用可以根据自身
配置动态传入 `lettre=off,rustls=off` 关闭这两个 target，也可以拼接更多 target 规则；这些规则
不是库内固定默认值。`init` 不会自动读取 `RUST_LOG`，也不提供运行时重载或公共 getter；需要安装
第三方 layer、字段过滤器或完全自定义 subscriber 的应用应只启用 `tracing` feature 并自行安装
subscriber。

## `LogUtils::is_initialized`

```text
pub fn is_initialized() -> bool
```

该方法只报告本 crate 是否成功安装了 subscriber；外部 subscriber 不会被误报为本 crate
已经初始化。它不执行 I/O，也不尝试探测或替换进程中的其他 subscriber。

```rust,no_run
use axutils::LogUtils;

let initialized_by_axutils = LogUtils::is_initialized();
let _ = initialized_by_axutils;
```

## `LogUtils::trace`

```text
pub fn trace(message: impl Display)
```

以固定 target `axutils::log` 发出 `TRACE` 消息；没有 subscriber 或被过滤时静默丢弃。

```rust,no_run
use axutils::LogUtils;

LogUtils::trace("进入详细诊断路径");
```

## `LogUtils::debug`

```text
pub fn debug(message: impl Display)
```

以固定 target `axutils::log` 发出 `DEBUG` 消息。

```rust,no_run
use axutils::LogUtils;

LogUtils::debug("缓存检查完成");
```

## `LogUtils::info`

```text
pub fn info(message: impl Display)
```

以固定 target `axutils::log` 发出 `INFO` 消息。

```rust,no_run
use axutils::LogUtils;

LogUtils::info("服务已经启动");
```

## `LogUtils::warn`

```text
pub fn warn(message: impl Display)
```

以固定 target `axutils::log` 发出 `WARN` 消息。

```rust,no_run
use axutils::LogUtils;

LogUtils::warn("即将重试外部调用");
```

## `LogUtils::error`

```text
pub fn error(message: impl Display)
```

以固定 target `axutils::log` 发出 `ERROR` 消息。

```rust,no_run
use axutils::LogUtils;

LogUtils::error("外部调用失败");
```

这五个方法不是独立的 `println!` 或另一套 logger；它们受相同 subscriber 和 EnvFilter 规则
控制。消息的 `Display` 文本会直接进入日志，因此不得传入密码、token、密钥、URL、SQL、请求/
邮件正文或其他敏感数据。需要自定义 target、结构化字段或 span 时，应直接使用 `tracing` 宏。

初始化不会创建 Tokio runtime、调用 `block_on` 或启动异步日志 worker；formatter 和 writer
均为同步实现，文件 I/O 可能阻塞产生日志的线程。writer 运行时的 I/O 错误不会递归记录新
事件，也不会改变已经返回的业务结果。

## `LogConfig`

`LogConfig` 使用 consuming builder 设置输出、默认最低级别、target directive 和文件配置。

### `LogConfig::new`

```text
pub fn new() -> LogConfig
```

`new()` 与 `default()` 等价：标准输出开启、最低级别为 `LogLevel::Info`、没有额外 directive、
没有文件输出。

```rust,no_run
use axutils::LogConfig;

let stdout_only = LogConfig::new();
let _ = stdout_only;
```

### `LogConfig::with_stdout`

```text
pub fn with_stdout(self, enabled: bool) -> LogConfig
```

设置标准输出是否开启。它不会删除文件配置，因此 `with_stdout(false)` 既可以构造纯文件
输出，也可以和 `with_file` 一起使用。标准输出与文件都关闭时，`LogUtils::init` 返回
`LogError::InvalidConfig { field: "output" }`，不会消耗一次性初始化机会。`field` 是固定的
配置类别，不包含调用方输入。

```rust,no_run
use axutils::LogConfig;

let without_stdout = LogConfig::new().with_stdout(false);
let _ = without_stdout;
```

### `LogConfig::with_level`

```text
pub fn with_level(self, level: LogLevel) -> LogConfig
```

设置两个输出目标共用的最低级别。`LogLevel` 的五个变体按详细程度排列为：

- `LogLevel::Trace`：最详细的诊断事件；
- `LogLevel::Debug`：调试事件；
- `LogLevel::Info`：常规运行信息，也是默认值；
- `LogLevel::Warn`：可恢复问题或重试事件；
- `LogLevel::Error`：需要关注的失败事件。

过滤在 subscriber 层执行；低于默认最低级别的事件不会写入标准输出或文件。更具体的
`with_directives` target 规则可以把某个 target 的级别调高、调低或关闭。

```rust,no_run
use axutils::{LogConfig, LogLevel};

let debug_and_above = LogConfig::new().with_level(LogLevel::Debug);
let _ = debug_and_above;
```

### `LogConfig::with_directives`

```text
pub fn with_directives(self, directives: impl Into<String>) -> LogConfig
```

接收一段与 `RUST_LOG` 类似、用英文逗号分隔的 EnvFilter 字符串；本方法只是使用相同语法，
`LogUtils::init` 不会读取 `RUST_LOG` 环境变量。最常用的格式是裸级别或 `target=级别`：

```rust,no_run
use axutils::{LogConfig, LogLevel};

let config = LogConfig::new()
    .with_level(LogLevel::Info)
    .with_directives("lettre=off,rustls=off,tower_http=debug,sqlx::query=warn");
let _ = config;
```

没有裸级别时，`with_level` 是未匹配 target 的默认值；上例相当于先使用 `info`，再追加四条
target 规则。支持的级别为 `trace`、`debug`、`info`、`warn`、`error` 和 `off`，其中日志级别
写作 `warn`，不是 `warning`。target 按前缀匹配，更具体的规则优先，例如
`axutils::http=debug` 会覆盖 `axutils=info`。

如果只想开启明确列出的 target，应提供裸级别 `off`：

```rust,no_run
use axutils::{LogConfig, LogUtils};

fn main() -> Result<(), axutils::LogError> {
    let config = LogConfig::new().with_directives(
        "off,axutils=info,axutils::http=debug,axutils::crypto=warn",
    );
    LogUtils::init(config)?;
    Ok(())
}
```

这会关闭未匹配 target，允许 `axutils` 的 `INFO` 及以上、HTTP 的 `DEBUG` 及以上，并把 Crypto
限制为 `WARN` 及以上；`LogUtils::info` 使用的 `axutils::log` 也会被 `axutils=info` 打开。
当字符串已包含 `off`、`info` 等裸级别时，该裸级别会作为显式默认值。

directive 在 `init` 时解析；语法无效返回
`LogError::InvalidConfig { field: "filter" }`，不会安装 subscriber，也不会消耗一次性初始化
机会。多次调用时后一次替换前一次；空白字符串表示不追加 directive。调用方不应把含有敏感
值的动态文本拼入过滤字符串。

### `LogConfig::with_file`

```text
pub fn with_file(self, file: LogFileConfig) -> LogConfig
```

设置唯一的文件输出配置。重复调用会替换尚未初始化的配置，不会安装多个文件 writer。
父目录在 `LogUtils::init` 时按需创建，构造 `LogFileConfig` 本身不访问文件系统。

```rust,no_run
use axutils::{LogConfig, LogFileConfig};

let file_only = LogConfig::new()
    .with_stdout(false)
    .with_file(LogFileConfig::new("var/app.log"));
let _ = file_only;
```

## `LogFileConfig` 与轮转

### `LogFileConfig::new`

```text
pub fn new(path: impl AsRef<Path>) -> LogFileConfig
```

`new(path)` 只保存调用方传入的路径，不访问文件系统，并默认使用 `LogRotation::Daily`。

```rust,no_run
use axutils::LogFileConfig;

let daily_file = LogFileConfig::new("var/app.log");
let _ = daily_file;
```

### `LogFileConfig::with_rotation`

```text
pub fn with_rotation(self, rotation: LogRotation) -> LogFileConfig
```

`with_rotation(rotation)` 会替换当前切分策略：

- `LogRotation::Never`：始终使用传入的精确文件名；
- `LogRotation::Minutely`：按分钟轮转；
- `LogRotation::Hourly`：按小时轮转；
- `LogRotation::Daily`：按天轮转，也是默认值。

分钟、小时、天轮转的带日期后缀文件名由当前 `tracing-appender` 版本决定；本 crate 不
提供文件名 getter，也不承诺后缀格式。轮转只负责 writer 的文件切分，不负责历史文件
retention、压缩、删除或磁盘配额。目录和文件权限沿用操作系统的默认创建权限、umask 或
ACL；部署时应由应用和系统明确控制日志目录权限。

路径在初始化时校验：必须能拆出非空 UTF-8 basename，不能是根路径或空路径。目录创建或
文件 writer 创建失败返回脱敏的 `LogError::FileInit { kind }`，只保留
`std::io::ErrorKind`，不保存路径、原始 I/O 错误文本或第三方错误对象。无效路径返回
`LogError::InvalidPath`。

```rust,no_run
use axutils::{LogConfig, LogFileConfig, LogRotation, LogUtils};

fn main() -> Result<(), axutils::LogError> {
    let file = LogFileConfig::new(std::env::temp_dir().join("axutils-app.log"))
        .with_rotation(LogRotation::Never);
    LogUtils::init(LogConfig::new().with_stdout(false).with_file(file))?;
    Ok(())
}
```

## `LogError`

公开错误变体及其脱敏边界如下：

- `InvalidConfig { field }`：配置无效；`field` 只使用固定类别：`"output"` 表示标准输出和文件
  输出同时关闭，`"filter"` 表示 EnvFilter directive 解析失败；
- `InvalidPath`：日志文件路径没有可用的文件名或不是可接受的路径；
- `FileInit { kind }`：目录或文件 writer 初始化失败，只保留 `io::ErrorKind`；
- `AlreadyInitialized`：本 crate 已成功安装 subscriber；
- `GlobalSubscriberAlreadySet`：其他代码已经占用全局 subscriber；
- `InitializationLockPoisoned`：初始化互斥锁因其他线程 panic 而中毒；
- `InitializationStateCorrupted`：内部一次性状态不变量被破坏。

`LogError` 不携带路径、日志正文、配置内容、凭据或第三方错误文本，适合在应用边界按
固定分类处理。`Display` 和 `Debug` 都不应被当作写入敏感上下文的替代品。

## 库内结构化事件

启用 `tracing` 后，库在调用方 subscriber 的上下文中发布固定 target 的事件；未安装
subscriber 时，这些事件不会自动写文件、标准输出或改变业务返回值。事件只记录诊断所需
的有限元数据，常见字段包括 `operation`、`mode`、`outcome`、`error_kind`、
`duration_ms`、`attempts`、`rows` 和受限计数。

主要 target 与 operation 包括：

| target | 所需 feature | 事件范围 |
| --- | --- | --- |
| `axutils::log` | `logging` | `log_init` 与 `LogUtils::{trace,debug,info,warn,error}` 应用消息 |
| `axutils::http` | `tracing + http` | `client_init`、`request_dispatch`、`request_retry`、`request_complete` |
| `axutils::redis` | `tracing + redis` | `client_init`、`connection_manager_init`、`connection`、`command` |
| `axutils::sqlx` | `tracing + sqlx + tokio` | `client_init`、`connect`、实际查询方法、`begin`、`close` |
| `axutils::email` | `tracing + lettre` | `client_init`、`transport_init`、`send` |
| `axutils::config` | `tracing + serde` | 文件 `read` 与内存 `parse` |
| `axutils::jwt` | `tracing + jwt` | 全局 codec 初始化 |
| `axutils::crypto` | `tracing + aes` | AES 全局初始化 |

事件不会包含 HTTP URL、Header、Cookie、认证信息、请求/响应 body、Redis 命令/key/value/
lock token、SQL 文本、bind 值、数据库 URL、邮件地址/正文/主机/密码、配置路径/原文/树、
JWT token/claims/secret 或 AES key/IV/密文。应用若需要关联业务请求，应使用自己控制的、
不含上述敏感数据的 request id，并自行评估 subscriber 的字段过滤和权限。

## 运行时注意事项

`tracing` feature 不改变现有 API 的错误语义、重试策略、连接池生命周期或配置限制；它只
增加可选事件。HTTP 的外层 execute 入口最多为一次请求发布一个最终完成事件，重试单独发布
重试事件；Redis 普通命令使用共同命令入口记录，事务和锁不额外复制领域事件；SQLx 只在
实际执行、连接、事务开始和关闭时记录，`query`/`query_as`/`query_scalar` 构造及 `.bind()`
本身不记录 SQL；配置读取与解析分别记录，永不记录路径和内容。

`logging` 使用 `tracing-subscriber` 的 `env-filter` feature 解析 `with_level` 与调用方传入的
`with_directives`，因此会额外带入 `matchers`、`once_cell`、`regex-automata`、`regex-syntax` 和
`thread_local` 等实现依赖。它不自动读取环境变量，也不启用 JSON、ANSI 或 `tracing-log`。

本地回归测试使用 loopback HTTP、SQLite `:memory:`、临时目录和合成 subscriber。真实 SMTP、
外部 Redis 和外部数据库不属于默认测试范围。
