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
- 流式传输领域类型：`FsChunkProcessor`、`FsTransferOptions`、`FsTransferStats`、
  `FsTransferError` 默认从 `axutils::fs` 与 crate 根导出；启用 `tokio` 后追加
  `FsAsyncChunkProcessor`，同样从 `axutils::fs` 与 crate 根导出。
- 临时配置/错误类型：启用 `tempfile` 或 `tempfile-async` 后，`FsTempConfig`、`FsTempError`、
  `FsUtilsContext` 从 `axutils::fs` 与 crate 根导出。
- 同步临时 wrapper：启用 `tempfile` 后，`FsTempFile`、`FsTempDir` 从 `axutils::fs` 与
  crate 根导出；异步临时 wrapper：启用 `tempfile-async` 后，`FsAsyncTempFile`、
  `FsAsyncTempDir` 从 `axutils::fs` 与 crate 根导出。`FsUtils` 不从 `axutils::fs` 导出。

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

临时文件能力是独立 opt-in：同步 wrapper 需要 `tempfile`，异步 wrapper 需要
`tempfile-async`；后者会联动本 crate 的 `tokio` feature，但只启用 `tokio` 不会导出任何
临时文件 API。两种 feature 都关闭依赖的默认 feature，不启用 `async-tempfile` 的 `uuid`
feature，也不修改进程级临时目录：

```toml
[dependencies]
axutils = { version = "0.1", default-features = false, features = ["tempfile"] }
# 异步临时文件改为 features = ["tempfile-async"]，并由应用直接依赖 Tokio runtime：
# tokio = { version = "1.53", features = ["macros", "rt-multi-thread"] }
```

`FsTempConfig` 只保存 `directory`、`prefix` 和 `suffix` 的拥有型配置；构造和
`FsUtils::with_temp_config` 不访问文件系统。指定的父目录必须已经存在，库不会隐式创建；
prefix/suffix 不得包含路径分隔符或 NUL。`FsUtilsContext` 持有配置，避免全局 override 或
跨任务共享可变状态。同步 wrapper 的 `as_file`/`as_file_mut` 暴露标准库文件句柄；异步
wrapper 不暴露 `async-tempfile` 的底层句柄或类型。

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
| `UnsupportedEntry { operation, path }` | `copy_file` 的最终源或已存在目标不是普通文件，例如目录、符号链接或其他非普通文件；流式 `copy_file_with` 会把源项包装在 `FsTransferError::SourceIo` 中、目标项包装在 `FsTransferError::DestinationIo` 中，并使用 `operation = "copy_file_with"`。 |
| `RuntimeRequired` | 异步入口首次 poll 时不在 Tokio runtime 中。同步入口不会返回该变体。 |

错误不会保存或回显底层错误文本、文件内容、字节内容、权限详情或凭据；调用方传入的路径
本身可能出现在错误中，因此敏感路径应由调用方自行避免。

## FsTransferError

`FsTransferError<E>` 是 `#[non_exhaustive]` 泛型错误，匹配时必须保留 wildcard。传输入口本身
不要求 `E: Display + Clone + Eq`，会在 `Processor` 中保留原始 `E`；只有当
`E: std::error::Error + 'static` 时，`FsTransferError<E>` 才实现 `std::error::Error`。

| 变体 | 语义 |
| --- | --- |
| `SourceIo { error }` | 源文件打开、填充读取或源路径预检失败；`error` 通常是 `FsError::Io` 或源项的 `FsError::UnsupportedEntry`。 |
| `DestinationIo { error }` | 目标创建、截断、写入、短写、`WriteZero`、flush 或目标路径预检失败；`error` 通常是 `FsError::Io` 或目标项的 `FsError::UnsupportedEntry`。 |
| `Processor { error, source, destination }` | 用户处理器返回错误；`error` 保留原值，源/目标路径用于定位，目标可能已经保留前序块。 |
| `OutputLimitExceeded { limit, observed }` | 当前处理结果写入前会使累计输出超过 `limit`；当前块不会写入，`observed` 是候选累计值。 |
| `OutputSizeOverflow` | 输出长度转换或累计输出 checked addition 溢出。 |
| `InputSizeOverflow` | 输入长度转换或累计输入 checked addition 溢出。 |
| `ChunkCountOverflow` | 成功块数的 checked addition 溢出。 |
| `InvalidOptions { field }` | `chunk_size` 不在 1 KiB 到 16 MiB 的闭区间内；在任何文件系统 I/O 前返回。 |
| `SameFile { source, destination }` | 未 canonicalize 的词法路径相等；不会识别所有硬链接别名或竞态替换。 |
| `RuntimeRequired` | 异步入口首次 poll 时没有调用方 Tokio runtime；同步入口不会返回它。 |

