# config 模块与 ConfigUtils 使用文档

> 需要 `serde` feature。JSON 和自实现 `.env` 随 `serde` 可用；YAML、TOML、INI 分别还需要
> `serde-saphyr`、`toml`、`rust-ini`。异步文件入口需要同时启用 `serde` 与 `tokio`，且由调用方
> 提供 Tokio runtime。

## 导出内容

公开模块路径：

- `axutils::config`：配置领域类型的直接模块路径；
- `axutils::utils::config_utils`：`ConfigUtils` 的公开子模块路径；
- `axutils::utils` 公开，但推荐使用 crate 根导入。

`ConfigLoader`、`ConfigFormat`、`ConfigValue`、`ConfigError` 均同时支持：

- 推荐 crate 根路径：`axutils::ConfigLoader`、`axutils::ConfigFormat`、
  `axutils::ConfigValue`、`axutils::ConfigError`；
- 次级领域模块路径：`axutils::config::ConfigLoader`、`axutils::config::ConfigFormat`、
  `axutils::config::ConfigValue`、`axutils::config::ConfigError`。

`ConfigUtils` 支持：`axutils::ConfigUtils`（推荐）、`axutils::utils::ConfigUtils` 和
`axutils::utils::config_utils::ConfigUtils`。`src/config/` 和 `src/utils/config_utils.rs` 是实现
文件路径，不能替代上述公共导入路径。

`ConfigFormat` 标记 `#[non_exhaustive]`，实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`。
恒定变体为 `Json`、`Env`；`Yaml` 需要 `serde-saphyr`，`Toml` 需要 `toml`，`Ini` 需要
`rust-ini`。`ConfigValue` 标记 `#[non_exhaustive]`，实现 `Debug`、`Clone`、`PartialEq` 和
`serde::Deserialize`，由于含 `f64` 不实现 `Eq`；变体为：

- `Null`；
- `Bool(bool)`；
- `Integer(i64)`；
- `Float(f64)`；
- `String(String)`；
- `Array(Vec<ConfigValue>)`；
- `Table(BTreeMap<String, ConfigValue>)`。

`ConfigError` 标记 `#[non_exhaustive]`，实现 `Debug`、`Clone`、`PartialEq`、`Eq`、`Display`
和 `std::error::Error`。完整字段见“错误类型”一节。匹配 `ConfigFormat`、`ConfigValue` 和
`ConfigError` 时都应保留 `_` 通配分支。

`ConfigLoader` 字段私有，不实现 `Clone` 或 `Debug`，实现 `Default`；它不持有文件句柄、缓存
或全局状态。`ConfigUtils` 是无状态工具结构体，无公共字段和 `new` 方法，实现 `Debug`、
`Clone`、`Copy`、`Default`。本模块没有公共自由函数、trait、类型别名、静态项或宏。

## 安装与启用

基础 JSON/`.env`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde"] }
```

所有格式和异步入口：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "tokio", "serde-saphyr", "toml", "rust-ini"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

格式后端 feature 相互独立，不会自动启用 `serde`；只启用后端 feature 而没有 `serde` 时，
配置模块不会导出。

## `ConfigLoader` 方法

### `ConfigLoader::new() -> ConfigLoader`

- **feature**：`serde`。
- **返回值**：默认 loader：文件大小上限 1 MiB、嵌套深度上限 64、格式按扩展名推断，`.env`
  插值允许在文件键中找不到时回退到进程环境变量。
- **示例**：

```rust
use axutils::ConfigLoader;

let loader = ConfigLoader::new();
let _ = loader;
```

### `ConfigLoader::with_format(self, format: ConfigFormat) -> ConfigLoader`

- **feature**：`serde`。
- **参数**：显式格式覆盖，只影响 `load_value`/`load` 及其异步版本的扩展名推断，不影响
  显式传入 `format` 的 `parse_value`/`parse`。
- **返回值**：带格式覆盖的新 loader；这是消费 `self` 的 `#[must_use]` builder。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigLoader};

