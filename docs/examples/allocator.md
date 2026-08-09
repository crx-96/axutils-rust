# 全局内存分配器使用文档

> 这是编译期 feature 能力，不是普通的工具类 API。`axutils` 不导出 allocator 类型、实例、
> handle 或运行时切换方法；启用 allocator feature 后，注册行为会作用于依赖该 library 的最终
> Rust binary。本文不作性能承诺，实际收益必须用目标平台和真实工作负载单独测量。

## 能力概览与 feature 矩阵

| 启用组合 | 行为 | 结果 |
| --- | --- | --- |
| 无 allocator feature | 不注册本 crate 的 `#[global_allocator]` | 继续使用 Rust/目标平台默认分配器 |
| `mimalloc` | 注册 `mimalloc::MiMalloc` | `Box`、`Vec` 和其他使用 Rust 全局分配路径的容器由该后端处理 |
| `rpmalloc` | 注册 `rpmalloc::RpMalloc` | `Box`、`Vec` 和其他使用 Rust 全局分配路径的容器由该后端处理 |
| `mimalloc,rpmalloc` | 显式编译失败 | 不采用隐式优先级，诊断包含 `axutils_allocator_conflict` |

`mimalloc` 和 `rpmalloc` 是互斥的 allocator 后端；它们可以分别与普通业务 feature 组合，
但不能同时启用。Cargo 的 `--all-features` 会同时打开两个后端，因此对本 crate 来说是
预期失败组合，不是成功构建配置。

## 导出内容

本能力没有公共模块、结构体、枚举、常量、自由函数、trait、类型别名、静态项或宏。`src/allocator.rs`
是 crate 内部私有模块，不能从以下路径导入：

- 不存在 `axutils::allocator`；
- 不存在 `axutils::utils::allocator`；
- 不存在任何 `AllocatorUtils` 或 allocator handle。

公共可见的配置入口只有 Cargo feature：`mimalloc` 和 `rpmalloc`。启用后产生的是链接/运行
范围的全局副作用，不增加 axutils 的普通业务调用 API。

## 安装与选择后端

不启用 allocator feature 时无需额外依赖：

```toml
[dependencies]
axutils = "0.1"
```

选择 mimalloc：

```toml
[dependencies]
axutils = { version = "0.1", features = ["mimalloc"] }
```

选择 rpmalloc：

```toml
[dependencies]
axutils = { version = "0.1", features = ["rpmalloc"] }
```

两个后端不能写在同一个 `features` 列表中。应用如果通过多个直接或间接依赖转发了
`axutils/mimalloc`，Cargo feature 会在整个依赖图中统一；应使用
`cargo tree -e normal,build,features` 检查最终启用来源。

## 下游 binary 的最小示例

allocator feature 的作用域是最终程序，而不是 `axutils` 某个模块。下面的代码不调用任何
allocator 专用 API，只使用标准 Rust 容器；同一段代码可以在无 allocator、mimalloc 或
rpmalloc 三种配置下编译。

```rust
use axutils::PathUtils;

fn main() {
    let current_dir = PathUtils::current_dir().expect("current directory should be available");
    assert!(!current_dir.as_os_str().is_empty());

    let mut values = Vec::with_capacity(4);
    values.extend([1_u32, 2, 3, 4]);
    values.reserve(64);
    values.push(5);
    let values = Box::new(values);
    assert_eq!(values.as_slice(), &[1, 2, 3, 4, 5]);
}
```

此示例只证明程序可以完成普通容器的分配、扩容和释放；分配是否发生、发生次数以及某个
后端是否更快都不是 Rust 代码可以稳定依赖的程序行为。内存耗尽、abort、链接错误或平台
不支持也不会被 axutils 转换成普通业务错误。

## 全局注册约束与兼容性

一个最终程序只能有一个有效的 Rust global allocator 注册。启用本 feature 前，确认应用
自身和递归依赖没有另外声明 `#[global_allocator]`；否则可能在链接/编译阶段因重复注册失败。
本 crate 不会自动探测、覆盖、回退或静默选择另一个 allocator，也不提供运行时切换、热替换、
按线程切换或按模块切换。

特别要注意：Cargo feature 是依赖图级别统一的。一个间接依赖转发 allocator feature 时，
最终 binary 也可能被动注册该 allocator；调用方应在依赖选择层面明确保留唯一后端。应使用
本仓库的 allocator feature fixture 验证冲突配置，而不是把双后端错误配置作为正常部署方案。

## 平台、构建工具和 FFI 边界

- `mimalloc` 的 native 构建需要 C compiler；`rpmalloc` 通过 `rpmalloc-sys` 构建 bundled
  C11 实现，也需要目标平台可用的 C toolchain、linker 和必要 SDK。
- Windows 的 rpmalloc 路径还需要链接器 SDK 提供 `Advapi32` 系统导入库；这是上游 C 实现
  使用 Windows token privilege API 的链接前置条件，不是 axutils 的运行时可选项。
- 上游 crate 当前文档列出的主要验证目标包括 `x86_64-pc-windows-msvc`、
  `x86_64-apple-darwin` 和 `x86_64-unknown-linux-gnu`；本项目不因此自动承诺所有目标、
  动态库边界、`cdylib`、`staticlib`、插件或交叉编译环境都可用。每个目标都应单独运行
  `cargo check`/链接验证，并记录工具链前置条件。
- 不要为了启用 allocator 自动安装、升级或切换 C compiler、linker、SDK 或 Rust toolchain。
  缺少工具链时应保留未覆盖记录。
- `#[global_allocator]` 只改变 Rust 全局分配路径。native/FFI 库的内部内存、库返回的缓冲区
  以及跨边界传递的指针仍必须按照对方 API 规定的分配器和释放函数管理，不能仅凭本 feature
  使用 `Box`/`Vec` 释放它们。
- 本期没有包装 mimalloc 的 `secure`/`v2`/debug 等调优开关，也没有包装 rpmalloc 的
  statistics、guards 或 cache 调优开关；这些选项若未来加入必须单独评估平台、性能和发布兼容性。

## 验证方式

建议至少分别验证以下配置：

```text
cargo test --no-default-features
cargo test --no-default-features --features mimalloc
cargo test --no-default-features --features rpmalloc
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
```

allocator fixture 会对无 feature、两个单 feature 以及各自与 `serde` 的组合执行最终链接并运行，
检查标准容器的基本分配/扩容/释放、跨线程分配和对齐；对 `mimalloc + rpmalloc` 检查固定冲突
诊断，对下游额外声明 `System` 的场景检查重复 global allocator 构建失败。依赖边界还应确认
未启用上游未评估的安全、调试、统计或缓存 feature。

## 不负责的能力

本能力不负责：

- 运行时选择、切换、重置或热替换 allocator；
- 统计所有分配、泄漏检测、内存采样或 benchmark；
- 让所有 native/FFI 分配都改用 axutils 选择的后端；
- 处理内存耗尽、平台链接失败或上游 allocator 的内部错误；
- 为已有 global allocator 的应用自动解决重复注册；
- 对任何工作负载作“必然更快”或“必然更省内存”的保证。