```rust
use axutils::FsTransferError;

fn classify(error: &FsTransferError<std::convert::Infallible>) -> &'static str {
    match error {
        FsTransferError::SourceIo { .. } => "source",
        FsTransferError::DestinationIo { .. } => "destination",
        FsTransferError::Processor { .. } => "processor",
        FsTransferError::OutputLimitExceeded { .. } => "output-limit",
        FsTransferError::OutputSizeOverflow
        | FsTransferError::InputSizeOverflow
        | FsTransferError::ChunkCountOverflow => "overflow",
        FsTransferError::InvalidOptions { .. } => "options",
        FsTransferError::SameFile { .. } => "same-file",
        FsTransferError::RuntimeRequired => "runtime",
        _ => "future-variant",
    }
}

assert_eq!(
    classify(&FsTransferError::<std::convert::Infallible>::RuntimeRequired),
    "runtime"
);
```

## 流式传输

### `FsTransferOptions`、`FsChunkProcessor` 与统计

`FsTransferOptions::default()` 使用 64 KiB 块；`chunk_size` 必须在 1 KiB 到 16 MiB 之间。
`max_output_bytes` 是累计输出上限，`Some(0)` 只允许空输出。处理器按顺序收到拥有所有权的
`Vec<u8>`，返回的 `Vec<u8>` 在检查累计上限和 checked 统计之后写入目标；空输入不会调用
处理器，非空输入即使处理器返回空输出也计为一个 chunk。库内部不创建每块任务、不建立无界
缓冲，也不把处理器错误强制要求为 `Display`、`Clone` 或 `Eq`。

```rust,no_run
use axutils::{FsChunkProcessor, FsTransferOptions, FsUtils};

struct Identity;
impl FsChunkProcessor for Identity {
    type Error = std::convert::Infallible;

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Ok(chunk)
    }
}

let stats = FsUtils::copy_file_with(
    "source.bin",
    "destination.bin",
    FsTransferOptions::default(),
    Identity,
)?;
assert_eq!(stats.output_bytes, stats.input_bytes);
# Ok::<(), axutils::FsTransferError<std::convert::Infallible>>(())
```

### `FsUtils::copy_file_with<P, Q, C>(source, destination, options, processor) -> Result<FsTransferStats, FsTransferError<C::Error>>`

签名：`pub fn copy_file_with<P, Q, C>(source: P, destination: Q, options: FsTransferOptions, processor: C) -> Result<FsTransferStats, FsTransferError<C::Error>>`；默认 feature 即可用，
同步 I/O 会阻塞当前线程。
同步入口先校验 options 和词法路径相等，再用 `symlink_metadata` 要求源以及已存在的目标是
普通文件；缺失目标允许创建，目标会被截断，父目录不会自动创建。它用填充循环处理普通
文件的短读，用 `write_all` 处理短写并在结束时 flush。源 I/O、目标 I/O、处理器错误、
输出上限、输入/输出/块计数溢出和无效配置分别归类；`Processor` 变体保留 `C::Error` 原值
和源/目标路径。失败可能已经写入前序块或留下空的截断目标，不提供 atomic replace、回滚、
canonicalize、硬链接别名检测或抗 TOCTOU 保证。

```rust,no_run
use axutils::{FsChunkProcessor, FsTransferOptions, FsUtils};

struct Identity;
impl FsChunkProcessor for Identity {
    type Error = std::convert::Infallible;

    fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        Ok(chunk)
    }
}

let _stats = FsUtils::copy_file_with(
    "source.bin",
    "destination.bin",
    FsTransferOptions::default(),
    Identity,
)?;
# Ok::<(), axutils::FsTransferError<std::convert::Infallible>>(())
```

### `FsAsyncChunkProcessor`

启用 `tokio` 后，异步处理器通过 GAT 返回借用当前处理器的 future；处理按源顺序串行，不为每个
块创建后台任务或 runtime。`process` 的输入块拥有所有权，处理器错误原值由传输错误保留；调用方
仍需自行控制处理器分配和总资源。

