# crypto 模块与 CryptoUtils 使用文档

> 十六进制编解码（`hex_encode`/`hex_encode_upper`/`hex_decode`）与 `TextEncoding::Utf8` 文本
> 编解码**默认可用**，不依赖任何第三方 crate。Base64 需要 `base64` feature，MD5 需要 `md5`
> feature（实际启用 crates.io 上的 `md-5` crate，**不是**同名的旧 `md5` crate），AES 需要 `aes`
> feature（聚合 `aes`/`aes-gcm`/`cbc`/`zeroize` 四个内部适配依赖）；`encoding_rs` feature 为
> `TextEncoding` 追加六个 legacy 编码变体；同时启用 `aes` 与 `base64` 后额外提供
> `aes_encrypt_base64`/`aes_decrypt_base64`。

## 能力概览与 feature 矩阵

| 启用组合 | 新增可用能力（累加在默认能力之上） |
| --- | --- |
| 无 | `CryptoUtils::{hex_encode,hex_encode_upper,hex_decode}`、`TextEncoding::Utf8` 文本编解码 |
| 仅 `encoding_rs` | `TextEncoding` 追加 `Gbk`/`Gb18030`/`Big5`/`ShiftJis`/`EucKr`/`Windows1252` |
| `base64` | `Base64Alphabet`、`Base64Options`、`base64_encode`/`base64_encode_text`/`base64_decode`/`base64_decode_text` |
| `md5` | `md5`/`md5_hex`/`md5_text`/`md5_hex_text` |
| `aes` | `AesKey`/`AesKeyBits`/`AesMode`、`aes_encrypt`/`aes_decrypt`/`aes_encrypt_with_iv`/`aes_decrypt_with_iv`/`aes_encrypt_hex`/`aes_decrypt_hex` |
| `aes` + `base64` | 以上全部，另加 `aes_encrypt_base64`/`aes_decrypt_base64` |
| `base64`/`md5` + `encoding_rs` | 对应 `*_text` 方法可传入 legacy `TextEncoding` 变体 |
| `--all-features` | 全部 |

MD5 是摘要算法，不是加密，不可逆；已存在实用碰撞攻击，**禁止**用于密码存储、数字签名、证书、
防篡改校验、内容寻址或任何对抗性场景，仅适用于与既有系统对接、且输入不受攻击者控制的非对抗性
一致性校验（如内部缓存键、去重）。`AesMode::CbcPkcs7` **不提供完整性认证**，密文可被篡改且存在
padding oracle 风险；新系统应使用 `AesMode::Gcm`，CBC 仅用于与旧系统互操作，且必须由上层协议
自行提供认证。`CryptoError` 不回显明文、密文、密钥、IV、摘要或原始文本内容，只包含长度、位置
偏移和编码名称。

## 导出内容

公开模块路径：

- `axutils::crypto`：crypto 领域类型的直接模块路径（默认可用）；
- `axutils::crypto_utils`、`axutils::utils::crypto_utils`：`CryptoUtils` 的公开子模块路径。

`CryptoUtils`、`CryptoError`、`TextEncoding` 均无 feature 守卫，同时支持：

- 推荐 crate 根路径：`axutils::CryptoUtils`、`axutils::CryptoError`、`axutils::TextEncoding`；
- 次级领域模块路径：`axutils::crypto::CryptoError`、`axutils::crypto::TextEncoding`；
- `axutils::utils::CryptoUtils`、`axutils::crypto_utils::CryptoUtils`、
  `axutils::utils::crypto_utils::CryptoUtils`。

`Base64Alphabet`、`Base64Options`（`feature = "base64"`）与 `AesKey`、`AesKeyBits`、`AesMode`
（`feature = "aes"`）同时支持 `axutils::*` 与 `axutils::crypto::*` 两条路径。`src/crypto/` 下的
`error.rs`、`text.rs`、`hex.rs`、`base64.rs`、`md5.rs`、`aes.rs` 是私有实现文件，不是公共导入
路径。

