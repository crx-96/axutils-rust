# axutils 开发者文档

本文档面向项目维护者和贡献者，不属于 crates.io 发布包。`Cargo.toml` 使用
`package.include` 白名单，将源码、`README.md`、`CHANGELOG.md`、`LICENSE`、Cargo 配置和
`docs/examples/` 打入发布包；`develop.md`、`AGENTS.md`、`CLAUDE.md` 以及 `docs/` 中的计划、
状态和其他开发资料不随包发布。

## 项目结构

```text
.
├── Cargo.toml       # 包元数据、feature 和依赖
├── README.md        # 面向使用者，随包发布
├── CHANGELOG.md     # 面向使用者的版本变更记录，随包发布
├── develop.md       # 面向开发者，不随包发布
├── AGENTS.md        # 项目协作规则，不随包发布
├── CLAUDE.md        # 项目协作规则（Claude Code 同步副本），不随包发布
├── docs/
│   ├── examples/     # 模块详细使用文档，随包发布
│   ├── module-map.md  # 工具类和公共模块定位，不随包发布
│   ├── plans/         # 设计与实施计划，不随包发布
│   └── status/        # 长任务状态记录，不随包发布
└── src/
    ├── config/         # 配置文件读取后端（需要 serde 及对应格式 feature）
    ├── crypto/         # 十六进制/TextEncoding 默认可用；Base64/MD5/AES 需要对应 feature
    ├── allocator.rs    # mimalloc/rpmalloc 的私有全局分配器选择（互斥 feature）
    ├── email/         # SMTP 配置、消息、错误与多实例客户端（需要 lettre feature）
    ├── lib.rs          # crate 入口和公共导出
    ├── redis/          # Redis 配置、客户端、命令、事务与错误（需要 redis feature）
    └── utils/
        ├── mod.rs        # 通用工具模块和公共导出
        ├── path_utils.rs  # PathUtils 实现与单元测试
        ├── random_utils.rs # RandomUtils 实现与单元测试（需要 rand feature）
        ├── redis_utils.rs # RedisUtils 一次初始化的全局转发（需要 redis feature）
        ├── reg_utils.rs  # RegUtils 实现与单元测试
        ├── time_utils.rs # TimeUtils 实现与单元测试
        ├── config_utils.rs # ConfigUtils 静态配置读取入口（需要 serde feature）
        └── crypto_utils.rs # CryptoUtils 静态入口（十六进制/TextEncoding 默认可用）
```

JWT 子树：

```text
src/
+-- jwt/                  # JWS algorithm/key/config/claims/codec/error
+-- utils/
    +-- jwt_utils.rs      # 一次初始化的 JwtUtils 全局转发
```

项目结构中 JWT 相关路径为 `src/jwt/`（算法、key、配置、Header、claims、clock、codec、错误）和
`src/utils/jwt_utils.rs`（一次初始化的全局转发），均仅在 `jwt` feature 下编译。

## 本地开发

项目当前最低支持 Rust 1.88，要求 Rust 工具链满足 `Cargo.toml` 中声明的
`rust-version`。常用检查命令如下：

第三方依赖统一声明最低兼容版本，使用 Cargo 默认 caret 约束，不在 `version` 字段使用等号前缀
精确锁定补丁版本。`axutils` 是 library crate，不提交 `Cargo.lock` 作为依赖版本策略；依赖
下限和兼容性通过 manifest、MSRV 以及从无锁状态开始的依赖解析验证。若安全修复需要提高下限，
应修改最低版本并重新执行完整验证。

