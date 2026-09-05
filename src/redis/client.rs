//! Redis 客户端外观与私有实现分层。

#[cfg(feature = "redis-async")]
mod asynchronous;
mod backend;
mod decode;
mod input;
mod sync;

#[cfg(test)]
mod tests;

pub use backend::RedisClient;
