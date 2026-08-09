# Redis 使用文档

Redis 能力由独立的 `redis` feature 提供。它使用 `redis-rs 1.5`、`r2d2 0.8`、
`rmp-serde 1.3` 和 Redis 专用的 `serde` 依赖；启用 `redis` 不会启用项目公共 `serde`
feature，因此不会额外导出配置文件能力。同步 API 只需要 `redis`；带 `_async` 后缀的
异步 API 需要同时启用 `redis,tokio`，并由调用方直接依赖 Tokio、提供 runtime。

```toml
[dependencies]
axutils = { version = "0.1", features = ["redis"] }
```

异步应用：

```toml
[dependencies]
axutils = { version = "0.1", features = ["redis", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

第一阶段只接受 `redis://` endpoint，不启用 TLS，也不接受 `rediss://`。配置、客户端和
`RedisUtils::init` 只做本地构造，不发送 PING 或建立网络连接；第一次命令调用才可能建立
连接并返回传输错误。同步命令会阻塞当前线程，Tokio 任务应使用异步方法；库不会创建
runtime、调用 `block_on` 或把同步调用转移到线程池。

## 公共导出路径

推荐从 `axutils::redis` 使用领域类型：

| 类型 | 推荐路径 | 其他公开路径 | feature |
| --- | --- | --- | --- |
| `RedisClient` | `axutils::redis::RedisClient` | `axutils::RedisClient` | `redis` |
| `RedisConfig` | `axutils::redis::RedisConfig` | `axutils::RedisConfig` | `redis` |
| `RedisError` | `axutils::redis::RedisError` | `axutils::RedisError` | `redis` |
| `RedisTransportErrorKind` | `axutils::redis::RedisTransportErrorKind` | `axutils::RedisTransportErrorKind` | `redis` |
| `RedisTransaction` | `axutils::redis::RedisTransaction` | `axutils::RedisTransaction` | `redis` |
| `RedisUtils` | `axutils::RedisUtils` | `axutils::utils::RedisUtils`、`axutils::utils::redis_utils::RedisUtils` | `redis` |

`axutils::redis::client`、`config`、`codec`、`commands`、`error` 和 `transaction` 是私有
实现模块，不是额外的公共导入路径。Redis 不导出第三方连接、Pipeline、pool、runtime 或
第三方错误类型。

## Feature 矩阵

| feature | 可用能力 |
| --- | --- |
| 无 feature | 没有 `redis` 模块、Redis 类型、`RedisUtils` 或 Redis 依赖 |
| `tokio` | 不改变 Redis API 可见性；不会单独导出 Redis |
| `redis` | 同步客户端、Cluster 普通命令、MessagePack/raw API、批量命令、事务和 `RedisUtils` |
| `redis,tokio` | 上述同步能力以及所有 `_async` 方法和异步事务 |
| `redis,serde` | Redis 能力与项目公共配置/Serde 能力同时可用，不重复导出 |
| `redis,tokio,serde` | Redis 同步/异步能力与项目公共配置/Serde 能力同时可用 |

## 资源边界和数据格式

| 配置项 | 默认值 | 可配置范围 |
| --- | ---: | ---: |
| endpoint | — | 最多 4 KiB；必须是 `redis://` 且无控制字符 |
| Cluster 初始节点 | — | 1–16 个 |
| key/field | 16 KiB | 1–16 KiB |
| 单值 raw/MessagePack 输出与输入 | 16 MiB | 1–64 MiB |
| 批量 item 数 | 1,024 | 1–16,384 |
| 批量参数字节数 | 64 MiB | 1–256 MiB |
| 多项响应字节数 | 64 MiB | 1–256 MiB |
| 集合返回项数 | 4,096 | 1–65,536 |
| 事务命令数 | 128 | 1–1,024 |
| 事务参数字节数 | 64 MiB | 1–256 MiB |
| 同步 pool 最大连接数 | 8 | 1–64 |
| 建连、checkout、响应超时 | 5 秒、5 秒、30 秒 | 1 ms–5 分钟 |

Serde 值使用受限 writer 生成紧凑 MessagePack。读取值会在反序列化前检查单值上限；多项
读取还检查累计 response 字节数和集合 item 数。由于 `redis-rs` 可能已经读入完整响应，
这些检查不是 socket 层的硬内存上限。MessagePack 数据按同一应用版本管理；结构体字段
变化可能影响兼容性，建议通过 key version/namespace 管理 schema。

`*_bytes` 是完全 raw 的 `Vec<u8>`，不做 UTF-8 转换；list/set 暂不提供 raw 变体。counter
方法操作 Redis 原生十进制整数，不兼容 `set<T: Serialize>` 生成的 MessagePack bytes。

批量 iterator 在消费时最多读取上限加一项来判定超限，超过上限或累计字节超限时不会发送
网络命令。空批量返回空结果、`0` 或 `()`，也不会访问 Redis。

## `RedisTransportErrorKind`

`RedisTransportErrorKind` 是 `Clone + Copy + Debug + Eq + Hash + PartialEq` 的
`#[non_exhaustive]` 枚举，变体如下：

- `Connection`：建连、拓扑或连接通道失败；
- `Authentication`：认证失败；
- `Timeout`：底层建连或读写响应超时；
- `Protocol`：协议解析或返回类型错误；
- `Server`：服务端命令错误；
- `Network`：底层网络 I/O 失败；
- `Other`：无法进一步分类。

它不保存 URL、认证信息、命令参数或第三方错误文本。

## `RedisError`

`RedisError` 是 `Clone + Copy + Debug + Eq + Hash + PartialEq` 的 `#[non_exhaustive]`
枚举，实现 `Display` 和 `std::error::Error`，但不提供第三方 `source()` 链。外部匹配必须
保留 `_` 分支。

| 变体 | 语义 |
| --- | --- |
| `InvalidConfig { field }` | 配置字段无效；`field` 只使用固定字段名：`url`、`scheme`、`database`、`nodes`、`credentials`、`pool_size`、`connection_timeout`、`pool_checkout_timeout`、`response_timeout`、`max_key_bytes`、`max_value_bytes`、`max_batch_items`、`max_batch_bytes`、`max_response_bytes`、`max_collection_items`、`max_transaction_commands`、`max_transaction_bytes`、`ttl` |
| `InvalidKey` / `InvalidField` | key/Hash field 为空或超过上限 |
| `ValueTooLarge { limit }` | 单值、批量参数或事务排队超过上限；`limit` 的单位按具体操作分别表示字节、批量项数或事务命令数 |
| `ResponseTooLarge { limit }` | 多项 response 累计字节超过上限 |
| `CollectionTooLarge { limit }` | 集合返回项数超过上限 |
| `Serialize` / `Deserialize` | MessagePack 编解码失败 |
| `Transport(kind)` | 底层 Redis 传输失败 |
| `Pool` | 同步 pool checkout/池状态失败 |
| `Timeout` | wrapper 自己的本地预算超时；底层读写超时使用 `Transport(Timeout)` |
| `RuntimeRequired` | 异步调用不在 Tokio runtime 中 |
| `TransactionFailed` | MULTI/EXEC 未能可靠完成 |
| `UnsupportedMode` | 当前模式不支持该操作，例如 Cluster 事务 |
| `CrossSlot` | Cluster 多 key 命令跨 hash slot |
| `NotInitialized` / `AlreadyInitialized` | `RedisUtils` 单例状态错误 |

错误的 `Display`/`Debug` 不包含 endpoint、用户名、密码、key、field、值或服务器原始文本。

## `RedisConfig`

`RedisConfig` 的字段私有，不实现 `Clone`，不提供 endpoint、数据库或凭据 getter。它实现
脱敏 `Debug`，只显示模式、节点数量、超时和限制数值。

