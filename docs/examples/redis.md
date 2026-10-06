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
| 进程内缓存失效队列 | `redis-invalidation` | 包含 `redis-async + tokio`，显式注入客户端。 |

单机同步：

```toml
[dependencies]
axutils = { version = "1.2", features = ["redis"] }
```

异步 Cluster：

```toml
[dependencies]
axutils = { version = "1.2", features = ["redis-cluster-async"] }
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

## 缓存失效队列

`RedisInvalidationQueue` 将调用方给出的缓存键去重后交给后台 worker，以 `delete_many_async` 删除。
它只管理进程内失效请求，不生成业务键，不读取应用配置，也不合并缓存读取或回填流程。
`RedisInvalidationConfig` 和 `RedisInvalidationEnqueue` 与队列一起从 `axutils::redis` 导入。

```toml
[dependencies]
axutils = { version = "1.2", default-features = false, features = ["redis-invalidation"] }
tokio = { version = "1", default-features = false, features = ["macros", "rt-multi-thread", "time", "net"] }
```

队列通过 `new(client, config)` 显式接收 `RedisClient`，构造只做本地配置校验，不连接 Redis，
不启动 worker，也不要求已进入 runtime。应用可以将 `Arc<RedisInvalidationQueue>` 放进共享状态，
在业务提交完成后调用同步的 `enqueue`。以下示例保留应用服务的等待位置；运行它会访问 Redis，
因此标为 `no_run`：

```rust,no_run
use std::{sync::Arc, time::Duration};

use axutils::redis::{
    RedisClient, RedisConfig, RedisError, RedisInvalidationConfig, RedisInvalidationQueue,
};

#[tokio::main]
async fn main() -> Result<(), RedisError> {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0")?)?;
    let queue = Arc::new(RedisInvalidationQueue::new(
        client,
        RedisInvalidationConfig {
            capacity: 4096,
            batch_items: 128,
            batch_bytes: 32 * 1024,
            io_timeout: Duration::from_secs(2),
            retry_delays: vec![Duration::from_millis(100), Duration::from_millis(500)],
        },
    )?);

    // 键由应用在事务提交后生成；同一次调用中的重复键也计入 accepted。
    let result = queue.enqueue([
        "cache:example:42".to_owned(),
        "cache:example:42".to_owned(),
    ]);
    assert_eq!(result.accepted, 2);
    assert_eq!(result.rejected, 0);
    if let Some(error) = result.worker_error {
        return Err(error);
    }

    // 实际应用在这里等待自己的服务循环或关闭信号，并让 queue 一直留在共享状态中。
    // pending 仅表示服务生命周期的占位，不代表 flush 或成功确认。
    std::future::pending::<()>().await;
    drop(queue);
    Ok(())
}
```

配置由应用明确给出，没有隐含业务默认值：

| 字段 | 含义与边界 |
| --- | --- |
| `capacity` | 待处理集合的不同键数上限，**不包含在途批次**；`0` 拒绝全部新键。 |
| `batch_items` | 一批最多选取的到期键数；`0` 按 `1` 处理。 |
| `batch_bytes` | 一批缓存键的 UTF-8 字节数之和，不包含 DEL 命令名、参数长度编码或其他协议开销；`0` 每批最多一个键。 |
| `io_timeout` | 一次后台删除尝试的超时；允许 `0`，不表示关闭超时。 |
| `retry_delays` | 每次失败后依次使用的等待时长；空列表不重试，最大尝试次数为 `1 + len()`。 |

容量不是总内存字节上限，调用方还应约束输入迭代器的长度、执行时间与单键长度。
无法表示为 deadline 的超时或重试间隔在构造时返回 `RedisError::InvalidConfig`。分批从已到期的键中
按 `String` 字典序选择，不保证 FIFO 或公平性。首个键即使超过 `batch_bytes`，也独立成批，避免
永远停留在队列；这不绕过 `RedisClient` 的 key、批量项数、批量参数字节或 Cluster slot 校验。
`InvalidKey`、预算错误、`CrossSlot`、传输错误和超时都会消耗同一套有限重试预算。应用应将队列
预算与 client 限制对齐；Cluster 还需启用 `redis-cluster-async`，并由应用保证同批键适合同 slot 删除。

去重与投递结果遵循以下语义：

- 待处理集合中同键只保存一条请求。再次入队将它立即设为到期并重置重试预算，即使集合已满也接受。
- `accepted` 按输入条目计数，包含同一次调用中的重复刷新；它不代表不同键数、成功删除数或投递确认。
  `rejected` 只计因容量不足被拒绝的新键；满队列不等待空位，也不淘汰已有键。
  两个计数达到 `usize::MAX` 时饱和。
- 键进入在途批次后会释放待处理容量；在删除尚未完成时，同键可以作为新请求再次入队，但仍需通过
  当时的容量检查。因此内存中可能同时存在一个在途旧请求和一个待处理新请求。
- 旧批次失败回队时，只补回待处理集合中不存在的键，不能覆盖新请求的到期时间和重试预算；容量
  已被其他新请求占满时丢弃该旧重试。重试次数耗尽后同样丢弃。

第一次存在待处理键的 `enqueue` 才尝试启动 worker；没有当前 Tokio runtime 时，
`worker_error` 返回 `RedisError::RuntimeRequired`，**已接受的待处理键仍保留**。以后在有效 runtime
中再次调用 `enqueue`（允许传空迭代器）可重新启动处理；空队列不会仅因空入队启动 worker。
已有 worker 存活时允许在 runtime 外投递。已启动 worker 在队列为空时等待通知，有新请求时被唤醒。

调用方必须维持可运行、启用 I/O 和 time driver 的 Tokio runtime。当前线程 runtime 只有被驱动时
才推进任务；缺少 driver 可能使后台任务 panic，runtime 关闭或任务异常退出也会中断 worker。下次 `enqueue` 会检测已退出
worker 并尝试重启，仍在待处理集合中的请求保留，退出时的在途请求不会自动恢复。queue 通过
`TokioTaskGuard` 持有 worker；最后一个 `Arc` 所有者释放队列时请求 abort，不等待完成，也不会排空。
没有 `flush`、等待完成确认或 graceful drain API。

后台失败、重试丢弃及满队列等诊断可由可选 `tracing` 接收，只记录分类与数量，不记录缓存键或内容；
未启用 `tracing` 时没有后台事件。`enqueue` 的 `worker_error` 只报告本次启动问题，后台删除错误不会
回填到该返回值。应用需处理拒绝计数，并自行决定告警、降级或 TTL 兜底策略。

该队列不持久化，不承诺每个已接受请求最终被删除，也不提供强一致性。超时和 abort 只取消本地等待，
不保证 Redis 没有执行命令；重试可能再次删除缓存，包括并发回填后的新值。应用应保留自己的缓存键
生成、业务提交时机、TTL 与回填策略，并评估该失效方式是否符合业务的一致性需求。