### `FsUtils::copy_file_with_async`

签名：`pub fn copy_file_with_async<P, Q, C>(source: P, destination: Q, options: FsTransferOptions, processor: C) -> impl Future<Output = Result<FsTransferStats, FsTransferError<C::Error>>> + 'static`；
要求 `tokio` feature、直接依赖并运行 Tokio，以及 `C: FsAsyncChunkProcessor`。首次 poll 先执行
options/词法路径检查，再要求 runtime；不创建 runtime、不调用 `block_on` 或隐式
`spawn_blocking`。输入按 `chunk_size` 分块，`max_output_bytes` 和 checked 统计提供有限上限；
源/目标 I/O、处理器、超限、溢出和 runtime 错误分别返回 `FsTransferError`。取消或失败可能留下
部分目标内容，不提供 atomic replace、回滚或 TOCTOU 防护。

```rust,no_run
# #[cfg(feature = "tokio")]
async fn example() -> Result<(), axutils::FsTransferError<std::convert::Infallible>> {
    use axutils::{FsAsyncChunkProcessor, FsTransferOptions, FsUtils};

    struct Identity;
    impl FsAsyncChunkProcessor for Identity {
        type Error = std::convert::Infallible;
        type Future<'a> = std::future::Ready<Result<Vec<u8>, Self::Error>> where Self: 'a;

        fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
            std::future::ready(Ok(chunk))
        }
    }

    let _ = FsUtils::copy_file_with_async(
        "source.bin",
        "destination.bin",
        FsTransferOptions::default(),
        Identity,
    ).await?;
    Ok(())
}
```

## 临时文件与目录

### `FsTempError`

`FsTempError` 是 `#[non_exhaustive]` 的 axutils 自有错误，不把 `tempfile` 或
`async-tempfile` 的错误类型放进公共签名。当前可观察的变体如下；`Access` 和 `Cleanup` 是
为后续句柄/错误报告能力保留的分类，第一版不会构造它们。

| 变体 | 语义 |
| --- | --- |
| `InvalidConfig { field }` | `prefix` 或 `suffix` 含 `/`、`\\` 或 NUL。 |
| `Create { operation, path, kind }` | 配置父目录无效或临时对象创建失败；保留稳定 operation、路径和 `ErrorKind`。异步创建没有 runtime 时使用顶层 `RuntimeRequired`。 |
| `Access { .. }` | 预留给未来显式打开/访问 API，当前不会产生。 |
| `Close { operation, path, kind }` | 同步 `close` 删除文件/目录失败；`Drop` 不会把清理错误返回给调用方。 |
| `Cleanup { .. }` | 预留给未来可观察异步清理 API，当前没有 `cleanup_async`，也不会产生。 |
| `RuntimeRequired` | 异步创建入口首次 poll 时不在 Tokio runtime 中。 |

调用方匹配非穷尽错误时保留 wildcard：

```rust
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
{
use axutils::FsTempError;

fn category(error: &FsTempError) -> &'static str {
    match error {
        FsTempError::InvalidConfig { .. } => "config",
        FsTempError::Create { .. } => "create",
        FsTempError::Close { .. } => "close",
        FsTempError::RuntimeRequired => "runtime",
        FsTempError::Access { .. } | FsTempError::Cleanup { .. } => "reserved",
        _ => "future-variant",
    }
}
# }
```

### `FsUtilsContext` 与 `FsTempConfig`

启用任一临时 feature 后，`FsUtils::with_temp_config(config)` 返回带配置的
`FsUtilsContext`。context 是拥有型、可 clone 的值；`config()` 只返回配置引用。启用
`tempfile` 时，context 提供 `create_temp_file` 和 `create_temp_dir`；启用
`tempfile-async` 时，提供 `create_temp_file_async` 和 `create_temp_dir_async`。无配置时使用
后端系统临时目录。

### `FsUtils::with_temp_config(config) -> FsUtilsContext`

签名：`pub fn with_temp_config(config: FsTempConfig) -> FsUtilsContext`；要求启用
`tempfile` 或 `tempfile-async`。只拥有配置，不访问文件系统或创建对象；无效父目录和命名片段
由后续创建方法报告。
该方法只拥有配置，不访问文件系统；`directory` 指定的父目录必须在实际创建时已经存在。
`FsTempConfig::with_prefix` 和 `with_suffix` 只接受文件名片段，不能包含路径分隔符或 NUL。