### `RedisConfig::single`

签名：`pub fn single(url: impl Into<String>) -> Result<RedisConfig, RedisError>`。
校验 endpoint、scheme、database 和 `redis-rs` 的本地 URL 结构，不建立网络连接。单机
可以使用 database `0` 之外的非负数据库编号。

调用示例：`RedisConfig::single("redis://127.0.0.1:6379/0")`。

```rust
use axutils::RedisConfig;
let config = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
let _ = config;
```

### `RedisConfig::cluster`

签名：`pub fn cluster<I, S>(nodes: I) -> Result<RedisConfig, RedisError> where I: IntoIterator<Item = S>, S: Into<String>`。
有界消费 1–16 个初始节点；节点必须是 `redis://`、database `0`，并且 userinfo 一致。
超过 16 项时最多消费第 17 项后返回 `InvalidConfig { field: "nodes" }`，不会无界收集。

调用示例：`RedisConfig::cluster(["redis://127.0.0.1:7000/0", "redis://127.0.0.1:7001/0"])`。

```rust
use axutils::RedisConfig;
let config = RedisConfig::cluster([
    "redis://127.0.0.1:7000/0",
    "redis://127.0.0.1:7001/0",
]).unwrap();
let _ = config;
```

### `RedisConfig::with_pool_size`

签名：`pub fn with_pool_size(self, max: usize) -> Result<Self, RedisError>`。
设置同步 pool 最大连接数，范围 `1..=64`；Cluster 的底层连接数量会随节点和 backend
行为放大。

调用示例：`config.with_pool_size(16)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_pool_size(4)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_connection_timeout`

签名：`pub fn with_connection_timeout(self, timeout: Duration) -> Result<Self, RedisError>`。
设置建立网络连接的时间预算，范围为 `1 ms..=5 min`。

调用示例：`config.with_connection_timeout(std::time::Duration::from_secs(2))`。

```rust
use axutils::RedisConfig;
use std::time::Duration;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_connection_timeout(Duration::from_secs(2))
    .unwrap();
let _ = config;
```

### `RedisConfig::with_pool_checkout_timeout`

签名：`pub fn with_pool_checkout_timeout(self, timeout: Duration) -> Result<Self, RedisError>`。
设置同步 pool 等待可用连接的时间预算；它与建连超时独立。

调用示例：`config.with_pool_checkout_timeout(std::time::Duration::from_millis(500))`。

```rust
use axutils::RedisConfig;
use std::time::Duration;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_pool_checkout_timeout(Duration::from_secs(2))
    .unwrap();
let _ = config;
```

### `RedisConfig::with_response_timeout`

签名：`pub fn with_response_timeout(self, timeout: Duration) -> Result<Self, RedisError>`。
设置同步读写和异步响应时间预算，范围为 `1 ms..=5 min`。

调用示例：`config.with_response_timeout(std::time::Duration::from_secs(10))`。

```rust
use axutils::RedisConfig;
use std::time::Duration;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_response_timeout(Duration::from_secs(10))
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_key_bytes`

签名：`pub fn with_max_key_bytes(self, limit: usize) -> Result<Self, RedisError>`。
同时限制 key 和 Hash field，范围为 `1..=16 KiB`；空输入始终拒绝。

调用示例：`config.with_max_key_bytes(8 * 1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_key_bytes(1024)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_value_bytes`

签名：`pub fn with_max_value_bytes(self, limit: usize) -> Result<Self, RedisError>`。
限制 raw 值和 MessagePack 编码/解码值，范围为 `1..=64 MiB`。

调用示例：`config.with_max_value_bytes(4 * 1024 * 1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_value_bytes(1024 * 1024)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_batch_items`

签名：`pub fn with_max_batch_items(self, limit: usize) -> Result<Self, RedisError>`。
限制 `mget`、`mset`、`delete_many`、`hset_many` 的输入项数，范围为 `1..=16,384`。

调用示例：`config.with_max_batch_items(256)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_batch_items(128)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_batch_bytes`

签名：`pub fn with_max_batch_bytes(self, limit: usize) -> Result<Self, RedisError>`。
限制批量命令参数累计字节数，范围为 `1..=256 MiB`。

调用示例：`config.with_max_batch_bytes(8 * 1024 * 1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_batch_bytes(1024 * 1024)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_response_bytes`

签名：`pub fn with_max_response_bytes(self, limit: usize) -> Result<Self, RedisError>`。
限制多项读取累计响应字节数，范围为 `1..=256 MiB`。

调用示例：`config.with_max_response_bytes(8 * 1024 * 1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_response_bytes(1024 * 1024)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_collection_items`

签名：`pub fn with_max_collection_items(self, limit: usize) -> Result<Self, RedisError>`。
限制 `hgetall`、`smembers` 和 `lrange` 的返回项数，范围为 `1..=65,536`。

调用示例：`config.with_max_collection_items(1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_collection_items(256)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_transaction_commands`

签名：`pub fn with_max_transaction_commands(self, limit: usize) -> Result<Self, RedisError>`。
限制一次事务排队的命令数，范围为 `1..=1,024`。

调用示例：`config.with_max_transaction_commands(64)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_transaction_commands(64)
    .unwrap();
let _ = config;
```

### `RedisConfig::with_max_transaction_bytes`

签名：`pub fn with_max_transaction_bytes(self, limit: usize) -> Result<Self, RedisError>`。
限制事务命令 packed bytes 的累计大小，范围为 `1..=256 MiB`。

调用示例：`config.with_max_transaction_bytes(8 * 1024 * 1024)`。

```rust
use axutils::RedisConfig;

let config = RedisConfig::single("redis://127.0.0.1:6379/0")
    .unwrap()
    .with_max_transaction_bytes(1024 * 1024)
    .unwrap();
let _ = config;
```


## `RedisClient`

`RedisClient` 是可独立创建的实例级客户端，支持 `Clone + Send + Sync`。Clone 只共享同一
个 pool 和异步状态，不复制凭据，也不会预热连接。

### `RedisClient::new`

签名：`pub fn new(config: RedisConfig) -> Result<RedisClient, RedisError>`。
只构造同步 pool、Cluster backend 和异步惰性状态，不建立网络连接；池固定使用惰性连接。

调用示例：`RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?`。

一个不访问 Redis 的完整构造示例：

```rust,no_run
use axutils::{RedisClient, RedisConfig};

fn build() -> Result<RedisClient, axutils::RedisError> {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")?
        .with_pool_size(4)?
        .with_max_value_bytes(2 * 1024 * 1024)?;
    RedisClient::new(config)
}
```

```rust
use axutils::{RedisClient, RedisConfig};
let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
    .unwrap();
let _clone = client.clone();
```

### `RedisClient::get`

签名：`pub fn get<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K) -> Result<Option<T>, RedisError>`。
读取 MessagePack；Redis nil 返回 `Ok(None)`，非 nil 解码失败返回 `Deserialize`。
调用示例：`client.get::<_, MyValue>("cache:key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::get::<&str, u8>;
```

### `RedisClient::get_bytes`

签名：`pub fn get_bytes<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, RedisError>`。
读取 raw bytes，不做 UTF-8 或 MessagePack 解释。调用示例：`client.get_bytes("cache:key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::get_bytes::<&str>;
```

### `RedisClient::set`

签名：`pub fn set<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<(), RedisError>`。
以 MessagePack 写入一个值。调用示例：`client.set("cache:key", payload)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set::<&str, u8>;
```

### `RedisClient::set_bytes`

签名：`pub fn set_bytes<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), RedisError>`。
写入 raw bytes。调用示例：`client.set_bytes("cache:key", [0, 1, 2])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_bytes::<&str, Vec<u8>>;
```

### `RedisClient::set_with_expiry`