```powershell
# 日常修改后的快速反馈：仅运行默认能力的库单元测试。
cargo test --no-default-features --lib

# 需要检查某个可选模块时，只编译并运行对应 feature 的测试目标，例如：
cargo test --no-default-features --features http --test http
cargo test --no-default-features --features redis --test redis
cargo test --no-default-features --features redis,tokio --doc
cargo tree --no-default-features --features redis -e normal,build

# 需要覆盖所有已启用模块、feature/API 依赖边界和文档时，使用下面的完整清单；其中
# feature/API/依赖边界矩阵是慢速测试，默认 cargo test 会跳过。allocator 后端必须分别验证；
# mimalloc + rpmalloc 是预期失败组合，不能把 --all-features 作为成功验收命令。
cargo fmt --all -- --check
cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
cargo test --no-default-features
cargo test --no-default-features --features mimalloc
cargo test --no-default-features --features rpmalloc
cargo test --no-default-features --features mimalloc --doc
cargo test --no-default-features --features rpmalloc --doc
cargo clippy --all-targets --no-default-features --features mimalloc -- -D warnings
cargo clippy --all-targets --no-default-features --features rpmalloc -- -D warnings
cargo tree --no-default-features --features mimalloc -e normal,build,features
cargo tree --no-default-features --features rpmalloc -e normal,build,features
cargo check --no-default-features --features lettre
cargo check --no-default-features --features lettre,tokio
cargo test --doc --no-default-features --features lettre,tokio
cargo check --no-default-features --features tokio
cargo check --no-default-features --features serde
cargo check --no-default-features --features serde,tokio
cargo check --no-default-features --features serde,tokio,toml,serde-saphyr,rust-ini
cargo check --no-default-features --features encoding_rs
cargo check --no-default-features --features base64
cargo check --no-default-features --features md5
cargo check --no-default-features --features aes
cargo check --no-default-features --features aes,base64
cargo check --no-default-features --features base64,md5,aes,encoding_rs
cargo test --no-default-features --features base64,md5,aes,encoding_rs
cargo check --no-default-features --features jwt
cargo test --no-default-features --features jwt
cargo test --no-default-features --features jwt --test jwt_global -- --test-threads=1
cargo tree --no-default-features --features jwt -e normal,build
cargo tree --no-default-features --features jwt -e normal,build,features
cargo tree --no-default-features --features tokio -e normal,build
cargo tree --no-default-features --features serde,tokio -e normal,build
cargo tree --no-default-features --features tokio -e features
cargo tree --no-default-features --features base64 -e normal
cargo tree --no-default-features --features md5 -e normal
cargo tree --no-default-features --features aes -e normal
cargo tree --no-default-features --features encoding_rs -e normal
cargo package --list
git diff --check
```

allocator 的负向契约单独执行，预期以非零状态退出并包含固定诊断；该命令不属于上面的成功清单：

```powershell
cargo check --no-default-features --features mimalloc,rpmalloc
```

每个公开方法都应同时具备：

1. API doc，说明行为、输入范围和限制；
2. `# Examples` doctest，确保 README/API 示例可编译运行；
3. 覆盖正常输入和边界输入的单元测试；
4. 在对应 `docs/examples/<前缀>.md` 中维护独立的方法小节、参数/返回值说明和可编译示例。

新增方法时优先评估性能和安全边界；新增、删除或重命名工具类/公共模块时，必须同步维护
`docs/module-map.md` 中的职责、导出、依赖和使用场景定位。

新增普通 feature 时，应同步更新 `Cargo.toml`、`README.md`、`CHANGELOG.md` 和本文件，并至少验证默认
feature、`--no-default-features`、相关单 feature、组合 feature 和适用的 `--all-features`。`mimalloc`
与 `rpmalloc` 是有意互斥的进程级 allocator feature，必须分别验证单 feature、依赖边界和下游
重复注册失败；双 feature 及由此触发的 `--all-features` 组合属于预期编译失败。
配置读取能力以 `serde` 为基础 feature；YAML、TOML、INI 分别还需要
`serde-saphyr`、`toml`、`rust-ini`，且单独启用这些后端 feature 时不得导出配置 API。
`CryptoUtils` 本身与十六进制/`TextEncoding::Utf8` 文本编解码能力默认可用（不依赖任何第三方
crate，与 `PathUtils` 同类）；`base64`/`md5`/`aes` 各自解锁对应算法，`encoding_rs` 单独启用会
为 `TextEncoding` 追加 legacy 编码变体（不是零效果的空 feature），`aes + base64` 才有
`aes_encrypt_base64`/`aes_decrypt_base64` 便捷方法。

