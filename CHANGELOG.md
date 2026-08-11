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
- 新增 `ConvertUtils`（`src/utils/convert_utils.rs`；整数、浮点和 UUID 的领域实现位于
  `src/convert/`），通过独立的 `itoa`、`ryu`/`zmij` 和 `uuid` feature
  提供整数、`f32`/`f64`、UUID 与字符串之间的标准解析和高性能格式化；格式化同时提供调用方
  buffer 的借用型入口、直接追加到已有 `String` 的入口和拥有型字符串入口，双浮点后端通过
  `FloatFormat` 显式选择，不自动启用其他公共 feature。
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
  `.env` 回退和错误脱敏语义；配置读取本身使用 Tokio 的 `fs`/`io-util` 能力，共享的 `tokio`
  feature 还因 HTTP/Redis 使用 `rt`/`sync`/`time`，crate 不创建 runtime。
- 新增 `CryptoUtils`（`src/utils/crypto_utils.rs`）与 `crypto` 模块（`src/crypto/`），提供内存
  数据的十六进制、Base64、MD5 和 AES 编码/摘要/加解密能力。十六进制编解码
  （`hex_encode`/`hex_encode_upper`/`hex_decode`）与 `TextEncoding::Utf8` 文本编解码不依赖任何
  第三方 crate，在任何 feature 组合下（含无 feature）都默认可用；`CryptoUtils`、`CryptoError`
  基线变体（`OddHexLength`/`InvalidHex`/`TextDecodeInvalid`/`OutputTooLarge`）与 `crypto`/
  `crypto_utils` 模块同样默认导出。
  - `base64` feature 提供 `Base64Alphabet`、`Base64Options`（`STANDARD`/`STANDARD_NO_PAD`/
    `URL_SAFE`/`URL_SAFE_NO_PAD` 四种组合）与 `base64_encode`/`base64_encode_text`/
    `base64_decode`/`base64_decode_text`；解码严格拒绝非规范填充、非法字符和非零尾随比特。
  - `md5` feature（实际启用 crates.io 上的 `md-5` crate，别名映射为 `dep:md5`，**不是**同名的
    旧 `md5` crate）提供 `md5`/`md5_hex`/`md5_text`/`md5_hex_text`。
  - `aes` feature（聚合 `aes`/`aes-gcm`/`cbc`/`zeroize` 四个内部适配依赖）提供 `AesKey`、
    `AesKeyBits`、`AesMode`（`Gcm`/`CbcPkcs7`）和可独立销毁的 `AesCipher` 实例；同时提供
    `CryptoUtils::aes_init`/`aes_init_from_bytes`/`aes_is_initialized`/`aes_mode` 一次初始化的
    进程级 AES 入口，以及不再逐次传入密钥/模式的 `aes_encrypt`/`aes_decrypt`/
    `aes_encrypt_with_iv`/`aes_decrypt_with_iv`/`aes_encrypt_hex`/`aes_decrypt_hex`。同时启用
    `aes` 与 `base64` 后额外提供对应的 `aes_encrypt_base64`/`aes_decrypt_base64`。支持
    AES-128/192/256、随机 IV/nonce（容器布局为 `iv || 密文(|| tag)`）与调用方显式提供 IV/nonce
    两条路径；新增的 `CryptoError::NotInitialized`/`AlreadyInitialized` 仅在 `aes` 下导出。
  - `encoding_rs` feature 为 `TextEncoding` 追加 `Gbk`/`Gb18030`/`Big5`/`ShiftJis`/`EucKr`/
     `Windows1252` 六个 legacy 编码变体，供 Base64/MD5 的 `*_text` 入口使用。
- 新增独立 `jwt` feature 下的 JWT JWS 能力：提供 `JwtAlgorithm`、拥有型
  `JwtSigningKey`/`JwtVerificationKey`、`JwtConfig`、`JwtValidation`、脱敏 `JwtError` 和一次初始化的
  `JwtUtils`；支持 HS256/384/512、RS256/384/512、PS256/384/512、ES256/384 与 Ed25519，支持泛型
  claims、标准 `exp`/`nbf`/`aud`/`iss`/`sub` 验证以及明确的 token/claims/key 资源上限。