签名：`pub fn set_with_expiry<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T, ttl: Duration) -> Result<(), RedisError>`。
用一个原子 `SET ... PX` 写入 MessagePack；正 duration 向上取整到毫秒。
调用示例：`client.set_with_expiry("cache:key", payload, Duration::from_secs(30))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_with_expiry::<&str, u8>;
```

### `RedisClient::set_bytes_with_expiry`

签名：`pub fn set_bytes_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V, ttl: Duration) -> Result<(), RedisError>`。
用一个原子 `SET ... PX` 写入 raw bytes。调用示例：`client.set_bytes_with_expiry("cache:key", bytes, Duration::from_secs(30))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_bytes_with_expiry::<&str, Vec<u8>>;
```

### `RedisClient::set_nx`

签名：`pub fn set_nx<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<bool, RedisError>`。
仅 key 不存在时写入 MessagePack，`true` 表示本次写入。调用示例：`client.set_nx("lock:key", token)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_nx::<&str, u8>;
```

### `RedisClient::set_nx_with_expiry`

签名：`pub fn set_nx_with_expiry<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T, ttl: Duration) -> Result<bool, RedisError>`。
用一个 `SET ... PX NX` 命令完成 MessagePack 条件写入和 TTL。调用示例：`client.set_nx_with_expiry("lock:key", token, Duration::from_secs(5))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_nx_with_expiry::<&str, u8>;
```

### `RedisClient::set_bytes_nx`

签名：`pub fn set_bytes_nx<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<bool, RedisError>`。
raw 版本的条件写入。调用示例：`client.set_bytes_nx("lock:key", token_bytes)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_bytes_nx::<&str, Vec<u8>>;
```

### `RedisClient::set_bytes_nx_with_expiry`

签名：`pub fn set_bytes_nx_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V, ttl: Duration) -> Result<bool, RedisError>`。
raw 版本的 `SET ... PX NX`。调用示例：`client.set_bytes_nx_with_expiry("lock:key", token_bytes, Duration::from_secs(5))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
```

### `RedisClient::delete`

签名：`pub fn delete<K: AsRef<[u8]>>(&self, key: K) -> Result<u64, RedisError>`。
删除一个 key，返回实际删除数量。调用示例：`client.delete("cache:key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::delete::<&str>;
```

### `RedisClient::delete_many`

签名：`pub fn delete_many<I, K>(&self, keys: I) -> Result<u64, RedisError> where I: IntoIterator<Item = K>, K: AsRef<[u8]>`。
使用一条有界 `DEL` 命令；保留空输入和 Redis 原生计数。Cluster 中跨 slot 时返回 `CrossSlot`。
调用示例：`client.delete_many(["cache:a", "cache:b"])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::delete_many::<[&str; 1], &str>;
```

### `RedisClient::exists`

签名：`pub fn exists<K: AsRef<[u8]>>(&self, key: K) -> Result<bool, RedisError>`。
判断 key 是否存在。调用示例：`client.exists("cache:key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::exists::<&str>;
```

### `RedisClient::mget`

签名：`pub fn mget<I, K, T>(&self, keys: I) -> Result<Vec<Option<T>>, RedisError> where I: IntoIterator<Item = K>, K: AsRef<[u8]>, T: DeserializeOwned`。
按输入顺序返回 MessagePack 值和缺失项。调用示例：`client.mget::<_, _, MyValue>(["a", "b"])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::mget::<[&str; 1], &str, u8>;
```

### `RedisClient::mget_bytes`

签名：`pub fn mget_bytes<I, K>(&self, keys: I) -> Result<Vec<Option<Vec<u8>>>, RedisError> where I: IntoIterator<Item = K>, K: AsRef<[u8]>`。
按输入顺序返回 raw 值。调用示例：`client.mget_bytes(["a", "b"])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::mget_bytes::<[&str; 1], &str>;
```

### `RedisClient::mset`

签名：`pub fn mset<I, K, T>(&self, entries: I) -> Result<(), RedisError> where I: IntoIterator<Item = (K, T)>, K: AsRef<[u8]>, T: Serialize`。
使用一条有界 `MSET` 写入 MessagePack。调用示例：`client.mset([("a", value_a), ("b", value_b)])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::mset::<[(&str, u8); 1], &str, u8>;
```

### `RedisClient::mset_bytes`

签名：`pub fn mset_bytes<I, K, V>(&self, entries: I) -> Result<(), RedisError> where I: IntoIterator<Item = (K, V)>, K: AsRef<[u8]>, V: AsRef<[u8]>`。
使用一条有界 `MSET` 写入 raw 值。调用示例：`client.mset_bytes([("a", [1, 2]), ("b", [3, 4])])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
```

### `RedisClient::hget`

签名：`pub fn hget<K: AsRef<[u8]>, F: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K, field: F) -> Result<Option<T>, RedisError>`。
读取 MessagePack Hash field。调用示例：`client.hget::<_, _, MyValue>("hash", "field")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hget::<&str, &str, u8>;
```

### `RedisClient::hget_bytes`

签名：`pub fn hget_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>>(&self, key: K, field: F) -> Result<Option<Vec<u8>>, RedisError>`。
读取 raw Hash field。调用示例：`client.hget_bytes("hash", "field")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hget_bytes::<&str, &str>;
```

### `RedisClient::hset`

签名：`pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(&self, key: K, field: F, value: T) -> Result<u64, RedisError>`。
写入 MessagePack field，返回新增 field 数量。调用示例：`client.hset("hash", "field", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hset::<&str, &str, u8>;
```

### `RedisClient::hset_bytes`

签名：`pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, field: F, value: V) -> Result<u64, RedisError>`。
写入 raw field。调用示例：`client.hset_bytes("hash", "field", bytes)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hset_bytes::<&str, &str, Vec<u8>>;
```

### `RedisClient::hgetall`

签名：`pub fn hgetall<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K) -> Result<Vec<(Vec<u8>, T)>, RedisError>`。
返回拥有型 field/value 对，保留 Redis 响应顺序并执行集合/响应上限。
调用示例：`client.hgetall::<_, MyValue>("hash")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hgetall::<&str, u8>;
```

### `RedisClient::hgetall_bytes`

签名：`pub fn hgetall_bytes<K: AsRef<[u8]>>(&self, key: K) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RedisError>`。
返回 raw field/value 对。调用示例：`client.hgetall_bytes("hash")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hgetall_bytes::<&str>;
```

### `RedisClient::hdel`

签名：`pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(&self, key: K, field: F) -> Result<u64, RedisError>`。
删除一个 field 并返回实际删除数量。调用示例：`client.hdel("hash", "field")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hdel::<&str, &str>;
```

### `RedisClient::hexists`

签名：`pub fn hexists<K: AsRef<[u8]>, F: AsRef<[u8]>>(&self, key: K, field: F) -> Result<bool, RedisError>`。
判断 Hash field 是否存在。调用示例：`client.hexists("hash", "field")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hexists::<&str, &str>;
```

### `RedisClient::hlen`

签名：`pub fn hlen<K: AsRef<[u8]>>(&self, key: K) -> Result<u64, RedisError>`。
返回 Hash field 数量。调用示例：`client.hlen("hash")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hlen::<&str>;
```

### `RedisClient::hset_many`

签名：`pub fn hset_many<I, K, F, T>(&self, key: K, entries: I) -> Result<u64, RedisError> where I: IntoIterator<Item = (F, T)>, K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize`。
使用一条有界 `HSET` 写入多个 MessagePack field。调用示例：`client.hset_many("hash", [("a", a), ("b", b)])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hset_many::<[(&str, u8); 1], &str, &str, u8>;
```

### `RedisClient::hset_many_bytes`

