# 按键并发准入

`axutils::concurrency::{KeyedAdmission, KeyedPermit, AdmissionError}` 默认可用，无需 feature
或第三方依赖。一个实例及其克隆共享进程内的键集合；不同实例互不影响。

同一键最多持有一个凭证，总占用量受创建时的容量限制。名额不可用时不排队等待，但内部
`std::sync::Mutex` 在竞争时可能短暂阻塞。成功返回的凭证不持锁，可以跨业务操作或 `.await`
持有；必须将它保留到操作结束，过早丢弃会立即归还名额。

```rust
use axutils::concurrency::{AdmissionError, KeyedAdmission};

let admission = KeyedAdmission::new(1);
let other_owner = admission.clone();
let permit = admission.try_enter("record:42").unwrap();

// 即使总容量已满，同键仍优先报告 Busy。
assert_eq!(other_owner.try_enter("record:42").unwrap_err(), AdmissionError::Busy);
assert_eq!(other_owner.try_enter("record:43").unwrap_err(), AdmissionError::Full);
drop(permit);

let next = other_owner.try_enter("record:42").unwrap();
drop(next);
assert_eq!(KeyedAdmission::new(0).try_enter("key").unwrap_err(), AdmissionError::Full);
```

`try_enter` 接受 `&str` 或 `String`，按字符串原样比较，空字符串也是有效键；大小写、空白、
租户前缀等规范化规则由调用方决定。凭证不可克隆，可以移动到其他线程或任务；准入对象先被
释放不会使仍存活的凭证失效。

正常返回、提前返回错误、panic unwind，以及异步取消导致 future 被丢弃时，凭证通过 `Drop`
归还名额。发出取消请求并不意味着 future 已被丢弃；需要确认资源已释放时，调用方仍需等待
任务退出。`mem::forget`、进程终止和 panic abort 不执行清理。

锁中毒后，新准入持续返回 `Unavailable`；已有凭证的 `Drop` 会恢复锁内状态以移除自己的键，
但不清除中毒标记。调用方负责决定是否更换整个准入实例及如何报告错误。本工具不提供分布式
互斥，不自动重试，也不映射 HTTP 响应或记录业务键。
