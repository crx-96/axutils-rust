use super::RedisConfig;
use std::fmt;
impl fmt::Debug for RedisConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RedisConfig")
            .field(
                "mode",
                if self.is_cluster() {
                    &"cluster"
                } else {
                    &"single"
                },
            )
            .field(
                "endpoint_count",
                &self.cluster_nodes().map_or(1, |nodes| nodes.len()),
            )
            .field("endpoints", &"[REDACTED]")
            .field("pool_size", &self.pool_size)
            .field("connection_timeout", &self.connection_timeout)
            .field("pool_checkout_timeout", &self.pool_checkout_timeout)
            .field("response_timeout", &self.response_timeout)
            .field("max_key_bytes", &self.max_key_bytes)
            .field("max_value_bytes", &self.max_value_bytes)
            .field("max_batch_items", &self.max_batch_items)
            .field("max_batch_bytes", &self.max_batch_bytes)
            .field("max_response_bytes", &self.max_response_bytes)
            .field("max_collection_items", &self.max_collection_items)
            .field("max_transaction_commands", &self.max_transaction_commands)
            .field("max_transaction_bytes", &self.max_transaction_bytes)
            .finish()
    }
}