签名：`pub fn hset_many_bytes<I, K, F, V>(&self, key: K, entries: I) -> Result<u64, RedisError> where I: IntoIterator<Item = (F, V)>, K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>`。
使用一条有界 `HSET` 写入多个 raw field。调用示例：`client.hset_many_bytes("hash", [("a", [1]), ("b", [2])])`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
```

### `RedisClient::expire`

签名：`pub fn expire<K: AsRef<[u8]>>(&self, key: K, ttl: Duration) -> Result<bool, RedisError>`。
以秒为单位设置 TTL；正但不足一秒的 duration 向上取 1 秒。调用示例：`client.expire("key", Duration::from_secs(30))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::expire::<&str>;
```

### `RedisClient::pexpire`

签名：`pub fn pexpire<K: AsRef<[u8]>>(&self, key: K, ttl: Duration) -> Result<bool, RedisError>`。
以毫秒为单位设置 TTL；正但不足一毫秒的 duration 向上取 1 ms。调用示例：`client.pexpire("key", Duration::from_millis(500))`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::pexpire::<&str>;
```

### `RedisClient::persist`

签名：`pub fn persist<K: AsRef<[u8]>>(&self, key: K) -> Result<bool, RedisError>`。
删除过期时间并返回 Redis 原生是否生效。调用示例：`client.persist("key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::persist::<&str>;
```

### `RedisClient::ttl`

签名：`pub fn ttl<K: AsRef<[u8]>>(&self, key: K) -> Result<i64, RedisError>`。
返回 Redis 原生秒数：`-1` 表示无过期，`-2` 表示 key 不存在。调用示例：`client.ttl("key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::ttl::<&str>;
```

### `RedisClient::pttl`

签名：`pub fn pttl<K: AsRef<[u8]>>(&self, key: K) -> Result<i64, RedisError>`。
返回 Redis 原生毫秒数并保留 `-1`/`-2`。调用示例：`client.pttl("key")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::pttl::<&str>;
```

### `RedisClient::incr`

签名：`pub fn incr<K: AsRef<[u8]>>(&self, key: K) -> Result<i64, RedisError>`。
对 Redis 原生整数加一；不兼容 MessagePack 值。调用示例：`client.incr("counter")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::incr::<&str>;
```

### `RedisClient::incr_by`

签名：`pub fn incr_by<K: AsRef<[u8]>>(&self, key: K, amount: i64) -> Result<i64, RedisError>`。
对 Redis 原生整数增加指定值。调用示例：`client.incr_by("counter", 10)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::incr_by::<&str>;
```

### `RedisClient::decr`

签名：`pub fn decr<K: AsRef<[u8]>>(&self, key: K) -> Result<i64, RedisError>`。
对 Redis 原生整数减一。调用示例：`client.decr("counter")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::decr::<&str>;
```

### `RedisClient::decr_by`

签名：`pub fn decr_by<K: AsRef<[u8]>>(&self, key: K, amount: i64) -> Result<i64, RedisError>`。
对 Redis 原生整数减少指定值。调用示例：`client.decr_by("counter", 10)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::decr_by::<&str>;
```

### `RedisClient::lpush`

签名：`pub fn lpush<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<u64, RedisError>`。
从列表左侧压入并返回列表长度。调用示例：`client.lpush("list", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::lpush::<&str, u8>;
```

### `RedisClient::rpush`

签名：`pub fn rpush<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<u64, RedisError>`。
从列表右侧压入并返回列表长度。调用示例：`client.rpush("list", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::rpush::<&str, u8>;
```

### `RedisClient::lpop`

签名：`pub fn lpop<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K) -> Result<Option<T>, RedisError>`。
从左侧弹出；空列表返回 `Ok(None)`。调用示例：`client.lpop::<_, MyValue>("list")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::lpop::<&str, u8>;
```

### `RedisClient::rpop`

签名：`pub fn rpop<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K) -> Result<Option<T>, RedisError>`。
从右侧弹出；空列表返回 `Ok(None)`。调用示例：`client.rpop::<_, MyValue>("list")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::rpop::<&str, u8>;
```

### `RedisClient::lrange`

签名：`pub fn lrange<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K, start: isize, stop: isize) -> Result<Vec<T>, RedisError>`。
读取列表范围；可计算的非负范围会在发送前受 `max_collection_items` 限制。调用示例：
`client.lrange::<_, MyValue>("list", 0, 99)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::lrange::<&str, u8>;
```

### `RedisClient::sadd`

签名：`pub fn sadd<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<u64, RedisError>`。
加入 MessagePack 成员并返回新增数量。调用示例：`client.sadd("set", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::sadd::<&str, u8>;
```

### `RedisClient::srem`

签名：`pub fn srem<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<u64, RedisError>`。
移除 MessagePack 成员并返回实际移除数量。调用示例：`client.srem("set", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::srem::<&str, u8>;
```

### `RedisClient::sismember`

签名：`pub fn sismember<K: AsRef<[u8]>, T: Serialize>(&self, key: K, value: T) -> Result<bool, RedisError>`。
判断 MessagePack 成员是否存在。调用示例：`client.sismember("set", value)`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::sismember::<&str, u8>;
```

### `RedisClient::smembers`

签名：`pub fn smembers<K: AsRef<[u8]>, T: DeserializeOwned>(&self, key: K) -> Result<Vec<T>, RedisError>`。
读取全部 MessagePack 成员；顺序不保证，并受 item/response 限制。调用示例：`client.smembers::<_, MyValue>("set")`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::smembers::<&str, u8>;
```

### `RedisClient::ping`

签名：`pub fn ping(&self) -> Result<String, RedisError>`。发送 Redis `PING` 并返回服务端
响应。调用示例：`client.ping()`。

```rust,no_run
use axutils::RedisClient;

let _ = RedisClient::ping;
```


## 异步 `RedisClient` 方法

以下每个方法只在 `all(feature = "redis", feature = "tokio")` 下导出，命名是对应同步
方法加 `_async`。参数在第一次网络 await 前完成 key/field 拥有化和 MessagePack 编码；
这些方法不创建 runtime。每个方法的返回语义与同名同步方法相同：

