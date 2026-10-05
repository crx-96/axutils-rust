use axutils::sqlx::{SqlxError, SqlxTransportErrorKind};

#[test]
fn infrastructure_unavailable_is_limited_to_pool_timeout_closed_and_network() {
    for error in [
        SqlxError::PoolAcquireTimeout,
        SqlxError::PoolClosed,
        SqlxError::Transport(SqlxTransportErrorKind::Timeout),
        SqlxError::Transport(SqlxTransportErrorKind::Network),
    ] {
        assert!(error.is_infrastructure_unavailable(), "{error:?}");
    }

    for error in [
        SqlxError::InvalidConfig { field: "url" },
        SqlxError::RuntimeRequired,
        SqlxError::NotInitialized,
        SqlxError::AlreadyInitialized,
        SqlxError::RowNotFound,
        SqlxError::RowLimitExceeded { limit: 1 },
        SqlxError::TransactionFailed,
        SqlxError::Transport(SqlxTransportErrorKind::Connection),
        SqlxError::Transport(SqlxTransportErrorKind::Protocol),
        SqlxError::Transport(SqlxTransportErrorKind::Server),
        SqlxError::Transport(SqlxTransportErrorKind::Decode),
        SqlxError::Transport(SqlxTransportErrorKind::Encode),
        SqlxError::Transport(SqlxTransportErrorKind::Tls),
        SqlxError::Transport(SqlxTransportErrorKind::Other),
    ] {
        assert!(!error.is_infrastructure_unavailable(), "{error:?}");
    }
}