- 新增独立 `http` feature 下的 HTTP 客户端能力：提供同步 `HttpClient`、`HttpUtils`、请求/响应、
  Header、重试和去重策略类型；同步使用 Rustls `ureq`，同时启用 `tokio` 后追加基于 Rustls 的异步
  `reqwest` 入口。客户端默认关闭系统代理、自动重定向、自动压缩和隐式重试，执行总时间与请求/响应
  大小受限，并提供安全方法默认 single-flight 与显式完成缓存。

- 新增 `http + serde` 下的三参数 Serde 便捷 HTTP 方法：`get`、`post`、`delete`、`patch`、
  `put`、`options`、`head` 及对应的 `*_bytes` 原始字节入口；方法支持可选 query/JSON body
  和 `HttpRequestOptions` 单次配置，异步版本要求同时启用 `tokio`。
- 新增 HTTP JSON/query 失败的稳定 `HttpError::JsonSerialize`、`QuerySerialize`、
  `JsonDeserialize` 错误分类，以及 `HttpResponse::into_body` 和受 `serde` feature 守卫的
  `HttpResponse::json`。
- 新增独立 `redis` feature 下的 Redis 能力：`RedisConfig`、可 Clone 的 `RedisClient`、
  `RedisError`/`RedisTransportErrorKind`、`RedisTransaction` 和一次初始化的 `RedisUtils`；
  提供惰性 r2d2 单机/Cluster 连接池、受限 MessagePack 值 API、raw 字节 API、批量命令、
  TTL/counter/list/set 及单机 `MULTI`/`EXEC` 事务。第一阶段只接受 `redis://`，不提供 TLS、
  Cluster 事务、WATCH/CAS 或无界 keys/scan；Cluster 跨 slot 错误统一映射为 `CrossSlot`。
- 新增 `RedisClient::try_lock`、`RedisLockGuard`、`RedisLockGuard::release`/`renew` 以及
  `RedisUtils::try_lock`，提供单 Redis 逻辑主节点/单 Cluster 拓扑的单键租约锁：使用 OS
  CSPRNG token、24 小时以内 TTL 和 token 校验 Lua `EVAL`，同步 guard 支持一次最佳努力
  `Drop` 释放，锁丢失时返回 `Ok(false)` 而不删除新持有者的锁。
- 新增 `redis,tokio` 组合下的 `_async` Redis 命令和独立事务通道；异步连接惰性初始化，
  不创建 Tokio runtime、不调用 `block_on`，调用方必须提供 runtime。Redis feature 直接启用
  专用 `serde` 依赖但不自动启用项目公共 `serde` feature。
- 新增 `redis,tokio` 组合下的 `RedisAsyncLockGuard`、`RedisClient::try_lock_async`、
  `RedisUtils::try_lock_async` 及异步 `release`/`renew`；异步 guard 的 `Drop` 不发起网络
  操作，取消和 runtime 关闭路径依赖 TTL 兜底。既有 `set_nx_with_expiry`/raw 变体仍是
  通用 NX 写入原语，不记录锁所有者或自动释放，普通 `delete`/`pexpire` 也不校验 token。
- 新增互斥的 `mimalloc` 和 `rpmalloc` allocator feature，分别使用可选的 `mimalloc 0.1.52`
  与 `rpmalloc 0.2.2` 依赖注册唯一 Rust 全局分配器；不启用时保持目标平台默认分配器。由于
  `axutils` 是 library，启用后会影响依赖它的最终 Rust binary；已有 `#[global_allocator]` 的
  应用或递归依赖不得重复启用，两个 allocator feature 同时启用会以编译错误拒绝。native
  构建需要目标平台的 C toolchain；Windows 的 rpmalloc 路径还需要 SDK 提供 `Advapi32` import
  library。

