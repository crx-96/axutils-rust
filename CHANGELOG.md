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
  `minijinja` feature 组合提供运行时模板渲染。
- 新增 `RegUtils`，在 `regex` feature 下提供常见/严格电子邮箱和中国大陆手机号码格式校验；同时启用
  `libphonenumber` feature 后提供国际 E.164 手机号码校验。
- 新增 `RandomUtils`、`LetterCase` 和 `RandomRangeError`，在 `rand` feature 下提供 ASCII 随机字符串、
  `i64` 闭区间和有限 `f64` 范围随机数能力。
- 新增 SMTP 邮件能力，在 `lettre` feature 下提供 `EmailConfig`、`EmailMessage`、`EmailClient`、
  `EmailUtils`、`EmailError` 等类型；支持多个独立客户端、同步连接池和一次初始化的单默认账号入口。
- 新增 `tokio` feature 下的异步邮件发送能力；异步邮件要求调用方同时启用 `lettre` 和 `tokio`，并自行
  提供 Tokio runtime。

### Changed

- 最低支持 Rust 版本为 1.85。
- 邮件传输固定使用 Rustls、`ring` 和 `webpki-roots`，支持强制 SMTPS 或强制 STARTTLS，不使用
  native-tls/OpenSSL。

### Security and compatibility

- 邮件配置、邮件头和正文执行大小及控制字符校验；错误信息不会回显密码、主题、正文、用户名、完整主机名
  或地址。
- 不支持明文 SMTP、机会式 STARTTLS、跳过证书校验、自签名证书、企业私有 CA relay、附件、抄送/密送、
  DKIM、OAuth2、自动重试、后台队列或邮件接收。
- `RandomUtils` 不承诺密码学安全随机数，不应用于密码、令牌或密钥生成；不可信输入的随机字符串长度应由调用方限制。