`CryptoError` 标记 `#[non_exhaustive]`，实现 `Debug`、`Clone`、`PartialEq`、`Eq`、`Display` 和
`std::error::Error`；匹配时应保留 `_` 通配分支。`TextEncoding`、`Base64Alphabet`、`AesKeyBits`、
`AesMode` 同样标记 `#[non_exhaustive]`，实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`。
`Base64Options` 字段私有，实现 `Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`。`AesKey` 字段私有，
手动实现脱敏 `Debug`（只输出密钥位数）与清零 `Drop`，不实现 `Display`、`Clone`、`Copy` 或任何
序列化 trait，也不提供导出密钥字节的公开方法。`CryptoUtils` 是无状态工具结构体，无公共字段和
`new` 方法，实现 `Debug`、`Clone`、`Copy`、`Default`。本模块没有公共自由函数、trait、类型
别名、静态项或宏。

## 安装与启用

十六进制与 UTF-8 文本编解码开箱即用，无需任何 feature：

```toml
[dependencies]
axutils = "0.1"
```

Base64：

```toml
[dependencies]
axutils = { version = "0.1", features = ["base64"] }
```

MD5：

```toml
[dependencies]
axutils = { version = "0.1", features = ["md5"] }
```

AES：

```toml
[dependencies]
axutils = { version = "0.1", features = ["aes"] }
```

AES 密文的 Base64 便捷方法：

```toml
[dependencies]
axutils = { version = "0.1", features = ["aes", "base64"] }
```

legacy 文本编码：

```toml
[dependencies]
axutils = { version = "0.1", features = ["encoding_rs"] }
```

## `CryptoError`

| 变体 | 触发条件 | feature 守卫 |
| --- | --- | --- |
| `OddHexLength { length }` | `hex_decode` 输入长度为奇数 | 无 |
| `InvalidHex { position }` | `hex_decode` 在该字节偏移遇到非 `0-9a-fA-F` 字符（含空白、`0x` 前缀） | 无 |
| `TextDecodeInvalid { encoding, position }` | 字节序列不是目标编码的合法文本；UTF-8 失败时 `position` 是有效前缀长度，legacy 编码无法提供可靠偏移时为 `None` | 无 |
| `OutputTooLarge { operation }` | 输出长度计算溢出，或无法为该操作预留结果空间 | 无 |
| `TextEncodeUnmappable { encoding, position }` | 文本无法用目标 legacy 编码表示（如 GBK 遇到需要 GB18030 4 字节序列的字符），`position` 为 `encoding_rs` 报告的已读取 UTF-8 字节数 | `encoding_rs` |
| `Base64Decode { position }` | Base64 输入含非法字符、长度非法、非规范尾随比特或与填充设置不符；上游提供可靠偏移时填写，否则为 `None` | `base64` |
| `InvalidKeyLength { length }` | AES 密钥长度不是 16、24 或 32 字节 | `aes` |
| `InvalidIvLength { expected, length }` | 显式 IV/nonce 长度与所选 `AesMode` 不匹配 | `aes` |
| `CiphertextTooShort { minimum, length }` | 密文/容器长度小于当前调用形态的绝对最小长度 | `aes` |
| `Decrypt` | 解密失败：认证失败、篡改或填充非法，**不区分具体原因**（避免 padding oracle） | `aes` |
| `Encrypt` | 加密失败；不区分具体上游原因 | `aes` |
| `RandomSource` | 操作系统随机源不可用（`AesKey::generate` 或随机 IV/nonce 生成） | `aes` |

```rust
use axutils::CryptoUtils;

let err = CryptoUtils::hex_decode("abc").unwrap_err();
assert_eq!(format!("{err}"), "hex string has odd length 3");
```

## `TextEncoding`

```rust
#[non_exhaustive]
pub enum TextEncoding {
    Utf8,
    // 以下变体需要 `encoding_rs` feature：
    // Gbk, Gb18030, Big5, ShiftJis, EucKr, Windows1252
}
```

`Utf8` 无需任何 feature；legacy 变体遵循 WHATWG Encoding Standard：ISO-8859-1/Latin-1 在该标准
中映射为 `windows-1252`；`Gbk` 编码器无法输出 GB18030 的 4 字节序列（需要完整覆盖时应选
`Gb18030`）。不处理也不生成 BOM，不做编码探测。

### `TextEncoding::as_str(&self) -> &'static str`

