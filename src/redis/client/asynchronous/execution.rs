use ::redis::{
    aio::{ConnectionManager, ConnectionManagerConfig},
    AsyncConnectionConfig,
};
#[cfg(feature = "redis-cluster-async")]
use ::redis::{cluster::ClusterConfig, cluster_async::ClusterConnection};
use tokio::runtime::Handle as RuntimeHandle;

use super::super::super::{
    config::{ASYNC_RECONNECT_MAX_DELAY, ASYNC_RECONNECT_RETRIES},
    error::RedisError,
    lock,
    transaction::RedisTransaction,
};
use super::super::backend::{self, AsyncBackend, RedisClient};

#[cfg(feature = "tracing")]
use crate::telemetry::redis as redis_trace;

impl RedisClient {
    #[cfg(feature = "redis-async")]
    pub(super) async fn execute_async<T: ::redis::FromRedisValue>(
        &self,
        command: &::redis::Cmd,
    ) -> Result<T, RedisError> {
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        #[cfg(feature = "tracing")]
        let backend = if self.inner.config.is_cluster() {
            "cluster"
        } else {
            "single"
        };
        let result = if RuntimeHandle::try_current().is_err() {
            Err(RedisError::RuntimeRequired)
        } else {
            match &self.inner.async_backend {
                AsyncBackend::Single { .. } => match self.async_single_connection().await {
                    Ok(mut connection) => command
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| RedisError::from_upstream(&error)),
                    Err(error) => Err(error),
                },
                #[cfg(feature = "redis-cluster-async")]
                AsyncBackend::Cluster { .. } => match self.async_cluster_connection().await {
                    Ok(mut connection) => command
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| RedisError::from_upstream(&error)),
                    Err(error) => Err(error),
                },
                #[cfg(all(feature = "redis-cluster", not(feature = "redis-cluster-async")))]
                AsyncBackend::UnsupportedCluster => Err(RedisError::UnsupportedMode),
                #[cfg(test)]
                AsyncBackend::Fake(backend) => backend.execute(),
            }
        };
        #[cfg(feature = "tracing")]
        redis_trace::record_command("async", backend, &result, false, started);
        result
    }

    #[cfg(feature = "redis-async")]
    pub(crate) async fn release_lock_async(
        &self,
        key: &[u8],
        token: &[u8],
    ) -> Result<i64, RedisError> {
        let command = lock::release_command(key, token);
        self.execute_async(&command).await
    }

    #[cfg(feature = "redis-async")]
    pub(crate) async fn renew_lock_async(
        &self,
        key: &[u8],
        token: &[u8],
        ttl_millis: i64,
    ) -> Result<i64, RedisError> {
        let command = lock::renew_command(key, token, ttl_millis);
        self.execute_async(&command).await
    }

    #[cfg(feature = "redis-async")]
    async fn async_single_connection(&self) -> Result<ConnectionManager, RedisError> {
        #[cfg(all(not(test), not(feature = "redis-cluster")))]
        let AsyncBackend::Single {
            client, manager, ..
        } = &self.inner.async_backend;
        #[cfg(any(test, feature = "redis-cluster"))]
        let AsyncBackend::Single {
            client, manager, ..
        } = &self.inner.async_backend
        else {
            return Err(RedisError::UnsupportedMode);
        };
        let mut guard = manager.lock().await;
        if let Some(connection) = guard.as_ref() {
            return Ok(connection.clone());
        }
        let config = ConnectionManagerConfig::new()
            .set_number_of_retries(ASYNC_RECONNECT_RETRIES)
            .set_max_delay(ASYNC_RECONNECT_MAX_DELAY)
            .set_connection_timeout(Some(self.inner.config.connection_timeout))
            .set_response_timeout(Some(self.inner.config.response_timeout));
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let connection_result = client
            .get_connection_manager_lazy(config)
            .map_err(|error| RedisError::from_upstream(&error));
        #[cfg(feature = "tracing")]
        match &connection_result {
            Ok(_) => redis_trace::record_connection(
                "connection_manager_init",
                "single",
                "ready",
                None,
                started,
            ),
            Err(error) => redis_trace::record_connection(
                "connection_manager_init",
                "single",
                "error",
                Some(error),
                started,
            ),
        }
        let connection = connection_result;
        let connection = connection?;
        *guard = Some(connection.clone());
        Ok(connection)
    }

    #[cfg(feature = "redis-cluster-async")]
    async fn async_cluster_connection(&self) -> Result<ClusterConnection, RedisError> {
        let AsyncBackend::Cluster { client, connection } = &self.inner.async_backend else {
            return Err(RedisError::UnsupportedMode);
        };
        // The first cluster connection is established while holding this slot lock. Concurrent
        // first commands therefore serialize behind the bounded connection timeout; cancellation
        // releases the lock, and later commands only clone an established connection.
        let mut guard = connection.lock().await;
        if let Some(connection) = guard.as_ref() {
            return Ok(connection.clone());
        }
        let config = ClusterConfig::new()
            .set_connection_timeout(self.inner.config.connection_timeout)
            .set_response_timeout(self.inner.config.response_timeout);
        #[cfg(feature = "tracing")]
        let started = std::time::Instant::now();
        let connection_result = client
            .get_async_connection_with_config(config)
            .await
            .map_err(|error| RedisError::from_upstream(&error));
        #[cfg(feature = "tracing")]
        match &connection_result {
            Ok(_) => {
                redis_trace::record_connection("connection", "cluster", "success", None, started)
            }
            Err(error) => redis_trace::record_connection(
                "connection",
                "cluster",
                "error",
                Some(error),
                started,
            ),
        }
        let connection = connection_result;
        let connection = connection?;
        *guard = Some(connection.clone());
        Ok(connection)
    }
    #[cfg(feature = "redis-async")]
    /// 异步执行单机 MULTI/EXEC 事务。
    ///
    /// 事务 callback 是一次性的同步排队闭包，不接受 async callback，也不会被重放。专用
    /// multiplexed connection 与普通命令分离；future 取消或连接状态不再可靠时该连接会被丢弃。
    /// 已完整读取响应的普通 Redis 服务端命令错误不会淘汰健康连接。
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axutils::redis::{RedisClient, RedisError, RedisTransaction};
    ///
    /// let _ = RedisClient::transaction_async::<fn(&mut RedisTransaction) -> Result<(), RedisError>>;
    /// ```
    pub async fn transaction_async<F>(&self, callback: F) -> Result<(), RedisError>
    where
        F: FnOnce(&mut RedisTransaction) -> Result<(), RedisError>,
    {
        if self.inner.config.is_cluster() {
            return Err(RedisError::UnsupportedMode);
        }
        if RuntimeHandle::try_current().is_err() {
            return Err(RedisError::RuntimeRequired);
        }
        let mut transaction = RedisTransaction::new(&self.inner.config);
        callback(&mut transaction)?;
        if transaction.is_empty() {
            return Ok(());
        }

        #[cfg(all(not(test), not(feature = "redis-cluster")))]
        let AsyncBackend::Single {
            client,
            transaction: slot,
            transaction_lock,
            ..
        } = &self.inner.async_backend;
        #[cfg(any(test, feature = "redis-cluster"))]
        let AsyncBackend::Single {
            client,
            transaction: slot,
            transaction_lock,
            ..
        } = &self.inner.async_backend
        else {
            return Err(RedisError::UnsupportedMode);
        };
        let _serial = transaction_lock.lock().await;
        let mut connection = {
            let mut guard = slot.lock().await;
            guard.take()
        };
        if connection.is_none() {
            let config = AsyncConnectionConfig::new()
                .set_connection_timeout(Some(self.inner.config.connection_timeout))
                .set_response_timeout(Some(self.inner.config.response_timeout));
            connection = Some(
                client
                    .get_multiplexed_async_connection_with_config(&config)
                    .await
                    .map_err(|error| RedisError::from_upstream(&error))?,
            );
        }
        let Some(mut connection) = connection else {
            return Err(RedisError::TransactionFailed);
        };
        let mut pipeline = ::redis::pipe();
        pipeline.atomic();
        for command in transaction.commands() {
            pipeline.add_command(command.clone());
        }
        match pipeline.exec_async(&mut connection).await {
            Ok(()) => {
                *slot.lock().await = Some(connection);
                Ok(())
            }
            Err(error) => {
                // MultiplexedConnection has no active `is_open` probe. A complete server error
                // is retained; connection/protocol/network/timeout errors are discarded. An
                // unknown dead connection can consequently be tried once more and will be
                // discarded when the next transaction reports its transport error.
                if !backend::should_discard_multiplexed_transaction_connection(&error) {
                    *slot.lock().await = Some(connection);
                }
                Err(RedisError::transaction_failure(&error))
            }
        }
    }
}
