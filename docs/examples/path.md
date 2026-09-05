# 路径

`PathUtils` 从 `axutils::utils::PathUtils` 导入，默认可用且只依赖标准库。`is_absolute` 与 `join`
只做词法路径处理，不检查存在性、权限、符号链接或 canonical path；`current_dir` 和
`executable_path` 会向操作系统查询当前进程路径。

## 核心用法

```rust
use axutils::utils::PathUtils;
use std::path::PathBuf;

let current = std::env::current_dir()?;
assert!(PathUtils::is_absolute(&current));
assert!(!PathUtils::is_absolute("relative/file"));

let path = PathUtils::join(["assets", "images", "..", "logo.svg"]);
assert_eq!(path, PathBuf::from("assets").join("logo.svg"));
# Ok::<(), std::io::Error>(())
```

`join` 按当前平台规则拼接，并在词法层面消除 `.` 与可消除的 `..`；它不会解析符号链接。因此不要
把它当作目录逃逸防护或授权检查。

## 当前进程位置

`current_dir` 与 `executable_path` 将标准库错误原样返回；例如进程工作目录不可读取或当前可执行
文件无法确定时会失败。

```rust
use axutils::utils::PathUtils;

let directory = PathUtils::current_dir()?;
let executable = PathUtils::executable_path()?;
assert!(directory.is_absolute());
assert!(executable.is_absolute());
# Ok::<(), std::io::Error>(())
```

路径字符串可能来自不可信来源。需要安全根、真实路径、权限、抗 TOCTOU 或沙箱边界时，调用方必须
使用目标平台和业务模型相匹配的文件系统策略。
