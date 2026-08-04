# axutils 工具类定位

本文档维护 `axutils` 中工具类和公共模块的职责边界，帮助贡献者在新增能力前找到合适的
归属位置，避免重复实现或职责交叉。涉及工具类、跨模块 API 或新增方法时，应先阅读本文档。

## 定位清单

| 工具类 | 源文件 | crate 根模块导出 | 可用条件与依赖 | 职责与主要使用场景 |
| --- | --- | --- | --- | --- |
| `PathUtils` | `src/utils/path_utils.rs` | `axutils::PathUtils`；模块为 `axutils::path_utils` | 默认可用；仅依赖 Rust 标准库 | 判断路径是否为绝对路径，获取当前工作目录和当前可执行文件路径，按平台规则拼接并词法规整多个路径；不负责文件系统存在性、权限、符号链接或真实路径解析 |
| `TimeUtils` | `src/utils/time_utils.rs`、`src/time/` | `axutils::TimeUtils`；时间戳模块为 `axutils::time_utils`，`TimeZoneOffset`、`TimeZoneOffsetError`、`TimeFormatError`、`TimeFormatToken`、`TimeValueKind` 默认从 crate 根导出 | Unix 时间戳默认可用且仅依赖标准库；日期格式化分别需要独立的 `chrono`、`time` 或 `jiff` feature | 获取当前 Unix 时间戳；按统一受限模板格式化各后端自身的日期、civil 日期时间及可选附加固定 UTC 偏移的日期时间。日期默认模板为 `yyyy-MM-dd`，含时间值默认模板为 `yyyy-MM-dd HH:mm:ss`；带偏移方法传入 `None` 时使用 `+08:00`，只有自定义模板显式包含 `XXX` 才输出偏移。固定偏移不等同于 IANA 时区，不查询 DST、不转换字段、不解析日期或执行日历运算；多个后端同时启用时必须调用带后缀 API |
| `FormatUtils` | `src/utils/format_utils.rs` | `axutils::FormatUtils`；模块为 `axutils::format_utils`；启用 `serde` 与至少一个模板后端时，`TemplateEngine` 定义于 `axutils::format_utils::TemplateEngine`，并重导出为 `axutils::utils::TemplateEngine` 和 `axutils::TemplateEngine` | `seconds_to_human` 默认可用且仅依赖标准库；模板能力须由用户显式同时启用 `serde` 和一个后端 feature（`strfmt` 或 `minijinja`）。每个后端 feature 仅启用其同名依赖；`serde` 是公共基础 feature，并启用内部所需的 `serde`、`serde_json` 依赖；`serde` 未启用或两个模板后端均未启用时不导出 `TemplateEngine` 和统一入口；单后端时枚举只包含对应变体，双后端时包含两个变体 | 将秒数格式化为中文持续时间字符串（天/小时/分钟/秒，最大单位为天，不足一天不显示天）；通过 `FormatUtils::template(template, context, default, engine)` 使用显式 `TemplateEngine` 渲染运行时模板。`TemplateEngine::Strfmt` 仅支持扁平命名变量 `{name}`；`TemplateEngine::MiniJinja` 支持 `{{ name }}`、嵌套字段、数组、条件和循环。渲染成功返回 `Some(String)`（包括空字符串），解析、序列化或渲染失败返回 `default` 的拥有副本或 `None`；不负责时区转换、周/月/年等更大单位、负数或小数秒，也不执行模板来源安全审计或资源限制 |
| `RegUtils` | `src/utils/reg_utils.rs` | 启用 `regex` feature 后提供 `axutils::RegUtils`；模块为 `axutils::reg_utils` | `regex` feature 提供模块、常见/严格邮箱和中国大陆手机号校验；可选的第三方 `regex` crate。`is_phone` 还要求独立的 `libphonenumber` feature，并通过依赖别名 `libphonenumber` 使用 crates.io 的 `phonenumber` crate | 校验常见和严格电子邮箱格式、中国大陆手机号码格式，以及启用两个 feature 后的国际 E.164 手机号码格式；只做本地格式、号段和号码类型校验，不验证地址或号码是否真实存在 |
| `RandomUtils` | `src/utils/random_utils.rs` | 启用 `rand` feature 后提供 `axutils::RandomUtils`、`axutils::LetterCase` 和 `axutils::RandomRangeError`；模块为 `axutils::random_utils` | 默认不可用；`rand` feature 提供能力，可选的第三方 `rand` crate | 生成数字、大小写字母、混合字母和数字字母 ASCII 字符串，从闭区间生成 `i64` 或可构造的有限 `f64` 随机数；字符串长度不设固定上限，调用方需限制不可信输入；不负责密码学安全随机数、密码、令牌、密钥或可复现随机序列 |
| `EmailClient` | `src/email/mod.rs`、`src/email/client.rs`、`src/email/config.rs`、`src/email/message.rs`、`src/email/error.rs` | 启用 `lettre` feature 后提供 `axutils::email::EmailClient` 和 `axutils::EmailClient`；配置、消息、错误和安全类型同时从 `axutils::email` 与 crate 根导出 | 仅显式启用 `lettre`；最低依赖版本为 `lettre 0.11.22`，关闭默认 feature，使用 `builder`、`smtp-transport`、`pool`、`rustls`、`ring`、`webpki-roots`，允许 Cargo 解析后续兼容的 `0.11.x` 版本；同时启用 `tokio` 后才提供异步方法，Tokio 最低版本为 `1.53.1` | 创建多个互不覆盖的 SMTP 账号客户端，使用强制 SMTPS 或强制 STARTTLS 发送纯文本/HTML 邮件，并复用每实例的同步/异步连接池；负责本地配置、消息规模与邮件头注入校验；不负责附件、抄送/密送、模板、DKIM、OAuth2、重试、队列、邮件接收或地址真实性验证 |
| `EmailUtils` | `src/utils/email_utils.rs` | 启用 `lettre` feature 后提供 `axutils::utils::EmailUtils` 和 `axutils::EmailUtils` | 仅随 `lettre` feature 导出，复用 `EmailClient`；异步方法要求 `lettre` 与 `tokio` 同时启用；Tokio 通过 `lettre?/tokio1-rustls` 弱依赖适配，不会被单独 `tokio` 激活 | 单默认账号的一次初始化全局便捷入口，提供初始化状态、同步发送和组合 feature 下的异步发送；不可 reset/replace，不能替代多账号实例生命周期管理；与 `RegUtils` 的地址格式校验无依赖关系 |
| `ConfigLoader` | `src/config/{mod,error,format,value,de,source,json,env,yaml,toml,ini}.rs` | 启用 `serde` feature 后提供 `axutils::ConfigLoader`、`axutils::ConfigFormat`、`axutils::ConfigValue`、`axutils::ConfigError`，模块为 `axutils::config`；YAML/TOML/INI 变体和后端分别需要额外启用 `serde-saphyr`/`toml`/`rust-ini` feature；同时启用 `tokio` feature 后，文件异步方法还要求 `serde + tokio`，Tokio 最低版本为 `1.53.1` | `serde` 提供 JSON 与自实现 `.env`（dotenv）解析，不需要额外第三方依赖（`serde_json` 已随 `serde` feature 引入）；`serde-saphyr`（YAML，嵌套深度 1–256、总别名回放事件最多 1,000,000 次、单个 anchor 最多展开 10,000 次）、`toml`（TOML）、`rust-ini`（INI）三个后端 feature 相互独立，仅通过 `dep:<name>` 启用同名依赖，最低版本分别为 `serde-saphyr 1.0.0`（要求 Rust 1.88，是本 crate 当前 `rust-version` 的来源）、`toml 1.1.4`、`rust-ini 0.21.3`；Tokio 生产依赖仅启用 `fs`/`io-util`，不包含 runtime 或宏 feature | 从磁盘或内存字符串按扩展名/显式指定读取单个配置文件，提供无类型 `ConfigValue` 与有类型 `serde::Deserialize` 两条路径；文件大小（1 KiB–16 MiB，默认 1 MiB）统一限制，JSON/TOML/YAML/INI 的无类型读取以及 YAML/INI 的有类型读取使用可配置嵌套深度（1–256，默认 64），JSON/TOML 有类型读取依赖后端自身递归保护；`serde + tokio` 下额外提供 `load_value_async`/`load_async`，异步路径只替换文件读取并复用解析器，不创建 runtime 或调用 `block_on`；错误不回显配置值或原始行内容；不做多文件合并、层叠覆盖、热重载、写回或 `include` 指令；`.env` 插值仅支持 `${VAR}`，变量名遵守 `[A-Za-z_][A-Za-z0-9_]*`，文件内键优先于进程环境变量，未定义变量报错而非空串，与 `dotenv`/`dotenvy` 存在已知差异 |
| `ConfigUtils` | `src/utils/config_utils.rs` | 启用 `serde` feature 后提供 `axutils::utils::ConfigUtils` 和 `axutils::ConfigUtils`；同时启用 `tokio` 后提供四个异步文件包装，实际要求 `serde + tokio` | 与 `ConfigLoader` 使用同一组 feature；无状态静态方法，等价于默认 `ConfigLoader` | 配置文件读取的静态便捷入口；需要自定义大小/深度上限或关闭 `.env` 环境回退时通过 `ConfigUtils::loader()` 获取可配置的 `ConfigLoader`；不引入全局单例、缓存或可变全局状态，与 `EmailUtils` 的一次初始化语义不同；异步入口不负责 runtime、CPU 解析调度或全局并发预算 |
| `CryptoUtils` | `src/utils/crypto_utils.rs`、`src/crypto/{mod,error,text,hex,base64,md5,aes}.rs` | `CryptoUtils`：`axutils::CryptoUtils`、`axutils::utils::CryptoUtils`、`axutils::crypto_utils::CryptoUtils`、`axutils::utils::crypto_utils::CryptoUtils`；`CryptoError`/`TextEncoding`：`axutils::CryptoError`/`axutils::TextEncoding` 与 `axutils::crypto::CryptoError`/`axutils::crypto::TextEncoding`；`Base64Alphabet`/`Base64Options`（`base64`）：`axutils::*` 与 `axutils::crypto::*`；`AesKey`/`AesKeyBits`/`AesMode`（`aes`）：`axutils::*` 与 `axutils::crypto::*`；公开模块：`axutils::crypto`、`axutils::crypto_utils`、`axutils::utils::crypto_utils` | 十六进制编解码（`hex_encode`/`hex_encode_upper`/`hex_decode`）与 `TextEncoding::Utf8` 文本编解码**默认可用**，仅依赖标准库，与 `PathUtils`/`FormatUtils::seconds_to_human` 同类；Base64/MD5/AES 分别需要显式启用 `base64`/`md5`/`aes` feature；`md5` feature 实际启用 crates.io 上的 `md-5` crate（别名映射，**不是**同名的旧 `md5` crate）；`aes` feature 聚合 `aes`/`aes-gcm`/`cbc`/`zeroize` 四个内部适配依赖；`encoding_rs` feature 为 `TextEncoding` 追加 `Gbk`/`Gb18030`/`Big5`/`ShiftJis`/`EucKr`/`Windows1252` 六个 legacy 变体；`aes` 与 `base64` 同时启用后额外提供 `aes_encrypt_base64`/`aes_decrypt_base64` | 把内存中的一段数据安全地编码（十六进制、Base64）、摘要（MD5）或加解密（AES-128/192/256，GCM 认证加密或 CBC+PKCS#7 互操作模式，随机或调用方显式提供的 IV/nonce）；错误（`CryptoError`）不回显明文、密文、密钥、IV 或原始文本内容，只包含长度、位置偏移和编码名称；不提供非对称密码学、口令派生（KDF）、密钥生命周期管理、流式/文件接口或 AAD；MD5 不可用于对抗性场景，CBC 无完整性认证；与 `RandomUtils` 无共用代码，`RandomUtils` 不提供密码学安全随机数，AES 的随机 IV/nonce/密钥生成仅使用操作系统随机源 |