配置文件异步读取要求调用方显式同时启用 `serde` 与 `tokio`；`tokio` 单 feature 不导出配置
模块，`serde` 单 feature 只提供同步入口。异步入口只替换受限文件读取，不创建 runtime 或
调用 `block_on`，解析阶段仍在当前 Tokio worker 中执行；测试需覆盖 `ConfigUtils` 四个包装、
`ConfigLoader` 两个方法、显式格式、大小/深度/BOM/UTF-8/错误脱敏和各格式后端。
邮件能力还必须验证 `tokio` 单 feature 不导出邮件 API、`lettre` 单 feature 不导出异步 API，
以及生产依赖树只包含 Rustls、`ring` 和 `webpki-roots` 方案，不包含 native-tls/OpenSSL。

邮件真实测试使用 `tests/email_live.rs`，函数固定 `#[ignore]`，且还需要一次性设置
`AXUTILS_EMAIL_LIVE_TEST=1`。测试从本地 `config/email-test.toml` 读取配置；该目录整体被
`.gitignore` 忽略，不能把账号或授权码写入源码、命令行、日志或文档。没有用户明确授权时，
不得运行 ignored 真实测试。

Redis 单机真实测试使用 `tests/redis_live.rs`，同步/异步测试固定 `#[ignore]`，并要求一次性
设置 `AXUTILS_REDIS_LIVE_TEST=1` 以及本地被忽略的 `config/redis-test.toml`；显式运行 ignored
测试时，缺少环境变量、配置文件或必填字段会明确失败，不会伪装成测试通过。Redis Cluster
测试使用 `tests/redis_cluster.rs` 和本地 `127.0.0.1:7000-7002` fixture，同样固定 `#[ignore]`。
没有用户明确授权、受控服务和本地配置时，不得运行这些 Redis ignored 真实测试；真实配置、
凭据和节点地址不得写入源码、命令行、日志或文档，测试内置的 loopback fixture 除外。

## 发布步骤

1. 确认工作区只包含本次发布需要的修改，并在 `Cargo.toml` 中更新 `version`。
2. 读取 `Cargo.toml` 中的 `[package].version`，在 `CHANGELOG.md` 中新增或更新对应版本条目，记录
   本次新增功能、行为变化、问题修复和兼容性影响。
3. 更新 `README.md`、公开 API 文档和测试，确保示例反映当前 API。
4. 运行完整验证：

   ```powershell
   cargo fmt --all -- --check
   cargo test --no-default-features --test feature_matrix -- --ignored --test-threads=1
   cargo test --no-default-features --features mimalloc
   cargo test --no-default-features --features rpmalloc
   cargo test --no-default-features --features mimalloc --doc
   cargo test --no-default-features --features rpmalloc --doc
   cargo clippy --all-targets --no-default-features --features mimalloc -- -D warnings
   cargo clippy --all-targets --no-default-features --features rpmalloc -- -D warnings
   ```

5. 检查发布包文件清单，确认详细使用文档已包含且开发者文档没有被包含：

   ```powershell
   cargo package --list
   cargo package --allow-dirty --list
   ```

   输出应包含 `README.md`、`CHANGELOG.md`、`src/` 和 `docs/examples/`，并按
   `docs/module-map.md` 的「使用示例文档」映射表逐项确认所有模块文档均在；
   不应包含 `develop.md`、`AGENTS.md`、`CLAUDE.md`、`docs/plans/`、`docs/status/` 或 `docs/skills/`。

6. 先执行发布 dry-run：

   ```powershell
   cargo publish --dry-run
   ```

