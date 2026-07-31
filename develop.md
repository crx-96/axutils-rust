# axutils 开发者文档

本文档面向项目维护者和贡献者，不属于 crates.io 发布包。`Cargo.toml` 使用
`package.include` 白名单，仅将源码、`README.md`、`LICENSE` 和 Cargo 配置打入发布包，
因此 `develop.md`、`AGENTS.md`、`CLAUDE.md` 以及 `docs/` 不会随包发布。

## 项目结构

```text
.
├── Cargo.toml       # 包元数据、feature 和依赖
├── README.md        # 面向使用者，随包发布
├── develop.md       # 面向开发者，不随包发布
├── AGENTS.md        # 项目协作规则，不随包发布
├── CLAUDE.md        # 项目协作规则（Claude Code 同步副本），不随包发布
├── .github/
│   └── workflows/ci.yml # 跨平台且不连接 SMTP relay 的 CI，不随包发布
├── docs/
│   ├── module-map.md  # 工具类和公共模块定位，不随包发布
│   ├── plans/         # 设计与实施计划，不随包发布
│   ├── release-notes/ # 发布说明草稿，不随包发布
│   └── status/        # 长任务状态记录，不随包发布
└── src/
    ├── email/         # SMTP 配置、消息、错误与多实例客户端（需要 lettre feature）
    ├── lib.rs          # crate 入口和公共导出
    └── utils/
        ├── mod.rs        # 通用工具模块和公共导出
        ├── path_utils.rs  # PathUtils 实现与单元测试
        ├── random_utils.rs # RandomUtils 实现与单元测试（需要 rand feature）
        ├── reg_utils.rs  # RegUtils 实现与单元测试
        └── time_utils.rs # TimeUtils 实现与单元测试
```

## 本地开发

项目当前最低支持 Rust 1.85，要求 Rust 工具链满足 `Cargo.toml` 中声明的
`rust-version`。常用检查命令如下：

第三方依赖统一声明最低兼容版本，使用 Cargo 默认 caret 约束，不在 `version` 字段使用等号前缀
精确锁定补丁版本。`axutils` 是 library crate，不提交 `Cargo.lock` 作为依赖版本策略；依赖
下限和兼容性通过 manifest、MSRV 以及从无锁状态开始的 CI 验证。若安全修复需要提高下限，
应修改最低版本并重新执行完整验证。

```powershell
cargo fmt --all -- --check
cargo test --all-features
cargo test --doc --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo test --no-default-features
cargo check --no-default-features --features lettre
cargo check --no-default-features --features lettre,tokio
cargo test --doc --no-default-features --features lettre,tokio
cargo package --list
git diff --check
```

每个公开方法都应同时具备：

1. API doc，说明行为、输入范围和限制；
2. `# Examples` doctest，确保 README/API 示例可编译运行；
3. 覆盖正常输入和边界输入的单元测试。

新增方法时优先评估性能和安全边界；新增、删除或重命名工具类/公共模块时，必须同步维护
`docs/module-map.md` 中的职责、导出、依赖和使用场景定位。

新增 feature 时，应同步更新 `Cargo.toml`、`README.md` 和本文件，并至少验证默认
feature、`--no-default-features`、相关单 feature、组合 feature 和 `--all-features`。
邮件能力还必须验证 `tokio` 单 feature 不导出邮件 API、`lettre` 单 feature 不导出异步 API，
以及生产依赖树只包含 Rustls、`ring` 和 `webpki-roots` 方案，不包含 native-tls/OpenSSL。

邮件真实测试使用 `tests/email_live.rs`，函数固定 `#[ignore]`，且还需要一次性设置
`AXUTILS_EMAIL_LIVE_TEST=1`。测试从本地 `config/email-test.toml` 读取配置；该目录整体被
`.gitignore` 忽略，不能把账号或授权码写入源码、命令行、日志或文档。没有用户明确授权时，
不得运行 ignored 真实测试。

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

   输出应包含 `README.md` 和 `src/`，不应包含 `develop.md`、`AGENTS.md`、`CLAUDE.md` 或
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

默认 feature 为空；不依赖第三方包的能力直接可用，所有第三方能力都通过显式 feature 提供。
`rand`、`regex`、`libphonenumber` 分别用于启用可选的第三方依赖 `rand`、`regex` 和
crates.io 上的 `phonenumber`；模板、日期和邮件能力使用下列独立 feature：

```toml
[features]
default = []
rand = ["dep:rand"]
regex = ["dep:regex"]
libphonenumber = ["dep:libphonenumber"]
serde = ["dep:serde", "dep:serde_json"]
strfmt = ["dep:strfmt"]
minijinja = ["dep:minijinja"]
chrono = ["dep:chrono"]
time = ["dep:time"]
jiff = ["dep:jiff"]
lettre = ["dep:lettre"]
tokio = ["dep:tokio", "lettre?/tokio1-rustls"]
```

调用方直接依赖 `axutils = "0.1"` 即可使用 `PathUtils` 和 `TimeUtils`；需要
`RandomUtils` 时显式选择：

```toml
axutils = { version = "0.1", features = ["rand"] }
```

需要 `RegUtils` 时显式选择：

```toml
axutils = { version = "0.1", features = ["regex"] }
```

国际手机号码校验的 `RegUtils::is_phone` 需要同时启用两个 feature：

```toml
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

需要第三方包的模块，应使用与依赖包容易识别的 feature 名，并将依赖声明为
`optional = true`，再通过 `dep:<dependency-name>` 绑定。例如本项目使用
`rand = ["dep:rand"]`、`regex = ["dep:regex"]` 和
`libphonenumber = ["dep:libphonenumber"]`，并用对应的 `cfg(feature = "...")` 守卫模块、
导出和方法。

不依赖第三方包的方法属于默认能力，不添加 feature 守卫，也不额外声明可选依赖；
这类方法应直接从 crate 根模块导出。新增 feature 或公共方法时，要同步更新
`Cargo.toml`、`README.md`、API doc、doctest 和测试。

邮件模块只由 `lettre` feature 导出；`tokio` feature 通过弱依赖语法为“已经启用的
`lettre`”打开 Tokio Rustls 适配，因此单独启用 `tokio` 不会拉入 `lettre`。生产依赖不使用
`tokio` 的 `full`、`macros` 或 `rt-multi-thread`；测试和 ignored 真实测试所需 runtime
feature 只放在 dev-dependency。`lettre` 最低版本为经核验的 `0.11.22`，使用 Cargo 默认 caret
约束，允许后续兼容版本；使用的 feature 包括 `builder`、`smtp-transport`、`pool`、`rustls`、
`ring` 和 `webpki-roots`，不启用 native-tls、OpenSSL 或
机会式 STARTTLS。

CI 目标为 Windows、Linux、macOS 上的 Rust 1.85 和 stable；当前仓库只提供不连接 SMTP relay 的测试和
feature/依赖边界验证，不在 CI 中安装邮件专用系统包或连接 SMTP relay。`axutils` 是 library
crate，不新增 Dockerfile；README 中的 Debian/Ubuntu 与 Alpine Docker 内容只是消费方替换
占位符后的说明性模板。