let loader = ConfigLoader::new().with_format(ConfigFormat::Json);
let _ = loader;
```

忽略返回值不会修改原 loader：

```rust
use axutils::{ConfigFormat, ConfigLoader};

let loader = ConfigLoader::new();
let _new_loader = loader.with_format(ConfigFormat::Env);
```

### `ConfigLoader::with_max_bytes(self, max_bytes: usize) -> Result<ConfigLoader, ConfigError>`

- **feature**：`serde`。
- **参数**：文件读取字节上限，也是 `.env` 插值后所有 key/value 累计内容的字节上限，允许
  `1 KiB..=16 MiB`。
- **返回值**：成功返回消费后的 loader；越界返回 `ConfigError::InvalidLimit`，调用方必须
  处理 `Result`。
- **示例**：

```rust
use axutils::{ConfigError, ConfigLoader};

let loader = ConfigLoader::new().with_max_bytes(64 * 1024).unwrap();
let _ = loader;
assert!(matches!(
    ConfigLoader::new().with_max_bytes(0),
    Err(ConfigError::InvalidLimit)
));
```

### `ConfigLoader::with_max_depth(self, max_depth: usize) -> Result<ConfigLoader, ConfigError>`

- **feature**：`serde`。
- **参数**：嵌套深度上限，允许 `1..=256`。
- **返回值**：成功返回消费后的 loader；越界返回 `ConfigError::InvalidLimit`。
- **示例**：

```rust
use axutils::{ConfigError, ConfigLoader};

let loader = ConfigLoader::new().with_max_depth(8).unwrap();
let _ = loader;
assert!(matches!(
    ConfigLoader::new().with_max_depth(0),
    Err(ConfigError::InvalidLimit)
));
```

### `ConfigLoader::with_env_substitution(self, enabled: bool) -> ConfigLoader`

- **feature**：`serde`。
- **参数**：是否允许 `.env` 插值在文件内找不到键时回退到进程环境变量；默认 `true`。
- **返回值**：消费后的 loader；这是 `#[must_use]` builder。关闭回退不会关闭文件内插值，
  也不会向进程环境变量写入内容。
- **示例**：

```rust
use axutils::{ConfigError, ConfigFormat, ConfigLoader};

let error = ConfigLoader::new()
    .with_env_substitution(false)
    .parse_value("A=plain\nB=\"${MISSING}\"\n", ConfigFormat::Env);
assert!(matches!(error, Err(ConfigError::UndefinedVariable { .. })));
```

### `ConfigLoader::load_value(&self, path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde`。
- **参数**：配置文件路径；格式默认由路径推断，也可由 `with_format` 覆盖。
- **返回值**：在文件大小上限内读取、去除 UTF-8 BOM 后解析为 `ConfigValue`。
- **示例**：

```rust,no_run
use std::io::Write;
use axutils::ConfigLoader;

let path = std::env::temp_dir().join(format!(
    "axutils-doc-loader-value-{}.json",
    std::process::id()
));
std::fs::File::create(&path)
    .unwrap()
    .write_all(br#"{"port": 8080}"#)
    .unwrap();
let value = ConfigLoader::new().load_value(&path).unwrap();
assert_eq!(value.get("port").and_then(|value| value.as_i64()), Some(8080));
std::fs::remove_file(path).ok();
```

读取超过 `max_bytes`、不是 UTF-8、格式未知或解析失败时返回相应 `ConfigError`；读取使用
`take(max_bytes + 1)`，不会仅依赖 metadata 造成无界读取。

### `async ConfigLoader::load_value_async(&self, path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：与 `load_value` 相同，但文件读取在调用方已有 Tokio runtime 中异步执行。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), axutils::ConfigError> {
    use axutils::ConfigLoader;

    let value = ConfigLoader::new().load_value_async("app.json").await?;
    let _ = value;
    Ok(())
}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

