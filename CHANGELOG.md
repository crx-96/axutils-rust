# Changelog

本文件仅记录 `axutils` 各版本的源码、公共 API、运行时行为、错误与安全边界，以及面向使用者的兼容性变化。
每次修改或增加功能时，先读取 `Cargo.toml` 中的 `[package].version`，再在对应版本条目中补充记录。

## [0.1.0]

### Added

- 建立按 feature 组织的 Rust 工具库；默认 feature 为空，不会自动引入第三方依赖。
- 新增 `PathUtils`，提供绝对路径判断、当前工作目录和可执行文件路径获取，以及不访问文件系统的多路径词法拼接。
- 新增 `TimeUtils`，提供当前 Unix 时间戳获取能力；通过独立的 `chrono`、`time` 或 `jiff` feature
  提供日期、日期时间和固定 UTC 偏移格式化能力。
- 新增 `FormatUtils`，提供将秒数格式化为中文持续时间的默认能力；通过 `serde` 与 `strfmt` 或
  `minijinja` feature 组合提供运行时模板渲染，并通过
  `FormatUtils::template(template, context, default, engine)` 显式选择 `TemplateEngine::Strfmt`
  或 `TemplateEngine::MiniJinja` 模板后端；缺少 `serde` 或未启用任何模板后端时不导出该统一
  入口和 `TemplateEngine`。
- 新增 `RegUtils`，在 `regex` feature 下提供常见/严格电子邮箱和中国大陆手机号码格式校验；同时启用
  `libphonenumber` feature 后提供国际 E.164 手机号码校验。
- 新增 `RandomUtils`、`LetterCase` 和 `RandomRangeError`，在 `rand` feature 下提供 ASCII 随机字符串、
  `i64` 闭区间和有限 `f64` 范围随机数能力。
- 新增 SMTP 邮件能力，在 `lettre` feature 下提供 `EmailConfig`、`EmailMessage`、`EmailClient`、
  `EmailUtils`、`EmailError` 等类型；支持多个独立客户端、同步连接池和一次初始化的单默认账号入口。
- 新增 `tokio` feature 下的异步邮件发送能力；异步邮件要求调用方同时启用 `lettre` 和 `tokio`，并自行
  提供 Tokio runtime。
- 新增统一的配置文件读取能力：`ConfigLoader`（`src/config/`）与静态便捷入口 `ConfigUtils`
  （`src/utils/config_utils.rs`），支持 JSON、YAML、TOML、INI 和 `.env`（dotenv）五种格式。`serde`
  feature 下即可读取 JSON 与自实现的 `.env`；YAML、TOML、INI 分别需要额外启用 `serde-saphyr`、
  `toml`、`rust-ini` feature。每种格式都提供无类型 `ConfigValue`（点号路径访问，例如
  `"server.tls.port"`）与有类型 `serde::Deserialize` 两条读取路径，共享同一套文件大小
  （1 KiB–16 MiB，默认 1 MiB）上限；JSON/TOML/YAML/INI 的无类型读取以及 YAML/INI 的有类型
  读取使用统一的嵌套深度上限（1–256，默认 64），JSON/TOML 有类型读取使用各自后端的递归保护。
- 新增 `serde,tokio` feature 组合下的异步配置文件读取：`ConfigLoader` 提供
  `load_value_async`/`load_async`，`ConfigUtils` 提供 `load_value_async`/`load_async`/
  `load_value_as_async`/`load_as_async`；异步读取复用现有格式解析、大小上限、BOM、UTF-8、深度、
  `.env` 回退和错误脱敏语义，Tokio 生产依赖仅增加 `fs`/`io-util` 能力，不创建 runtime。

### Changed

- 最低支持 Rust 版本从 1.85 提升为 1.88。原因：新增的 YAML 后端 `serde-saphyr 1.0.0`
  （2026-07-31 发布）声明 `edition = "2024"` 并使用了 let-chains 语法，实测在 Rust 1.85 下无法
  编译（27 个 `E0658` 错误），Rust 1.88（let-chains 稳定化版本）起可正常编译；`toml`（自身要求
  1.85）与 `rust-ini`（自身要求 1.64）不受影响。
- 邮件传输固定使用 Rustls、`ring` 和 `webpki-roots`，支持强制 SMTPS 或强制 STARTTLS，不使用
  native-tls/OpenSSL。

### Fixed

- 配置读取的 YAML 后端显式固定有限的别名回放预算（总回放事件最多 1,000,000 次、单个 anchor
  最多展开 10,000 次）；INI/`.env` 类型化读取在违反 serde map 访问顺序时返回错误，不再触发
  内部 panic。
- 配置解析拒绝 JSON 根值后的尾随非空内容；INI section 构建遵守配置的嵌套深度上限；`.env` 插值
  变量名遵守 `[A-Za-z_][A-Za-z0-9_]*` 规则。

### Security and compatibility

- 邮件配置、邮件头和正文执行大小及控制字符校验；错误信息不会回显密码、主题、正文、用户名、完整主机名
  或地址。
- 不支持明文 SMTP、机会式 STARTTLS、跳过证书校验、自签名证书、企业私有 CA relay、附件、抄送/密送、
  DKIM、OAuth2、自动重试、后台队列或邮件接收。
- `RandomUtils` 不承诺密码学安全随机数，不应用于密码、令牌或密钥生成；不可信输入的随机字符串长度应由调用方限制。
- 配置读取的错误类型不回显配置文件的原始内容、解析出的值或原始出错行文本；文件大小采用
  `Read::take` 流式截断而非依赖 `fs::metadata`，避免 TOCTOU 及命名管道一类特殊文件耗尽内存。
- YAML 后端显式设置别名展开预算（`serde-saphyr` 的 `Budget`/`AliasLimits`）以防御别名炸弹
  （billion laughs）类拒绝服务输入；TOML 语法本身禁止重复键，YAML 显式配置为拒绝重复键，INI 与
  `.env` 由本 crate 检测重复键，`serde_json` 的“后者覆盖”语义无法配置，五种格式在该点上行为不完全
  一致，已在文档中如实说明。
- `.env` 的 `${VAR}` 插值优先使用文件中已解析的键，找不到时可选择性回退到进程环境变量（默认允许，
  可通过 `ConfigLoader::with_env_substitution(false)` 关闭）；未定义变量返回错误而不会静默替换为
  空字符串；本 crate 只读取文件与（可选）进程环境变量，不写入、不合并、不修改进程环境。这些语义与
  `dotenv`/`dotenvy` 存在已知差异，不声称与其完全兼容。
