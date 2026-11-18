#[cfg(feature = "redis-async")]
use super::backend::should_discard_multiplexed_transaction_connection;
use super::RedisClient;
use super::{
    backend::{should_discard_connection, should_discard_transaction_connection},
    decode::{check_hash_entries, check_optional_values, decode_collection},
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
fn cluster_multi_key_input_rejects_cross_slot_before_transport() {
    use super::input::collect_value_pairs;

    let config = RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).unwrap();
    assert_eq!(
        collect_keys(["{first}:a", "{second}:b"], &config),
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        collect_raw_pairs([("{first}:a", "value"), ("{second}:b", "value")], &config),
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        collect_value_pairs([("{first}:a", 1_u8), ("{second}:b", 2_u8)], &config),
        Err(RedisError::CrossSlot)
    );
}

#[cfg(feature = "redis-cluster")]
#[test]
fn cluster_multi_key_input_preserves_tags_values_and_validation_priority() {
    use super::input::collect_value_pairs;

    let config = RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).unwrap();
    let keys = [b"\x00{same}:a".as_slice(), b"\xff{same}:b".as_slice()];
    assert_eq!(
        collect_keys(keys, &config).unwrap(),
        keys.map(<[u8]>::to_vec)
    );
    assert!(collect_keys(std::iter::empty::<&str>(), &config)
        .unwrap()
        .is_empty());
    assert!(collect_keys(["one"], &config).is_ok());
    assert_eq!(
        collect_raw_pairs([("{same}:a", "{first}"), ("{same}:b", "{second}")], &config).unwrap(),
        ["{same}:a", "{first}", "{same}:b", "{second}"].map(|value| value.as_bytes().to_vec())
    );
    assert!(collect_value_pairs([("{same}:a", 1_u8), ("{same}:b", 2_u8)], &config).is_ok());
    assert_eq!(
        collect_keys(["{first}:a", "{second}:b", ""], &config),
        Err(RedisError::InvalidKey)
    );
    let one_item = config.with_max_batch_items(1).unwrap();
    assert_eq!(
        collect_keys(["{first}:a", "{second}:b"], &one_item),
        Err(RedisError::ValueTooLarge { limit: 1 })
    );
    let single = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
    assert!(collect_keys(["{first}:a", "{second}:b"], &single).is_ok());
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
fn hash_pair_collectors_keep_empty_and_exact_budget_contracts() {
    use super::input::{collect_hash_pairs, collect_hash_raw_pairs};

    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .unwrap()
        .with_max_batch_items(1)
        .unwrap()
        .with_max_batch_bytes(3)
        .unwrap();
    assert_eq!(
        collect_hash_raw_pairs("k", [("f", "v")], &config).unwrap(),
        [b"k".to_vec(), b"f".to_vec(), b"v".to_vec()]
    );
    assert_eq!(
        collect_hash_pairs("k", [("f", 1_u8)], &config).unwrap(),
        [b"k".to_vec(), b"f".to_vec(), vec![1]]
    );
    assert_eq!(
        collect_hash_raw_pairs("key", [("f", "v")], &config),
        Err(RedisError::ValueTooLarge { limit: 3 })
    );
    assert_eq!(
        collect_hash_pairs("k", [("f", 1_u8), ("g", 2_u8)], &config),
        Err(RedisError::ValueTooLarge { limit: 1 })
    );
    assert_eq!(
        collect_hash_pairs("long-key", std::iter::empty::<(&str, u8)>(), &config).unwrap(),
        [b"long-key".to_vec()]
    );
    assert_eq!(
        collect_hash_raw_pairs("long-key", std::iter::empty::<(&str, &str)>(), &config).unwrap(),
        [b"long-key".to_vec()]
    );
}

fn hash_reply_client(wire: &[u8]) -> RedisClient {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0").unwrap();
    let value = ::redis::parse_redis_value(wire).unwrap();
    RedisClient::test_response(config, Ok(value)).0
}

const HGETALL_RESP2: &[u8] = b"*4\r\n$1\r\n\xff\r\n$1\r\n\x01\r\n$1\r\na\r\n$1\r\n\x02\r\n";
const HGETALL_RESP3: &[u8] = b"%2\r\n$1\r\n\xff\r\n$1\r\n\x01\r\n$1\r\na\r\n$1\r\n\x02\r\n";

