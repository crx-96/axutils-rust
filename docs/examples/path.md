# PathUtils 使用文档

> 默认可用，仅依赖 Rust 标准库；推荐从 `axutils::PathUtils` 导入。本文档覆盖路径语法判断、
> 当前进程路径获取和词法路径拼接，不检查文件存在性或权限。

## 导出内容

公开模块路径：

- `axutils::path_utils`（推荐的直接模块路径）；
- `axutils::utils::path_utils`（`utils` 下的公开子模块路径）；
- `axutils::utils` 始终公开，但 `PathUtils` 的重导出路径见下方。

`PathUtils` 是无公共字段、无 `new` 构造方法的零大小工具结构体。它可以从以下路径导入：

- 推荐：`axutils::PathUtils`；
- `axutils::path_utils::PathUtils`；
- `axutils::utils::PathUtils`；
- `axutils::utils::path_utils::PathUtils`。

该类型实现 `Debug`、`Clone`、`Copy` 和 `Default`。所有 API 都是静态关联方法，不需要创建
实例。模块没有公共自由函数、trait、类型别名、常量、静态项或宏。

## 安装与启用

不需要额外 feature：

```toml
[dependencies]
axutils = "0.1"
```

## 函数与方法详解

### `PathUtils::is_absolute<P: AsRef<Path>>(path: P) -> bool`

- **feature**：默认可用。
- **参数**：`path` 可以是 `Path`、`PathBuf`、字符串切片或其他实现 `AsRef<Path>` 的值。
- **返回值**：按当前操作系统的路径语法判断是否为绝对路径。
- **示例**：方法只检查路径语法，不访问文件系统。

```rust
use axutils::PathUtils;
use std::path::Path;

assert!(PathUtils::is_absolute(Path::new("/var/log")) || cfg!(windows));
assert!(!PathUtils::is_absolute("./var/log"));
```

在跨平台代码中，建议用当前进程目录构造一个确定的绝对路径：

```rust,no_run
use axutils::PathUtils;

let current = PathUtils::current_dir().expect("current directory should be available");
assert!(PathUtils::is_absolute(current));
```

**注意**：该方法不解析符号链接，不判断路径是否存在，也不验证调用方是否有访问权限。

### `PathUtils::current_dir() -> std::io::Result<PathBuf>`

- **feature**：默认可用。
- **参数**：无。
- **返回值**：成功时返回当前进程工作目录；操作系统无法返回工作目录、目录已被删除或
  调用方无权访问时返回 `std::io::Error`。
- **示例**：应处理操作系统错误，而不是把成功当作绝对保证。

```rust,no_run
use axutils::PathUtils;

match PathUtils::current_dir() {
    Ok(path) => assert!(!path.as_os_str().is_empty()),
    Err(error) => eprintln!("cannot read the current directory: {error}"),
}
```

**注意**：返回值只表示工作目录路径，方法不扫描目录内容，也不保证该路径在返回后仍然可用。

### `PathUtils::executable_path() -> std::io::Result<PathBuf>`

- **feature**：默认可用。
- **参数**：无。
- **返回值**：成功时返回当前进程可执行文件路径；平台接口不可用时返回
  `std::io::Error`。
- **示例**：

```rust,no_run
use axutils::PathUtils;

match PathUtils::executable_path() {
    Ok(path) => assert!(!path.as_os_str().is_empty()),
    Err(error) => eprintln!("cannot read the executable path: {error}"),
}
```

**注意**：方法不主动解析符号链接；返回路径也不承诺在后续时刻仍指向同一个文件。

### `PathUtils::join<I, P>(paths: I) -> PathBuf`

- **feature**：默认可用。
- **参数**：`I: IntoIterator<Item = P>`；每个 `P: AsRef<Path>`。片段会按迭代顺序传给
  平台的 `PathBuf::push` 规则。
- **返回值**：拼接并完成 `.`/`..` 词法归一后的新 `PathBuf`，不会返回 I/O 错误。
- **示例**：

```rust,no_run
use axutils::PathUtils;
use std::path::PathBuf;

let path = PathUtils::join(["project", "src", "..", "README.md"]);
assert_eq!(path, PathBuf::from("project").join("README.md"));
```

空输入返回当前目录的词法表示 `.`：

```rust
use axutils::PathUtils;
use std::path::PathBuf;

assert_eq!(
    PathUtils::join(std::iter::empty::<&str>()),
    PathBuf::from(".")
);
```

相对路径开头超出当前片段的 `..` 会保留，有根路径根目录之外的 `..` 会被忽略：

```rust
use axutils::PathUtils;
use std::path::PathBuf;

assert_eq!(PathUtils::join(["..", "..", "src"]), PathBuf::from("../../src"));
```

后续绝对路径会按平台 `PathBuf::push` 规则替换此前的内容；该行为是词法操作，不是安全
沙箱或目录限制：

```rust
use axutils::PathUtils;
use std::path::PathBuf;

let current = PathUtils::current_dir().expect("current directory should be available");
let result = PathUtils::join([
    PathBuf::from("ignored"),
    current,
    PathBuf::from("README.md"),
]);
assert!(result.ends_with("README.md"));
```

**注意**：`join` 不访问文件系统、不解析符号链接、挂载点或实际存在性。时间和额外空间开销
与输入路径总长度线性相关；来自不可信输入的片段仍需由调用方限制长度和数量。

## 使用场景与限制

适合跨平台的路径语法判断、构造当前进程相关路径和在不需要访问文件系统时做词法拼接。
它不负责创建、删除、打开或规范化真实文件，不解析符号链接，不做 canonicalization、权限
检查、路径遍历防护或沙箱约束。需要安全访问用户提供的路径时，调用方必须自行执行允许的
根目录检查、文件系统检查和资源限制。

## 更多信息

- [工具类定位文档](../module-map.md)
- [README 简短示例](../../README.md)
- [docs.rs API 文档](https://docs.rs/axutils/)
