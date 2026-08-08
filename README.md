# axutils

`axutils` 是一个按 feature 组织的 Rust 常用工具库。

当前项目最低支持 Rust 1.88。配置文件 YAML 后端使用 `serde-saphyr 1.0.0`，其 `edition = "2024"`
和 let-chains 语法要求 Rust 1.88；邮件能力使用的 `lettre 0.11.22` 只要求 Rust 1.85。
因此，使用新版本 `axutils` 的 Rust 1.76—1.87 项目需要先升级工具链。

默认 feature 为空。默认可用的是 `PathUtils`、`TimeUtils`、`FormatUtils::seconds_to_human`，以及时间
格式化共用的根类型；`CryptoUtils` 的十六进制编解码和 `TextEncoding::Utf8` 文本编解码同样默认可用。
`RegUtils` 需要 `regex`，`RandomUtils` 需要 `rand`，模板、日期后端、邮件、配置读取和 `CryptoUtils`
的 Base64/MD5/AES 能力都需要显式 feature。公共导出路径和完整边界见各模块使用文档。

邮件能力使用 Rustls 强制 SMTPS/STARTTLS，不提供明文或机会式降级；配置文件读取统一限制文件大小，
错误不回显配置值。真实凭据只能由调用方在本地安全管理，不能硬编码或提交到 Git。

HTTP 能力通过独立的 `http` feature 提供；默认关闭系统代理、自动重定向、自动压缩和隐式重试，
并限制请求/响应体大小。需要三参数的 Serde JSON/query/字节快捷方法时再启用 `serde`；异步
HTTP 还需要同时启用 `tokio`，且必须运行在调用方提供的 Tokio runtime 中。详细的请求、重试、
去重、缓存和便捷方法边界见 HTTP 使用文档。

## 安装

在项目的 `Cargo.toml` 中添加默认依赖：

```toml
[dependencies]
axutils = "0.1"
```

这会提供 `PathUtils`、`TimeUtils` 和 `FormatUtils::seconds_to_human`。需要正则校验时启用
`regex`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex"] }
```

需要随机字符串或随机范围时启用 `rand`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["rand"] }
```

国际手机号码校验必须同时启用 `regex` 和 `libphonenumber`；只启用后者不会导出 `RegUtils`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["regex", "libphonenumber"] }
```

日期格式化后端彼此独立，调用方还要直接依赖对应日期 crate。下面的版本是最低兼容版本，遵循
Cargo 默认 caret 兼容范围：

```toml
[dependencies]
axutils = { version = "0.1", features = ["chrono"] }
chrono = { version = "0.4.45", default-features = false }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["time"] }
time = { version = "0.3.36", default-features = false }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["jiff"] }
jiff = { version = "0.2.35", default-features = false }
```

同时启用多个日期后端时必须使用带后缀的方法（`*_chrono`、`*_time`、`*_jiff`）；只有一个后端
时才会导出无后缀简写。

模板渲染必须同时启用 `serde` 和一个模板后端。`strfmt` 适合扁平变量，`minijinja` 支持嵌套字段、
数组、条件和循环：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "strfmt"] }
serde = { version = "1", features = ["derive"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "minijinja"] }
serde = { version = "1", features = ["derive"] }
```