```rust,no_run
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    use axutils::{FsTempConfig, FsUtils};

    let context = FsUtils::with_temp_config(
        FsTempConfig::new()
            .with_directory("existing-temp-parent")
            .with_prefix("job-")
            .with_suffix(".part"),
    );
    assert_eq!(context.config().prefix.as_deref(), Some("job-"));
}
```

### `FsUtils::create_temp_file`

签名：`pub fn create_temp_file() -> Result<FsTempFile, FsTempError>`；只在 `tempfile` feature
下提供。使用默认 `FsTempConfig` 创建同步临时文件并访问本地文件系统；创建失败返回
`FsTempError::Create` 或 `InvalidConfig`，成功后 wrapper 负责清理，Drop 不能回传清理错误。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file()?;
    file.close()?;
    Ok(())
}
```

### `FsUtils::create_temp_dir`

签名：`pub fn create_temp_dir() -> Result<FsTempDir, FsTempError>`；只在 `tempfile` feature
下提供。使用默认配置创建同步临时目录；创建 I/O 失败返回 `FsTempError::Create`，wrapper
Drop 或显式 `close` 负责递归清理，不删除系统临时目录本身。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir()?;
    directory.close()?;
    Ok(())
}
```

### `FsUtils::create_temp_file_async`

签名：`pub fn create_temp_file_async() -> impl Future<Output = Result<FsAsyncTempFile, FsTempError>> + 'static`；
只在 `tempfile-async` feature 下提供，并联动本 crate 的 `tokio`。使用默认配置创建异步临时
文件；首次 poll 无调用方 Tokio runtime 返回 `RuntimeRequired`，创建失败返回 `Create` 或
`InvalidConfig`。成功后 wrapper 负责清理，取消创建 future 不保证已删除。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file_async().await?;
    file.drop_async().await;
    Ok(())
}
```

### `FsUtils::create_temp_dir_async`

签名：`pub fn create_temp_dir_async() -> impl Future<Output = Result<FsAsyncTempDir, FsTempError>> + 'static`；
只在 `tempfile-async` feature 下提供并要求调用方 Tokio runtime。首次 poll 无 runtime 返回
`RuntimeRequired`；创建失败返回 `FsTempError`，成功后 wrapper 负责递归清理，取消不提供已删除
保证。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir_async().await?;
    directory.drop_async().await;
    Ok(())
}
```

### `FsUtilsContext::config()`

签名：`pub fn config(&self) -> &FsTempConfig`；要求启用任一临时 feature。同步返回 context
持有的配置借用，不访问文件系统、不返回错误，也不提供修改其他 context 或进程级默认目录的入口。
`config()` 返回 context 自身配置的只读引用，不提供修改其他 context 或进程级默认目录的
入口。若要变更配置，应创建另一个 context。

```rust,no_run
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    use axutils::{FsTempConfig, FsUtils};

    let context = FsUtils::with_temp_config(FsTempConfig::new().with_prefix("job-"));
    assert_eq!(context.config().prefix.as_deref(), Some("job-"));
}
```

### `FsTempConfig::new`

签名：`pub fn new() -> FsTempConfig`；要求 `tempfile` 或 `tempfile-async` feature。创建使用
后端系统临时目录和默认命名规则的拥有型配置，不访问文件系统、不返回错误；父目录和命名片段
只在后续创建方法中校验。

```rust
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    let config = axutils::FsTempConfig::new();
    assert!(config.directory.is_none());
}
```

### `FsTempConfig::with_directory`

签名：`pub fn with_directory<P: Into<PathBuf>>(self, directory: P) -> FsTempConfig`；要求任一
临时 feature。只保存拥有型父目录路径，不检查或创建目录，也不返回错误；目录不存在会在创建
临时对象时返回 `FsTempError::Create`。

```rust
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    let config = axutils::FsTempConfig::new().with_directory("existing-temp-parent");
    assert_eq!(config.directory.as_deref().and_then(|p| p.to_str()), Some("existing-temp-parent"));
}
```

### `FsTempConfig::with_prefix`

签名：`pub fn with_prefix<S: Into<String>>(self, prefix: S) -> FsTempConfig`；要求任一临时
feature。保存文件名片段，不在 builder 阶段校验；创建时若含 `/`、`\\` 或 NUL 返回
`FsTempError::InvalidConfig { field: "prefix" }`，不产生 I/O 副作用。

```rust
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    let config = axutils::FsTempConfig::new().with_prefix("upload-");
    assert_eq!(config.prefix.as_deref(), Some("upload-"));
}
```

### `FsTempConfig::with_suffix`

签名：`pub fn with_suffix<S: Into<String>>(self, suffix: S) -> FsTempConfig`；要求任一临时
feature。保存文件名片段，不在 builder 阶段校验；创建时若含 `/`、`\\` 或 NUL 返回
`FsTempError::InvalidConfig { field: "suffix" }`。

```rust
# #[cfg(any(feature = "tempfile", feature = "tempfile-async"))]
fn example() {
    let config = axutils::FsTempConfig::new().with_suffix(".tmp");
    assert_eq!(config.suffix.as_deref(), Some(".tmp"));
}
```

### `FsUtilsContext::create_temp_file`

签名：`pub fn create_temp_file(&self) -> Result<FsTempFile, FsTempError>`；只在 `tempfile`
feature 下提供。按 context 配置创建同步命名临时文件并访问本地文件系统；父目录缺失或命名
片段非法分别返回 `Create` 或 `InvalidConfig`。wrapper 持有句柄和清理责任，创建失败不返回
对象；显式 `close` 才能观察删除错误，Drop 只能尽力清理。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let context = axutils::FsUtils::with_temp_config(axutils::FsTempConfig::new());
    let file = context.create_temp_file()?;
    file.close()?;
    Ok(())
}
```

