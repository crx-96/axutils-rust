# axutils 开发者文档

本文档面向项目维护者和贡献者，不属于 crates.io 发布包。`Cargo.toml` 使用
`package.include` 白名单，仅将源码、`README.md`、`LICENSE` 和 Cargo 配置打入发布包，
因此 `develop.md`、`AGENTS.md` 以及 `docs/` 不会随包发布。

## 项目结构

```text
.
├── Cargo.toml       # 包元数据、feature 和依赖
├── README.md        # 面向使用者，随包发布
├── develop.md       # 面向开发者，不随包发布
├── AGENTS.md        # 项目协作规则，不随包发布
├── docs/
│   └── module-map.md # 工具类和公共模块定位，不随包发布
└── src/
    ├── lib.rs       # crate 入口和公共导出
    └── utils/
        ├── mod.rs        # 通用工具模块和公共导出
        ├── reg_utils.rs  # RegUtils 实现与单元测试
        └── time_utils.rs # TimeUtils 实现与单元测试
```

## 本地开发

项目当前最低支持 Rust 1.76，要求 Rust 工具链满足 `Cargo.toml` 中声明的
`rust-version`。常用检查命令如下：

```powershell
cargo fmt --all -- --check
cargo test --all-features
cargo test --doc --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo test --no-default-features
```

每个公开方法都应同时具备：

1. API doc，说明行为、输入范围和限制；
2. `# Examples` doctest，确保 README/API 示例可编译运行；
3. 覆盖正常输入和边界输入的单元测试。

新增方法时优先评估性能和安全边界；新增、删除或重命名工具类/公共模块时，必须同步维护
`docs/module-map.md` 中的职责、导出、依赖和使用场景定位。

新增 feature 时，应同步更新 `Cargo.toml`、`README.md` 和本文件，并至少验证默认
feature、`--no-default-features` 和 `--all-features` 三种配置。

## 发布步骤

1. 确认工作区只包含本次发布需要的修改，并在 `Cargo.toml` 中更新 `version`。
2. 更新 `README.md`、公开 API 文档和测试，确保示例反映当前 API。
3. 运行完整验证：

   ```powershell
   cargo fmt --all -- --check
   cargo test --all-features
   cargo test --doc --all-features
   cargo clippy --all-targets --all-features -- -D warnings
   ```

4. 检查发布包文件清单，确认开发者文档没有被包含：

   ```powershell
   cargo package --list
   cargo package --allow-dirty --list
   ```

   输出应包含 `README.md` 和 `src/`，不应包含 `develop.md`、`AGENTS.md` 或
   `docs/skills/`。

5. 先执行发布 dry-run：

   ```powershell
   cargo publish --dry-run
   ```

6. 使用已配置的 crates.io 身份发布。首次使用时通过 `cargo login` 配置 token，
   不要将 token 写入仓库或文档：

   ```powershell
   cargo publish
   ```

7. 发布成功后创建并推送与版本一致的 Git tag，例如 `v0.1.0`，再在 GitHub 上记录
   本次发布内容。

## Feature 约定

默认 feature 为空；不依赖第三方包的能力直接可用。当前 `regex` 和
`libphonenumber` 都是显式启用的 feature，分别用于启用可选的第三方依赖 `regex` 和
crates.io 上的 `phonenumber`：

```toml
[features]
default = []
regex = ["dep:regex"]
libphonenumber = ["dep:libphonenumber"]
```

调用方直接依赖 `axutils = "0.1"` 即可使用 `TimeUtils`；需要 `RegUtils` 时显式选择：

```toml
axutils = { version = "0.1", features = ["regex"] }
```

国际手机号码校验的 `RegUtils::is_phone` 需要同时启用两个 feature：

```toml
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

需要第三方包的模块，应使用与依赖包容易识别的 feature 名，并将依赖声明为
`optional = true`，再通过 `dep:<dependency-name>` 绑定。例如本项目使用
`regex = ["dep:regex"]` 和 `libphonenumber = ["dep:libphonenumber"]`，并用对应的
`cfg(feature = "...")` 守卫模块、导出和方法。

不依赖第三方包的方法属于默认能力，不添加 feature 守卫，也不额外声明可选依赖；
这类方法应直接从 crate 根模块导出。新增 feature 或公共方法时，要同步更新
`Cargo.toml`、`README.md`、API doc、doctest 和测试。
