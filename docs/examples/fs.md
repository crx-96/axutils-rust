# FsUtils 使用文档

`FsUtils` 是默认可用的本地文件系统 I/O facade。它是无状态的 unit-like 类型，实现
`Debug`、`Clone`、`Copy` 和 `Default`，不保存句柄、根目录、权限上下文或全局状态。推荐从
`axutils::FsUtils` 导入；领域错误类型是 `axutils::FsError`。同步方法直接调用标准库文件系统
API，会阻塞当前线程；带 `_async` 后缀的方法只在 `tokio` feature 下提供，使用调用方已有的
Tokio runtime。

## 导出、feature 与职责边界

支持的公共路径如下：

- `FsUtils`：`axutils::FsUtils`（推荐）、`axutils::utils::FsUtils`、
  `axutils::utils::fs_utils::FsUtils`；三个路径是同一个类型。
- `FsError`：`axutils::FsError`（推荐）、`axutils::fs::FsError`；两个路径是同一个类型。
- 领域模块：`axutils::fs`，只公开 `FsError`。`FsUtils` 不从 `axutils::fs` 导出。

`axutils::fs_utils` 不是公共模块，`axutils::utils::FsError` 和
`axutils::utils::fs_utils::FsError` 也不是公共路径。领域实现文件 `src/fs/ops.rs` 是 crate
内部实现，不是调用方导入路径。

默认 feature 已提供所有同步方法，不需要第三方依赖：

```toml
[dependencies]
axutils = "0.1"
```

异步方法需要 `tokio` feature，并且应用要直接依赖 Tokio、负责创建和保持 runtime：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

crate 不创建 runtime、不调用 `block_on`，也不把同步 I/O 包装成 `spawn_blocking` 来伪装成异步。
异步入口会在返回 future 前拥有化路径/内容，因此返回的 future 不借用调用方参数；future 首次
poll 时再检查限制和 runtime。调用方必须保持 runtime 存活到 future 完成。

FsUtils 只负责调用方指定的本地路径操作，不负责安全根、授权、沙箱、路径遍历防护、
canonicalize、权限/所有权/ACL 修改、原子写、fsync、文件锁、watcher、递归复制、备份或回滚。
`PathUtils` 仍然只做路径词法处理，不检查文件是否存在。

## FsError

`FsError` 实现 `Debug`、`Clone`、`PartialEq`、`Eq`、`Display` 和 `std::error::Error`，并标记为
`#[non_exhaustive]`。调用方匹配时必须保留 wildcard：

```rust
use axutils::FsError;

fn classify(error: FsError) -> &'static str {
    match error {
        FsError::Io { .. } | FsError::PairIo { .. } => "io",
        FsError::NotUtf8 { .. } => "utf8",
        FsError::FileTooLarge { .. } | FsError::DirectoryEntriesTooMany { .. } => "limit",
        FsError::InvalidLimit { .. } => "invalid-limit",
        FsError::UnsupportedEntry { .. } => "unsupported-entry",
        FsError::RuntimeRequired => "runtime",
        _ => "future-error-variant",
    }
}
```

| 变体 | 语义 |
| --- | --- |
| `Io { operation, path, kind }` | 单路径查询、创建、读取、写入、追加、列表或删除失败；`operation` 是稳定小写 token，`kind` 是 `std::io::ErrorKind`。 |
| `PairIo { operation, source, destination, kind }` | 移动或复制等双路径操作失败；保留源、目标和底层错误分类。 |
| `NotUtf8 { path }` | `read_to_string` 或其异步版本的内容不是严格合法 UTF-8；不会替换非法字节。 |
| `FileTooLarge { path, limit }` | 读取实际读到超过 `max_bytes` 的字节；不依赖 metadata 作为唯一防线。 |
| `DirectoryEntriesTooMany { path, limit }` | 列举实际观察到第 `max_entries + 1` 个直接子项。 |
| `InvalidLimit { field }` | `max_entries == usize::MAX`，或 `max_bytes + 1` 无法 checked-add/无损转换为 `u64`。 |
| `UnsupportedEntry { operation, path }` | `copy_file` 的最终源或已存在目标不是普通文件，例如目录、符号链接或其他非普通文件。 |
| `RuntimeRequired` | 异步入口首次 poll 时不在 Tokio runtime 中。同步入口不会返回该变体。 |