### `FsUtilsContext::create_temp_dir`

签名：`pub fn create_temp_dir(&self) -> Result<FsTempDir, FsTempError>`；只在 `tempfile`
feature 下提供。创建同步临时目录并返回拥有型 wrapper；父目录或命名配置失败返回
`FsTempError`，目录及其内容的清理由 wrapper 负责。创建会进行本地文件系统 I/O，不自动创建
配置中的父目录。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let context = axutils::FsUtils::with_temp_config(axutils::FsTempConfig::new());
    let directory = context.create_temp_dir()?;
    directory.close()?;
    Ok(())
}
```

### 同步 wrapper

启用 `tempfile` 后，`FsTempFile` 提供 `path`、`as_file`、`as_file_mut` 和同步 `close`；
`FsTempDir` 提供 `path` 和同步 `close`。wrapper 持有后端对象，正常 Drop 会自动清理；
`close` 显式关闭并删除对象，删除失败会返回 `FsTempError::Close`。配置父目录缺失、配置
命名片段非法和创建失败分别保留为稳定错误分类。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), Box<dyn std::error::Error>> {
    use axutils::{FsTempConfig, FsUtils};

    let context = FsUtils::with_temp_config(
        FsTempConfig::default().with_prefix("axutils-")
    );
    let mut file = context.create_temp_file()?;
    use std::io::Write;
    file.as_file_mut().write_all(b"temporary")?;
    let _path = file.path().to_path_buf();
    file.close()?;
    Ok(())
}
```

### `FsTempFile::path`

签名：`pub fn path(&self) -> &Path`；只在 `tempfile` feature 下提供。返回临时文件当前路径的
借用；路径只在 wrapper 仍存活时使用，`close` 或 Drop 后不再属于调用方。本方法不访问文件系统、
不返回错误，也不改变清理状态。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file()?;
    let _path = file.path().to_path_buf();
    file.close()?;
    Ok(())
}
```

### `FsTempFile::as_file`

签名：`pub fn as_file(&self) -> &std::fs::File`；只在 `tempfile` feature 下提供。借用 wrapper
内部的只读文件句柄，不转移所有权，不访问额外路径或返回 axutils 错误；标准库句柄操作可能
阻塞并返回 `std::io::Error`。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = axutils::FsUtils::create_temp_file()?;
    let _metadata = file.as_file().metadata()?;
    file.close()?;
    Ok(())
}
```

### `FsTempFile::as_file_mut`