返回与 WHATWG 标签一致的编码名称：`"UTF-8"`、`"GBK"`、`"gb18030"`、`"Big5"`、`"Shift_JIS"`、
`"EUC-KR"`、`"windows-1252"`。

```rust
use axutils::TextEncoding;

assert_eq!(TextEncoding::Utf8.as_str(), "UTF-8");
```

### `TextEncoding::encode(&self, text: &str) -> Result<Vec<u8>, CryptoError>`

把 `text` 编码为字节。UTF-8 分支永不因内容失败；legacy 分支遇到无法表示的字符返回
`CryptoError::TextEncodeUnmappable`（`position` 是 `encoding_rs` 报告的已读取 UTF-8 字节数）；可
检查的容量失败返回 `CryptoError::OutputTooLarge`。

```rust
use axutils::TextEncoding;

let bytes = TextEncoding::Utf8.encode("hello").unwrap();
assert_eq!(bytes, b"hello");
```

```rust
# #[cfg(feature = "encoding_rs")]
# fn main() {
use axutils::{CryptoError, TextEncoding};

let err = TextEncoding::Gbk.encode("𠀀").unwrap_err();
assert!(matches!(err, CryptoError::TextEncodeUnmappable { encoding: "GBK", .. }));
# }
# #[cfg(not(feature = "encoding_rs"))]
# fn main() {}
```

### `TextEncoding::decode(&self, bytes: impl AsRef<[u8]>) -> Result<String, CryptoError>`

把字节按本编码解码为 `String`。非法字节序列返回 `CryptoError::TextDecodeInvalid`；可检查的容量
失败返回 `CryptoError::OutputTooLarge`。

```rust
use axutils::TextEncoding;

assert_eq!(TextEncoding::Utf8.decode(b"hello").unwrap(), "hello");
assert!(TextEncoding::Utf8.decode(&[0xff, 0xfe]).is_err());
```

## 十六进制方法（`CryptoUtils`，默认可用）

### `CryptoUtils::hex_encode(input: impl AsRef<[u8]>) -> Result<String, CryptoError>`

编码为小写十六进制字符串。

```rust
use axutils::CryptoUtils;

assert_eq!(CryptoUtils::hex_encode([0x00, 0xff]).unwrap(), "00ff");
assert_eq!(CryptoUtils::hex_encode([]).unwrap(), "");
```

### `CryptoUtils::hex_encode_upper(input: impl AsRef<[u8]>) -> Result<String, CryptoError>`

编码为大写十六进制字符串。

```rust
use axutils::CryptoUtils;

assert_eq!(CryptoUtils::hex_encode_upper([0x00, 0xff]).unwrap(), "00FF");
```

### `CryptoUtils::hex_decode(input: &str) -> Result<Vec<u8>, CryptoError>`

同时接受大小写；拒绝空白、`0x` 前缀和奇数长度。

```rust
use axutils::{CryptoError, CryptoUtils};

assert_eq!(CryptoUtils::hex_decode("00Ff").unwrap(), vec![0x00, 0xff]);
assert!(matches!(
    CryptoUtils::hex_decode("abc").unwrap_err(),
    CryptoError::OddHexLength { length: 3 }
));
assert!(CryptoUtils::hex_decode("0x0f").is_err());
```

`hex_encode`/`hex_encode_upper`/`hex_decode` 是通用字节↔十六进制转换，MD5/AES 的字符串输出只是
其调用方之一；它们不是常量时间实现，不要用于比较机密值（见下方“安全边界”一节）。

## Base64（`feature = "base64"`）

### `Base64Alphabet`

`Standard`（RFC 4648 §4，含 `+`/`/`）或 `UrlSafe`（RFC 4648 §5，含 `-`/`_`）。

### `Base64Options`

带私有字段的结构体，正交表达“字母表 × 是否填充”：