错误不会保存或回显底层错误文本、文件内容、字节内容、权限详情或凭据；调用方传入的路径
本身可能出现在错误中，因此敏感路径应由调用方自行避免。

## 查询方法

### `FsUtils::try_exists<P: AsRef<Path>>(path: P) -> Result<bool, FsError>`

跟随符号链接查询目标是否存在。目标不存在（包括坏链接）返回 `Ok(false)`；权限或其他 I/O
错误返回 `FsError::Io`（operation token 为 `try_exists`），不会像 `Path::exists` 一样吞掉错误。
该结果不能作为删除授权或抗竞态
安全检查。

```rust,no_run
use axutils::FsUtils;

let exists = FsUtils::try_exists("example.txt")?;
let _ = exists;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::try_exists_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<bool, FsError>> + 'static`

`try_exists_async` 只在 `tokio` feature 下提供；参数和正常返回值与同步方法相同，目标不存在
（包括坏链接）返回 `Ok(false)`，其他 I/O 错误返回 `FsError::Io`（operation token 为
`try_exists`）。入口先复制 `path` 再返回
拥有所有权的 future；首次 poll 无 runtime 时返回 `RuntimeRequired`。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::try_exists_async("example.txt").await?;
# Ok(())
# }
```

### `FsUtils::is_file<P: AsRef<Path>>(path: P) -> Result<bool, FsError>`

跟随符号链接查询目标是否为普通文件。不存在返回 `Ok(false)`，其他 I/O 错误返回 `FsError::Io`
（operation token 为 `is_file`）。

```rust,no_run
use axutils::FsUtils;

let _ = FsUtils::is_file("example.txt")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::is_file_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<bool, FsError>> + 'static`

`is_file_async` 只在 `tokio` feature 下提供；参数和正常返回值与同步方法相同，不存在返回
`Ok(false)`，其他 I/O 错误返回 `FsError::Io`（operation token 为 `is_file`）。入口先复制
`path`；首次 poll 无 runtime 时
返回 `RuntimeRequired`。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::is_file_async("example.txt").await?;
# Ok(())
# }
```

### `FsUtils::is_dir<P: AsRef<Path>>(path: P) -> Result<bool, FsError>`

跟随符号链接查询目标是否为目录。不存在返回 `Ok(false)`，其他 I/O 错误返回 `FsError::Io`
（operation token 为 `is_dir`）。

```rust,no_run
use axutils::FsUtils;

let _ = FsUtils::is_dir("example-dir")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::is_dir_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<bool, FsError>> + 'static`

`is_dir_async` 只在 `tokio` feature 下提供；参数和正常返回值与同步方法相同，不存在返回
`Ok(false)`，其他 I/O 错误返回 `FsError::Io`（operation token 为 `is_dir`）。入口先复制
`path`；首次 poll 无 runtime 时
返回 `RuntimeRequired`。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::is_dir_async("example-dir").await?;
# Ok(())
# }
```

### `FsUtils::metadata<P: AsRef<Path>>(path: P) -> Result<std::fs::Metadata, FsError>`

获取跟随符号链接的标准库元数据；I/O 失败返回 `FsError::Io`，operation token 为 `metadata`。
不执行权限、沙箱或大小安全判断。

```rust,no_run
use axutils::FsUtils;

let metadata = FsUtils::metadata("example.txt")?;
let _ = metadata.len();
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::metadata_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static`

`metadata_async` 只在 `tokio` feature 下提供，返回同样的 `std::fs::Metadata`，跟随最终符号
链接；I/O 错误返回 `FsError::Io`（operation token 为 `metadata`），首次 poll 无 runtime 时返回
`RuntimeRequired`。入口先复制
`path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::metadata_async("example.txt").await?;
# Ok(())
# }
```

### `FsUtils::symlink_metadata<P: AsRef<Path>>(path: P) -> Result<std::fs::Metadata, FsError>`

获取最终路径项自身的元数据，不跟随符号链接。I/O 失败返回 `FsError::Io`（operation token
为 `symlink_metadata`）。要判断链接本身，应使用该方法并检查返回的 `file_type()`。

```rust,no_run
use axutils::FsUtils;