签名：`pub fn as_file_mut(&mut self) -> &mut std::fs::File`；只在 `tempfile` feature 下提供。
借用可变标准库句柄，调用方负责 `std::io` 错误和可能的阻塞；句柄不会从 wrapper 中脱离，
临时文件仍由 wrapper 拥有。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut file = axutils::FsUtils::create_temp_file()?;
    file.as_file_mut().write_all(b"temporary")?;
    file.close()?;
    Ok(())
}
```

### `FsTempFile::close`

签名：`pub fn close(self) -> Result<(), FsTempError>`；只在 `tempfile` feature 下提供。先关闭
句柄再删除临时文件；删除失败返回 `FsTempError::Close`。操作可能阻塞当前线程；不调用时由
Drop 尽力清理，但错误无法返回。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file()?;
    file.close()
}
```

### `FsTempDir::path`

签名：`pub fn path(&self) -> &Path`；只在 `tempfile` feature 下提供。返回目录路径借用，不访问
文件系统、不返回错误；路径只在 wrapper 存活期间有效。`close` 或 Drop 后不得继续使用该路径。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir()?;
    let _path = directory.path().to_path_buf();
    directory.close()?;
    Ok(())
}
```

### `FsTempDir::close`

签名：`pub fn close(self) -> Result<(), FsTempError>`；只在 `tempfile` feature 下提供。递归删除
临时目录及其内容但不删除配置中的父目录；失败返回 `FsTempError::Close`，调用可能阻塞当前
线程。Drop 仍会尽力清理但不回传错误。

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir()?;
    directory.close()?;
    Ok(())
}
```

#### 使用同步临时文件承接大文件流式处理

临时 wrapper 只负责拥有和清理目标路径；流式处理仍使用 `copy_file_with`。处理成功后若
需要永久保留，第一版没有统一 `persist`/`keep` API，应在 wrapper 存活时显式复制到已确定的
业务路径；否则离开作用域会自动删除临时结果：

```rust,no_run
# #[cfg(feature = "tempfile")]
fn example() -> Result<(), Box<dyn std::error::Error>> {
    use axutils::{FsChunkProcessor, FsTempConfig, FsTransferOptions, FsUtils};

    struct Identity;
    impl FsChunkProcessor for Identity {
        type Error = std::convert::Infallible;

        fn process(&mut self, chunk: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
            Ok(chunk)
        }
    }

    let context = FsUtils::with_temp_config(FsTempConfig::new());
    let temporary = context.create_temp_file()?;
    let _stats = FsUtils::copy_file_with(
        "large-source.bin",
        temporary.path(),
        FsTransferOptions::default(),
        Identity,
    )?;
    FsUtils::copy_file(temporary.path(), "large-result.bin")?;
    Ok(())
}
```

### 异步 wrapper

启用 `tempfile-async` 后，`FsAsyncTempFile` 和 `FsAsyncTempDir` 只暴露 `path`、
`drop_async` 和同步 `close`。`drop_async` 把后端清理放到异步方法中，但后端不返回删除
错误，因此该方法的返回值是 `()`；正常完成的 `drop_async` 使用异步删除，但后端的隐式
Drop、取消或 panic 后备仍可能在 runtime worker 上执行同步删除。需要观察关闭错误时使用
可能阻塞当前线程的 `close`。
没有额外的 `cleanup_async`、`persist`、`keep` 或底层 `open_ro`/`open_rw` wrapper，避免对
后端未承诺的错误和生命周期语义作不实抽象。首次 poll 不在 Tokio runtime 时返回
`FsTempError::RuntimeRequired`；取消创建或清理 future 时仍应按 RAII 后备清理语义处理，
不能把取消当作成功删除保证。

### `FsUtilsContext::create_temp_file_async`

签名：`pub fn create_temp_file_async(&self) -> impl Future<Output = Result<FsAsyncTempFile, FsTempError>> + 'static`；
只在 `tempfile-async` feature 下提供，并联动本 crate 的 `tokio`。路径和配置在返回 future 前被
拥有化，首次 poll 需要调用方直接依赖并保持 Tokio runtime；无 runtime 返回 `RuntimeRequired`，
父目录或命名配置错误返回 `Create`/`InvalidConfig`。创建会访问本地文件系统，不创建 runtime。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let context = axutils::FsUtils::with_temp_config(axutils::FsTempConfig::new());
    let file = context.create_temp_file_async().await?;
    file.drop_async().await;
    Ok(())
}
```

### `FsUtilsContext::create_temp_dir_async`

签名：`pub fn create_temp_dir_async(&self) -> impl Future<Output = Result<FsAsyncTempDir, FsTempError>> + 'static`；
只在 `tempfile-async` feature 下提供并要求调用方 Tokio runtime。首次 poll 无 runtime 返回
`RuntimeRequired`，父目录/命名失败返回 `FsTempError`；成功后 wrapper 负责递归清理，取消
future 不保证已经删除。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let context = axutils::FsUtils::with_temp_config(axutils::FsTempConfig::new());
    let directory = context.create_temp_dir_async().await?;
    directory.drop_async().await;
    Ok(())
}
```

### `FsAsyncTempFile::path`

签名：`pub fn path(&self) -> &Path`；只在 `tempfile-async` feature 下提供。只借用当前路径，
不访问文件系统、不返回错误；路径只在 wrapper 存活时有效，清理后不得继续使用。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file_async().await?;
    let _path = file.path().to_path_buf();
    file.drop_async().await;
    Ok(())
}
```

