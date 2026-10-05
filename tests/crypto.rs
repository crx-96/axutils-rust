#[cfg(all(feature = "base64", feature = "md5", feature = "aes"))]
#[path = "crypto/codecs.rs"]
mod codecs;

#[cfg(feature = "secure-random")]
#[path = "crypto/random.rs"]
mod random;
