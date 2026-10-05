//! 系统安全随机生成；缓冲区和随机源失败均以脱敏错误传播。

use super::{facade::CryptoUtils, hex, CryptoError};

/// 数字拒绝采样每次读取的最大字节数；限制临时栈空间，不限制业务输出长度。
const DIGIT_BATCH_SIZE: usize = 64;

impl CryptoUtils {
    /// 生成 `length` 字节的系统安全随机数据，需要 `secure-random` feature。
    ///
    /// 零长度返回空结果且不访问随机源。随机源失败返回 `CryptoError::RandomSource`，
    /// 不返回部分结果；可报告的预留失败返回 `CryptoError::OutputTooLarge`。
    /// 业务长度上限由调用方控制；系统随机源在平台启动早期等情况下可能阻塞。
    /// 返回的缓冲区由调用方拥有，本接口不自动清零其内容。
    ///
    /// ```
    /// # #[cfg(feature = "secure-random")] {
    /// use axutils::utils::CryptoUtils;
    /// assert_eq!(CryptoUtils::secure_random_bytes(16).unwrap().len(), 16);
    /// # }
    /// ```
    pub fn secure_random_bytes(length: usize) -> Result<Vec<u8>, CryptoError> {
        // 统一经可失败分配和系统随机源适配；私有注入点只用于测试失败路径。
        bytes_with(length, fill_random)
    }

    /// 生成 `length` 个 ASCII 数字，需要 `secure-random` feature，保留前导零。
    ///
    /// 采用拒绝采样避免取模偏差；零长度不访问随机源。失败类型、阻塞可能性和
    /// 输出所有权与 [`Self::secure_random_bytes`] 相同，不返回已生成的部分字符串。
    ///
    /// ```
    /// # #[cfg(feature = "secure-random")] {
    /// use axutils::utils::CryptoUtils;
    /// let digits = CryptoUtils::secure_random_digits(6).unwrap();
    /// assert_eq!(digits.len(), 6);
    /// assert!(digits.bytes().all(|byte| byte.is_ascii_digit()));
    /// # }
    /// ```
    pub fn secure_random_digits(length: usize) -> Result<String, CryptoError> {
        // 不借助整数格式化，因此采样得到的前导零不会被删掉。
        digits_with(length, fill_random)
    }

    /// 生成 `byte_length` 字节对应的小写 Hex 字符串，需要 `secure-random` feature。
    ///
    /// 输出长度为 `byte_length * 2`，例如 16 字节对应 32 字符；本接口不定义业务标识协议。
    /// 长度溢出或可报告的容量预留失败返回 `CryptoError::OutputTooLarge`；
    /// 零长度、随机源失败和阻塞语义与 [`Self::secure_random_bytes`] 相同。
    ///
    /// ```
    /// # #[cfg(feature = "secure-random")] {
    /// use axutils::utils::CryptoUtils;
    /// let token = CryptoUtils::secure_random_hex(16).unwrap();
    /// assert_eq!(token.len(), 32);
    /// assert!(token.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    /// # }
    /// ```
    pub fn secure_random_hex(byte_length: usize) -> Result<String, CryptoError> {
        // 长度预检先于采样，最终编码复用同领域 Hex 实现。
        hex_with(byte_length, fill_random)
    }
}

/// 填充系统随机字节，仅保留可供调用方分支处理的失败类别。
fn fill_random(output: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(output).map_err(|_| CryptoError::RandomSource)
}

/// 预留并填满拥有型字节缓冲区；私有填充回调使故障注入不进入公共 API。
fn bytes_with<F>(length: usize, mut fill: F) -> Result<Vec<u8>, CryptoError>
where
    F: FnMut(&mut [u8]) -> Result<(), CryptoError>,
{
    // 必须先预留再调整长度，确保可报告的分配错误不会转为分配 panic。
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "secure_random_bytes",
        })?;
    output.resize(length, 0);
    // 空结果直接成功；填充失败时拥有型缓冲区销毁，不向调用方泄露部分结果。
    if !output.is_empty() {
        fill(&mut output)?;
    }
    Ok(output)
}

/// 使用 0..250 的等概率子集生成数字；拒绝 250..=255，避免十个余数出现次数不同。
fn digits_with<F>(length: usize, mut fill: F) -> Result<String, CryptoError>
where
    F: FnMut(&mut [u8]) -> Result<(), CryptoError>,
{
    // 每个数字恰占一个 UTF-8 字节；预留一次后，循环内不再为结果扩容。
    let mut output = String::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| CryptoError::OutputTooLarge {
            operation: "secure_random_digits",
        })?;
    let mut bytes = [0; DIGIT_BATCH_SIZE];
    // 零长度不进入循环；每次最多索取剩余位数，拒绝后再补充随机字节。
    while output.len() < length {
        let count = (length - output.len()).min(bytes.len());
        fill(&mut bytes[..count])?;
        for &byte in &bytes[..count] {
            if byte < 250 {
                output.push(char::from(b'0' + byte % 10));
            }
        }
    }
    Ok(output)
}

/// 预检可表示的 Hex 长度，再采样并调用已有编码器；任一步失败均不返回部分数据。
fn hex_with<F>(byte_length: usize, fill: F) -> Result<String, CryptoError>
where
    F: FnMut(&mut [u8]) -> Result<(), CryptoError>,
{
    // String 的容量还受 isize::MAX 限制，提前拒绝必定不可能容纳的请求。
    byte_length
        .checked_mul(2)
        .filter(|&length| length <= isize::MAX as usize)
        .ok_or(CryptoError::OutputTooLarge {
            operation: "secure_random_hex",
        })?;
    let bytes = bytes_with(byte_length, fill)?;
    hex::encode_lower(&bytes)
}

#[cfg(test)]
mod tests;