- `Base64Options::STANDARD`：标准字母表 + 有填充；
- `Base64Options::STANDARD_NO_PAD`：标准字母表 + 无填充；
- `Base64Options::URL_SAFE`：URL-safe 字母表 + 有填充；
- `Base64Options::URL_SAFE_NO_PAD`：URL-safe 字母表 + 无填充。

#### `Base64Options::new(alphabet: Base64Alphabet, padding: bool) -> Self`

显式构造字母表与填充组合。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Alphabet, Base64Options};

let options = Base64Options::new(Base64Alphabet::UrlSafe, false);
assert_eq!(options, Base64Options::URL_SAFE_NO_PAD);
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

#### `Base64Options::alphabet(&self) -> Base64Alphabet`

返回当前使用的字母表。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Alphabet, Base64Options};

assert_eq!(Base64Options::STANDARD.alphabet(), Base64Alphabet::Standard);
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

#### `Base64Options::padding(&self) -> bool`

返回是否包含 `=` 填充。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::Base64Options;

assert!(!Base64Options::STANDARD_NO_PAD.padding());
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

### `CryptoUtils::base64_encode(input: impl AsRef<[u8]>, options: Base64Options) -> Result<String, CryptoError>`

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Options, CryptoUtils};

assert_eq!(CryptoUtils::base64_encode("foobar", Base64Options::STANDARD).unwrap(), "Zm9vYmFy");
assert_eq!(CryptoUtils::base64_encode("foob", Base64Options::STANDARD_NO_PAD).unwrap(), "Zm9vYg");
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

### `CryptoUtils::base64_encode_text(text: &str, encoding: TextEncoding, options: Base64Options) -> Result<String, CryptoError>`

先按 `encoding` 编码为字节，再编码为 Base64。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Options, CryptoUtils, TextEncoding};

let encoded =
    CryptoUtils::base64_encode_text("foobar", TextEncoding::Utf8, Base64Options::STANDARD).unwrap();
assert_eq!(encoded, "Zm9vYmFy");
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

### `CryptoUtils::base64_decode(input: &str, options: Base64Options) -> Result<Vec<u8>, CryptoError>`

**解码严格**：`options.padding()` 为 `true` 时要求规范填充，为 `false` 时拒绝任何 `=`；一律拒绝
空白字符、非法字符和非零尾随比特。该严格程度高于不少语言的标准库实现；对方系统产生无填充
Base64 时应改用 `Base64Options::*_NO_PAD`。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Options, CryptoUtils};

assert_eq!(CryptoUtils::base64_decode("Zm9vYmFy", Base64Options::STANDARD).unwrap(), b"foobar");
// 用错字母表会被拒绝：
let url_only = CryptoUtils::base64_encode(&[0xfbu8, 0xff, 0xfe], Base64Options::URL_SAFE).unwrap();
assert!(CryptoUtils::base64_decode(&url_only, Base64Options::STANDARD).is_err());
// 携带 `=` 填充的输入在无填充设置下被拒绝：
assert!(CryptoUtils::base64_decode("Zm9vYg==", Base64Options::STANDARD_NO_PAD).is_err());
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

### `CryptoUtils::base64_decode_text(input: &str, encoding: TextEncoding, options: Base64Options) -> Result<String, CryptoError>`

解码 Base64 为字节，再按 `encoding` 解码为文本。

```rust
# #[cfg(feature = "base64")]
# fn main() {
use axutils::{Base64Options, CryptoUtils, TextEncoding};

let text =
    CryptoUtils::base64_decode_text("Zm9vYmFy", TextEncoding::Utf8, Base64Options::STANDARD).unwrap();
assert_eq!(text, "foobar");
# }
# #[cfg(not(feature = "base64"))]
# fn main() {}
```

## MD5（`feature = "md5"`）

> **MD5 是摘要算法，不是加密，不可逆；已存在实用碰撞攻击，禁止用于密码存储、数字签名、证书、
> 防篡改校验、内容寻址或任何对抗性场景**，仅适用于与既有系统对接、且输入不受攻击者控制的非
> 对抗性一致性校验（如内部缓存键、去重）。需要抗碰撞性的输入应改用现代摘要算法（如 SHA-2），
> 本 crate 首期不提供。

