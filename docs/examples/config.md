# 配置：`config` 与 `ConfigUtils`

配置领域类型从 `axutils::config` 导入；无状态的默认 loader 便利入口仅从
`axutils::utils::ConfigUtils` 导入。crate 根不平铺导出类型，`utils` 的叶模块也不是公共 API。

本领域只读取和解析**单个**配置文件或内存文本；不做多文件合并、层叠覆盖、热重载、写回、
`include`/`import`、表达式执行，且 `.env` 以外不做变量插值。

## 启用与导入

`config` 提供 JSON、`.env`、无类型值树和有类型反序列化基础能力。各格式后端以及异步文件读取
单独选择：

```toml
[dependencies]
axutils = { version = "1.0", default-features = false, features = [
    "config",       # JSON 与 .env
    "config-yaml",  # 同时包含 config
    "config-toml",  # 同时包含 config
    "config-ini",   # 同时包含 config
    "config-async", # 同时包含 config；仅异步文件读取
] }
serde = { version = "1", features = ["derive"] } # 使用 load::<T>/parse::<T> 时由应用直接依赖
tokio = { version = "1", features = ["rt-multi-thread", "macros"] } # 使用 config-async 时
```

`config-async` 不会让其他领域的异步 API 出现；通用 `tokio` feature 也不会反向开放 Config 的异步
入口。YAML、TOML、INI 只在对应 `config-yaml`、`config-toml`、`config-ini` feature 下存在。

```rust
use axutils::{
    config::{ConfigFormat, ConfigLoader, ConfigValue},
    utils::ConfigUtils,
};

let _ = (ConfigFormat::Json, ConfigLoader::new(), ConfigUtils, Option::<ConfigValue>::None);
```

## 默认 loader 与内存解析

`ConfigLoader::new()` 的默认文件上限为 1 MiB、嵌套深度上限为 64；可接受范围分别为 1 KiB–16 MiB
和 1–256。设置越界返回 `ConfigError::InvalidLimit`。内存解析的原始文本不经过文件读取上限，但
仍受深度限制；解析 `.env` 时，插值展开后的累计内容继续受 `max_bytes` 约束。

`ConfigUtils` 等价于默认 `ConfigLoader`，适合不需要改变限制或 `.env` 行为的调用点：

```rust
use axutils::{
    config::{ConfigError, ConfigFormat},
    utils::ConfigUtils,
};

let value = ConfigUtils::parse_value(r#"{"server":{"port":8080}}"#, ConfigFormat::Json)?;
assert_eq!(value.get("server.port").and_then(|value| value.as_i64()), Some(8080));
# Ok::<(), ConfigError>(())
```

需要限制文件规模、嵌套深度、显式格式或控制 `.env` 环境回退时，直接保留领域 loader：

```rust
use axutils::config::{ConfigError, ConfigFormat, ConfigLoader};

let loader = ConfigLoader::new()
    .with_max_bytes(64 * 1024)?
    .with_max_depth(16)?
    .with_env_substitution(false)
    .with_format(ConfigFormat::Json);
let _value = loader.parse_value(r#"{"enabled":true}"#, ConfigFormat::Json)?;
# Ok::<(), ConfigError>(())
```

`parse_value`/`load_value` 返回 `ConfigValue`。它支持 `get("a.b")`、`kind`、`as_bool`、`as_i64`、
`as_f64`、`as_str`、`as_array`、`as_table`，但不做隐式类型转换，也不支持数组下标、通配符或表达式。
`ConfigValue` 可能含密码或 token，不应整体写入日志。

`parse::<T>`/`load::<T>` 和 `ConfigUtils` 的同名方法通过 `serde::Deserialize` 将内容映射到调用方类型；
调用方应直接声明 `serde` 依赖并为其配置类型派生或实现 `Deserialize`。

## 文件读取与显式格式

`ConfigLoader::load_value` 与 `ConfigLoader::load` 通过扩展名推断格式；loader 可先调用
`with_format` 覆盖推断，`ConfigUtils` 另提供 `load_value_as` 与 `load_as` 便利入口。
文件操作可失败，应在示例和生产代码中处理 `ConfigError`：