**注意**：crate 不创建 runtime、不调用 `block_on`，解析阶段仍在当前任务同步执行；每个并发
调用独立占用最多约 `max_bytes + 1` 字节缓冲区，调用方负责限制路径来源、任务数量和总内存。

### `ConfigLoader::load<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError>`

- **feature**：`serde`。
- **参数**：路径和调用方的目标类型 `T`；格式按扩展名推断或使用 `with_format` 覆盖。
- **返回值**：读取文件后直接反序列化为 `T`；文件/格式/解析/类型错误返回 `ConfigError`。
- **示例**：

```rust,no_run
use serde::Deserialize;
use std::io::Write;
use axutils::ConfigLoader;

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
}

let path = std::env::temp_dir().join(format!(
    "axutils-doc-loader-typed-{}.json",
    std::process::id()
));
std::fs::File::create(&path)
    .unwrap()
    .write_all(br#"{"port": 8080}"#)
    .unwrap();
let config: AppConfig = ConfigLoader::new().load(&path).unwrap();
assert_eq!(config.port, 8080);
std::fs::remove_file(path).ok();
```

JSON/TOML 的有类型递归保护由对应后端提供；YAML/INI 的有类型路径使用 loader 深度上限。

### `async ConfigLoader::load_async<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：与 `load` 相同，但文件读取异步；解析和类型反序列化仍在当前异步任务中
  同步完成。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
async fn read_typed() -> Result<(), axutils::ConfigError> {
    use axutils::ConfigLoader;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct AppConfig {
        port: u16,
    }

    let config: AppConfig = ConfigLoader::new().load_async("app.json").await?;
    let _ = config;
    Ok(())
}
# #[cfg(all(feature = "serde", feature = "tokio"))]
# fn main() {}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

### `ConfigLoader::parse_value(&self, text: &str, format: ConfigFormat) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde`。
- **参数**：内存文本和显式格式；不读取文件，也不做文件大小校验。
- **返回值**：按 loader 的深度/`.env` 配置解析为无类型值。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigLoader};

let value = ConfigLoader::new()
    .parse_value(r#"{"port": 8080}"#, ConfigFormat::Json)
    .unwrap();
assert_eq!(value.get("port").and_then(|value| value.as_i64()), Some(8080));
```

### `ConfigLoader::parse<T: DeserializeOwned>(&self, text: &str, format: ConfigFormat) -> Result<T, ConfigError>`

- **feature**：`serde`。
- **参数**：内存文本、显式格式和目标类型 `T`；不读取文件、不做文件大小校验。
- **返回值**：按后端反序列化为 `T`，类型不匹配时返回相应错误。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigLoader};
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
}

let config: AppConfig = ConfigLoader::new()
    .parse(r#"{"port": 8080}"#, ConfigFormat::Json)
    .unwrap();
assert_eq!(config.port, 8080);
```

## `ConfigUtils` 静态方法

`ConfigUtils` 的同步方法等价于使用新的默认 `ConfigLoader`；它不引入全局单例、缓存或可变
全局状态。需要自定义限制时使用 `loader()`。

### `ConfigUtils::parse_value(text: &str, format: ConfigFormat) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：内存文本和显式格式；等价于默认 loader 的 `parse_value`，不做文件大小校验。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value(r#"{"port": 8080}"#, ConfigFormat::Json).unwrap();
assert_eq!(value.get("port").and_then(|value| value.as_i64()), Some(8080));
```

### `ConfigUtils::parse<T: DeserializeOwned>(text: &str, format: ConfigFormat) -> Result<T, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：内存文本、显式格式和目标类型；等价于默认 loader 的 `parse`。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigUtils};
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
}