### Changed

- HTTP 配置 builder 的字段均可省略；`HttpConfig::default()` 和空 builder 可以直接构造配置，
  默认提供 30 秒请求总超时、10 秒连接超时和最多 3 次（包括首次请求）网络尝试。未设置
  `base_url` 时，相对 URL 会返回 `HttpError::InvalidUrl`；即使配置了基地址，请求自身的绝对
  HTTP/HTTPS URL 也始终优先。
- `RetryPolicy::with_max_retries`、`RetryPolicy::max_retries` 和
  `HttpRequestOptions::with_max_retries` 的数值语义改为“包括首次请求的最大总网络尝试次数”，
  方法名保持不变以兼容现有调用路径；默认值为 3，设置为 1 表示不自动重试，0 不再是有效值。
- `CryptoUtils` 的 8 个 AES 方法改为使用一次初始化的进程级密钥与模式，不再接受显式密钥/模式
  参数；这是破坏性 API 变更。多密钥、多模式或需要可控 `Drop` 清零的调用方应迁移到
  `AesCipher` 实例，或在进程启动时调用 `aes_init`/`aes_init_from_bytes` 后使用新的无密钥参数
  入口。`Base64Options` 仍逐次传入。
- 最低支持 Rust 版本从 1.85 提升为 1.88。原因：新增的 YAML 后端 `serde-saphyr 1.0.0`
  （2026-07-31 发布）声明 `edition = "2024"` 并使用了 let-chains 语法，实测在 Rust 1.85 下无法
  编译（27 个 `E0658` 错误），Rust 1.88（let-chains 稳定化版本）起可正常编译；`toml`（自身要求
  1.85）与 `rust-ini`（自身要求 1.64）不受影响。
- 邮件传输固定使用 Rustls、`ring` 和 `webpki-roots`，支持强制 SMTPS 或强制 STARTTLS，不使用
  native-tls/OpenSSL。
- `tokio` feature 的生产依赖增加 `rt`、`sync` 和 `time`，用于 HTTP 异步客户端的 runtime 检测、
  single-flight 通知和受总时间预算约束的异步退避；crate 仍不创建 runtime 或调用 `block_on`。
- `serde` feature 追加可选的 `serde_urlencoded 0.7.1`，用于 HTTP 快捷方法的 query 编码；
  `http` 不会自动启用 `serde`，因此不改变仅启用 HTTP 时的公共 API 和依赖边界。
- `redis` 单 feature 只启用同步连接池与 Cluster 所需的 redis-rs 子 feature；异步
  `cluster-async`、`connection-manager`、`tokio-comp` 子 feature 改由 `tokio` 对 Redis 的弱依赖
  映射在 `redis + tokio` 组合下启用，避免同步用户编译无公共 API 可用的异步依赖。
- HTTP 准备请求时直接移出调用方已经拥有的请求体，同步发送尝试借用同一缓冲区；
  `HttpResponse::into_body` 在响应体未被缓存或 single-flight 共享时直接取回底层 `Vec<u8>`，仅在
  确有共享引用时复制，降低大请求/响应的峰值内存和复制开销。
- Redis `ValueTooLarge` 的错误文本不再把批量项数或事务命令数误标为字节，`limit` 的单位改为
  随具体操作语义解释。

### Fixed

- HTTP 配置存在 `base_url` 且请求使用跨 origin 绝对 URL 时，不再继承默认
  `Authorization`、`Cookie` 或 `Set-Cookie`，避免把默认凭据发送到另一个 origin；请求上显式设置
  的敏感 Header 仍按调用方意图发送。
- `.env` 插值的逐次追加和解析后累计 key/value 现在受 `max_bytes` 限制，超限返回新增的
  `ConfigError::ExpandedValueTooLarge`，避免短配置通过链式重复引用产生指数级 CPU/内存占用。
- Redis 同步/异步事务遇到已完整读取的普通服务端命令错误时保留健康连接，只在协议、网络、
  连接、超时或关闭状态下淘汰连接，避免可重复的 `WRONGTYPE` 等错误造成持续重连。