let metadata = FsUtils::symlink_metadata("example.txt")?;
let _ = metadata.file_type();
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::symlink_metadata_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<std::fs::Metadata, FsError>> + 'static`

`symlink_metadata_async` 只在 `tokio` feature 下提供，不跟随最终符号链接；I/O 错误返回
`FsError::Io`（operation token 为 `symlink_metadata`），首次 poll 无 runtime 时返回
`RuntimeRequired`。入口先复制 `path`，返回的 future
不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::symlink_metadata_async("example.txt").await?;
# Ok(())
# }
```

## 创建、列表与删除

### `FsUtils::create_file<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

使用 `create_new` 创建空文件。父目录必须存在；目标已存在（包括普通文件、目录或链接）时
返回 `FsError::Io`（operation token 为 `create_file`，通常为 `AlreadyExists`），不截断已有内容。
需要创建或截断文件时使用 `write`。

```rust,no_run
use axutils::FsUtils;

FsUtils::create_file("new-file")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::create_file_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`create_file_async` 只在 `tokio` feature 下提供，保留 `create_new` 和不覆盖语义；目标已存在、
父目录缺失或其他 I/O 错误返回 `FsError::Io`（operation token 为 `create_file`），首次 poll
无 runtime 时返回 `RuntimeRequired`。
入口先复制 `path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::create_file_async("new-file").await?;
# Ok(())
# }
```

### `FsUtils::create_dir<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

只创建最后一级目录，不自动创建缺失的父目录；已有目标、父目录缺失和类型错误返回
`FsError::Io`，operation token 为 `create_dir`。

```rust,no_run
use axutils::FsUtils;

FsUtils::create_dir("new-dir")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::create_dir_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`create_dir_async` 只在 `tokio` feature 下提供，语义与同步方法一致；底层 I/O 错误返回
`FsError::Io`，首次 poll 无 runtime 时返回 `RuntimeRequired`。入口先复制 `path`，返回的 future
不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::create_dir_async("new-dir").await?;
# Ok(())
# }
```

### `FsUtils::create_dir_all<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

递归创建缺失的父目录；已有目录幂等成功。同名文件、权限错误、组件类型不匹配或其他底层
失败返回 `FsError::Io`（operation token 为 `create_dir_all`）。同一目标的并发创建允许按底层
语义成功；创建过程非原子，失败可能留下部分父目录，调用方不应把它当作事务操作。

```rust,no_run
use axutils::FsUtils;

FsUtils::create_dir_all("parent/child")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::create_dir_all_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`create_dir_all_async` 只在 `tokio` feature 下提供；已有目录幂等成功，同名文件、权限、组件
类型或其他底层失败返回 `FsError::Io`（operation token 为 `create_dir_all`）；同一目标的并发
创建允许按底层语义成功，创建过程非原子，失败可能留下部分父目录；首次 poll 无 runtime 时返回
`RuntimeRequired`。入口先复制 `path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::create_dir_all_async("parent/child").await?;
# Ok(())
# }
```

### `FsUtils::list_dir<P: AsRef<Path>>(path: P, max_entries: usize) -> Result<Vec<PathBuf>, FsError>`

只返回直接子项的 `PathBuf`，不递归、不排序，也不保证稳定顺序。`max_entries = 0` 是有效上限：
空目录成功，非空目录在观察到第一项时返回 `DirectoryEntriesTooMany { limit: 0 }`。
实现最多观察 `max_entries + 1` 项；`usize::MAX` 在任何文件系统 I/O 前返回
`FsError::InvalidLimit`，其他 I/O 失败返回 `FsError::Io`（operation token 为 `list_dir`）。

```rust,no_run
use axutils::FsUtils;

let children = FsUtils::list_dir("example-dir", 100)?;
let _ = children;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::list_dir_async<P: AsRef<Path>>(path: P, max_entries: usize) -> impl Future<Output = Result<Vec<PathBuf>, FsError>> + 'static`