let config: AppConfig = ConfigUtils::parse(r#"{"port": 8080}"#, ConfigFormat::Json).unwrap();
assert_eq!(config.port, 8080);
```

### `ConfigUtils::load_value(path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：按扩展名推断并读取文件为无类型值，等价于默认 loader 的 `load_value`。
- **示例**：

```rust,no_run
use std::io::Write;
use axutils::ConfigUtils;

let path = std::env::temp_dir().join(format!("axutils-doc-utils-value-{}.json", std::process::id()));
std::fs::File::create(&path).unwrap().write_all(br#"{"ok":true}"#).unwrap();
let value = ConfigUtils::load_value(&path).unwrap();
assert_eq!(value.get("ok").and_then(|value| value.as_bool()), Some(true));
std::fs::remove_file(path).ok();
```

### `ConfigUtils::load<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：按扩展名推断、读取并反序列化为 `T`。
- **示例**：

```rust,no_run
use serde::Deserialize;
use std::io::Write;
use axutils::ConfigUtils;

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
}

let path = std::env::temp_dir().join(format!("axutils-doc-utils-load-{}.json", std::process::id()));
std::fs::File::create(&path).unwrap().write_all(br#"{"port":8080}"#).unwrap();
let config: AppConfig = ConfigUtils::load(&path).unwrap();
assert_eq!(config.port, 8080);
std::fs::remove_file(path).ok();
```

### `ConfigUtils::load_value_as(path: impl AsRef<Path>, format: ConfigFormat) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：显式格式覆盖扩展名推断，读取文件为无类型值。
- **示例**：

```rust,no_run
use std::io::Write;
use axutils::{ConfigFormat, ConfigUtils};

let path = std::env::temp_dir().join(format!("axutils-doc-utils-value-as-{}.txt", std::process::id()));
std::fs::File::create(&path).unwrap().write_all(br#"{"port":8080}"#).unwrap();
let value = ConfigUtils::load_value_as(&path, ConfigFormat::Json).unwrap();
assert_eq!(value.get("port").and_then(|value| value.as_i64()), Some(8080));
std::fs::remove_file(path).ok();
```

### `ConfigUtils::load_as<T: DeserializeOwned>(path: impl AsRef<Path>, format: ConfigFormat) -> Result<T, ConfigError>`

- **feature**：`serde`。
- **参数/返回值**：显式格式、文件路径和目标类型；读取后反序列化为 `T`。
- **示例**：

```rust,no_run
use serde::Deserialize;
use std::io::Write;
use axutils::{ConfigFormat, ConfigUtils};

#[derive(Deserialize)]
struct AppConfig {
    port: u16,
}

let path = std::env::temp_dir().join(format!("axutils-doc-utils-as-{}.txt", std::process::id()));
std::fs::File::create(&path).unwrap().write_all(br#"{"port":8080}"#).unwrap();
let config: AppConfig = ConfigUtils::load_as(&path, ConfigFormat::Json).unwrap();
assert_eq!(config.port, 8080);
std::fs::remove_file(path).ok();
```

### `async ConfigUtils::load_value_async(path: impl AsRef<Path>) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：按扩展名异步读取为无类型值，等价于默认 loader 的 `load_value_async`。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
async fn read_value() -> Result<(), axutils::ConfigError> {
    let value = axutils::ConfigUtils::load_value_async("app.json").await?;
    let _ = value;
    Ok(())
}
# #[cfg(all(feature = "serde", feature = "tokio"))]
# fn main() {}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

### `async ConfigUtils::load_async<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：按扩展名异步读取并反序列化为目标类型。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
async fn read_typed() -> Result<(), axutils::ConfigError> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct AppConfig {
        port: u16,
    }

    let config: AppConfig = axutils::ConfigUtils::load_async("app.json").await?;
    let _ = config;
    Ok(())
}
# #[cfg(all(feature = "serde", feature = "tokio"))]
# fn main() {}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

### `async ConfigUtils::load_value_as_async(path: impl AsRef<Path>, format: ConfigFormat) -> Result<ConfigValue, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：显式格式覆盖扩展名并异步读取为无类型值。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
async fn read_explicit() -> Result<(), axutils::ConfigError> {
    use axutils::{ConfigFormat, ConfigUtils};

    let value = ConfigUtils::load_value_as_async("app.conf", ConfigFormat::Json).await?;
    let _ = value;
    Ok(())
}
# #[cfg(all(feature = "serde", feature = "tokio"))]
# fn main() {}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

### `async ConfigUtils::load_as_async<T: DeserializeOwned>(path: impl AsRef<Path>, format: ConfigFormat) -> Result<T, ConfigError>`

- **feature**：`serde` + `tokio`。
- **参数/返回值**：显式格式覆盖扩展名，异步读取并反序列化为目标类型。
- **示例**：

```rust,no_run
# #[cfg(all(feature = "serde", feature = "tokio"))]
async fn read_explicit_typed() -> Result<(), axutils::ConfigError> {
    use axutils::{ConfigFormat, ConfigUtils};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct AppConfig {
        port: u16,
    }

    let config: AppConfig = ConfigUtils::load_as_async("app.conf", ConfigFormat::Json).await?;
    let _ = config;
    Ok(())
}
# #[cfg(all(feature = "serde", feature = "tokio"))]
# fn main() {}
# #[cfg(not(all(feature = "serde", feature = "tokio")))]
# fn main() {}
```

### `ConfigUtils::loader() -> ConfigLoader`

- **feature**：`serde`。
- **参数/返回值**：无参数，返回一个可独立配置的默认 `ConfigLoader`，不会修改其他调用方
  或引入全局状态。
- **示例**：

```rust
use axutils::ConfigUtils;

let loader = ConfigUtils::loader().with_max_depth(2).unwrap();
let error = loader
    .parse_value(r#"{"a":{"b":{"c":1}}}"#, axutils::ConfigFormat::Json)
    .unwrap_err();
assert!(matches!(error, axutils::ConfigError::DepthLimitExceeded { limit: 2 }));
```

## `ConfigValue` 方法

### `ConfigValue::get(&self, path: &str) -> Option<&ConfigValue>`

- **feature**：`serde`。
- **参数**：点号分隔的表字段路径，如 `server.tls.port`。
- **返回值**：逐段查找得到值时返回引用；路径穿过非表节点或字段不存在时返回 `None`。
  空路径按单个空键查找，通常返回 `None`；该方法不支持数组下标、通配符或表达式。键名本身
  含点号不能通过点号路径访问。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigUtils, ConfigValue};

