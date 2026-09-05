//! 操作系统随机源适配与临时随机缓冲区清零。

use ::aes_gcm::aead::Generate;
use ::zeroize::Zeroize;

use crate::crypto::CryptoError;

/// 使用操作系统随机源生成 `len`（12、16、24 或 32）字节；失败映射为
/// [`CryptoError::RandomSource`]。
///
/// 注：上游 `Generate::try_generate` 在随机源失败时不会把部分写入的缓冲区暴露给调用方（失败
/// 分支不返回该数组），因此这里不需要（也无法）在本 crate 内对一个不存在的缓冲区做 zeroize。
pub(super) fn random_bytes(len: usize) -> Result<Vec<u8>, CryptoError> {
    match len {
        12 => <[u8; 12]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        16 => <[u8; 16]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        24 => <[u8; 24]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        32 => <[u8; 32]>::try_generate()
            .map(array_to_vec_and_zeroize)
            .map_err(|_| CryptoError::RandomSource),
        _ => unreachable!("only called with AesMode::iv_length() or AesKeyBits::byte_length()"),
    }
}

fn array_to_vec_and_zeroize<const N: usize>(array: [u8; N]) -> Vec<u8> {
    let mut array = array;
    let result = array.to_vec();
    array.zeroize();
    result
}
