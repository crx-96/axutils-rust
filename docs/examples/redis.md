# Redis

Redis 是显式分层的领域能力。客户端、配置、错误、事务与锁都从 `axutils::redis` 导入；唯一的
全局生命周期入口是 `axutils::utils::RedisUtils`。不要使用 crate 根路径、公开叶模块或旧的静态
命令转发 API。

## 启用

| 需要的能力 | `axutils` feature | 契约 |
| --- | --- | --- |
| 单机同步、`r2d2` 池、MessagePack、锁 | `redis` | 最小 Redis 客户端能力。 |
| 同步 Cluster | `redis-cluster` | 包含 `redis`，追加 Cluster 后端。 |
| 单机异步 | `redis-async` | 包含 `redis`，追加 `_async` 方法和连接管理。 |
| 异步 Cluster | `redis-cluster-async` | 包含 Cluster 与异步能力。 |

单机同步：

```toml
[dependencies]
axutils = { version = "1.0", features = ["redis"] }
```

异步 Cluster：

```toml
[dependencies]
axutils = { version = "1.0", features = ["redis-cluster-async"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
```

`tokio` feature 本身不会开放 Redis 异步 API。自定义值使用 MessagePack，因此应用若需要派生
`Serialize`/`Deserialize`，应直接依赖 `serde`；原始二进制互操作使用 `*_bytes` 方法。

## 导入、配置与实例

`RedisConfig::single`、`RedisConfig::cluster` 和 `RedisClient::new` 只进行本地校验或惰性构造，
不会在构造时连接服务器。一个应用可以创建多个 `RedisClient`，每个实例持有自己的配置与后端状态。

```rust,no_run
use std::time::Duration;

use axutils::redis::{RedisClient, RedisConfig, RedisError};

fn main() -> Result<(), RedisError> {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")?
        .with_pool_size(8)?
        .with_connection_timeout(Duration::from_secs(2))?
        .with_pool_checkout_timeout(Duration::from_secs(2))?
        .with_response_timeout(Duration::from_secs(5))?
        .with_max_value_bytes(2 * 1024 * 1024)?;
    let _client = RedisClient::new(config)?;
    Ok(())
}
```

Cluster 需要 `redis-cluster`；配置节点时用户名、密码和 database 必须保持一致。客户端第一阶段只
接受 `redis://`，不启用 TLS。Cluster 的多 key 操作必须位于同一 hash slot；否则返回
`RedisError::CrossSlot`。

```rust,no_run
use axutils::redis::{RedisClient, RedisConfig, RedisError};

fn main() -> Result<(), RedisError> {
    let config = RedisConfig::cluster([
        "redis://127.0.0.1:7000/0",
        "redis://127.0.0.1:7001/0",
    ])?;
    let _client = RedisClient::new(config)?;
    Ok(())
}
```

## 命令与 MessagePack

常用字符串、key、hash、列表和集合方法以 MessagePack 编解码泛型值；`*_bytes` 保留原始 bytes，适合
缓存二进制或与非 axutils 客户端互操作。以下代码会连接本地 Redis，因此标为 `no_run`。

```rust,no_run
use axutils::redis::{RedisClient, RedisConfig, RedisError};

fn main() -> Result<(), RedisError> {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    client.set("profile:42", "Ada")?;
    let name: Option<String> = client.get("profile:42")?;

    client.set_bytes("image:42", [0_u8, 1, 2, 3])?;
    let image = client.get_bytes("image:42")?;
    let _ = (name, image, client.delete("profile:42")?);
    Ok(())
}
```

输入 key/field、单值、批量、响应和集合结果均受 `RedisConfig` 预算约束。`RedisError` 不包含 endpoint、
凭据、key、value、服务端原始回复或第三方错误文本；匹配它和 `RedisTransportErrorKind` 时应保留
wildcard，因为两者均为 `non_exhaustive`。

## 事务与单键租约锁

`transaction` 在 callback 中只做本地参数校验、MessagePack 编码和排队；callback 正常返回后才执行
单机 `MULTI/EXEC`。它不提供读取、`WATCH`、CAS、callback 重放或自动重试。Cluster 模式事务明确返回
`RedisError::UnsupportedMode`，不会伪装成跨节点原子操作。

