use ::redis::ConnectionLike;

use super::super::super::{error::RedisError, lock, transaction::RedisTransaction};
use super::super::backend::{self, RedisClient, SyncBackend};

#[cfg(feature = "tracing")]
use crate::telemetry::redis as redis_trace;

impl RedisClient {
    /// 同步执行一个原子 MULTI/EXEC 事务。
    ///
    /// callback 只允许同步排队写入命令；它返回错误时不会 checkout 连接或发送任何命令。
    /// 空事务直接返回成功。Cluster 模式返回 [`RedisError::UnsupportedMode`]。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::{RedisClient, RedisError, RedisTransaction};
    ///
    /// let _ = RedisClient::transaction::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub fn transaction<F>(&self, callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        if self.inner.config.is_cluster() {
            return Err(RedisError::UnsupportedMode);
        }
        let mut transaction = RedisTransaction::new(&self.inner.config);
        callback(&mut transaction)?;
        if transaction.is_empty() {
            return Ok(());
        }

        let mut connection = match &self.inner.sync {
            SyncBackend::Single(pool) => pool.get().map_err(|error| backend::pool_error(&error))?,
            #[cfg(feature = "redis-cluster")]
            SyncBackend::Cluster(_) => return Err(RedisError::UnsupportedMode),
            #[cfg(test)]
            SyncBackend::Fake(_) => return Err(RedisError::UnsupportedMode),
        };
        let mut pipeline = ::redis::pipe();
        pipeline.atomic();
        for command in transaction.commands() {
            pipeline.add_command(command.clone());
        }
        match pipeline.exec(&mut *connection) {
            Ok(()) => Ok(()),
            Err(error) => {
                if backend::should_discard_transaction_connection(&error, connection.is_open()) {
                    connection.mark_broken();
                }
                Err(RedisError::transaction_failure(&error))
            }
        }
    }

    pub(super) fn execute_sync<T: ::redis::FromRedisValue>(
        &self,
        command: &::redis::Cmd,
    ) -> Result<T, RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let mut connection_discarded = false;
        #[cfg(feature = "tracing")]
        let backend = if self.inner.config.is_cluster() {
            "cluster"
        } else {
            "single"
        };
        let result = match &self.inner.sync {
            SyncBackend::Single(pool) => match pool.get() {
                Ok(mut connection) => match command.query(&mut *connection) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mapped = RedisError::from_upstream(&error);
                        if backend::should_discard_connection(&mapped, connection.is_open()) {
                            connection.mark_broken();
                            connection_discarded = true;
                        }
                        Err(mapped)
                    }
                },
                Err(error) => Err(backend::pool_error(&error)),
            },
            #[cfg(feature = "redis-cluster")]
            SyncBackend::Cluster(pool) => match pool.get() {
                Ok(mut connection) => match command.query(&mut *connection) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mapped = RedisError::from_upstream(&error);
                        if backend::should_discard_connection(&mapped, connection.is_open()) {
                            connection.mark_broken();
                            connection_discarded = true;
                        }
                        Err(mapped)
                    }
                },
                Err(error) => Err(backend::pool_error(&error)),
            },
            #[cfg(test)]
            SyncBackend::Fake(backend) => backend.execute(),
        };
        #[cfg(not(feature = "tracing"))]
        let _ = connection_discarded;
        #[cfg(feature = "tracing")]
        redis_trace::record_command("sync", backend, &result, connection_discarded, started);
        result
    }

    pub(crate) fn release_lock_sync(&self, key: &[u8], token: &[u8]) -> Result<i64, RedisError> {
        let command = lock::release_command(key, token);
        self.execute_sync(&command)
    }

    pub(crate) fn renew_lock_sync(
        &self,
        key: &[u8],
        token: &[u8],
        ttl_millis: i64,
    ) -> Result<i64, RedisError> {
        let command = lock::renew_command(key, token, ttl_millis);
        self.execute_sync(&command)
    }
}