### `CryptoUtils::md5(input: impl AsRef<[u8]>) -> [u8; 16]`

返回定长数组，避免无谓分配。

```rust
# #[cfg(feature = "md5")]
# fn main() {
use axutils::CryptoUtils;

let digest = CryptoUtils::md5("abc");
assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
# }
# #[cfg(not(feature = "md5"))]
# fn main() {}
```

### `CryptoUtils::md5_hex(input: impl AsRef<[u8]>) -> String`

32 字符小写十六进制结果；大写结果通过组合得到：
`CryptoUtils::hex_encode_upper(CryptoUtils::md5(x))?`。

```rust
# #[cfg(feature = "md5")]
# fn main() {
use axutils::CryptoUtils;

assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
assert_eq!(CryptoUtils::md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
# }
# #[cfg(not(feature = "md5"))]
# fn main() {}
```

### `CryptoUtils::md5_text(text: &str, encoding: TextEncoding) -> Result<[u8; 16], CryptoError>`

先按 `encoding` 编码为字节，再计算 MD5。

```rust
# #[cfg(feature = "md5")]
# fn main() {
use axutils::{CryptoUtils, TextEncoding};

let digest = CryptoUtils::md5_text("abc", TextEncoding::Utf8).unwrap();
assert_eq!(CryptoUtils::hex_encode(digest).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
# }
# #[cfg(not(feature = "md5"))]
# fn main() {}
```

### `CryptoUtils::md5_hex_text(text: &str, encoding: TextEncoding) -> Result<String, CryptoError>`

```rust
# #[cfg(feature = "md5")]
# fn main() {
use axutils::{CryptoUtils, TextEncoding};

let hex = CryptoUtils::md5_hex_text("abc", TextEncoding::Utf8).unwrap();
assert_eq!(hex, "900150983cd24fb0d6963f7d28e17f72");
# }
# #[cfg(not(feature = "md5"))]
# fn main() {}
```

## AES（`feature = "aes"`）

### `AesKeyBits`

`Aes128`/`Aes192`/`Aes256`，分别对应 16/24/32 字节密钥。

#### `AesKeyBits::bit_length(&self) -> usize`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesKeyBits;

assert_eq!(AesKeyBits::Aes256.bit_length(), 256);
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

#### `AesKeyBits::byte_length(&self) -> usize`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesKeyBits;

assert_eq!(AesKeyBits::Aes256.byte_length(), 32);
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `AesKey`

对称密钥；`Debug` 只输出密钥位数，`Drop` 时清零内部缓冲区（`zeroize`），不实现 `Display`、
`Clone` 或任何序列化 trait，也不提供导出密钥字节的公开方法。

#### `AesKey::from_bytes(key: impl AsRef<[u8]>) -> Result<Self, CryptoError>`

长度必须是 16、24 或 32 字节。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesKey;

let key = AesKey::from_bytes([0x00; 16]).unwrap();
assert_eq!(key.bits().byte_length(), 16);
assert!(AesKey::from_bytes([0x00; 15]).is_err());
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

#### `AesKey::generate(bits: AesKeyBits) -> Result<Self, CryptoError>`

使用操作系统随机源生成新密钥；随机源不可用时返回 `CryptoError::RandomSource`。这是本地随机
密钥材料的构造便捷入口，不提供密钥存储、轮换或封装等密钥管理策略。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesKeyBits};

let key = AesKey::generate(AesKeyBits::Aes256).unwrap();
assert_eq!(key.bits(), AesKeyBits::Aes256);
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

#### `AesKey::bits(&self) -> AesKeyBits`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesKeyBits};

let key = AesKey::from_bytes([0x00; 24]).unwrap();
assert_eq!(key.bits(), AesKeyBits::Aes192);
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `AesMode`

`Gcm`（带认证加密，12 字节 nonce、16 字节 tag，推荐默认）或 `CbcPkcs7`（**无完整性认证**，16
字节 IV，仅用于与旧系统互操作）。

#### `AesMode::iv_length(&self) -> usize`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesMode;