7. 使用已配置的 crates.io 身份发布。首次使用时通过 `cargo login` 配置 token，
   不要将 token 写入仓库或文档：

   ```powershell
   cargo publish
   ```

8. 发布成功后创建并推送与版本一致的 Git tag，例如 `v0.1.0`，再在 GitHub 上记录
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
toml = ["dep:toml"]
serde-saphyr = ["dep:serde-saphyr"]
rust-ini = ["dep:rust-ini"]
base64 = ["dep:base64"]
md5 = ["dep:md5"]
aes = ["dep:aes", "dep:aes-gcm", "dep:cbc", "dep:zeroize"]
encoding_rs = ["dep:encoding_rs"]
jwt = ["dep:jsonwebtoken", "dep:serde", "dep:serde_json"]
mimalloc = ["dep:mimalloc"]
rpmalloc = ["dep:rpmalloc"]
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
`Cargo.toml`、`README.md`、`CHANGELOG.md`、API doc、doctest 和测试。

`mimalloc` 与 `rpmalloc` 是可选依赖对应的互斥 feature；它们只负责在 crate 私有模块中注册
唯一的 `#[global_allocator]`，不启用其他项目 feature，也不提供运行时 API。启用 allocator
feature 后，最终 binary 只能保留一个 global allocator；现有 `--all-features` 组合会同时打开
两个后端，必须按预期失败处理。正向 fixture 需完成 `cargo run` 的最终链接和运行，负向 fixture
需覆盖双后端及下游额外 `System` 注册。

邮件模块只由 `lettre` feature 导出；`tokio` feature 通过弱依赖语法为“已经启用的
`lettre`”打开 Tokio Rustls 适配，因此单独启用 `tokio` 不会拉入 `lettre`。同一个 `tokio`
feature 也为 `serde + tokio` 配置异步入口提供 `fs`/`io-util`；生产依赖不使用 `tokio` 的
`full`、`macros` 或 `rt-multi-thread`，测试和 ignored 真实测试所需 runtime feature 只放在
dev-dependency。`lettre` 最低版本为经核验的 `0.11.22`，使用 Cargo 默认 caret
约束，允许后续兼容版本；使用的 feature 包括 `builder`、`smtp-transport`、`pool`、`rustls`、
`ring` 和 `webpki-roots`，不启用 native-tls、OpenSSL 或
机会式 STARTTLS。

本仓库不再维护 GitHub Actions CI 工作流。跨平台验证需要维护者在 Windows、Linux、macOS 的 Rust 1.88
和 stable 环境中自行执行；mimalloc/rpmalloc 还需要目标平台可用的 C compiler、linker 和 SDK，缺少
时不得自动修改系统工具链。当前仓库只提供不连接 SMTP relay 的测试和 feature/依赖边界验证，不会连接
SMTP relay。`axutils` 是 library crate，不新增 Dockerfile；README 中的 Debian/Ubuntu 与 Alpine Docker
内容只是消费方替换占位符后的说明性模板。

JWT 使用 `jsonwebtoken 11.0.0` 的 `rust_crypto` 与 `use_pem` features，依赖声明为 optional，
crate 自身不启用 `aws_lc_rs`。`jwt` feature 直接启用 `dep:jsonwebtoken`、`dep:serde` 和
`dep:serde_json`，但不引用项目已有的 `serde` feature，因此仅启用 `jwt` 时不导出配置模块。
JWT 的领域实现必须留在 `src/jwt/`，`src/utils/jwt_utils.rs` 只负责 `OnceLock` 生命周期和转发。
`tests/fixtures/jwt_feature_matrix/` 负责公共路径与 feature 边界，`tests/jwt_codec.rs` 通过测试编译时
源码模块复用覆盖 fixture 依赖的私有 codec/非对称算法，`tests/jwt_global.rs` 只有一个全局单例测试入口；
fixture 和 `tests/` 不进入发布包。