| 方法 | 调用示例 |
| --- | --- |
| `get_async<K, T>` | `client.get_async::<_, MyValue>("key").await` |
| `get_bytes_async<K>` | `client.get_bytes_async("key").await` |
| `set_async<K, T>` | `client.set_async("key", value).await` |
| `set_bytes_async<K, V>` | `client.set_bytes_async("key", bytes).await` |
| `set_with_expiry_async<K, T>` | `client.set_with_expiry_async("key", value, ttl).await` |
| `set_bytes_with_expiry_async<K, V>` | `client.set_bytes_with_expiry_async("key", bytes, ttl).await` |
| `set_nx_async<K, T>` | `client.set_nx_async("key", value).await` |
| `set_nx_with_expiry_async<K, T>` | `client.set_nx_with_expiry_async("key", value, ttl).await` |
| `set_bytes_nx_async<K, V>` | `client.set_bytes_nx_async("key", bytes).await` |
| `set_bytes_nx_with_expiry_async<K, V>` | `client.set_bytes_nx_with_expiry_async("key", bytes, ttl).await` |
| `delete_async<K>` | `client.delete_async("key").await` |
| `delete_many_async<I, K>` | `client.delete_many_async(["a", "b"]).await` |
| `exists_async<K>` | `client.exists_async("key").await` |
| `mget_async<I, K, T>` | `client.mget_async::<_, _, MyValue>(["a", "b"]).await` |
| `mget_bytes_async<I, K>` | `client.mget_bytes_async(["a", "b"]).await` |
| `mset_async<I, K, T>` | `client.mset_async([("a", value)]).await` |
| `mset_bytes_async<I, K, V>` | `client.mset_bytes_async([("a", bytes)]).await` |
| `hget_async<K, F, T>` | `client.hget_async::<_, _, MyValue>("hash", "field").await` |
| `hget_bytes_async<K, F>` | `client.hget_bytes_async("hash", "field").await` |
| `hset_async<K, F, T>` | `client.hset_async("hash", "field", value).await` |
| `hset_bytes_async<K, F, V>` | `client.hset_bytes_async("hash", "field", bytes).await` |
| `hgetall_async<K, T>` | `client.hgetall_async::<_, MyValue>("hash").await` |
| `hgetall_bytes_async<K>` | `client.hgetall_bytes_async("hash").await` |
| `hdel_async<K, F>` | `client.hdel_async("hash", "field").await` |
| `hexists_async<K, F>` | `client.hexists_async("hash", "field").await` |
| `hlen_async<K>` | `client.hlen_async("hash").await` |
| `hset_many_async<I, K, F, T>` | `client.hset_many_async("hash", [("a", value)]).await` |
| `hset_many_bytes_async<I, K, F, V>` | `client.hset_many_bytes_async("hash", [("a", bytes)]).await` |
| `expire_async<K>` | `client.expire_async("key", ttl).await` |
| `pexpire_async<K>` | `client.pexpire_async("key", ttl).await` |
| `persist_async<K>` | `client.persist_async("key").await` |
| `ttl_async<K>` | `client.ttl_async("key").await` |
| `pttl_async<K>` | `client.pttl_async("key").await` |
| `incr_async<K>` / `decr_async<K>` | `client.incr_async("counter").await` / `client.decr_async("counter").await` |
| `incr_by_async<K>` / `decr_by_async<K>` | `client.incr_by_async("counter", 2).await` / `client.decr_by_async("counter", 2).await` |
| `lpush_async<K, T>` / `rpush_async<K, T>` | `client.lpush_async("list", value).await` / `client.rpush_async("list", value).await` |
| `lpop_async<K, T>` / `rpop_async<K, T>` | `client.lpop_async::<_, MyValue>("list").await` / `client.rpop_async::<_, MyValue>("list").await` |
| `lrange_async<K, T>` | `client.lrange_async::<_, MyValue>("list", 0, 99).await` |
| `sadd_async<K, T>` / `srem_async<K, T>` | `client.sadd_async("set", value).await` / `client.srem_async("set", value).await` |
| `sismember_async<K, T>` | `client.sismember_async("set", value).await` |
| `smembers_async<K, T>` | `client.smembers_async::<_, MyValue>("set").await` |
| `ping_async` | `client.ping_async().await` |

为便于逐项查阅，以下列出每个异步方法的独立调用入口；返回值、错误和资源边界分别与同名
同步方法一致：

### `RedisClient::get_async`

调用示例：`client.get_async::<_, MyValue>("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::get_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::get_bytes_async`

调用示例：`client.get_bytes_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::get_bytes_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_async`

调用示例：`client.set_async("key", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_bytes_async`

调用示例：`client.set_bytes_async("key", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_bytes_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_with_expiry_async`

调用示例：`client.set_with_expiry_async("key", value, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_with_expiry_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_bytes_with_expiry_async`

调用示例：`client.set_bytes_with_expiry_async("key", bytes, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_bytes_with_expiry_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_nx_async`

调用示例：`client.set_nx_async("key", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_nx_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_nx_with_expiry_async`

调用示例：`client.set_nx_with_expiry_async("key", value, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_nx_with_expiry_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_bytes_nx_async`

调用示例：`client.set_bytes_nx_async("key", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_bytes_nx_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::set_bytes_nx_with_expiry_async`

调用示例：`client.set_bytes_nx_with_expiry_async("key", bytes, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::delete_async`

调用示例：`client.delete_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::delete_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::delete_many_async`

调用示例：`client.delete_many_async(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::delete_many_async::<[&str; 1], &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::exists_async`

调用示例：`client.exists_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::exists_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::mget_async`

调用示例：`client.mget_async::<_, _, MyValue>(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::mget_async::<[&str; 1], &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::mget_bytes_async`

调用示例：`client.mget_bytes_async(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::mget_bytes_async::<[&str; 1], &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::mset_async`

调用示例：`client.mset_async([("a", value)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::mset_async::<[(&str, u8); 1], &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::mset_bytes_async`

调用示例：`client.mset_bytes_async([("a", bytes)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hget_async`

调用示例：`client.hget_async::<_, _, MyValue>("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hget_async::<&str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hget_bytes_async`

调用示例：`client.hget_bytes_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hget_bytes_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hset_async`

调用示例：`client.hset_async("hash", "field", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hset_async::<&str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hset_bytes_async`

调用示例：`client.hset_bytes_async("hash", "field", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hset_bytes_async::<&str, &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hgetall_async`

调用示例：`client.hgetall_async::<_, MyValue>("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hgetall_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hgetall_bytes_async`

调用示例：`client.hgetall_bytes_async("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hgetall_bytes_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hdel_async`

调用示例：`client.hdel_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hdel_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hexists_async`

调用示例：`client.hexists_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hexists_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hlen_async`

调用示例：`client.hlen_async("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hlen_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hset_many_async`

调用示例：`client.hset_many_async("hash", [("a", value)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::hset_many_bytes_async`

调用示例：`client.hset_many_bytes_async("hash", [("a", bytes)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::expire_async`

调用示例：`client.expire_async("key", ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::expire_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::pexpire_async`

调用示例：`client.pexpire_async("key", ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::pexpire_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::persist_async`

调用示例：`client.persist_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::persist_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::ttl_async`

调用示例：`client.ttl_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::ttl_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::pttl_async`

调用示例：`client.pttl_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::pttl_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::incr_async`

调用示例：`client.incr_async("counter").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::incr_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::incr_by_async`

调用示例：`client.incr_by_async("counter", 2).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::incr_by_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::decr_async`

调用示例：`client.decr_async("counter").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::decr_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::decr_by_async`

调用示例：`client.decr_by_async("counter", 2).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::decr_by_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::lpush_async`

调用示例：`client.lpush_async("list", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::lpush_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::rpush_async`

调用示例：`client.rpush_async("list", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::rpush_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::lpop_async`

调用示例：`client.lpop_async::<_, MyValue>("list").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::lpop_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::rpop_async`

调用示例：`client.rpop_async::<_, MyValue>("list").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::rpop_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::lrange_async`

调用示例：`client.lrange_async::<_, MyValue>("list", 0, 99).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::lrange_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::sadd_async`

调用示例：`client.sadd_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::sadd_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::srem_async`

调用示例：`client.srem_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::srem_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::sismember_async`

调用示例：`client.sismember_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::sismember_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::smembers_async`

调用示例：`client.smembers_async::<_, MyValue>("set").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::smembers_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisClient::ping_async`

调用示例：`client.ping_async().await`。

异步客户端实现使用单机 `ConnectionManager` 或惰性 Cluster connection；普通命令可并发
复用。单机事务另有独立 `MultiplexedConnection` 和串行锁，普通命令不会插入事务序列。若
不在 Tokio runtime 中调用，返回 `RedisError::RuntimeRequired`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisClient;

let _ = RedisClient::ping_async;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```


## 事务

### `RedisClient::transaction`

签名：`pub fn transaction<F>(&self, callback: F) -> Result<(), RedisError> where F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>`。
callback 只同步排队命令；排队错误会在网络操作前返回，不发送部分命令；空事务返回 `Ok(())`
且不访问 Redis。成功执行一个 `MULTI`/`EXEC` atomic pipeline。Cluster 模式返回
`UnsupportedMode`，不执行伪事务；不提供读取、WATCH、CAS、自动重试或 callback 重放。

调用示例：`client.transaction(|tx| { tx.set("key", value)?; tx.expire("key", ttl) })`。

```rust,no_run
use axutils::{RedisClient, RedisError, RedisTransaction};