let value = ConfigUtils::parse_value(
    r#"{"server":{"port":8080},"items":[1]}"#,
    ConfigFormat::Json,
).unwrap();
assert_eq!(value.get("server.port").and_then(ConfigValue::as_i64), Some(8080));
assert!(value.get("items.0").is_none());
assert!(value.get("").is_none());
```

### `ConfigValue::kind(&self) -> &'static str`

- **feature**：`serde`。
- **返回值**：值的稳定类别名：`null`、`bool`、`integer`、`float`、`string`、`array` 或
  `table`。
- **示例**：

```rust
use axutils::ConfigValue;

assert_eq!(ConfigValue::Bool(true).kind(), "bool");
assert_eq!(ConfigValue::Null.kind(), "null");
```

### `ConfigValue::as_bool(&self) -> Option<bool>`

- **feature**：`serde`。
- **返回值**：仅 `Bool` 返回其值，其他类型返回 `None`，不做隐式转换。
- **示例**：

```rust
use axutils::ConfigValue;

assert_eq!(ConfigValue::Bool(true).as_bool(), Some(true));
assert_eq!(ConfigValue::Integer(1).as_bool(), None);
```

### `ConfigValue::as_i64(&self) -> Option<i64>`

- **feature**：`serde`。
- **返回值**：仅 `Integer` 返回其值，其他类型返回 `None`。
- **示例**：

```rust
use axutils::ConfigValue;

assert_eq!(ConfigValue::Integer(42).as_i64(), Some(42));
assert_eq!(ConfigValue::Float(42.0).as_i64(), None);
```

### `ConfigValue::as_f64(&self) -> Option<f64>`

- **feature**：`serde`。
- **返回值**：仅 `Float` 返回其值，整数不会隐式转换为浮点数。
- **示例**：