```rust,no_run
use axutils::{
    config::{ConfigError, ConfigFormat, ConfigLoader},
    utils::ConfigUtils,
};

fn read_settings() -> Result<(), ConfigError> {
    let value = ConfigUtils::load_value("app.json")?;
    let _port = value.get("server.port").and_then(|value| value.as_i64());

    let text_with_json_extension = ConfigLoader::new()
        .with_format(ConfigFormat::Json)
        .load_value("settings.conf")?;
    let _ = text_with_json_extension;
    Ok(())
}
```

`.env` 识别 `.env` 与 `.env.*`。双引号中的 `${VAR}` 优先引用此前已经解析的文件键；默认可回退到
进程环境变量。用 `with_env_substitution(false)` 禁止这一回退，未定义变量返回
`ConfigError::UndefinedVariable`。库从不向进程环境变量写入内容。

```rust
use axutils::config::{ConfigError, ConfigFormat, ConfigLoader};

let error = ConfigLoader::new()
    .with_env_substitution(false)
    .parse_value("TOKEN=\"${MISSING}\"\n", ConfigFormat::Env)
    .unwrap_err();
assert!(matches!(error, ConfigError::UndefinedVariable { .. }));
```

## 格式后端

`ConfigFormat::Json` 和 `ConfigFormat::Env` 由 `config` 提供。启用相应 feature 后，
`ConfigFormat` 还包含 `Yaml`、`Toml`、`Ini`；未知扩展名返回 `ConfigError::UnknownExtension`，
已知但未启用的后端返回 `ConfigError::FormatNotEnabled`。

```rust
use axutils::config::{ConfigError, ConfigFormat, ConfigLoader};

let loader = ConfigLoader::new();
let value = loader.parse_value("[server]\nport = 8080\n", ConfigFormat::Toml)?;
assert_eq!(value.get("server.port").and_then(|value| value.as_i64()), Some(8080));
# Ok::<(), ConfigError>(())
```

上例要求 `config-toml`。YAML 与 INI 分别使用 `ConfigFormat::Yaml` 和 `ConfigFormat::Ini`，要求
`config-yaml` 与 `config-ini`。INI 无类型值通常保留为字符串；不要假设所有格式都会自动把字符串
转换为数值或布尔值。

## 异步文件读取（`config-async`）

`load_value_async`、`load_async`、`load_value_as_async` 和 `load_as_async` 仅在 `config-async` 下提供。
它们只异步化文件 I/O；解析仍在当前 async 任务中同步执行。调用方必须提供 Tokio runtime，库不创建
runtime、不调用 `block_on`。每个并发读取最多使用约 `max_bytes + 1` 字节缓冲区，调用方负责限制
任务数、路径来源和总内存。

```rust,no_run
use axutils::{
    config::{ConfigError, ConfigLoader},
    utils::ConfigUtils,
};

async fn read_settings() -> Result<(), ConfigError> {
    let _value = ConfigUtils::load_value_async("app.json").await?;
    let _explicit = ConfigLoader::new().load_value_async("other.json").await?;
    Ok(())
}
```

## 错误、安全与资源边界

`ConfigError` 是 `#[non_exhaustive]`；匹配时应保留通配分支。常见分类包括 I/O、文件过大、非 UTF-8、
未知或未启用格式、解析/深度/重复键、未定义 `.env` 变量、数值范围、类型不匹配和无效限制。
错误不会回显原始配置内容或底层解析错误文本，但路径和键名本身仍可能敏感。

文件读取以 `max_bytes + 1` 检测超限；`.env` 插值后的累计内容也受同一上限。无类型 JSON 和各格式的
受控解析路径使用深度预算；YAML 别名回放另有总事件、单 anchor 展开和回放栈上限。不要把不受信配置、
`ConfigValue`、反序列化结果或错误上下文原样输出到日志。