`list_dir_async` 只在 `tokio` feature 下提供；参数和正常返回值与同步方法相同，只列直接子项、
不排序，观察到第 `max_entries + 1` 项时返回 `DirectoryEntriesTooMany`。首次 poll 时先验证
`max_entries`，再检查 runtime；因此无 runtime 时无效上限仍优先返回 `InvalidLimit`，有效上限
但无 runtime 时返回 `RuntimeRequired`；其他 I/O 失败返回 `FsError::Io`（operation token 为
`list_dir`）。入口先复制 `path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::list_dir_async("example-dir", 100).await?;
# Ok(())
# }
```

### `FsUtils::remove_file<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

删除文件或文件类符号链接；传入目录、缺失目标或权限不足时返回 `FsError::Io`，operation
token 为 `remove_file`。删除链接只删除链接自身，不删除其目标。

```rust,no_run
use axutils::FsUtils;

FsUtils::remove_file("example.txt")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::remove_file_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`remove_file_async` 只在 `tokio` feature 下提供，不会把缺失目标视为成功；目录、权限和其他
I/O 错误返回 `FsError::Io`，首次 poll 无 runtime 时返回 `RuntimeRequired`。入口先复制 `path`，
返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::remove_file_async("example.txt").await?;
# Ok(())
# }
```

### `FsUtils::remove_dir<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

只删除空目录。非空目录、文件、链接、缺失目标或权限不足时返回 `FsError::Io`（operation token
为 `remove_dir`）。

```rust,no_run
use axutils::FsUtils;

FsUtils::remove_dir("empty-dir")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::remove_dir_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`remove_dir_async` 只在 `tokio` feature 下提供，语义与同步方法一致；非空目录、文件、链接、
权限和其他 I/O 错误返回 `FsError::Io`（operation token 为 `remove_dir`），首次 poll 无 runtime 时返回 `RuntimeRequired`。入口先
复制 `path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::remove_dir_async("empty-dir").await?;
# Ok(())
# }
```

### `FsUtils::remove_dir_all<P: AsRef<Path>>(path: P) -> Result<(), FsError>`

删除目录树及目录自身，直接采用标准库/Tokio 的递归删除语义；I/O 失败返回 `FsError::Io`，
operation token 为 `remove_dir_all`。它不先 canonicalize，不提供
最大深度、最大条目数、事务或回滚；失败或取消可能已经部分完成。最终目录项的符号链接不会被
主动递归到其外部目标，但路径中间组件仍按操作系统规则解析；这不是抗 TOCTOU 安全删除器，
不应对不可信目录树使用。

```rust,no_run
use axutils::FsUtils;

FsUtils::remove_dir_all("temporary-tree")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::remove_dir_all_async<P: AsRef<Path>>(path: P) -> impl Future<Output = Result<(), FsError>> + 'static`

`remove_dir_all_async` 只在 `tokio` feature 下提供；底层 I/O 错误返回 `FsError::Io`（operation
token 为 `remove_dir_all`）。取消
future 不保证底层删除立即停止，也不保证最终目录状态或回滚；首次 poll 无 runtime 时返回
`RuntimeRequired`。入口先复制 `path`，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::remove_dir_all_async("temporary-tree").await?;
# Ok(())
# }
```

## 移动、复制、读取和写入

### `FsUtils::move_path<P: AsRef<Path>, Q: AsRef<Path>>(source: P, destination: Q) -> Result<(), FsError>`

直接映射 `std::fs::rename`，文件和目录均可移动。同一文件系统内通常具有操作系统提供的
原子性；跨设备时返回 `FsError::PairIo`（operation token 为 `move_path`），保留
`ErrorKind::CrossesDevices` 等底层分类，不执行 copy-delete fallback。目标冲突语义由当前
操作系统决定。

```rust,no_run
use axutils::FsUtils;

FsUtils::move_path("source", "destination")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::move_path_async<P: AsRef<Path>, Q: AsRef<Path>>(source: P, destination: Q) -> impl Future<Output = Result<(), FsError>> + 'static`