```rust
use axutils::ConfigValue;

assert_eq!(ConfigValue::Float(1.5).as_f64(), Some(1.5));
assert_eq!(ConfigValue::Integer(1).as_f64(), None);
```

### `ConfigValue::as_str(&self) -> Option<&str>`

- **feature**：`serde`。
- **返回值**：仅 `String` 返回字符串切片；其他类型返回 `None`。
- **示例**：

```rust
use axutils::ConfigValue;

assert_eq!(ConfigValue::String("x".to_owned()).as_str(), Some("x"));
assert_eq!(ConfigValue::Bool(true).as_str(), None);
```

### `ConfigValue::as_array(&self) -> Option<&[ConfigValue]>`

- **feature**：`serde`。
- **返回值**：仅 `Array` 返回切片；其他类型返回 `None`。
- **示例**：

```rust
use axutils::ConfigValue;

let array = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
assert_eq!(array.as_array().map(<[_]>::len), Some(1));
assert_eq!(ConfigValue::Bool(true).as_array(), None);
```

### `ConfigValue::as_table(&self) -> Option<&BTreeMap<String, ConfigValue>>`

- **feature**：`serde`。
- **返回值**：仅 `Table` 返回有序键表；其他类型返回 `None`。
- **示例**：

```rust
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value(r#"{"a":1}"#, ConfigFormat::Json).unwrap();
assert_eq!(value.as_table().map(|table| table.len()), Some(1));
```

## `ConfigFormat` 方法和格式覆盖

### `ConfigFormat::from_path(path: impl AsRef<Path>) -> Result<ConfigFormat, ConfigError>`

- **feature**：`serde`；后端变体还需要各自的 feature。
- **参数**：文件路径或文件名。
- **返回值**：按文件名和扩展名推断格式。`.env` 和 `.env.*` 优先识别为 `Env`，其余扩展名
  不区分大小写：`json` → `Json`、`env` → `Env`、`yaml`/`yml` → `Yaml`、`toml` → `Toml`、
  `ini`/`cfg`/`conf` → `Ini`。未知或无扩展名返回 `UnknownExtension`；已知但未启用后端返回
  `FormatNotEnabled { extension }`。
- **示例**：

```rust
use axutils::{ConfigError, ConfigFormat};

assert_eq!(ConfigFormat::from_path("app.JSON").unwrap(), ConfigFormat::Json);
assert_eq!(ConfigFormat::from_path(".env").unwrap(), ConfigFormat::Env);
assert_eq!(ConfigFormat::from_path(".env.local").unwrap(), ConfigFormat::Env);
assert!(matches!(
    ConfigFormat::from_path("app.unknownext"),
    Err(ConfigError::UnknownExtension)
));
```

未启用后端的行为必须显式处理：

```rust
# #[cfg(not(feature = "toml"))]
# fn main() {
use axutils::{ConfigError, ConfigFormat};

assert!(matches!(
    ConfigFormat::from_path("app.toml"),
    Err(ConfigError::FormatNotEnabled { extension }) if extension == "toml"
));
# }
# #[cfg(feature = "toml")]
# fn main() {
use axutils::ConfigFormat;
assert_eq!(ConfigFormat::from_path("app.toml").unwrap(), ConfigFormat::Toml);
# }
```

### `ConfigFormat::as_str(&self) -> &'static str`

- **feature**：`serde`。
- **返回值**：格式的稳定小写名称；后端变体的名称只有在对应 feature 下可用。
- **示例**：

```rust
use axutils::ConfigFormat;

assert_eq!(ConfigFormat::Json.as_str(), "json");
assert_eq!(ConfigFormat::Env.as_str(), "env");
```

```rust
# #[cfg(feature = "serde-saphyr")]
# fn main() {
use axutils::ConfigFormat;
assert_eq!(ConfigFormat::Yaml.as_str(), "yaml");
# }
# #[cfg(not(feature = "serde-saphyr"))]
# fn main() {}
```

