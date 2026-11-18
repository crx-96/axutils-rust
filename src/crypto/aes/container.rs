//! AES 容器格式、显式 IV/nonce 与共享校验。

use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

use super::{cbc, gcm, random, AesKey, AesMode};

/// 使用系统随机 IV/nonce 加密，返回带前缀的容器。
pub(crate) fn encrypt(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    encrypt_with_random(plaintext, key, mode, random::random_bytes)
}

/// 校验容器最小长度，并使用其前缀解密主体。
pub(crate) fn decrypt(input: &[u8], key: &AesKey, mode: AesMode) -> Result<Vec<u8>, CryptoError> {
    let iv_len = mode.iv_length();
    let minimum = iv_len
        .checked_add(mode.min_body_length())
        .expect("iv_length + min_body_length never overflows usize");
    if input.len() < minimum {
        return Err(CryptoError::CiphertextTooShort {
            minimum,
            length: input.len(),
        });
    }
    let (iv, body) = input.split_at(iv_len);
    decrypt_explicit_iv(body, key, iv, mode)
}

/// 用调用方提供的 IV/nonce 加密，仅返回密文主体（GCM 包含标签）。
pub(crate) fn encrypt_explicit_iv(
    plaintext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    encrypt_body(plaintext, key, mode, Vec::new(), Some(iv))
}

/// 从随机源获取容器前缀，并将该缓冲区扩展为最终密文。
pub(super) fn encrypt_with_random<F>(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
    source: F,
) -> Result<Vec<u8>, CryptoError>
where
    F: FnOnce(usize) -> Result<Vec<u8>, CryptoError>,
{
    let prefix = source(mode.iv_length())?;
    encrypt_body(plaintext, key, mode, prefix, None)
}

/// 在 `output` 末尾就地生成密文主体。随机路径的初始内容是 IV，显式 IV 路径从空缓冲区开始。
fn encrypt_body(
    plaintext: &[u8],
    key: &AesKey,
    mode: AesMode,
    mut output: Vec<u8>,
    detached_iv: Option<&[u8]>,
) -> Result<Vec<u8>, CryptoError> {
    // 先检查 IV，再计算主体长度。即使明文已对齐，PKCS#7 也要写入整个填充块。
    validate_iv(detached_iv.unwrap_or(&output), mode)?;
    let (overhead, operation) = match mode {
        AesMode::Gcm => (16, "aes_gcm_encrypt"),
        AesMode::CbcPkcs7 => (16 - plaintext.len() % 16, "aes_cbc_encrypt"),
    };
    let body_len = plaintext
        .len()
        .checked_add(overhead)
        .ok_or(CryptoError::OutputTooLarge { operation })?;
    let prefix_len = output.len();
    let operation = if detached_iv.is_some() {
        operation
    } else {
        "aes_encrypt"
    };
    let total = prefix_len
        .checked_add(body_len)
        .ok_or(CryptoError::OutputTooLarge { operation })?;

    // 获取全部容量后才复制明文，避免在扩容时留下旧的明文分配。
    output
        .try_reserve_exact(body_len)
        .map_err(|_| CryptoError::OutputTooLarge { operation })?;
    output.extend_from_slice(plaintext);
    output.resize(total, 0);
    let (prefix, body) = output.split_at_mut(prefix_len);
    let iv = detached_iv.unwrap_or(prefix);
    let result = match mode {
        AesMode::Gcm => gcm::encrypt_in_place(body, plaintext.len(), key, iv),
        AesMode::CbcPkcs7 => cbc::encrypt_in_place(body, plaintext.len(), key, iv),
    };
    // 后端失败时主体可能仍含明文或部分加密结果，释放前擦除整段。
    if let Err(error) = result {
        body.zeroize();
        return Err(error);
    }
    Ok(output)
}

/// 校验外置 IV/nonce 和主体最小长度后，执行对应模式的解密。
pub(crate) fn decrypt_explicit_iv(
    ciphertext: &[u8],
    key: &AesKey,
    iv: &[u8],
    mode: AesMode,
) -> Result<Vec<u8>, CryptoError> {
    validate_iv(iv, mode)?;
    let minimum = mode.min_body_length();
    if ciphertext.len() < minimum {
        return Err(CryptoError::CiphertextTooShort {
            minimum,
            length: ciphertext.len(),
        });
    }
    match mode {
        AesMode::Gcm => gcm::decrypt(ciphertext, key, iv),
        AesMode::CbcPkcs7 => cbc::decrypt(ciphertext, key, iv),
    }
}

/// 拒绝当前模式不支持的 IV 长度，错误只携带长度。
fn validate_iv(iv: &[u8], mode: AesMode) -> Result<(), CryptoError> {
    if iv.len() != mode.iv_length() {
        return Err(CryptoError::InvalidIvLength {
            expected: mode.iv_length(),
            length: iv.len(),
        });
    }
    Ok(())
}
