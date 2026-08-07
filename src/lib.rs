//! `axutils` 是一个按 feature 组织的 Rust 常用工具库。
//!
//! 默认不启用第三方依赖，因此 `PathUtils`、`TimeUtils` 和 `FormatUtils` 的持续时间格式化
//! 能力可以直接使用。
//! 需要发送 SMTP 邮件时，显式启用 `lettre` feature；它提供强制 SMTPS/STARTTLS、连接池、
//! 多实例 `EmailClient` 和一次初始化的全局 `EmailUtils`。如果还要使用异步发送，必须
//! 同时启用 `lettre` 与 `tokio` feature，异步调用方需要自行运行在 Tokio runtime 中。
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["lettre"] }
//! # 异步邮件改为 features = ["lettre", "tokio"]，并由调用方提供 Tokio runtime。
//! ```
//! 需要随机工具时，
//! 通过 `rand` feature 显式启用 `RandomUtils`；需要邮箱和中国大陆手机号码校验时，
//! 通过 `regex` feature 显式启用 `RegUtils`；`is_phone` 还需要同时启用独立的
//! `libphonenumber` feature：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["regex"] }
//! ```

//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
//! ```

//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["rand"] }
//! ```
//!
//! `FormatUtils` 的运行时模板能力需要用户显式启用 `serde` 和一个后端 feature：`strfmt`
//! 使用 `{name}` 语法并只支持扁平顶层变量；`minijinja` 使用 `{{ name }}` 语法，支持嵌套
//! 字段、数组、条件和循环。后端 feature 不会自动启用 `serde`；通过
//! `FormatUtils::template(template, context, default, engine)` 的 `engine` 参数显式选择
//! `TemplateEngine::Strfmt` 或 `TemplateEngine::MiniJinja` 模板语法，同时启用两个后端时也使用
//! 同一个入口：
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["serde", "minijinja"] }
//! ```
//!
//! `TimeUtils` 的日期格式化能力分别由 `chrono`、`time` 和 `jiff` feature 提供。三个
//! feature 相互独立；只启用一个后端时可以使用无后缀方法，同时启用多个后端时应调用带
//! 后缀的方法以明确日期类型。日期默认模板为 `yyyy-MM-dd`，含时间值默认模板为
//! `yyyy-MM-dd HH:mm:ss`；带偏移方法的 `offset: Option<TimeZoneOffset>` 传入 `None` 时
//! 使用 `+08:00`。格式化采用本 crate 的统一模板：`yyyy`、`MM`、`dd`、`HH`、`mm`、
//! `ss`、`SSS` 与固定偏移专用的 `XXX`。
//!
//! 需要读取配置文件时，通过 `serde` feature 显式启用 `ConfigLoader`/`ConfigUtils`，提供
//! JSON 与自实现 `.env`（dotenv）读取；YAML、TOML、INI 分别需要额外启用
//! `serde-saphyr`、`toml`、`rust-ini` feature。每种格式都提供无类型 [`ConfigValue`]（点号
//! 路径访问）与有类型 `serde::Deserialize` 两条读取路径；文件大小上限统一，JSON/TOML/YAML/INI
//! 的无类型路径以及 YAML/INI 的有类型路径使用配置的嵌套深度上限，JSON/TOML 有类型路径使用
//! 各自后端的递归保护；错误不回显配置文件内容：
//! 同时启用 `tokio` feature 后还可使用六个异步文件入口；调用方必须自行提供 Tokio runtime，
//! crate 不创建 runtime 或调用 `block_on`，解析阶段仍在当前异步任务中执行。
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["serde", "serde-saphyr", "toml", "rust-ini"] }
//! ```
//!
//! `CryptoUtils` 的十六进制编解码（`hex_encode`/`hex_encode_upper`/`hex_decode`）与
//! `TextEncoding::Utf8` 文本编解码默认可用，仅依赖标准库；Base64、MD5、AES 分别需要显式启用
//! `base64`、`md5`（实际启用 crates.io 上的 `md-5` crate）、`aes` feature，`encoding_rs`
//! feature 为 `TextEncoding` 追加六个 legacy 编码变体。AES 支持 GCM（推荐，带认证）与
//! CBC+PKCS#7（**无完整性认证**，仅用于旧系统互操作）两种模式，随机 IV/nonce 只使用操作系统
//! 随机源。`CryptoUtils` 的 AES 入口通过 `aes_init` 或 `aes_init_from_bytes` 初始化一次进程级
//! 单例，密钥与模式随后不可修改且常驻进程内；需要多密钥或可控密钥生命周期时使用可独立销毁的
//! `AesCipher` 实例。若同时启用 `aes` 与 `base64`，额外提供 `aes_encrypt_base64`/
//! `aes_decrypt_base64`。
//! MD5 是摘要算法，已存在实用碰撞攻击，**禁止**用于密码存储、数字签名或任何对抗性场景。
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["aes", "base64"] }
//! ```

//! JWT 能力需要显式启用 `jwt` feature。它提供固定算法的 JWS 签发/验证和泛型 claims；
//! JWT payload 不是加密内容，`JwtUtils` 的全局入口只能成功初始化一次且不支持热轮换。
//! 详细的算法、key 格式、claims 验证和安全边界见
//! [`JWT 使用文档`](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/jwt.md)。
//!
//! ```toml
//! [dependencies]
//! axutils = { version = "0.1", features = ["jwt"] }
//! ```

mod time;
pub mod utils;

#[cfg(feature = "jwt")]
pub mod jwt;

#[cfg(feature = "regex")]
pub use utils::reg_utils;

#[cfg(feature = "regex")]
pub use utils::RegUtils;

#[cfg(feature = "rand")]
pub use utils::random_utils;

#[cfg(feature = "rand")]
pub use utils::{LetterCase, RandomRangeError, RandomUtils};

pub use utils::path_utils;
pub use utils::PathUtils;

pub use utils::time_utils;
pub use utils::TimeUtils;

pub use time::{
    TimeFormatError, TimeFormatToken, TimeValueKind, TimeZoneOffset, TimeZoneOffsetError,
};

pub use utils::format_utils;
pub use utils::FormatUtils;
#[cfg(all(feature = "serde", any(feature = "strfmt", feature = "minijinja")))]
pub use utils::TemplateEngine;

#[cfg(feature = "lettre")]
pub mod email;

#[cfg(feature = "lettre")]
pub use email::{
    EmailBody, EmailClient, EmailConfig, EmailError, EmailMessage, EmailSecurity,
    EmailTransportErrorKind,
};

#[cfg(feature = "lettre")]
pub use utils::EmailUtils;

#[cfg(feature = "serde")]
pub mod config;

#[cfg(feature = "serde")]
pub use config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};

#[cfg(feature = "serde")]
pub use utils::ConfigUtils;

pub mod crypto;

pub use crypto::{CryptoError, TextEncoding};
pub use utils::crypto_utils;
pub use utils::CryptoUtils;

#[cfg(feature = "jwt")]
pub use jwt::{
    JwtAlgorithm, JwtConfig, JwtError, JwtSigningKey, JwtValidation, JwtVerificationKey,
};

#[cfg(feature = "jwt")]
pub use utils::JwtUtils;

#[cfg(feature = "base64")]
pub use crypto::{Base64Alphabet, Base64Options};

#[cfg(feature = "aes")]
pub use crypto::{AesCipher, AesKey, AesKeyBits, AesMode};