## 五种格式的解析示例

### JSON（`serde`）

JSON 通过 `ConfigFormat::Json` 使用无类型或有类型路径：

```rust
use axutils::{ConfigFormat, ConfigUtils};
use serde::Deserialize;

let value = ConfigUtils::parse_value(r#"{"enabled":true,"port":8080}"#, ConfigFormat::Json).unwrap();
assert_eq!(value.get("port").and_then(|value| value.as_i64()), Some(8080));

#[derive(Deserialize)]
struct AppConfig {
    enabled: bool,
    port: u16,
}
let config: AppConfig = ConfigUtils::parse(r#"{"enabled":true,"port":8080}"#, ConfigFormat::Json).unwrap();
assert!(config.enabled);
assert_eq!(config.port, 8080);
```

无类型与有类型 JSON 都拒绝任意嵌套对象中的重复键；非法语法、无类型路径超过深度或整数超出
`i64` 等情况会返回对应错误。有类型 JSON 的递归深度由后端自身保护。

### `.env`（`serde`）

`.env` 是本 crate 自实现的解析器：

```rust
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value(
    "BASE=hello\nDERIVED=\"${BASE} world\"\nexport FLAG=true\n",
    ConfigFormat::Env,
).unwrap();
assert_eq!(value.get("DERIVED").and_then(|value| value.as_str()), Some("hello world"));
assert_eq!(value.get("FLAG").and_then(|value| value.as_str()), Some("true"));
```

完整语法和边界如下：

- 赋值形态为 `KEY=VALUE`；`KEY` 遵守 `[A-Za-z_][A-Za-z0-9_]*`；
- `export` 可选，但关键字后必须有空格或制表符；
- 空行和整行 `#` 注释会跳过；无引号值、单引号值和双引号值都支持；
- 无引号值中的 ` #` 会截断行尾注释，其他空格保留到去掉行尾水平空白；
- 单引号不处理转义或插值；双引号支持 `\n`、`\r`、`\t`、`\\`、`\"`、`\$`；
- 只有双引号中的 `${VAR}` 会插值，变量名必须符合上述 key 规则；
- 变量先在此前已经解析的文件键中查找，找不到时默认回退到进程环境变量；关闭回退只需
  `with_env_substitution(false)`，不会关闭文件内插值；
- 未定义变量、无效 key、未闭合引号和重复键返回错误，不会静默变成空字符串；
- 与 `dotenv`/`dotenvy` 的兼容性不是目标，尤其是转义、插值时机和错误语义以本 crate 文档为准。

```rust
use axutils::{ConfigError, ConfigFormat, ConfigLoader};

let error = ConfigLoader::new()
    .with_env_substitution(false)
    .parse_value("VALUE=\"${NOT_DEFINED}\"\n", ConfigFormat::Env)
    .unwrap_err();
assert!(matches!(error, ConfigError::UndefinedVariable { .. }));
```

### YAML（`serde` + `serde-saphyr`）

```rust
# #[cfg(feature = "serde-saphyr")]
# fn main() {
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value("server:\n  port: 8080\n", ConfigFormat::Yaml).unwrap();
assert_eq!(value.get("server.port").and_then(|value| value.as_i64()), Some(8080));
# }
# #[cfg(not(feature = "serde-saphyr"))]
# fn main() {}
```

YAML 无类型和 YAML 有类型读取使用 `max_depth`；别名回放总事件最多 1,000,000 次，单个
anchor 最多展开 10,000 次，回放栈深度也受 loader 的嵌套深度上限约束。超出预算时返回错误，
不把原始配置值写入错误文本。

### TOML（`serde` + `toml`）

```rust
# #[cfg(feature = "toml")]
# fn main() {
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value("[server]\nport = 8080\n", ConfigFormat::Toml).unwrap();
assert_eq!(value.get("server.port").and_then(|value| value.as_i64()), Some(8080));
# }
# #[cfg(not(feature = "toml"))]
# fn main() {}
```

