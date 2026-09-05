# 编码、摘要与 AES

领域类型从 `axutils::crypto` 导入，静态工具从 `axutils::utils::CryptoUtils` 导入。该能力只处理内存
数据：不提供密钥存储、口令派生、非对称密码学、文件流或密钥轮换。

## 启用

十六进制和 UTF-8 文本编码默认可用。其余能力按需启用：

```toml
[dependencies]
axutils = { version = "1.0", features = ["base64", "md5", "aes", "encoding_rs"] }
```

| feature | 能力 |
| --- | --- |
| `base64` | `Base64Alphabet`、`Base64Options` 与 Base64 编解码 |
| `md5` | 原始 MD5 摘要及小写十六进制摘要 |
| `aes` | `AesKey`、`AesMode`、`AesCipher` 与全局 cipher 生命周期入口 |
| `encoding_rs` | `TextEncoding` 的 GBK、Big5 等 legacy 文本编码变体 |

## 十六进制与文本

十六进制解码接受大小写，拒绝空白、`0x` 前缀和奇数长度。`CryptoError` 的文本不包含原始输入、
明文、密文或密钥。

```rust
use axutils::{crypto::TextEncoding, utils::CryptoUtils};

assert_eq!(CryptoUtils::hex_encode([0, 255]).unwrap(), "00ff");
assert_eq!(CryptoUtils::hex_encode_upper([0, 255]).unwrap(), "00FF");
assert_eq!(CryptoUtils::hex_decode("00Ff").unwrap(), vec![0, 255]);
assert!(CryptoUtils::hex_decode("0x0f").is_err());
assert_eq!(TextEncoding::Utf8.decode(b"Rust").unwrap(), "Rust");
```

## Base64

Base64 严格按照 `Base64Options` 的字母表和填充规则解码；无填充对端数据必须选择相应的
`*_NO_PAD` 选项。

```rust
use axutils::{crypto::Base64Options, utils::CryptoUtils};

let options = Base64Options::URL_SAFE_NO_PAD;
let encoded = CryptoUtils::base64_encode([0xfb, 0xff, 0xfe], options).unwrap();
assert_eq!(encoded, "-__-");
assert_eq!(CryptoUtils::base64_decode(&encoded, options).unwrap(), [0xfb, 0xff, 0xfe]);
assert!(CryptoUtils::base64_decode("Zm8=", Base64Options::STANDARD_NO_PAD).is_err());
```

## MD5

MD5 是非对抗性摘要，不是加密。不得用于密码、签名、证书、防篡改、内容寻址或任何攻击者可控
输入；它仅适合兼容既有系统的非安全一致性校验。

```rust
use axutils::utils::CryptoUtils;

assert_eq!(CryptoUtils::md5_hex("abc"), "900150983cd24fb0d6963f7d28e17f72");
assert_eq!(CryptoUtils::md5("abc").len(), 16);
```

## 实例级 AES

新代码应持有 `AesCipher` 实例以限定密钥生命周期。`Gcm` 提供认证加密；`CbcPkcs7` 不认证密文，
仅用于兼容协议，且上层必须另行提供完整性保护。`encrypt` 的容器路径生成并携带随机 IV/nonce；
`*_with_iv` 用于协议互操作，调用方必须按模式提供正确长度且保证 GCM nonce 不重复。

```rust
use axutils::crypto::{AesCipher, AesMode};

let cipher = AesCipher::from_key_bytes([7_u8; 32], AesMode::Gcm).unwrap();
let encrypted = cipher.encrypt("secret message").unwrap();
assert_eq!(cipher.decrypt(&encrypted).unwrap(), b"secret message");
assert!(cipher.decrypt(b"tampered").is_err());
```

`AesKey::generate` 使用操作系统随机源；`AesKey` 与 `AesCipher` 的 `Debug` 输出会脱敏。调用方仍须
自行保护传入的原始 key 数组或 `Vec<u8>` 副本。

## 全局 cipher

`CryptoUtils` 的 AES 全局入口只管理生命周期：`aes_init`、`aes_init_from_bytes`、
`aes_is_initialized` 和 `cipher`。初始化成功后全局 cipher 不可替换，竞争初始化的未获胜方返回
`CryptoError::AlreadyInitialized`；初始化失败不会占位。它与进程同寿命，密钥不能在正常退出前由本
crate 清零，因此长期服务和多租户场景应使用实例 API。

```rust,no_run
use axutils::{crypto::AesMode, utils::CryptoUtils};

if !CryptoUtils::aes_is_initialized() {
    CryptoUtils::aes_init_from_bytes([9_u8; 32], AesMode::Gcm).unwrap();
}
let cipher = CryptoUtils::cipher()?;
let ciphertext = cipher.encrypt("payload")?;
assert_eq!(cipher.decrypt(ciphertext)?, b"payload");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## 错误与资源边界

- `CryptoError` 使用分类、长度或位置等非敏感信息；不要把敏感输入附加到自己的错误或日志。
- 编码、加密和解密返回拥有的缓冲区，调用方应限制不可信输入大小；本模块不提供流式接口或统一
  输入上限。
- `TextEncoding::Utf8` 默认可用；legacy 编码只有启用 `encoding_rs` 后存在，并应避免把解码后的
  敏感文本写入日志。
