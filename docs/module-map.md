# axutils 工具类定位

本文档维护 `axutils` 中工具类和公共模块的职责边界，帮助贡献者在新增能力前找到合适的
归属位置，避免重复实现或职责交叉。涉及工具类、跨模块 API 或新增方法时，应先阅读本文档。

## 定位清单

| 工具类 | 源文件 | crate 根模块导出 | 可用条件与依赖 | 职责与主要使用场景 |
| --- | --- | --- | --- | --- |
| `PathUtils` | `src/utils/path_utils.rs` | `axutils::PathUtils`；模块为 `axutils::path_utils` | 默认可用；仅依赖 Rust 标准库 | 判断路径是否为绝对路径，获取当前工作目录和当前可执行文件路径，按平台规则拼接并词法规整多个路径；不负责文件系统存在性、权限、符号链接或真实路径解析 |
| `TimeUtils` | `src/utils/time_utils.rs`、`src/time/` | `axutils::TimeUtils`；时间戳模块为 `axutils::time_utils`，`TimeZoneOffset`、`TimeZoneOffsetError`、`TimeFormatError`、`TimeFormatToken`、`TimeValueKind` 默认从 crate 根导出 | Unix 时间戳默认可用且仅依赖标准库；日期格式化分别需要独立的 `chrono`、`time` 或 `jiff` feature | 获取当前 Unix 时间戳；按统一受限模板格式化各后端自身的日期、civil 日期时间及可选附加固定 UTC 偏移的日期时间。日期默认模板为 `yyyy-MM-dd`，含时间值默认模板为 `yyyy-MM-dd HH:mm:ss`；带偏移方法传入 `None` 时使用 `+08:00`，只有自定义模板显式包含 `XXX` 才输出偏移。固定偏移不等同于 IANA 时区，不查询 DST、不转换字段、不解析日期或执行日历运算；多个后端同时启用时必须调用带后缀 API |
| `FormatUtils` | `src/utils/format_utils.rs` | `axutils::FormatUtils`；模块为 `axutils::format_utils` | `seconds_to_human` 默认可用且仅依赖标准库；模板能力须由用户显式同时启用 `serde` 和一个后端 feature（`strfmt` 或 `minijinja`）。每个后端 feature 仅启用其同名依赖；`serde` 是公共基础 feature，并启用内部所需的 `serde`、`serde_json` 依赖 | 将秒数格式化为中文持续时间字符串（天/小时/分钟/秒，最大单位为天，不足一天不显示天）；可选地渲染运行时模板。`strfmt` 仅支持扁平命名变量 `{name}`，MiniJinja 支持 `{{ name }}`、嵌套字段、数组、条件和循环；不负责时区转换、周/月/年等更大单位、负数或小数秒，也不执行模板来源安全审计或资源限制 |
| `RegUtils` | `src/utils/reg_utils.rs` | 启用 `regex` feature 后提供 `axutils::RegUtils`；模块为 `axutils::reg_utils` | `regex` feature 提供模块、常见/严格邮箱和中国大陆手机号校验；可选的第三方 `regex` crate。`is_phone` 还要求独立的 `libphonenumber` feature，并通过依赖别名 `libphonenumber` 使用 crates.io 的 `phonenumber` crate | 校验常见和严格电子邮箱格式、中国大陆手机号码格式，以及启用两个 feature 后的国际 E.164 手机号码格式；只做本地格式、号段和号码类型校验，不验证地址或号码是否真实存在 |
| `RandomUtils` | `src/utils/random_utils.rs` | 启用 `rand` feature 后提供 `axutils::RandomUtils`、`axutils::LetterCase` 和 `axutils::RandomRangeError`；模块为 `axutils::random_utils` | 默认不可用；`rand` feature 提供能力，可选的第三方 `rand` crate | 生成数字、大小写字母、混合字母和数字字母 ASCII 字符串，从闭区间生成 `i64` 或可构造的有限 `f64` 随机数；字符串长度不设固定上限，调用方需限制不可信输入；不负责密码学安全随机数、密码、令牌、密钥或可复现随机序列 |
| `EmailClient` | `src/email/mod.rs`、`src/email/client.rs`、`src/email/config.rs`、`src/email/message.rs`、`src/email/error.rs` | 启用 `lettre` feature 后提供 `axutils::email::EmailClient` 和 `axutils::EmailClient`；配置、消息、错误和安全类型同时从 `axutils::email` 与 crate 根导出 | 仅显式启用 `lettre`；最低依赖版本为 `lettre 0.11.22`，关闭默认 feature，使用 `builder`、`smtp-transport`、`pool`、`rustls`、`ring`、`webpki-roots`，允许 Cargo 解析后续兼容的 `0.11.x` 版本；同时启用 `tokio` 后才提供异步方法，Tokio 最低版本为 `1.53.1` | 创建多个互不覆盖的 SMTP 账号客户端，使用强制 SMTPS 或强制 STARTTLS 发送纯文本/HTML 邮件，并复用每实例的同步/异步连接池；负责本地配置、消息规模与邮件头注入校验；不负责附件、抄送/密送、模板、DKIM、OAuth2、重试、队列、邮件接收或地址真实性验证 |
| `EmailUtils` | `src/utils/email_utils.rs` | 启用 `lettre` feature 后提供 `axutils::utils::EmailUtils` 和 `axutils::EmailUtils` | 仅随 `lettre` feature 导出，复用 `EmailClient`；异步方法要求 `lettre` 与 `tokio` 同时启用；Tokio 通过 `lettre?/tokio1-rustls` 弱依赖适配，不会被单独 `tokio` 激活 | 单默认账号的一次初始化全局便捷入口，提供初始化状态、同步发送和组合 feature 下的异步发送；不可 reset/replace，不能替代多账号实例生命周期管理；与 `RegUtils` 的地址格式校验无依赖关系 |

## 新增工具类时的定位要求

新增工具类或公共模块时，应在同一变更中补充以下信息：

1. 源文件路径和 crate 根模块的公共导出路径；
2. 是否默认可用、对应 feature 以及第三方依赖；
3. 单一且清晰的职责边界、主要使用场景和明确不负责的范围；
4. 与现有工具类的关系，尤其是可能重叠的 API 和复用方式。

如果工具类的职责、公共导出、feature、依赖或适用范围发生变化，也必须同步更新本清单。