```rust,no_run
use std::time::Duration;

use axutils::redis::{RedisClient, RedisConfig, RedisError};

fn main() -> Result<(), RedisError> {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    client.transaction(|transaction| {
        transaction.set("order:42", "created")?;
        transaction.hset("order:42:meta", "source", "api")?;
        transaction.expire("order:42", Duration::from_secs(300))
    })
}
```

`try_lock` 是单 Redis 逻辑主节点或单个 Cluster 拓扑上的单键租约锁，使用不可预测 token 和 TTL；它不是
Redlock，不提供 fencing token，也不能替代数据库条件更新、唯一约束、事务或幂等设计。必须显式释放，
因为 guard 的 `Drop` 不执行网络 I/O；进程中断、任务取消或 runtime 关闭只能依赖 TTL 兜底。

```rust,no_run
use std::time::Duration;

use axutils::redis::{RedisClient, RedisConfig, RedisError};

fn main() -> Result<(), RedisError> {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    let Some(mut lease) = client.try_lock("receipt-audit:42", Duration::from_secs(30))? else {
        return Ok(());
    };
    // 在这里完成受保护操作；续租失败或锁丢失后必须停止继续写入。
    let _released = lease.release()?;
    Ok(())
}
```

## 异步 Redis

`redis-async` 增加带 `_async` 后缀的单机 API；`redis-cluster-async` 再增加异步 Cluster。异步 API
必须在调用方 Tokio runtime 中调用，库不会隐式创建 runtime。`transaction_async` 和
`try_lock_async` 保持与同步版本相同的事务、取消与锁边界。

取消等待中的异步命令不意味着服务端从未接收该命令，因此不能把取消后的写入当作可安全重放的操作；
应用应以业务幂等键、状态查询或补偿流程处理不确定结果。异步 guard 的 `Drop` 同样不发送释放命令，
取消任务后由 TTL 兜底，并应避免继续执行原先受该锁保护的写入。

```rust,no_run
use std::time::Duration;

use axutils::redis::{RedisClient, RedisConfig, RedisError};

#[tokio::main]
async fn main() -> Result<(), RedisError> {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    client.set_async("job:42", "queued").await?;
    let value: Option<String> = client.get_async("job:42").await?;

    if let Some(mut lease) = client
        .try_lock_async("job:42:lease", Duration::from_secs(15))
        .await?
    {
        let _released = lease.release().await?;
    }
    let _ = value;
    Ok(())
}
```

## 全局生命周期入口

`RedisUtils` 只适用于一个进程级默认客户端。`init` 在同步 `PING` 获得 `PONG` 后写入全局槽；
`init_async`（需要 `redis-async`）在当前 Tokio runtime 中执行同一验证。随后只用 `client()` 取得实例，
并在实例上执行命令、事务或锁操作。

```rust,no_run
use axutils::{
    redis::{RedisConfig, RedisError},
    utils::RedisUtils,
};

fn main() -> Result<(), RedisError> {
    RedisUtils::init(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    assert!(RedisUtils::is_initialized());
    let _pong = RedisUtils::client()?.ping()?;
    Ok(())
}
```

```rust,no_run
use axutils::{
    redis::{RedisConfig, RedisError},
    utils::RedisUtils,
};

#[tokio::main]
async fn main() -> Result<(), RedisError> {
    RedisUtils::init_async(RedisConfig::single("redis://127.0.0.1:6379/0")?).await?;
    let _pong = RedisUtils::client()?.ping_async().await?;
    Ok(())
}
```

首次成功初始化后不能 reset、replace 或读取连接 URL/凭据；重复初始化返回
`RedisError::AlreadyInitialized`，未初始化调用 `client()` 返回 `RedisError::NotInitialized`。初始化
失败不会占用全局槽。真实服务不可用、连接、认证、超时和协议失败均作为稳定分类的 `RedisError` 返回；
不要把错误文本当作 Redis 服务端诊断或凭据记录载体。