- 配置无类型 JSON 与有类型 JSON 均拒绝任意嵌套层级的重复对象键；TOML 无类型转换在遇到内部日期时间伪表时先完整消费当前映射，并将同名的合法用户表保留为表，不再因提前返回丢弃同层字段或误判用户数据。
- HTTP 同步与异步响应读取在追加数据块前检查响应体上限，避免超限数据造成上限之外的瞬时缓冲区扩容。
- HTTP 完成缓存同时遵守请求侧和响应侧的 `Cache-Control: no-store`/`no-cache` 指令，避免调用方明确禁止缓存的请求被写入完成缓存。
- 配置读取的 YAML 后端显式固定有限的别名回放预算（总回放事件最多 1,000,000 次、单个 anchor
  最多展开 10,000 次）；INI/`.env` 类型化读取在违反 serde map 访问顺序时返回错误，不再触发
  内部 panic。
- 配置解析拒绝 JSON 根值后的尾随非空内容；INI section 构建遵守配置的嵌套深度上限；`.env` 插值
  变量名遵守 `[A-Za-z_][A-Za-z0-9_]*` 规则。
- JWT 解码将空 payload 或空 signature 统一归类为 Header/三段结构错误，返回
  `JwtError::InvalidHeader { field: "segments" }`，与其他三段结构错误保持一致。
- Redis 同步连接池健康检查不再在 checkout 时隐式发送 `PING`；协议、网络和超时类命令错误
  会标记连接为不可复用，避免将可能处于未知状态的连接重新放回池中。
- Redis Cluster 的结构化 `CROSSSLOT` 错误码现在与文本形式保持一致，统一映射为
  `RedisError::CrossSlot`，避免误分类为普通服务端错误。
- Redis 集群无法找到可用节点和 RESP3 协商失败现在分别稳定映射为
  `Transport(Connection)` 与 `Transport(Protocol)`，不再落入不明确的 `Other` 分类。
- Redis 异步 Cluster 事务现在会在 runtime 检查前稳定返回 `UnsupportedMode`，不会因调用方
  未处于 Tokio runtime 而误报为 `RuntimeRequired`。

### Security and compatibility

- 邮件配置、邮件头和正文执行大小及控制字符校验；错误信息不会回显密码、主题、正文、用户名、完整主机名
  或地址。
- HTTP 只接受 HTTP/HTTPS URL，拒绝用户信息、Header 注入和超限请求/响应；同步入口拒绝在 Tokio runtime
  中阻塞，异步入口要求调用方提供 runtime；错误不回显 URL、Header 值、请求体、响应体或第三方传输文本。
- HTTP 默认关闭代理、重定向、压缩和隐式重试；默认只对 GET/HEAD/OPTIONS 重试有限的传输失败与瞬态
  状态码，非幂等方法必须显式允许。完成缓存只保存满足安全 Header、请求/响应缓存指令约束的 2xx GET/HEAD。
- 不支持明文 SMTP、机会式 STARTTLS、跳过证书校验、自签名证书、企业私有 CA relay、附件、抄送/密送、
  DKIM、OAuth2、自动重试、后台队列或邮件接收。
- `RandomUtils` 不承诺密码学安全随机数，不应用于密码、令牌或密钥生成；不可信输入的随机字符串长度应由调用方限制。
- 配置读取的错误类型不回显配置文件的原始内容、解析出的值或原始出错行文本；文件大小采用
  `Read::take` 流式截断而非依赖 `fs::metadata`，避免 TOCTOU 及命名管道一类特殊文件耗尽内存。
- YAML 后端显式设置别名展开预算（`serde-saphyr` 的 `Budget`/`AliasLimits`）以防御别名炸弹
  （billion laughs）类拒绝服务输入；TOML 语法本身禁止重复键，JSON/YAML 显式配置或检测为拒绝重复键，
  INI 与 `.env` 由本 crate 检测重复键，五种格式在该点上统一拒绝同一作用域的重复键。