let _ = RedisClient::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
```

### `RedisClient::transaction_async`

签名：`pub async fn transaction_async<F>(&self, callback: F) -> Result<(), RedisError> where F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>`。
callback 仍是一次性的同步闭包，不接受 async callback；网络执行发生在返回 future 中。
取消或 callback panic 会使 future 无法归还专用连接；协议、网络、连接、超时等状态不确定的
执行失败也会丢弃连接。已完整读取的普通 Redis 服务端命令错误会保留健康连接；不会污染普通
`ConnectionManager`，也不会重放 callback。

调用示例：`client.transaction_async(|tx| { tx.set("key", value)?; tx.persist("key") }).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::{RedisClient, RedisError, RedisTransaction};

let _ = RedisClient::transaction_async::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisTransaction::set`

签名：`pub fn set<K: AsRef<[u8]>, T: Serialize>(&mut self, key: K, value: T) -> Result<(), RedisError>`。
排队 MessagePack `SET`。调用示例：`tx.set("key", value)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::set::<&str, u8>;
```

### `RedisTransaction::set_with_expiry`

签名：`pub fn set_with_expiry<K: AsRef<[u8]>, T: Serialize>(&mut self, key: K, value: T, ttl: Duration) -> Result<(), RedisError>`。
排队原子 `SET ... PX`。调用示例：`tx.set_with_expiry("key", value, ttl)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::set_with_expiry::<&str, u8>;
```

### `RedisTransaction::set_bytes`

签名：`pub fn set_bytes<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) -> Result<(), RedisError>`。
排队 raw `SET`。调用示例：`tx.set_bytes("key", bytes)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::set_bytes::<&str, Vec<u8>>;
```

### `RedisTransaction::set_bytes_with_expiry`

签名：`pub fn set_bytes_with_expiry<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V, ttl: Duration) -> Result<(), RedisError>`。
排队 raw `SET ... PX`。调用示例：`tx.set_bytes_with_expiry("key", bytes, ttl)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::set_bytes_with_expiry::<&str, Vec<u8>>;
```

### `RedisTransaction::delete`

签名：`pub fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), RedisError>`。
排队单 key `DEL`。调用示例：`tx.delete("key")`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::delete::<&str>;
```

### `RedisTransaction::hset`

签名：`pub fn hset<K: AsRef<[u8]>, F: AsRef<[u8]>, T: Serialize>(&mut self, key: K, field: F, value: T) -> Result<(), RedisError>`。
排队 MessagePack `HSET`。调用示例：`tx.hset("hash", "field", value)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::hset::<&str, &str, u8>;
```

### `RedisTransaction::hset_bytes`

签名：`pub fn hset_bytes<K: AsRef<[u8]>, F: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, field: F, value: V) -> Result<(), RedisError>`。
排队 raw `HSET`。调用示例：`tx.hset_bytes("hash", "field", bytes)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::hset_bytes::<&str, &str, Vec<u8>>;
```

### `RedisTransaction::hdel`

签名：`pub fn hdel<K: AsRef<[u8]>, F: AsRef<[u8]>>(&mut self, key: K, field: F) -> Result<(), RedisError>`。
排队 `HDEL`。调用示例：`tx.hdel("hash", "field")`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::hdel::<&str, &str>;
```

### `RedisTransaction::expire`

签名：`pub fn expire<K: AsRef<[u8]>>(&mut self, key: K, ttl: Duration) -> Result<(), RedisError>`。
排队秒级 `EXPIRE`，正 duration 向上取整到秒。调用示例：`tx.expire("key", ttl)`。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::expire::<&str>;
```

### `RedisTransaction::persist`

签名：`pub fn persist<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), RedisError>`。
排队 `PERSIST`。调用示例：`tx.persist("key")`。

事务排队同时受命令数量和 packed bytes 上限限制；内部不保存连接，也不暴露命令集合。

```rust
use axutils::RedisTransaction;

let _ = RedisTransaction::persist::<&str>;
```


## `RedisUtils`

`RedisUtils` 使用 `OnceLock<RedisClient>`。`init` 只有在客户端本地构造成功后才占用单例；
竞争初始化时仅一个调用成功，之后返回 `AlreadyInitialized`。未初始化调用返回
`NotInitialized`。没有 `client()`、配置/凭据 getter、reset 或 replace 公共 API。

### `RedisUtils::init`

签名：`pub fn init(config: RedisConfig) -> Result<(), RedisError>`。
调用示例：`RedisUtils::init(RedisConfig::single("redis://127.0.0.1:6379/0")?)?`。

```rust,no_run
use axutils::{RedisConfig, RedisUtils};

let config = RedisConfig::single("redis://127.0.0.1:6379/0")?;
RedisUtils::init(config)?;
# Ok::<(), axutils::RedisError>(())
```

### `RedisUtils::is_initialized`

签名：`pub fn is_initialized() -> bool`。调用示例：`RedisUtils::is_initialized()`。

```rust
use axutils::RedisUtils;

let _ = RedisUtils::is_initialized();
```

### `RedisUtils` 同步转发方法

以下方法与 `RedisClient` 使用完全相同的参数、返回值和限制；唯一差异是未初始化时先返
回 `NotInitialized`。每个调用示例均为静态方法：

| 方法 | 调用示例 |
| --- | --- |
| `get` / `get_bytes` | `RedisUtils::get::<_, MyValue>("key")` / `RedisUtils::get_bytes("key")` |
| `set` / `set_bytes` | `RedisUtils::set("key", value)` / `RedisUtils::set_bytes("key", bytes)` |
| `set_with_expiry` / `set_bytes_with_expiry` | `RedisUtils::set_with_expiry("key", value, ttl)` / `RedisUtils::set_bytes_with_expiry("key", bytes, ttl)` |
| `set_nx` / `set_nx_with_expiry` | `RedisUtils::set_nx("key", value)` / `RedisUtils::set_nx_with_expiry("key", value, ttl)` |
| `set_bytes_nx` / `set_bytes_nx_with_expiry` | `RedisUtils::set_bytes_nx("key", bytes)` / `RedisUtils::set_bytes_nx_with_expiry("key", bytes, ttl)` |
| `delete` / `delete_many` | `RedisUtils::delete("key")` / `RedisUtils::delete_many(["a", "b"])` |
| `exists` | `RedisUtils::exists("key")` |
| `mget` / `mget_bytes` | `RedisUtils::mget::<_, _, MyValue>(["a", "b"])` / `RedisUtils::mget_bytes(["a", "b"])` |
| `mset` / `mset_bytes` | `RedisUtils::mset([("a", value)])` / `RedisUtils::mset_bytes([("a", bytes)])` |
| `hget` / `hget_bytes` | `RedisUtils::hget::<_, _, MyValue>("hash", "field")` / `RedisUtils::hget_bytes("hash", "field")` |
| `hset` / `hset_bytes` | `RedisUtils::hset("hash", "field", value)` / `RedisUtils::hset_bytes("hash", "field", bytes)` |
| `hgetall` / `hgetall_bytes` | `RedisUtils::hgetall::<_, MyValue>("hash")` / `RedisUtils::hgetall_bytes("hash")` |
| `hdel` / `hexists` / `hlen` | `RedisUtils::hdel("hash", "field")` / `RedisUtils::hexists("hash", "field")` / `RedisUtils::hlen("hash")` |
| `hset_many` / `hset_many_bytes` | `RedisUtils::hset_many("hash", [("a", value)])` / `RedisUtils::hset_many_bytes("hash", [("a", bytes)])` |
| `expire` / `pexpire` / `persist` | `RedisUtils::expire("key", ttl)` / `RedisUtils::pexpire("key", ttl)` / `RedisUtils::persist("key")` |
| `ttl` / `pttl` | `RedisUtils::ttl("key")` / `RedisUtils::pttl("key")` |
| `incr` / `incr_by` / `decr` / `decr_by` | `RedisUtils::incr("counter")` / `RedisUtils::incr_by("counter", 2)` / `RedisUtils::decr("counter")` / `RedisUtils::decr_by("counter", 2)` |
| `lpush` / `rpush` | `RedisUtils::lpush("list", value)` / `RedisUtils::rpush("list", value)` |
| `lpop` / `rpop` / `lrange` | `RedisUtils::lpop::<_, MyValue>("list")` / `RedisUtils::rpop::<_, MyValue>("list")` / `RedisUtils::lrange::<_, MyValue>("list", 0, 9)` |
| `sadd` / `srem` / `sismember` / `smembers` | `RedisUtils::sadd("set", value)` / `RedisUtils::srem("set", value)` / `RedisUtils::sismember("set", value)` / `RedisUtils::smembers::<_, MyValue>("set")` |
| `ping` | `RedisUtils::ping()` |
| `transaction` | `RedisUtils::transaction(|tx| tx.set("key", value))` |

