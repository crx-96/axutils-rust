# 文件系统：`fs` 与 `FsUtils`

`axutils` 将文件系统的错误和传输模型放在 `axutils::fs`，将无状态操作入口放在
`axutils::utils::FsUtils`。crate 根不重导出这些类型，`utils` 的实现叶模块也不是公共 API。

`FsUtils` 不保存根目录、权限上下文、句柄或全局状态；它只对调用方给出的路径执行操作。
同步 API 默认可用并阻塞当前线程。

## 启用与导入

默认同步能力不依赖第三方 crate：

```toml
[dependencies]
axutils = "1.0"
```

异步一般文件操作、同步临时资源和异步临时资源分别是独立能力：

```toml
[dependencies]
axutils = { version = "1.0", default-features = false, features = [
    "fs-async",      # 完整的带 _async 后缀的 FS 操作
    "fs-temp",       # 同步临时文件和目录
    "fs-temp-async", # 仅异步临时文件和目录
] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- `fs-async` 提供 `FsUtils` 的一般异步读写、目录、复制与流式传输 API。
- `fs-temp` 提供 `FsTempFile`、`FsTempDir` 和同步临时创建 API。
- `fs-temp-async` 只提供 `FsAsyncTempFile`、`FsAsyncTempDir` 及其创建 API；**不会**开放完整异步 FS API。
- 启用通用 `tokio` feature 不会开放上述任何领域异步 API。

典型导入保持来源可见：

```rust
use axutils::{
    fs::{FsError, FsTransferOptions},
    utils::FsUtils,
};

let _ = (FsError::RuntimeRequired, FsTransferOptions::default(), FsUtils);
```

## 同步读写、查询与目录操作

`try_exists`、`is_file`、`is_dir`、`metadata` 和 `symlink_metadata` 提供查询；
`create_file`、`create_dir`、`create_dir_all`、`write`、`append`、`move_path`、`copy_file`、
`remove_file`、`remove_dir`、`remove_dir_all` 提供变更操作。`list_dir` 只列举直接子项，且不承诺排序。

读取必须始终提供上限。`read_bytes` 与 `read_to_string` 用实际读取量而非 metadata 判断限制；
超出限制返回 `FsError::FileTooLarge`。`0` 是有效上限，`usize::MAX` 这类无法安全计算的预算返回
`FsError::InvalidLimit`。

```rust,no_run
use axutils::{fs::FsError, utils::FsUtils};

fn read_small_text(path: &str) -> Result<String, FsError> {
    FsUtils::read_to_string(path, 64 * 1024)
}

FsUtils::write("state.txt", b"ready\n")?;
FsUtils::append("state.txt", b"next\n")?;
let _text = read_small_text("state.txt")?;
let _children = FsUtils::list_dir(".", 100)?;
# Ok::<(), FsError>(())
```

`write` 会创建或截断目标，`append` 会创建或追加；两者都不自动创建父目录、不保证原子更新或
`fsync`。`move_path` 不做跨设备 copy-delete fallback。`remove_dir_all` 是不可回滚的破坏性操作，
只能用于受信路径；失败后可能已经删除部分目录树。

## 流式复制与转换

`copy_file` 要求源是普通文件；目标缺失时会创建，已存在时也必须是普通文件。需要按块转换时使用
`copy_file_with` 和 `FsTransferOptions`；处理器串行运行，已存在目标会被截断，错误或取消后可能
留下部分输出。
选项限制块大小及可选累计输出大小，避免把不受信文件无界读入内存。

```rust,no_run
use axutils::{
    fs::{FsChunkProcessor, FsTransferError, FsTransferOptions},
    utils::FsUtils,
};

struct Identity;

impl FsChunkProcessor for Identity {
    type Error = std::convert::Infallible;

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Ok(chunk)
    }
}

let _stats = FsUtils::copy_file_with(
    "input.bin",
    "output.bin",
    FsTransferOptions::default(),
    Identity,
)?;
# Ok::<(), FsTransferError<std::convert::Infallible>>(())
```

## 异步文件操作（`fs-async`）

带 `_async` 后缀的 `FsUtils` 方法要求 `fs-async`，并使用调用方已有的 Tokio runtime；库不会
创建 runtime、调用 `block_on`，也不会把同步操作包装成后台任务。future 在首次 poll 时检查 runtime，
缺失时返回 `FsError::RuntimeRequired`（流式传输对应 `FsTransferError::RuntimeRequired`）。

```rust,no_run
use axutils::{fs::FsError, utils::FsUtils};

async fn copy_in_runtime() -> Result<u64, FsError> {
    FsUtils::copy_file_async("input.bin", "output.bin").await
}
```

异步读取同样需要调用方提供大小或条目上限。异步 future 被取消、复制或写入失败时，目标文件或目录
可能只完成了一部分；调用方应使用自己定义的临时命名和提交协议处理原子替换需求。

## 临时文件与目录

临时对象的模型和错误位于 `axutils::fs`：

```rust
use axutils::{fs::FsTempConfig, utils::FsUtils};

let context = FsUtils::with_temp_config(
    FsTempConfig::new().with_prefix("job-").with_suffix(".tmp"),
);
assert_eq!(context.config().prefix.as_deref(), Some("job-"));
```

`FsTempConfig` 只保存拥有型配置，不在构造时创建文件或修改进程临时目录。自定义 `directory`
在创建时必须已经存在；`prefix` 和 `suffix` 不得含路径分隔符或 NUL。`FsUtilsContext` 仅持有此配置，
不引入全局可变状态。

同步临时资源要求 `fs-temp`。使用 `close` 可观察显式删除结果；仅依赖析构时，清理错误不能返回：

```rust,no_run
use axutils::{fs::FsTempError, utils::FsUtils};

fn temporary_output() -> Result<(), FsTempError> {
    let file = FsUtils::create_temp_file()?;
    let path = file.path().to_path_buf();
    file.close()?;
    assert!(!path.exists());
    Ok(())
}
```

异步临时资源要求 `fs-temp-async`，不要求或暗示完整的 `fs-async`。在调用方 runtime 内用
`drop_async` 执行正常路径的异步尽力清理；取消或隐式 `Drop` 仍可能回退到后端的同步析构语义：

```rust,no_run
use axutils::{fs::FsTempError, utils::FsUtils};

async fn temporary_output() -> Result<(), FsTempError> {
    let file = FsUtils::create_temp_file_async().await?;
    let path = file.path().to_path_buf();
    file.drop_async().await;
    assert!(!path.exists());
    Ok(())
}
```

## 错误、安全与边界

`FsError`、`FsTransferError<E>` 和 `FsTempError` 是领域错误；它们保存稳定操作 token、路径和
`io::ErrorKind` 等诊断信息，而不回显文件内容或底层错误文本。它们均为 `#[non_exhaustive]`，匹配时应
保留通配分支。

本领域不提供安全根、授权、canonicalize 沙箱、权限修改、抗 TOCTOU、文件锁、递归复制、备份、回滚、
原子写或 `fsync`。符号链接、特殊文件和并发目录变更仍由操作系统语义决定；请在调用方验证路径来源并
在需要时自行建立安全根和提交协议。