- `MD5` 是摘要算法，已存在实用碰撞攻击，不提供不可逆加密或抗碰撞安全性；**禁止**用于密码存储、
  数字签名、证书、防篡改校验、内容寻址或任何对抗性场景，仅适用于输入不受攻击者控制的非对抗性
  一致性校验（如内部缓存键、去重）。
- `AesMode::CbcPkcs7` **不提供完整性认证**，密文可被篡改且存在 padding oracle 风险；新系统应使用
  `AesMode::Gcm`，CBC 仅用于与旧系统互操作，且必须由上层协议自行提供认证。通过最小长度检查后，
  认证失败、填充非法或 CBC 密文非整块等解密失败统一映射为单一的 `CryptoError::Decrypt`，不区分具体
  原因；低于绝对最小长度的输入返回 `CryptoError::CiphertextTooShort`。
- AES-GCM 随机 96-bit nonce 在同一密钥下的安全消息数上限约为 2^32；`aes_encrypt_with_iv`/
  `aes_decrypt_with_iv` 把 nonce/IV 唯一性责任交给调用方，重用 nonce 会破坏机密性与完整性。
- `AesKey` 内部密钥字节在 `Drop` 时清零（`zeroize`）；`AesCipher` 实例丢弃时会触发该清零，
  `Debug` 只输出模式和密钥位数，不提供导出密钥字节的公开方法。`CryptoUtils` 的全局
  `OnceLock` 与进程同寿命，正常退出前不会触发 `Drop`，全局密钥会常驻内存；需要可控密钥生命周期
  的调用方必须使用 `AesCipher`。随机源仅使用操作系统随机源，失败返回 `CryptoError::RandomSource`，
  不 panic、不回退到非密码学随机源。
- `CryptoError` 不回显明文、密文、密钥、IV、摘要或原始文本内容，仅包含必要的初始化状态、长度、位置
  偏移和编码名称等非敏感信息；
  Base64/十六进制/AES 的可检查输出长度溢出或分配失败统一返回 `CryptoError::OutputTooLarge`。
- 修正 `TextEncodeUnmappable.position` 的语义为 `encoding_rs` 返回的已读取 UTF-8 字节数；AES
  随机密钥材料、`AesCipher` 实例中的密钥、加解密临时缓冲和便捷编码路径中的中间密文在使用后
  或错误返回前清零，减少敏感数据残留在可复用堆内存中的风险；`CryptoUtils` 全局单例是进程级
  常驻状态，不承诺在进程退出前清零。
- `TextEncoding` 的文本编解码严格失败，不做静默字符替换；`Gbk` 编码器无法输出 GB18030 的 4 字节
  序列，WHATWG 标准中 ISO-8859-1/Latin-1 映射为 `Windows1252`。
- `.env` 的 `${VAR}` 插值优先使用文件中已解析的键，找不到时可选择性回退到进程环境变量（默认允许，
   可通过 `ConfigLoader::with_env_substitution(false)` 关闭）；未定义变量返回错误而不会静默替换为
   空字符串；本 crate 只读取文件与（可选）进程环境变量，不写入、不合并、不修改进程环境。这些语义与
   `dotenv`/`dotenvy` 存在已知差异，不声称与其完全兼容。
- JWT 的 Header 只允许固定的 `typ`/`alg`，不支持 `none`、算法降级、动态 key 路由或 JWE 加密；
  claims 在签名验证前后均受重复键、深度、成员、数组元素和大小限制。`JwtError`、key、config 和
  全局 codec 不回显 token、claims、secret、PEM、私钥或第三方原始错误；全局 key 与进程同寿命且
  不支持 reset、轮换、JWKS、撤销、黑名单或重放保护。`jwt` 自身只启用 `rust_crypto` backend；
  通过 feature unification 引入第二个 jsonwebtoken backend 的 provider 竞争不在本 crate 保证范围内。