assert_eq!(AesMode::Gcm.iv_length(), 12);
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

#### `AesMode::is_authenticated(&self) -> bool`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesMode;

assert!(AesMode::Gcm.is_authenticated());
assert!(!AesMode::CbcPkcs7.is_authenticated());
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

#### `AesMode::as_str(&self) -> &'static str`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::AesMode;

assert_eq!(AesMode::Gcm.as_str(), "AES-GCM");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### 密文布局

| 模式 | `aes_encrypt` 输出 | 最小长度 | `aes_encrypt_with_iv` 输出 | 最小长度（`_with_iv`） |
| --- | --- | --- | --- | --- |
| `Gcm` | `nonce(12) \|\| 密文(n) \|\| tag(16)` | 28 | `密文(n) \|\| tag(16)` | 16 |
| `CbcPkcs7` | `iv(16) \|\| 密文(16 的倍数)` | 32 | `密文(16 的倍数)` | 16 |

`aes_decrypt`/`aes_decrypt_with_iv` 首先按上表检查绝对长度下限，不足返回
`CryptoError::CiphertextTooShort`；CBC 密文部分若不是 16 字节整数倍，或进入实际解密/填充/认证
校验阶段后失败，一律归为 `CryptoError::Decrypt`（不区分具体原因，避免 padding oracle）。
跨语言对接：Java 的 `AES/GCM/NoPadding` 对应本模块 `AesMode::Gcm` 的 `_with_iv` 路径（tag 前置
到输出末尾，与 Java 一致），`AES/CBC/PKCS5Padding` 对应 `AesMode::CbcPkcs7` 的 `_with_iv` 路径
（Java 的 `PKCS5Padding` 在 AES 场景下与 PKCS#7 等价）。

### `CryptoUtils::aes_encrypt(plaintext: impl AsRef<[u8]>, key: &AesKey, mode: AesMode) -> Result<Vec<u8>, CryptoError>`

内部生成随机 IV/nonce，输出布局见上表。同一密钥下 GCM 随机 96-bit nonce 的安全消息数上限约为
2^32；长期高频场景应自行轮换密钥。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 32]).unwrap();
let ciphertext = CryptoUtils::aes_encrypt("hello world", &key, AesMode::Gcm).unwrap();
assert_eq!(ciphertext.len(), 12 + 11 + 16);
let plaintext = CryptoUtils::aes_decrypt(&ciphertext, &key, AesMode::Gcm).unwrap();
assert_eq!(plaintext, b"hello world");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_decrypt(input: impl AsRef<[u8]>, key: &AesKey, mode: AesMode) -> Result<Vec<u8>, CryptoError>`

输入必须是 `aes_encrypt` 的完整输出（含前置 IV）。直接传入 `&str` 时按其 UTF-8 字节处理，不会
自动把十六进制/Base64 文本解码；文本密文应使用 `aes_decrypt_hex` 或（启用 `base64` 后）
`aes_decrypt_base64`。解密到字符串时对结果调用 `TextEncoding::decode`。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let ciphertext = CryptoUtils::aes_encrypt("secret", &key, AesMode::CbcPkcs7).unwrap();
assert_eq!(CryptoUtils::aes_decrypt(&ciphertext, &key, AesMode::CbcPkcs7).unwrap(), b"secret");
assert!(matches!(
    CryptoUtils::aes_decrypt(&[0u8; 10], &key, AesMode::Gcm),
    Err(axutils::CryptoError::CiphertextTooShort { minimum: 28, length: 10 })
));
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_encrypt_with_iv(plaintext: impl AsRef<[u8]>, key: &AesKey, iv: &[u8], mode: AesMode) -> Result<Vec<u8>, CryptoError>`

