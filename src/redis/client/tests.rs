#[cfg(feature = "redis-async")]
use super::backend::should_discard_multiplexed_transaction_connection;
use super::RedisClient;
use super::{
    backend::{should_discard_connection, should_discard_transaction_connection},
    decode::{check_optional_values, decode_collection, decode_hash_entries},
    input::{collect_keys, collect_raw_pairs},
};
use crate::redis::{RedisConfig, RedisError, RedisTransportErrorKind};
use ::redis::{ErrorKind, RedisError as UpstreamRedisError, ServerErrorKind};

#[test]
fn construction_is_local_and_supports_clone() {
    let client = RedisClient::new(RedisConfig::single("redis://127.0.0.1:6379/0").unwrap())
        .expect("client construction should not connect");
    let clone = client.clone();
    assert!(format!("{clone:?}").contains("RedisClient"));
}

#[cfg(feature = "redis-cluster")]
#[test]
fn cluster_transaction_is_rejected_before_callback() {
    let client = RedisClient::new(RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).unwrap())
        .expect("client construction should not connect");
    let result = client.transaction(|_| panic!("callback must not run"));
    assert_eq!(result, Err(RedisError::UnsupportedMode));
}

#[test]
fn local_batch_and_response_limits_are_checked_before_network() {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .unwrap()
        .with_max_batch_items(1)
        .unwrap()
        .with_max_batch_bytes(2)
        .unwrap()
        .with_max_response_bytes(3)
        .unwrap()
        .with_max_collection_items(1)
        .unwrap();
    assert_eq!(
        collect_keys(["a", "b"], &config),
        Err(RedisError::ValueTooLarge { limit: 1 })
    );
    assert_eq!(
        collect_raw_pairs([("a", [1_u8, 2, 3])], &config),
        Err(RedisError::ValueTooLarge { limit: 2 })
    );
    assert_eq!(
        decode_collection::<u8>(vec![vec![1], vec![2]], &config),
        Err(RedisError::CollectionTooLarge { limit: 1 })
    );
    assert_eq!(
        check_optional_values(vec![Some(vec![1, 2]), Some(vec![3, 4])], &config),
        Err(RedisError::ResponseTooLarge { limit: 3 })
    );
}

#[test]
fn hash_response_shape_and_limits_are_checked_locally() {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .unwrap()
        .with_max_key_bytes(8)
        .unwrap()
        .with_max_response_bytes(5)
        .unwrap()
        .with_max_collection_items(1)
        .unwrap();

    assert_eq!(
        decode_hash_entries(vec![b"f".to_vec(), b"v".to_vec()], &config),
        Ok(vec![(b"f".to_vec(), b"v".to_vec())])
    );
    assert_eq!(
        decode_hash_entries(vec![b"f".to_vec()], &config),
        Err(RedisError::Transport(RedisTransportErrorKind::Protocol))
    );
    assert_eq!(
        decode_hash_entries(vec![Vec::new(), b"v".to_vec()], &config),
        Err(RedisError::InvalidField)
    );
    assert_eq!(
        decode_hash_entries(
            vec![b"f".to_vec(), b"v".to_vec(), b"g".to_vec(), b"w".to_vec()],
            &config,
        ),
        Err(RedisError::CollectionTooLarge { limit: 1 })
    );
    assert_eq!(
        decode_hash_entries(vec![b"field".to_vec(), b"v".to_vec()], &config),
        Err(RedisError::ResponseTooLarge { limit: 5 })
    );
}

#[test]
fn uncertain_transport_errors_discard_open_connections() {
    assert!(should_discard_connection(
        &RedisError::Transport(RedisTransportErrorKind::Protocol),
        true
    ));
    assert!(should_discard_connection(
        &RedisError::Transport(RedisTransportErrorKind::Timeout),
        true
    ));
    assert!(!should_discard_connection(
        &RedisError::Transport(RedisTransportErrorKind::Server),
        true
    ));
    assert!(should_discard_connection(
        &RedisError::Transport(RedisTransportErrorKind::Server),
        false
    ));
}

#[test]
fn transaction_discards_only_connections_with_unreliable_state() {
    let server_error = UpstreamRedisError::from((
        ErrorKind::Server(ServerErrorKind::ResponseError),
        "server error",
        "WRONGTYPE operation against a key holding the wrong kind of value".to_owned(),
    ));
    assert!(!should_discard_transaction_connection(&server_error, true));
    assert!(should_discard_transaction_connection(&server_error, false));

    let protocol_error = UpstreamRedisError::from((ErrorKind::Parse, "invalid Redis response"));
    assert!(should_discard_transaction_connection(&protocol_error, true));
}

#[cfg(feature = "redis-async")]
#[test]
fn multiplexed_transaction_keeps_complete_server_errors_but_discards_transport_errors() {
    let server_error = UpstreamRedisError::from((
        ErrorKind::Server(ServerErrorKind::ResponseError),
        "server error",
        "WRONGTYPE operation against a key holding the wrong kind of value".to_owned(),
    ));
    let protocol_error = UpstreamRedisError::from((ErrorKind::Parse, "invalid Redis response"));

    // MultiplexedConnection has no active liveness probe; a complete server response is
    // observable and can be retained, while protocol/transport errors make state uncertain.
    assert!(!should_discard_multiplexed_transaction_connection(
        &server_error
    ));
    assert!(should_discard_multiplexed_transaction_connection(
        &protocol_error
    ));
}

#[cfg(feature = "redis-async")]
#[test]
fn async_commands_require_a_runtime_before_network_access() {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };

    struct NoopWaker;

    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    let client = RedisClient::new(
        RedisConfig::single("redis://127.0.0.1:6379/0").expect("local URL should parse"),
    )
    .expect("client construction should not connect");
    let mut future = Box::pin(client.get_async::<_, u8>("runtime:key"));
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);

    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err(RedisError::RuntimeRequired))
    ));
}

#[cfg(feature = "redis-cluster-async")]
#[test]
fn async_cluster_transaction_is_rejected_before_runtime_check() {
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };

    struct NoopWaker;

    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }

    let client = RedisClient::new(
        RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).expect("cluster URL should parse"),
    )
    .expect("client construction should not connect");
    let mut future =
        Box::pin(client.transaction_async(|_| panic!("cluster transaction callback must not run")));
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);

    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Err(RedisError::UnsupportedMode))
    ));
}

#[cfg(all(
    feature = "redis-cluster",
    feature = "redis-async",
    not(feature = "redis-cluster-async")
))]
#[tokio::test(flavor = "current_thread")]
async fn async_cluster_without_async_cluster_feature_is_rejected_without_network_access() {
    let client = RedisClient::new(
        RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).expect("cluster URL should parse"),
    )
    .expect("sync Cluster construction should remain local");

    assert_eq!(client.ping_async().await, Err(RedisError::UnsupportedMode));
}