以下是上述转发表中每个同步方法的独立条目：

### `RedisUtils::get`

调用示例：`RedisUtils::get::<_, MyValue>("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::get::<&str, u8>;
```

### `RedisUtils::get_bytes`

调用示例：`RedisUtils::get_bytes("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::get_bytes::<&str>;
```

### `RedisUtils::set`

调用示例：`RedisUtils::set("key", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set::<&str, u8>;
```

### `RedisUtils::set_bytes`

调用示例：`RedisUtils::set_bytes("key", bytes)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes::<&str, Vec<u8>>;
```

### `RedisUtils::set_with_expiry`

调用示例：`RedisUtils::set_with_expiry("key", value, ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_with_expiry::<&str, u8>;
```

### `RedisUtils::set_bytes_with_expiry`

调用示例：`RedisUtils::set_bytes_with_expiry("key", bytes, ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_with_expiry::<&str, Vec<u8>>;
```

### `RedisUtils::set_nx`

调用示例：`RedisUtils::set_nx("key", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_nx::<&str, u8>;
```

### `RedisUtils::set_nx_with_expiry`

调用示例：`RedisUtils::set_nx_with_expiry("key", value, ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_nx_with_expiry::<&str, u8>;
```

### `RedisUtils::set_bytes_nx`

调用示例：`RedisUtils::set_bytes_nx("key", bytes)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_nx::<&str, Vec<u8>>;
```

### `RedisUtils::set_bytes_nx_with_expiry`

调用示例：`RedisUtils::set_bytes_nx_with_expiry("key", bytes, ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_nx_with_expiry::<&str, Vec<u8>>;
```

### `RedisUtils::delete`

调用示例：`RedisUtils::delete("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::delete::<&str>;
```

### `RedisUtils::delete_many`

调用示例：`RedisUtils::delete_many(["a", "b"])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::delete_many::<[&str; 1], &str>;
```

### `RedisUtils::exists`

调用示例：`RedisUtils::exists("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::exists::<&str>;
```

### `RedisUtils::mget`

调用示例：`RedisUtils::mget::<_, _, MyValue>(["a", "b"])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::mget::<[&str; 1], &str, u8>;
```

### `RedisUtils::mget_bytes`

调用示例：`RedisUtils::mget_bytes(["a", "b"])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::mget_bytes::<[&str; 1], &str>;
```

### `RedisUtils::mset`

调用示例：`RedisUtils::mset([("a", value)])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::mset::<[(&str, u8); 1], &str, u8>;
```

### `RedisUtils::mset_bytes`

调用示例：`RedisUtils::mset_bytes([("a", bytes)])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::mset_bytes::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
```

### `RedisUtils::hget`

调用示例：`RedisUtils::hget::<_, _, MyValue>("hash", "field")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hget::<&str, &str, u8>;
```

### `RedisUtils::hget_bytes`

调用示例：`RedisUtils::hget_bytes("hash", "field")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hget_bytes::<&str, &str>;
```

### `RedisUtils::hset`

调用示例：`RedisUtils::hset("hash", "field", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hset::<&str, &str, u8>;
```

### `RedisUtils::hset_bytes`

调用示例：`RedisUtils::hset_bytes("hash", "field", bytes)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hset_bytes::<&str, &str, Vec<u8>>;
```

### `RedisUtils::hgetall`

调用示例：`RedisUtils::hgetall::<_, MyValue>("hash")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hgetall::<&str, u8>;
```

### `RedisUtils::hgetall_bytes`

调用示例：`RedisUtils::hgetall_bytes("hash")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hgetall_bytes::<&str>;
```

### `RedisUtils::hdel`

调用示例：`RedisUtils::hdel("hash", "field")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hdel::<&str, &str>;
```

### `RedisUtils::hexists`

调用示例：`RedisUtils::hexists("hash", "field")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hexists::<&str, &str>;
```

### `RedisUtils::hlen`

调用示例：`RedisUtils::hlen("hash")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hlen::<&str>;
```

### `RedisUtils::hset_many`

调用示例：`RedisUtils::hset_many("hash", [("a", value)])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hset_many::<[(&str, u8); 1], &str, &str, u8>;
```

### `RedisUtils::hset_many_bytes`