const HGETALL_ATTRIBUTED: &[u8] = b"|1\r\n+source\r\n+test\r\n%2\r\n|0\r\n$1\r\n\xff\r\n$1\r\n\x01\r\n$1\r\na\r\n|0\r\n$1\r\n\x02\r\n";
const INVALID_HGETALL: &[&[u8]] = &[
    b"*1\r\n*2\r\n$1\r\nf\r\n$1\r\nv\r\n",
    b"*2\r\n$1\r\nf\r\n*1\r\n:65\r\n",
    b"%1\r\n$1\r\nf\r\n_\r\n",
    b"%1\r\n+f\r\n$1\r\nv\r\n",
    b"*2\r\n$1\r\nf\r\n:1\r\n",
    b"~0\r\n",
    b"_\r\n",
];

#[test]
fn hgetall_rejects_non_hash_replies() {
    for wire in INVALID_HGETALL {
        assert_eq!(
            hash_reply_client(wire).hgetall_bytes("k"),
            Err(RedisError::Transport(RedisTransportErrorKind::Protocol)),
            "{wire:?}"
        );
    }
}

#[test]
fn hgetall_maps_resp2_and_resp3_in_wire_order() {
    for wire in [HGETALL_RESP2, HGETALL_RESP3, HGETALL_ATTRIBUTED] {
        let client = hash_reply_client(wire);
        assert_eq!(
            client.hgetall_bytes("k").unwrap(),
            [(vec![0xff], vec![1]), (vec![b'a'], vec![2])]
        );
    }
    for empty in [b"*0\r\n", b"%0\r\n"] {
        assert!(hash_reply_client(empty)
            .hgetall_bytes("k")
            .unwrap()
            .is_empty());
    }
    assert_eq!(
        hash_reply_client(b"*1\r\n$1\r\nf\r\n").hgetall_bytes("k"),
        Err(RedisError::Transport(RedisTransportErrorKind::Protocol))
    );
}

#[cfg(feature = "redis-async")]
#[tokio::test]
async fn async_hgetall_decodes_resp2_and_resp3_in_wire_order() {
    for wire in [HGETALL_RESP2, HGETALL_RESP3, HGETALL_ATTRIBUTED] {
        assert_eq!(
            hash_reply_client(wire)
                .hgetall_async::<_, u8>("k")
                .await
                .unwrap(),
            [(vec![0xff], 1), (vec![b'a'], 2)]
        );
    }
    for wire in INVALID_HGETALL {
        assert_eq!(
            hash_reply_client(wire).hgetall_bytes_async("k").await,
            Err(RedisError::Transport(RedisTransportErrorKind::Protocol)),
            "{wire:?}"
        );
    }
}

#[test]
fn hash_response_limits_preserve_error_priority() {
    let config = RedisConfig::single("redis://127.0.0.1:6379/0")
        .unwrap()
        .with_max_key_bytes(4)
        .unwrap()
        .with_max_value_bytes(2)
        .unwrap()
        .with_max_response_bytes(5)
        .unwrap()
        .with_max_collection_items(1)
        .unwrap();
    let check = |field: &[u8], value: &[u8]| {
        check_hash_entries(vec![(field.to_vec(), value.to_vec())], &config)
    };
    assert!(check(b"key", b"12").is_ok());
    assert_eq!(
        check_hash_entries(vec![(vec![], vec![]); 2], &config),
        Err(RedisError::CollectionTooLarge { limit: 1 })
    );
    assert_eq!(check(b"", b"123"), Err(RedisError::InvalidField));
    assert_eq!(check(b"field", b"1"), Err(RedisError::InvalidField));
    assert_eq!(
        check(b"abcd", b"123"),
        Err(RedisError::ValueTooLarge { limit: 2 })
    );
    assert_eq!(
        check(b"abcd", b"12"),
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
        task::{Context, Poll, Waker},
    };

    let client = RedisClient::new(
        RedisConfig::single("redis://127.0.0.1:6379/0").expect("local URL should parse"),
    )
    .expect("client construction should not connect");
    let mut future = Box::pin(client.get_async::<_, u8>("runtime:key"));
    let mut context = Context::from_waker(Waker::noop());

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
        task::{Context, Poll, Waker},
    };

    let client = RedisClient::new(
        RedisConfig::cluster(["redis://127.0.0.1:7000/0"]).expect("cluster URL should parse"),
    )
    .expect("client construction should not connect");
    let mut future =
        Box::pin(client.transaction_async(|_| panic!("cluster transaction callback must not run")));
    let mut context = Context::from_waker(Waker::noop());

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