## 新增工具类时的定位要求

新增工具类或公共模块时，应在同一变更中补充以下信息：

1. 源文件路径和 crate 根模块的公共导出路径；
2. 是否默认可用、对应 feature 以及第三方依赖；
3. 单一且清晰的职责边界、主要使用场景和明确不负责的范围；
4. 与现有工具类的关系，尤其是可能重叠的 API 和复用方式。

如果工具类的职责、公共导出、feature、依赖或适用范围发生变化，也必须同步更新本清单。

## 使用示例文档

每个公共能力单元在 `docs/examples/` 维护详细使用文档（命名取能力前缀、不带 `Utils`
后缀），作为 README 简单示例的补充：

| 能力单元 | 使用文档 |
| --- | --- |
| `PathUtils` | `docs/examples/path.md` |
| `TimeUtils` 与时间类型 | `docs/examples/time.md` |
| `FormatUtils` 与 `TemplateEngine` | `docs/examples/format.md` |
| `RegUtils` | `docs/examples/reg.md` |
| `RandomUtils` 与相关类型 | `docs/examples/random.md` |
| `EmailClient`、email 配置/消息/错误类型 | `docs/examples/email.md` |
| `EmailUtils`（`axutils::EmailUtils`、`axutils::utils::EmailUtils`、`axutils::utils::email_utils::EmailUtils`） | `docs/examples/email.md` |
| `ConfigLoader`、配置格式/值/错误类型 | `docs/examples/config.md` |
| `ConfigUtils`（`axutils::ConfigUtils`、`axutils::utils::ConfigUtils`、`axutils::utils::config_utils::ConfigUtils`） | `docs/examples/config.md` |
| `CryptoUtils` 与 `crypto` 类型（`CryptoError`、`TextEncoding`、`Base64Alphabet`、`Base64Options`、`AesKey`、`AesKeyBits`、`AesMode`） | `docs/examples/crypto.md` |

新增、删除或重命名能力单元时，必须同步更新本表与对应文档；文档随 crate 发布。