互操作路径：IV/nonce 由调用方提供，输出**不含** IV。**警告：GCM 下重用 nonce 会破坏机密性与
完整性，可能导致多条消息的明文恢复或伪造**；调用方必须自行保证每次调用的 `iv` 唯一。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let iv = [0x00; 12];
let ciphertext = CryptoUtils::aes_encrypt_with_iv("hello", &key, &iv, AesMode::Gcm).unwrap();
assert_eq!(ciphertext.len(), 5 + 16);
assert!(matches!(
    CryptoUtils::aes_encrypt_with_iv("x", &key, &[0u8; 11], AesMode::Gcm),
    Err(axutils::CryptoError::InvalidIvLength { expected: 12, length: 11 })
));
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_decrypt_with_iv(ciphertext: impl AsRef<[u8]>, key: &AesKey, iv: &[u8], mode: AesMode) -> Result<Vec<u8>, CryptoError>`

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let iv = [0x00; 12];
let ciphertext = CryptoUtils::aes_encrypt_with_iv("hello", &key, &iv, AesMode::Gcm).unwrap();
let plaintext = CryptoUtils::aes_decrypt_with_iv(&ciphertext, &key, &iv, AesMode::Gcm).unwrap();
assert_eq!(plaintext, b"hello");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_encrypt_hex(plaintext: impl AsRef<[u8]>, key: &AesKey, mode: AesMode) -> Result<String, CryptoError>`

