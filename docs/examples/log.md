# 日志

启用 `logging` 后，配置与错误类型位于 `axutils::logging`，进程级初始化入口 `LogUtils` 位于
`axutils::utils`。业务日志直接使用标准 `tracing` 宏，而不是通过工具类转发。

```toml
[dependencies]
axutils = { version = "1.0", features = ["logging"] }
tracing = "0.1"
```

## 初始化与业务日志

全局 subscriber 只能成功安装一次，之后不可 reset、replace、关闭或重配。配置、filter 或 appender
构造失败不会把 `LogUtils` 标记为已初始化，可以修正本地配置后重试。若应用或其他库已经安装全局
subscriber，则进程的 tracing 全局槽已永久占用，`LogUtils` 会返回
`GlobalSubscriberAlreadySet` 且不能再接管；此时 `is_initialized()` 仍为 `false`。

```rust,no_run
use axutils::{
    logging::{LogConfig, LogError, LogLevel},
    utils::LogUtils,
};

fn initialize_logging() -> Result<(), LogError> {
    let config = LogConfig::new()
        .with_stdout(true)
        .with_level(LogLevel::Info);
    LogUtils::init(config)
}

fn handle_request(request_id: &str) {
    tracing::info!(%request_id, "request accepted");
    tracing::warn!(%request_id, "retry budget is low");
}
```

`LogUtils::is_initialized()` 仅报告本 crate 是否成功安装了自己的 subscriber；外部 subscriber
不会被误报。文件输出可用 `LogFileConfig` 配置，但路径校验、目录创建与 appender 构造会访问文件系统；
当前 stdout/file writer 是同步写入，会占用产生日志的线程。应用应在启动阶段完成配置，并避免将敏感
数据写入日志。

```rust,no_run
use axutils::{
    logging::{LogConfig, LogError, LogFileConfig, LogRotation},
    utils::LogUtils,
};

fn initialize_file_logging() -> Result<(), LogError> {
    let file = LogFileConfig::new("./logs/application.log").with_rotation(LogRotation::Daily);
    LogUtils::init(LogConfig::new().with_stdout(false).with_file(file))
}
```

不要在字段、错误文本或 span 中记录密码、token、完整请求体或数据库连接 URL。