同步 SMTP 邮件只需要 `lettre`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre"] }
```

异步 SMTP 邮件必须同时启用 `lettre` 和 `tokio`，应用还要直接依赖 Tokio 并提供自己的 runtime：

```toml
[dependencies]
axutils = { version = "0.1", features = ["lettre", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

`axutils` 的 Tokio 依赖只提供异步文件 I/O 和邮件传输所需能力，不会创建 runtime 或调用
`block_on`；只启用 `tokio` 不会导出邮件或 HTTP API。

配置读取启用 `serde` 后提供 JSON 和 `.env`；YAML、TOML、INI 分别额外启用
`serde-saphyr`、`toml`、`rust-ini`：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "serde-saphyr", "toml", "rust-ini"] }
```

异步读取配置文件还要启用 `tokio`，并由应用直接依赖 Tokio runtime：

```toml
[dependencies]
axutils = { version = "0.1", features = ["serde", "tokio"] }
tokio = { version = "1.53.1", features = ["macros", "rt-multi-thread"] }
```

`CryptoUtils` 的十六进制编解码默认可用；Base64、MD5、AES 分别需要显式启用 `base64`、`md5`、
`aes`；`encoding_rs` 为文本编解码追加 GBK/GB18030/Big5/Shift_JIS/EUC-KR/windows-1252 六种
legacy 编码：

```toml
[dependencies]
axutils = { version = "0.1", features = ["base64"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["md5"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["aes"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["aes", "base64"] }
```

```toml
[dependencies]
axutils = { version = "0.1", features = ["encoding_rs"] }
```

所有 feature 都可以同时启用；各后端仍须满足自己的组合前提。完整 API 和每个方法的可编译示例见
下面对应的模块文档。

## 使用 JWT

启用独立的 `jwt` feature 后可使用固定算法的 JWS 签发/验证、泛型 claims 和一次初始化的
`JwtUtils` 入口；它不会启用配置模块或其他 feature。JWT 签名只提供完整性和来源认证，不加密
payload；全局 key 不支持 reset、replace 或热轮换。

```toml
[dependencies]
axutils = { version = "0.1", features = ["jwt"] }
serde = { version = "1", features = ["derive"] }
```

```rust
# #[cfg(feature = "jwt")]
# fn main() -> Result<(), axutils::JwtError> {
use axutils::{JwtAlgorithm, JwtConfig, JwtSigningKey, JwtUtils, JwtValidation};

let config = JwtConfig::new(
    JwtAlgorithm::Hs256,
    Some(JwtSigningKey::from_hmac_secret([0x11; 32])?),
    None,
    JwtValidation::new(),
)?;
let _ = (config, JwtUtils::is_initialized());
# Ok(())
# }
# #[cfg(not(feature = "jwt"))]
# fn main() {}
```

使用说明、key PEM/DER 格式、标准 claims 规则、完整导出路径和安全边界见 [JWT 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/jwt.md)。

## 使用 `PathUtils`

`PathUtils` 提供平台相关的绝对路径判断、当前工作目录/可执行文件路径获取，以及不访问文件系统的
词法路径拼接：

```rust
use axutils::PathUtils;

let path = PathUtils::join(["project", "src", "..", "README.md"]);
assert_eq!(path, std::path::PathBuf::from("project").join("README.md"));
```

完整示例与边界说明见 [PathUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/path.md)。

## 使用 `TimeUtils`

`TimeUtils` 提供 `timestamp`、`timestamp_seconds`、`timestamp_milliseconds`、
`timestamp_microseconds` 和 `timestamp_nanoseconds` 五个 Unix 时间戳入口；日期格式化后端按需
启用 `chrono`、`time` 或 `jiff`。

```rust
use axutils::TimeUtils;

let (seconds, milliseconds, microseconds, nanoseconds) = TimeUtils::timestamp();
assert!(seconds > 0);
assert!(milliseconds >= seconds as u128 * 1_000);
assert!(microseconds >= milliseconds * 1_000);
assert!(nanoseconds >= microseconds * 1_000);
```

系统时间早于 Unix 纪元时，时间戳方法会 panic；日期模板、固定偏移和各后端方法见详细文档。

完整示例与边界说明见 [TimeUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/time.md)。

## 使用 `FormatUtils`

默认方法将秒数格式化为中文持续时间：

```rust
use axutils::FormatUtils;

assert_eq!(FormatUtils::seconds_to_human(90), "1分钟30秒");
```

启用 `serde` 与 `strfmt` 或 `minijinja` 后，可以通过 `TemplateEngine` 选择模板后端：

```rust
# #[cfg(all(feature = "serde", feature = "strfmt"))]
# fn main() {
use axutils::{FormatUtils, TemplateEngine};

#[derive(serde::Serialize)]
struct Greeting<'a> {
    name: &'a str,
}

let greeting = Greeting { name: "小王" };
assert_eq!(
    FormatUtils::template("你好，{name}", &greeting, None, TemplateEngine::Strfmt),
    Some("你好，小王".to_owned()),
);
# }
# #[cfg(not(all(feature = "serde", feature = "strfmt")))]
# fn main() {}
```

不可信模板应限制模板长度、调用频率和数据规模；完整模板语义与后端边界见详细文档。

完整示例与边界说明见 [FormatUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/format.md)。

## 使用 `RegUtils`

启用 `regex` 后，`RegUtils` 提供常见邮箱、严格邮箱和中国大陆手机号格式校验；这些方法只校验
输入格式，不验证邮箱或号码当前是否真实可用：

```rust
# #[cfg(feature = "regex")]
# fn main() {
use axutils::RegUtils;

assert!(RegUtils::is_email("user@example.com"));
assert!(RegUtils::is_email_strict("first.last+tag@example.co.uk"));
assert!(RegUtils::is_phone_cn("13812345678"));
# }
# #[cfg(not(feature = "regex"))]
# fn main() {}
```

国际手机号码的 `is_phone` 还需要同时启用 `regex` 与 `libphonenumber`：

```rust
# #[cfg(all(feature = "regex", feature = "libphonenumber"))]
# fn main() {
use axutils::RegUtils;
assert!(RegUtils::is_phone("+8613812345678"));
# }
# #[cfg(not(all(feature = "regex", feature = "libphonenumber")))]
# fn main() {}
```

完整示例与边界说明见 [RegUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/reg.md)。

## 使用 `RandomUtils`

启用 `rand` 后，`RandomUtils` 可生成普通 ASCII 随机字符串，并从闭区间中取得随机整数或浮点数：

```rust
# #[cfg(feature = "rand")]
# fn main() {
use axutils::{LetterCase, RandomUtils};

let value = RandomUtils::alphanumeric_string(16).expect("the string should be allocatable");
assert!(value.bytes().all(|byte| byte.is_ascii_alphanumeric()));
let lower = RandomUtils::alphabetic_string(8, LetterCase::Lower)
    .expect("the string should be allocatable");
assert!(lower.bytes().all(|byte| byte.is_ascii_lowercase()));
# }
# #[cfg(not(feature = "rand"))]
# fn main() {}
```

该工具不承诺密码学安全，不应直接用于密码、Session Token、API 密钥或其他安全敏感数据；对不可信
长度应由调用方先做业务上限校验。

完整示例与边界说明见 [RandomUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/random.md)。

## 使用 SMTP 邮件能力

启用 `lettre` 后可以创建独立的 `EmailClient`。构造客户端不会连接网络，`send` 才会执行阻塞
SMTP I/O；下面的 `no_run` 示例只编译，不会向任何 relay 发信：

```rust,no_run
# #[cfg(feature = "lettre")]
# fn main() -> Result<(), axutils::EmailError> {
use axutils::{EmailClient, EmailConfig, EmailMessage, EmailSecurity};

let config = EmailConfig::new(
    "smtp.example.com",
    465,
    EmailSecurity::ImplicitTls,
    "sender@example.com",
    "application-password",
    "sender@example.com",
)?;
let client = EmailClient::new(config)?;
let message = EmailMessage::text(
    vec!["receiver@example.com".to_owned()],
    "A test message",
    "Hello from axutils.",
)?;
client.send(message)?;
# Ok(())
# }
# #[cfg(not(feature = "lettre"))]
# fn main() {}
```

异步发送和 `EmailUtils` 全局入口分别受 `lettre,tokio` 与 `lettre` feature 约束；异步调用必须运行
在调用方已有的 Tokio runtime 中。传输固定使用 Rustls、`ring` 和 `webpki-roots`，私有 CA relay
暂不支持，也不能通过关闭证书校验绕过；邮件凭据、授权码和真实收件地址不得写入源码或日志。

完整示例与边界说明见 [Email 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/email.md)。

## 使用 HTTP 能力

启用 `http` 后可使用同步 `HttpClient`；`HttpConfig::default()` 或
`HttpConfig::builder().build()` 可以不传 `base_url`、超时和重试配置，异步入口需要同时启用
`tokio`，并由应用提供 runtime。
客户端默认不读取系统代理、不跟随重定向、不协商压缩，也不会隐式重试非幂等方法；请求和响应均有
有限大小与总时间预算。默认最多进行 3 次网络尝试（包括首次请求），设置为 1 可禁用自动重试。
不配置 `base_url` 时只能使用绝对 HTTP/HTTPS URL；配置了 `base_url` 时，请求自身的绝对 URL 优先。
下面的 `no_run` 示例只编译，不会访问网络：

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig, HttpMethod, HttpRequest};

let config = HttpConfig::builder()
    .base_url("https://api.example.com/")?
    .build()?;
let client = HttpClient::new(config)?;
let request = HttpRequest::new(HttpMethod::Get, "/health")?;
let response = client.execute(request)?;
assert!(response.status() >= 100);
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

`HttpUtils` 是一次初始化的全局便捷入口，不能 reset 或替换；异步执行仍要求 `http,tokio` 和已有
runtime。HTTP 不承诺 SSRF 防护，调用方必须自行限制目标 URL、网络出口和业务认证信息。

启用 `http,serde` 后可以用三个参数调用常用动词：URL、可选 query 或 JSON body、可选
`HttpRequestOptions`。返回类型实现 `serde::Deserialize` 即可；默认按 JSON 解码，`*_bytes`
方法返回原始字节。异步快捷方法需要 `http,serde,tokio`：

~~~toml
[dependencies]
axutils = { version = "0.1", features = ["http", "serde"] }
serde = { version = "1", features = ["derive"] }
~~~

~~~rust,no_run
# #[cfg(all(feature = "http", feature = "serde"))]
# fn main() -> Result<(), axutils::HttpError> {
use axutils::{HttpClient, HttpConfig};
use serde::Deserialize;

#[derive(Deserialize)]
struct Reply {
    ok: bool,
}

let client = HttpClient::new(HttpConfig::default())?;
let reply: Reply = client.get("https://api.example.com/health", None::<()>, None)?;
assert!(reply.ok);
let _bytes = client.get_bytes("https://api.example.com/image", None::<()>, None)?;
# Ok(())
# }
# #[cfg(not(all(feature = "http", feature = "serde")))]
# fn main() {}
~~~

完整 API、每个公开方法的示例、feature 矩阵和安全边界见 [HTTP 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/http.md)。

## 使用配置文件读取能力

启用 `serde` 后，`ConfigUtils` 和 `ConfigLoader` 提供 JSON 与 `.env` 读取；YAML、TOML、INI 分别
需要 `serde-saphyr`、`toml`、`rust-ini`。最小的无类型解析示例：

```rust
# #[cfg(feature = "serde")]
# fn main() -> Result<(), axutils::ConfigError> {
use axutils::{ConfigFormat, ConfigUtils};

let value = ConfigUtils::parse_value(
    r#"{"server":{"port":8080}}"#,
    ConfigFormat::Json,
)?;
assert_eq!(value.get("server.port").and_then(|value| value.as_i64()), Some(8080));
# Ok(())
# }
# #[cfg(not(feature = "serde"))]
# fn main() {}
```

异步文件入口需要 `serde,tokio`，并且只异步化文件 I/O；调用方必须自行提供 runtime、限制并发和
总内存。配置值可能包含凭据，不要把整棵 `ConfigValue` 或反序列化结果写入日志。

完整示例与边界说明见 [Config 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/config.md)。

## 使用 `CryptoUtils`

十六进制编解码默认可用：

```rust
use axutils::CryptoUtils;

let encoded = CryptoUtils::hex_encode([0x00, 0xff]).unwrap();
assert_eq!(encoded, "00ff");
assert_eq!(CryptoUtils::hex_decode(&encoded).unwrap(), vec![0x00, 0xff]);
```

启用 `base64` 后可以按标准/URL-safe 字母表和有/无填充编解码：

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Options, CryptoUtils};

let encoded = CryptoUtils::base64_encode("foobar", Base64Options::STANDARD).unwrap();
assert_eq!(encoded, "Zm9vYmFy");
assert_eq!(CryptoUtils::base64_decode(&encoded, Base64Options::STANDARD).unwrap(), b"foobar");
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

启用 `md5` 后可以计算 MD5 摘要；**MD5 不是加密，已存在实用碰撞攻击，禁止用于密码存储、数字
签名或任何对抗性场景**：

```rust
# #[cfg(feature = "md5")]
# fn main() {
use axutils::CryptoUtils;

assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
# }
# #[cfg(not(feature = "md5"))]
# fn main() {}
```

启用 `aes` 后可以用 AES-GCM（推荐）或 AES-CBC+PKCS#7（仅用于旧系统互操作，**无完整性认证**）
加解密。`CryptoUtils` 的 AES 入口先初始化一次进程级密钥与模式，之后不再逐次传入密钥；全局
密钥会常驻进程且不可轮换。需要多密钥或可控密钥生命周期时，使用 `AesCipher` 实例：

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesMode, CryptoUtils};

CryptoUtils::aes_init_from_bytes([0x00; 32], AesMode::Gcm).unwrap();
let ciphertext = CryptoUtils::aes_encrypt("hello world").unwrap();
let plaintext = CryptoUtils::aes_decrypt(&ciphertext).unwrap();
assert_eq!(plaintext, b"hello world");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesCipher, AesMode};

let cipher = AesCipher::from_key_bytes([0x00; 16], AesMode::Gcm).unwrap();
let ciphertext = cipher.encrypt("hello world").unwrap();
assert_eq!(cipher.decrypt(&ciphertext).unwrap(), b"hello world");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

`CryptoError` 不回显明文、密文、密钥、IV 或原始文本内容；AES 的随机 IV/nonce/密钥生成只使用
操作系统随机源，失败返回 `CryptoError::RandomSource`，不 panic、不降级到非密码学随机源。

完整示例与边界说明见 [CryptoUtils 使用文档](https://github.com/crx-96/axutils-rust/blob/main/docs/examples/crypto.md)。

## 构建与部署依赖

`axutils` 是 library crate，本身不生成可部署的二进制或容器。下表是消费方应用已有 Rust
工具链时的常见基础构建包；邮件功能本身不增加 TLS 系统包。应用的数据库、图像、压缩或其他
native 依赖仍需按其自身文档安装对应包。

| 环境 | 消费方源码构建的基础包 | 邮件功能额外 TLS 包 |
| --- | --- | --- |
| Debian/Ubuntu（含 slim） | `build-essential` | 无 |
| Alpine Linux | `build-base` | 无 |
| Fedora/RHEL | `gcc`、`make`、`glibc-devel`、按需 `binutils` | 无 |
| Windows MSVC | Visual Studio Build Tools 的 C++ workload 和 Windows SDK | 无 OpenSSL |
| macOS | Xcode Command Line Tools | 无 OpenSSL |

例如，已安装 Rust 但系统较精简时，消费方可以自行安装：

```bash
# Debian / Ubuntu
sudo apt-get update
sudo apt-get install -y --no-install-recommends build-essential

# Alpine Linux
sudo apk add --no-cache build-base

# Fedora / RHEL
sudo dnf install -y gcc make glibc-devel binutils

# macOS
xcode-select --install
```

这些命令是消费方主动配置开发环境的示例。Docker builder 还需要按镜像和取包方式提供 HTTPS
下载所需的工具/CA；那是构建期依赖，与 SMTP 运行时的 `webpki-roots` 不同。

### Debian/Ubuntu 多阶段 Docker 说明性模板

以下模板只适用于消费方自行替换后的应用，不是本仓库提供或验证过的 Dockerfile。请替换
`<pinned-version>`、`<binary-name>`、workspace 清单和 `src` 路径，并按应用的其他依赖调整：

```dockerfile
ARG RUST_VERSION
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/<binary-name> /usr/local/bin/app
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/app"]
```

`RUST_VERSION` 应由消费方替换为不低于 1.88 的固定版本，正式部署还应按供应链策略固定
基础镜像 digest。runtime 阶段不需要为 axutils 邮件功能安装 OpenSSL 或 CA 包；应用其他
功能需要的动态库、时区数据或健康检查工具须单独评估。

### Alpine/musl 多阶段 Docker 说明性模板

原生构建当前容器架构时，官方 Alpine Rust builder 通常直接执行 `cargo build`，不要无条件
写死 `x86_64-unknown-linux-musl`；Buildx 同时构建 ARM64 时尤其要避免把 AMD64 target 写死：

```dockerfile
ARG RUST_VERSION
FROM rust:${RUST_VERSION}-alpine AS builder

RUN apk add --no-cache build-base
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM alpine:<pinned-version> AS runtime
RUN addgroup -S app && adduser -S -G app app
WORKDIR /app
COPY --from=builder /app/target/release/<binary-name> /usr/local/bin/app
USER app
ENTRYPOINT ["/usr/local/bin/app"]
```

Alpine runtime 不需要为邮件功能安装 `openssl`、`libssl3` 或 `ca-certificates`；`curl` 也只有
在应用健康检查或其他功能实际需要时才添加。Alpine/musl、ARM64、WASM、Android、iOS 和
FreeBSD 不属于本 crate 首期已承诺的验证目标；消费方必须自行验证其完整应用和镜像。

## API 文档

发布后可在 [docs.rs/axutils](https://docs.rs/axutils) 查看完整 API 文档。`docs/examples/` 会随
发布包进入 docs.rs 的版本化 source browser；README 中的详细模块入口统一指向 GitHub `main`
分支，发布版本的精确文档以对应 `.crate` 包和 docs.rs 版本 source 为准。

默认 feature 为空：`RandomUtils` 及其相关类型仅在 `rand` 下导出，`RegUtils` 仅在 `regex` 下
导出，邮件类型仅在 `lettre` 下导出，配置类型仅在 `serde` 下导出；模板、日期和配置后端需要
满足各自的组合前提。`RegUtils::is_phone` 必须同时启用 `regex` 与 `libphonenumber`，异步邮件
必须同时启用 `lettre` 与 `tokio`。`CryptoUtils` 的十六进制编解码与 `TextEncoding::Utf8` 默认
可用；`Base64*`/`md5*`/`Aes*` 分别仅在 `base64`/`md5`/`aes` 下导出，`aes_encrypt_base64`/
`aes_decrypt_base64` 需要同时启用 `aes` 与 `base64`。