等价于 `hex_encode(aes_encrypt(..)?)`；使用随机 IV/nonce。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let hex = CryptoUtils::aes_encrypt_hex("hello", &key, AesMode::Gcm).unwrap();
assert_eq!(CryptoUtils::aes_decrypt_hex(&hex, &key, AesMode::Gcm).unwrap(), b"hello");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_decrypt_hex(input: &str, key: &AesKey, mode: AesMode) -> Result<Vec<u8>, CryptoError>`

等价于 `aes_decrypt(hex_decode(..)?, ..)`。

```rust
# #[cfg(feature = "aes")]
# fn main() {
use axutils::{AesKey, AesMode, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let hex = CryptoUtils::aes_encrypt_hex("hello", &key, AesMode::CbcPkcs7).unwrap();
assert_eq!(CryptoUtils::aes_decrypt_hex(&hex, &key, AesMode::CbcPkcs7).unwrap(), b"hello");
# }
# #[cfg(not(feature = "aes"))]
# fn main() {}
```

### `CryptoUtils::aes_encrypt_base64(plaintext: impl AsRef<[u8]>, key: &AesKey, mode: AesMode, options: Base64Options) -> Result<String, CryptoError>`

仅在同时启用 `aes` 与 `base64` 时提供；等价于 `base64_encode(aes_encrypt(..)?, options)`。

```rust
# #[cfg(all(feature = "aes", feature = "base64"))]
# fn main() {
use axutils::{AesKey, AesMode, Base64Options, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let text =
    CryptoUtils::aes_encrypt_base64("hello", &key, AesMode::Gcm, Base64Options::STANDARD).unwrap();
let plaintext =
    CryptoUtils::aes_decrypt_base64(&text, &key, AesMode::Gcm, Base64Options::STANDARD).unwrap();
assert_eq!(plaintext, b"hello");
# }
# #[cfg(not(all(feature = "aes", feature = "base64")))]
# fn main() {}
```

### `CryptoUtils::aes_decrypt_base64(input: &str, key: &AesKey, mode: AesMode, options: Base64Options) -> Result<Vec<u8>, CryptoError>`

仅在同时启用 `aes` 与 `base64` 时提供；等价于 `aes_decrypt(base64_decode(.., options)?, ..)`。

```rust
# #[cfg(all(feature = "aes", feature = "base64"))]
# fn main() {
use axutils::{AesKey, AesMode, Base64Options, CryptoUtils};

let key = AesKey::from_bytes([0x00; 16]).unwrap();
let text = CryptoUtils::aes_encrypt_base64(
    "hello",
    &key,
    AesMode::Gcm,
    Base64Options::STANDARD,
)
.unwrap();
let plaintext = CryptoUtils::aes_decrypt_base64(
    &text,
    &key,
    AesMode::Gcm,
    Base64Options::STANDARD,
)
.unwrap();
assert_eq!(plaintext, b"hello");
# }
# #[cfg(not(all(feature = "aes", feature = "base64")))]
# fn main() {}
```

## 安全边界

1. **MD5**：见本文档 MD5 一节开头的警告，另见 README/CHANGELOG 的同等口径说明。
2. **CBC 无认证**：`AesMode::CbcPkcs7` 不提供完整性保护，密文可被篡改且存在 padding oracle
   风险；新系统应使用 `Gcm`，CBC 仅用于互操作，且必须由上层协议提供认证。
3. **GCM nonce 唯一性**：`aes_encrypt` 每次调用生成新的随机 12 字节 nonce；同一密钥下随机
   96-bit nonce 的安全消息数上限约为 2^32。`aes_encrypt_with_iv` 把唯一性责任交给调用方——
   **重用 nonce 会破坏机密性与完整性，可能导致多条消息的明文恢复或伪造**。GCM 单次调用的明文
   长度还受 NIST SP 800-38D 限制（约 64 GiB），超限时返回 `CryptoError::Encrypt`，极少在内存
   数据场景触达。
4. **不提供 KDF**：密钥必须是 16/24/32 字节高熵材料，推荐 `AesKey::generate`；口令派生需要
   调用方使用 Argon2/PBKDF2 等专用库。**不要**用 `CryptoUtils::md5(password)` 当作 AES 密钥。
5. **密钥材料卫生**：`AesKey` 在 `Drop` 时清零、`Debug` 脱敏、不可导出、不实现序列化；中间
   缓冲区（如调用方持有的明文 `Vec<u8>`）由调用方负责，本 crate 无法控制其生命周期。
6. **随机源**：只使用操作系统随机源；失败返回 `CryptoError::RandomSource`，不 panic、不降级到
   `RandomUtils`（`rand` feature）等非密码学 RNG；两者定位不同、无共用代码。
7. **错误脱敏**：`CryptoError` 的 `Display`/`Debug` 都不包含明文、密文、密钥、IV、摘要或原始
   文本片段，只包含长度、位置偏移和编码名称。
8. **复杂度与内存**：全部 API 为输入长度的线性时间；输出规模为 hex = 2n、Base64 ≈ 4n/3
   （含填充向上取整）、AES-GCM = n + 28、AES-CBC ≤ n + 32。容量计算使用 checked 算术与
   `try_reserve`，可检查的溢出/预留失败返回 `CryptoError::OutputTooLarge`；底层分配器在真正
   OOM 时的 abort 不属于本 crate 可恢复的错误语义。首期不设硬性输入上限，对不可信输入应由
   调用方限制规模；本 crate 不提供流式接口。
9. **无隐式副作用**：不读写文件、不访问网络、不读取环境变量、不使用全局可变状态、不缓存密钥；
   除 `AesKey::generate`/随机 IV 生成外不消耗系统熵。
10. **不承诺常量时间**：除 `aes-gcm` 内部的 tag 校验外，本 crate 不承诺任何常量时间行为；
    十六进制/Base64 解码、`PartialEq` 比较都不是常量时间，不要用它们比较机密值。
11. **依赖 unsafe 面**：`base64` 关闭 `simd-unsafe`；`encoding_rs` 不启用 `simd-accel`；
    RustCrypto 系列自带实现按其上游默认配置使用，不额外开启 `hazmat` 等低级 feature。

## 首期非目标

任何非对称密码学（RSA、ECDSA、Ed25519、X25519、证书、PKI）；其他摘要与 MAC（SHA 系列、
BLAKE、HMAC-\*、CMAC、Poly1305 独立入口）；口令派生与口令哈希（PBKDF2、scrypt、Argon2、
bcrypt）；密钥生命周期管理（生成策略、存储、轮换、封装、KMS/HSM、密钥文件格式）；流式/增量
接口、文件加解密、`Read`/`Write` 适配器、异步接口；除 GCM 自带认证外的 AAD 参数，以及 SIV、
XTS、CCM、OCB、CFB、OFB、ECB（ECB 明确永不提供）；常量时间比较等侧信道加固的公共 API；
Base64 的 MIME 换行变体、Base32、Base58、Base85、URL 百分号编码；编码探测/猜测。需要完整
密码学能力的调用方应直接使用 RustCrypto 或 `ring`。