调用示例：`RedisUtils::hset_many_bytes("hash", [("a", bytes)])`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::hset_many_bytes::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
```

### `RedisUtils::expire`

调用示例：`RedisUtils::expire("key", ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::expire::<&str>;
```

### `RedisUtils::pexpire`

调用示例：`RedisUtils::pexpire("key", ttl)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::pexpire::<&str>;
```

### `RedisUtils::persist`

调用示例：`RedisUtils::persist("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::persist::<&str>;
```

### `RedisUtils::ttl`

调用示例：`RedisUtils::ttl("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::ttl::<&str>;
```

### `RedisUtils::pttl`

调用示例：`RedisUtils::pttl("key")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::pttl::<&str>;
```

### `RedisUtils::incr`

调用示例：`RedisUtils::incr("counter")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::incr::<&str>;
```

### `RedisUtils::incr_by`

调用示例：`RedisUtils::incr_by("counter", 2)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::incr_by::<&str>;
```

### `RedisUtils::decr`

调用示例：`RedisUtils::decr("counter")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::decr::<&str>;
```

### `RedisUtils::decr_by`

调用示例：`RedisUtils::decr_by("counter", 2)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::decr_by::<&str>;
```

### `RedisUtils::lpush`

调用示例：`RedisUtils::lpush("list", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::lpush::<&str, u8>;
```

### `RedisUtils::rpush`

调用示例：`RedisUtils::rpush("list", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::rpush::<&str, u8>;
```

### `RedisUtils::lpop`

调用示例：`RedisUtils::lpop::<_, MyValue>("list")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::lpop::<&str, u8>;
```

### `RedisUtils::rpop`

调用示例：`RedisUtils::rpop::<_, MyValue>("list")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::rpop::<&str, u8>;
```

### `RedisUtils::lrange`

调用示例：`RedisUtils::lrange::<_, MyValue>("list", 0, 9)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::lrange::<&str, u8>;
```

### `RedisUtils::sadd`

调用示例：`RedisUtils::sadd("set", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::sadd::<&str, u8>;
```

### `RedisUtils::srem`

调用示例：`RedisUtils::srem("set", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::srem::<&str, u8>;
```

### `RedisUtils::sismember`

调用示例：`RedisUtils::sismember("set", value)`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::sismember::<&str, u8>;
```

### `RedisUtils::smembers`

调用示例：`RedisUtils::smembers::<_, MyValue>("set")`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::smembers::<&str, u8>;
```

### `RedisUtils::ping`

调用示例：`RedisUtils::ping()`。

```rust,no_run
use axutils::RedisUtils;

let _ = RedisUtils::ping;
```

### `RedisUtils::transaction`

调用示例：`RedisUtils::transaction(|tx| tx.set("key", value))`。

```rust,no_run
use axutils::{RedisError, RedisTransaction, RedisUtils};

let _ = RedisUtils::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
```

### `RedisUtils` 异步转发方法

`redis,tokio` 下追加与 `RedisClient` 异步表同名的静态 `_async` 方法，例如
`RedisUtils::get_async::<_, MyValue>("key").await`、`RedisUtils::set_async("key", value).await`、
`RedisUtils::hgetall_async::<_, MyValue>("hash").await`、`RedisUtils::ping_async().await` 和
`RedisUtils::transaction_async(|tx| tx.set("key", value)).await`。其余
`get_bytes_async`、`set_bytes_async`、两组 expiry/NX、delete/mget/mset、全部 Hash、TTL、
counter、list/set 方法分别与前述异步 `RedisClient` 方法一一对应，参数/返回/边界不变。

### `RedisUtils::get_async`

调用示例：`RedisUtils::get_async::<_, MyValue>("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::get_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::get_bytes_async`

调用示例：`RedisUtils::get_bytes_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::get_bytes_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_async`

调用示例：`RedisUtils::set_async("key", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_bytes_async`

调用示例：`RedisUtils::set_bytes_async("key", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_with_expiry_async`

调用示例：`RedisUtils::set_with_expiry_async("key", value, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_with_expiry_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_bytes_with_expiry_async`

调用示例：`RedisUtils::set_bytes_with_expiry_async("key", bytes, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_with_expiry_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_nx_async`

调用示例：`RedisUtils::set_nx_async("key", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_nx_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_nx_with_expiry_async`

调用示例：`RedisUtils::set_nx_with_expiry_async("key", value, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_nx_with_expiry_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_bytes_nx_async`

调用示例：`RedisUtils::set_bytes_nx_async("key", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_nx_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::set_bytes_nx_with_expiry_async`

调用示例：`RedisUtils::set_bytes_nx_with_expiry_async("key", bytes, ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::set_bytes_nx_with_expiry_async::<&str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::delete_async`

调用示例：`RedisUtils::delete_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::delete_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::delete_many_async`

调用示例：`RedisUtils::delete_many_async(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::delete_many_async::<[&str; 1], &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::exists_async`

调用示例：`RedisUtils::exists_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::exists_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::mget_async`

调用示例：`RedisUtils::mget_async::<_, _, MyValue>(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::mget_async::<[&str; 1], &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::mget_bytes_async`

调用示例：`RedisUtils::mget_bytes_async(["a", "b"]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::mget_bytes_async::<[&str; 1], &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::mset_async`

调用示例：`RedisUtils::mset_async([("a", value)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::mset_async::<[(&str, u8); 1], &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::mset_bytes_async`

调用示例：`RedisUtils::mset_bytes_async([("a", bytes)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::mset_bytes_async::<[(&str, Vec<u8>); 1], &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hget_async`

调用示例：`RedisUtils::hget_async::<_, _, MyValue>("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hget_async::<&str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hget_bytes_async`

调用示例：`RedisUtils::hget_bytes_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hget_bytes_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hset_async`

调用示例：`RedisUtils::hset_async("hash", "field", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hset_async::<&str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hset_bytes_async`

调用示例：`RedisUtils::hset_bytes_async("hash", "field", bytes).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hset_bytes_async::<&str, &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hgetall_async`

调用示例：`RedisUtils::hgetall_async::<_, MyValue>("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hgetall_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hgetall_bytes_async`

调用示例：`RedisUtils::hgetall_bytes_async("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hgetall_bytes_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hdel_async`

调用示例：`RedisUtils::hdel_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hdel_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hexists_async`

调用示例：`RedisUtils::hexists_async("hash", "field").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hexists_async::<&str, &str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hlen_async`

调用示例：`RedisUtils::hlen_async("hash").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hlen_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hset_many_async`

调用示例：`RedisUtils::hset_many_async("hash", [("a", value)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hset_many_async::<[(&str, u8); 1], &str, &str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::hset_many_bytes_async`

调用示例：`RedisUtils::hset_many_bytes_async("hash", [("a", bytes)]).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::hset_many_bytes_async::<[(&str, Vec<u8>); 1], &str, &str, Vec<u8>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::expire_async`

调用示例：`RedisUtils::expire_async("key", ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::expire_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::pexpire_async`

调用示例：`RedisUtils::pexpire_async("key", ttl).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::pexpire_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::persist_async`

调用示例：`RedisUtils::persist_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::persist_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::ttl_async`

调用示例：`RedisUtils::ttl_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::ttl_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::pttl_async`

调用示例：`RedisUtils::pttl_async("key").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::pttl_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::incr_async`

调用示例：`RedisUtils::incr_async("counter").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::incr_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::incr_by_async`

调用示例：`RedisUtils::incr_by_async("counter", 2).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::incr_by_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::decr_async`

调用示例：`RedisUtils::decr_async("counter").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::decr_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::decr_by_async`

调用示例：`RedisUtils::decr_by_async("counter", 2).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::decr_by_async::<&str>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::lpush_async`

调用示例：`RedisUtils::lpush_async("list", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::lpush_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::rpush_async`

调用示例：`RedisUtils::rpush_async("list", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::rpush_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::lpop_async`

调用示例：`RedisUtils::lpop_async::<_, MyValue>("list").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::lpop_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::rpop_async`

调用示例：`RedisUtils::rpop_async::<_, MyValue>("list").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::rpop_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::lrange_async`

调用示例：`RedisUtils::lrange_async::<_, MyValue>("list", 0, 9).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::lrange_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::sadd_async`

调用示例：`RedisUtils::sadd_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::sadd_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::srem_async`

调用示例：`RedisUtils::srem_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::srem_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::sismember_async`

调用示例：`RedisUtils::sismember_async("set", value).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::sismember_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::smembers_async`

调用示例：`RedisUtils::smembers_async::<_, MyValue>("set").await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::smembers_async::<&str, u8>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::ping_async`

调用示例：`RedisUtils::ping_async().await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::RedisUtils;

let _ = RedisUtils::ping_async;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```

### `RedisUtils::transaction_async`

调用示例：`RedisUtils::transaction_async(|tx| tx.set("key", value)).await`。

```rust,no_run
# #[cfg(all(feature = "redis", feature = "tokio"))]
# fn main() {
use axutils::{RedisError, RedisTransaction, RedisUtils};

let _ = RedisUtils::transaction_async::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
# }
# #[cfg(not(all(feature = "redis", feature = "tokio")))]
# fn main() {}
```


## 安全和部署边界

- 不把连接 URL、密码、key、field、值或第三方错误文本写入 `Debug`/`Display`；业务日志也
  不应自行打印这些输入。
- `RedisConfig` 第一阶段不接受 TLS。不要把 `rediss://` 当作明文 fallback；后续 TLS 必须
  作为单独 feature 设计证书和校验边界。
- Cluster 普通多 key 命令仍受 Redis hash slot 规则限制；跨 slot 统一返回 `CrossSlot`。
  Cluster 事务始终返回 `UnsupportedMode`。
- ConnectionManager 的有限重连只重建连接，不重放已经返回不确定结果的业务命令；非幂等
  写入不要在应用层盲目重试。
- 事务不提供 WATCH/CAS、读改写、自动重试或 callback 重放。需要这些语义时使用 Redis
  原生 counter 或单独设计脚本/CAS API。
- 真实凭据只从调用方自己的安全配置注入；示例中的 `example.com`、loopback 和占位值不
  能替代生产密钥管理。