TOML 日期时间在无类型路径中保留为字符串；TOML 语法/重复键/整数范围错误按后端映射，
有类型路径使用 TOML 自身的递归保护。

### INI（`serde` + `rust-ini`）

```rust
# #[cfg(feature = "rust-ini")]
# fn main() {
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value("[server]\nport=8080\n", ConfigFormat::Ini).unwrap();
assert_eq!(value.get("server.port").and_then(|value| value.as_str()), Some("8080"));
# }
# #[cfg(not(feature = "rust-ini"))]
# fn main() {}
```

INI 无类型值以字符串为主；section 和 key 的深度、重复键以及有类型转换错误按后端实现
映射，调用方不应假设会自动把所有字符串转换为数字或布尔值。

## `ConfigError` 错误类型

`ConfigError` 当前有以下 13 个变体；字段是诊断元数据，不应被当作可回显配置内容：

| 变体 | 字段与语义 |
| --- | --- |
| `Io` | `path: PathBuf`、`kind: io::ErrorKind`；打开或读取失败 |
| `FileTooLarge` | `path: PathBuf`、`limit: usize`；超过文件字节上限 |
| `ExpandedValueTooLarge` | `limit: usize`；`.env` 插值后的累计内容超过字节上限 |
| `NotUtf8` | `path: PathBuf`；文件不是合法 UTF-8 |
| `UnknownExtension` | 无字段；无法从路径推断格式 |
| `FormatNotEnabled` | `extension: String`；已知格式后端未启用 |
| `Parse` | `format: &'static str`、`line: Option<usize>`、`column: Option<usize>`；语法或结构解析失败 |
| `DepthLimitExceeded` | `limit: usize`；超过 loader 深度限制 |
| `DuplicateKey` | `key: String`；同一作用域重复键 |
| `UndefinedVariable` | `key: String`、`line: usize`；`.env` 插值变量未定义 |
| `ValueOutOfRange` | `key: String`；整数超出 `i64` 等可表示范围 |
| `TypeMismatch` | `key: String`、`expected: &'static str`；有类型反序列化类型不匹配 |
| `InvalidLimit` | 无字段；`with_max_bytes` 或 `with_max_depth` 参数越界 |

错误的 `Display` 可能包含路径、key、格式名、行列号和限制，但不会回显配置值、原始错误行、
密码、token 或整个配置树。处理错误时仍应避免把 `ConfigValue`、反序列化结果和带路径的敏感
文件名直接写入公共日志。

```rust
use axutils::{ConfigError, ConfigFormat};

let error = ConfigError::Parse {
    format: ConfigFormat::Json.as_str(),
    line: Some(1),
    column: Some(2),
};
let description = match error {
    ConfigError::Parse { format, .. } => format,
    _ => "other",
};
assert_eq!(description, "json");
```

## 使用场景、资源与限制

文件读取统一使用 1 KiB–16 MiB 的可配置上限（默认 1 MiB），通过 `take(max_bytes + 1)` 检测
超限；`.env` 解析在每次追加普通字符或插值内容前检查同一上限，所有解析后 key/value 的累计
字节数也不能超过该上限，避免短输入通过重复插值产生指数级内存占用。无类型
YAML/JSON/TOML/INI 及 YAML/INI 有类型读取受嵌套深度限制，默认 64、允许 1–256。
每个异步并发读取最多约占用 `max_bytes + 1` 字节独立缓冲区，crate 不提供全局并发或总内存
配额；调用方应限制路径来源、任务数、文件总量、解析调度和日志内容。解析阶段仍在当前 Tokio
任务同步执行，如需 CPU 隔离由调用方自行设计 `spawn_blocking` 和并发上限。

配置文件可能包含密码、token 和其他敏感值；不要直接记录 `ConfigValue`、有类型配置或原始
文本。该模块只负责读取并解析单个配置文件，不负责多文件合并、层叠覆盖、热重载、写回、
`include`/`import` 指令、表达式执行或非 `.env` 格式的插值。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