`move_path_async` 只在 `tokio` feature 下提供，直接映射 `tokio::fs::rename`，不执行跨设备
fallback；返回值和 `FsError::PairIo` 错误分类（operation token 为 `move_path`）与同步方法一致，首次 poll 无 runtime 时返回
`RuntimeRequired`。入口先复制两个路径，返回的 future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::move_path_async("source", "destination").await?;
# Ok(())
# }
```

### `FsUtils::copy_file<P: AsRef<Path>, Q: AsRef<Path>>(source: P, destination: Q) -> Result<u64, FsError>`

只复制普通文件并返回复制字节数。源和已存在的目标最终路径项都会先用不跟随链接的元数据
做普通文件预检；目录、符号链接或其他非普通文件在无竞态时返回 `UnsupportedEntry`。目标
不存在时允许底层 copy 创建它，但不会创建目标父目录。检查和实际复制之间存在 TOCTOU 竞态，
因此该方法不是安全复制器；同一路径或 hard-link alias 不做额外身份判断，行为交给底层 copy；
失败、并发或取消可能留下部分目标文件，也没有 `max_file_bytes` 参数。源/目标 I/O 失败返回
`FsError::PairIo`（operation token 为 `copy_file`）。

```rust,no_run
use axutils::FsUtils;

let copied = FsUtils::copy_file("source.txt", "destination.txt")?;
let _ = copied;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::copy_file_async<P: AsRef<Path>, Q: AsRef<Path>>(source: P, destination: Q) -> impl Future<Output = Result<u64, FsError>> + 'static`

`copy_file_async` 只在 `tokio` feature 下提供，返回复制字节数；普通文件预检、
`FsError::UnsupportedEntry`、`FsError::PairIo`（operation token 为 `copy_file`）、目标父目录和 TOCTOU 边界与同步版本一致，失败或取消可能留下
部分目标。首次 poll 无 runtime 时返回 `RuntimeRequired`；入口先复制两个路径，返回的 future
不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::copy_file_async("source.txt", "destination.txt").await?;
# Ok(())
# }
```

### `FsUtils::read_bytes<P: AsRef<Path>>(path: P, max_bytes: usize) -> Result<Vec<u8>, FsError>`

使用 `open + take(max_bytes + 1)` 流式读取。限制先经过 `checked_add(1)` 和 `u64::try_from`
验证；有效范围为 `0 <= max_bytes <= min(usize::MAX - 1, u64::MAX - 1)`。`max_bytes = 0`
是有效限制，空文件成功，非空文件返回 `FileTooLarge`。实际读取内容超过限制才失败，不依赖
metadata 大小，因此可以处理大小报告不可靠的文件，但 FIFO、设备、socket 或 `/proc` 等特殊
文件仍可能阻塞。无效限制返回 `FsError::InvalidLimit`，其他 I/O 失败返回 `FsError::Io`
（operation token 为 `read_bytes`）。调用方必须自行限制路径来源、文件规模、并发和总内存。

```rust,no_run
use axutils::FsUtils;

let bytes = FsUtils::read_bytes("example.bin", 1024)?;
let _ = bytes;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::read_bytes_async<P: AsRef<Path>>(path: P, max_bytes: usize) -> impl Future<Output = Result<Vec<u8>, FsError>> + 'static`

`read_bytes_async` 只在 `tokio` feature 下提供，返回受 `max_bytes` 限制的 `Vec<u8>`；超过上限
返回 `FsError::FileTooLarge`，I/O 错误返回 `FsError::Io`（operation token 为 `read_bytes`）。首次 poll 时先验证 `max_bytes`，有效限制才
检查 runtime；无效限制返回 `InvalidLimit`，有效限制但无 runtime 时返回 `RuntimeRequired`。
入口先复制 `path`，返回的 future 不借用调用方参数；调用方必须保持 runtime 存活到读取完成。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::read_bytes_async("example.bin", 1024).await?;
# Ok(())
# }
```

### `FsUtils::read_to_string<P: AsRef<Path>>(path: P, max_bytes: usize) -> Result<String, FsError>`

复用受限二进制读取后执行严格 UTF-8 解码。不剥离 BOM、不替换非法字节；非法内容返回
`NotUtf8`，超过限制返回 `FileTooLarge`，I/O 失败保留 `read_to_string` operation token。

```rust,no_run
use axutils::FsUtils;

let text = FsUtils::read_to_string("example.txt", 1024)?;
let _ = text;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::read_to_string_async<P: AsRef<Path>>(path: P, max_bytes: usize) -> impl Future<Output = Result<String, FsError>> + 'static`