### `FsAsyncTempFile::drop_async`

签名：`pub async fn drop_async(self)`；只在 `tempfile-async` feature 下提供，要求调用方 Tokio
runtime。后端异步删除不返回删除错误，因此返回 `()`；取消或 panic 会走后端同步 Drop 后备，
可能在 runtime worker 上执行同步文件系统调用，不能把取消视为清理成功。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file_async().await?;
    file.drop_async().await;
    Ok(())
}
```

### `FsAsyncTempFile::close`

签名：`pub fn close(self) -> Result<(), FsTempError>`；只在 `tempfile-async` feature 下提供。
同步关闭并删除文件，失败返回 `FsTempError::Close`；可能阻塞当前线程，异步上下文优先使用
`drop_async`。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let file = axutils::FsUtils::create_temp_file_async().await?;
    file.close()?;
    Ok(())
}
```

### `FsAsyncTempDir::path`

签名：`pub fn path(&self) -> &Path`；只在 `tempfile-async` feature 下提供。借用目录路径，不
访问文件系统、不返回错误；路径只在 wrapper 存活期间有效。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir_async().await?;
    let _path = directory.path().to_path_buf();
    directory.drop_async().await;
    Ok(())
}
```

### `FsAsyncTempDir::drop_async`

签名：`pub async fn drop_async(self)`；只在 `tempfile-async` feature 下提供，要求调用方 Tokio
runtime。正常完成时异步递归删除，返回 `()`；取消或 panic 使用可能阻塞 worker 的同步 Drop
后备，不能宣称已清理。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir_async().await?;
    directory.drop_async().await;
    Ok(())
}
```

### `FsAsyncTempDir::close`

签名：`pub fn close(self) -> Result<(), FsTempError>`；只在 `tempfile-async` feature 下提供。
同步递归关闭并删除目录，失败返回 `FsTempError::Close`，可能阻塞当前线程；异步上下文优先
使用 `drop_async`。

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    let directory = axutils::FsUtils::create_temp_dir_async().await?;
    directory.close()?;
    Ok(())
}
```

#### 使用异步临时文件承接大文件流式处理

异步临时 wrapper 可以作为流式目标；处理完成后应在 wrapper 仍存活时校验或复制结果，
随后显式 `drop_async()`：

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), Box<dyn std::error::Error>> {
    use axutils::{FsAsyncChunkProcessor, FsTransferOptions, FsUtils};

    struct Identity;
    impl FsAsyncChunkProcessor for Identity {
        type Error = std::convert::Infallible;
        type Future<'a> = std::future::Ready<Result<Vec<u8>, Self::Error>> where Self: 'a;

        fn process<'a>(&'a mut self, chunk: Vec<u8>) -> Self::Future<'a> {
            std::future::ready(Ok(chunk))
        }
    }

    let temporary = FsUtils::create_temp_file_async().await?;
    let _stats = FsUtils::copy_file_with_async(
        "large-source.bin",
        temporary.path(),
        FsTransferOptions::default(),
        Identity,
    ).await?;
    FsUtils::copy_file_async(temporary.path(), "large-result.bin").await?;
    temporary.drop_async().await;
    Ok(())
}
```

```rust,no_run
# #[cfg(feature = "tempfile-async")]
async fn example() -> Result<(), axutils::FsTempError> {
    use axutils::FsUtils;

    let file = FsUtils::create_temp_file_async().await?;
    let path = file.path().to_path_buf();
    file.drop_async().await;
    assert!(!path.exists());
    Ok(())
}
```

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