`read_to_string_async` 只在 `tokio` feature 下提供，返回严格 UTF-8 的 `String`；非法字节返回
`FsError::NotUtf8`，超过限制返回 `FsError::FileTooLarge`，无效限制返回 `FsError::InvalidLimit`，I/O 错误返回
`FsError::Io`（operation token 为 `read_to_string`），有效限制但无 runtime 时返回 `RuntimeRequired`。入口先复制 `path`，返回的
future 不借用调用方参数。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
let _ = axutils::FsUtils::read_to_string_async("example.txt", 1024).await?;
# Ok(())
# }
```

### `FsUtils::write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), FsError>`

创建或截断目标文件并写完 `contents`；空内容也会创建/截断文件。普通打开语义可能跟随最终
符号链接。不自动创建父目录；I/O 失败返回 `FsError::Io`（operation token 为 `write`），不保证
原子更新、fsync 或异常后的完整目标，调用方需要原子更新时应在库外组合临时文件、flush/fsync
和 rename。

```rust,no_run
use axutils::FsUtils;

FsUtils::write("example.txt", b"content")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::write_async<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> impl Future<Output = Result<(), FsError>> + 'static`

`write_async` 只在 `tokio` feature 下提供；返回值和 `FsError::Io` 错误映射（operation token 为
`write`）与同步方法一致，
入口返回 future 前会拥有化路径和内容，不创建 runtime；首次 poll 无 runtime 时返回
`RuntimeRequired`，取消可能留下部分写入结果。普通打开语义可能跟随最终符号链接。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::write_async("example.txt", b"content").await?;
# Ok(())
# }
```

### `FsUtils::append<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<(), FsError>`

以追加模式创建（若不存在）并写入 `contents`。普通打开语义可能跟随最终符号链接。不承诺
多进程/多任务下的记录级原子性；I/O 失败返回 `FsError::Io`（operation token 为 `append`），
异常可能留下部分内容。

```rust,no_run
use axutils::FsUtils;

FsUtils::append("example.log", b"line\n")?;
# Ok::<(), axutils::FsError>(())
```

#### `FsUtils::append_async<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> impl Future<Output = Result<(), FsError>> + 'static`

`append_async` 只在 `tokio` feature 下提供；返回值和 `FsError::Io` 错误映射（operation token 为
`append`）与同步方法一致，
入口返回 future 前会拥有化路径和内容，不创建 runtime；首次 poll 无 runtime 时返回
`RuntimeRequired`，取消或并发可能留下部分内容，且不提供记录级原子性。普通打开语义可能跟随
最终符号链接；调用方负责 runtime 和并发/总磁盘预算。

```rust,no_run
# #[cfg(feature = "tokio")]
# async fn example() -> Result<(), axutils::FsError> {
axutils::FsUtils::append_async("example.log", b"line\n").await?;
# Ok(())
# }
```

## 资源、平台和测试边界

- 所有限制只约束已经读入内存的字节或已观察的直接子项；`max_entries` 的有效范围为
  `0 <= max_entries < usize::MAX`；`copy_file` 没有文件大小上限，
  `remove_dir_all` 没有递归深度/条目预算。
- 所有路径直接交给操作系统，不自动阻止 `..`、当前目录、根目录、路径遍历或中间组件符号链接。
- `metadata`/`is_file`/`is_dir` 跟随链接，`symlink_metadata` 观察最终链接项自身；
  `read_bytes`、`read_to_string`、`write` 和 `append` 使用普通打开语义，可能跟随最终链接；
  `copy_file` 只做最终路径项预检，检查后仍可能发生 TOCTOU 替换。
- 权限、目标冲突、跨设备 rename、特殊文件和 Windows 符号链接权限属于平台行为；调用方应按
  目标平台测试，不能把单平台错误文本或成功结果当作跨平台保证。
- crate 不自动记录路径、内容或 I/O 结果；测试和示例应使用独立临时目录、占位数据和 `no_run`，
  不触碰仓库文件、用户目录或真实服务。

更多定位信息见 [工具类定位文档](https://github.com/crx-96/axutils-rust/blob/main/docs/module-map.md)，
简短概览见 [README](../../README.md)，
API 细节见 [docs.rs](https://docs.rs/axutils/)。
