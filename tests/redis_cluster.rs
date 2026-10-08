#![cfg(feature = "redis-cluster")]

use axutils::redis::{RedisClient, RedisConfig, RedisError};

const CLUSTER_LIVE_ENV: &str = "AXUTILS_REDIS_CLUSTER_LIVE_TEST";

fn cluster_live_authorized(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

fn require_cluster_live_authorization() {
    assert!(
        cluster_live_authorized(std::env::var_os(CLUSTER_LIVE_ENV).as_deref()),
        "set {CLUSTER_LIVE_ENV}=1 before running Redis Cluster live tests"
    );
}

fn cluster_client() -> RedisClient {
    RedisClient::new(
        RedisConfig::cluster([
            "redis://127.0.0.1:7000/0",
            "redis://127.0.0.1:7001/0",
            "redis://127.0.0.1:7002/0",
        ])
        .expect("cluster configuration"),
    )
    .expect("cluster client construction")
}

#[test]
fn cluster_transaction_is_rejected_without_network_access() {
    let client = RedisClient::new(
        RedisConfig::cluster(["redis://127.0.0.1:7000/0", "redis://127.0.0.1:7001/0"]).unwrap(),
    )
    .expect("cluster construction should be lazy");
    assert_eq!(
        client.transaction(|_| panic!("cluster callback must not run")),
        Err(RedisError::UnsupportedMode)
    );
}

#[test]
fn cross_slot_command_families_fail_before_connecting() {
    let (listener, client) = validation_client();
    let keys = ["{first}:a", "{second}:b"];
    assert_eq!(client.delete_many(keys), Err(RedisError::CrossSlot));
    assert_eq!(client.mget::<_, _, u8>(keys), Err(RedisError::CrossSlot));
    assert_eq!(client.mget_bytes(keys), Err(RedisError::CrossSlot));
    assert_eq!(
        client.mset([(keys[0], 1_u8), (keys[1], 2_u8)]),
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        client.mset_bytes([(keys[0], "one"), (keys[1], "two")]),
        Err(RedisError::CrossSlot)
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[cfg(feature = "redis-cluster-async")]
#[tokio::test]
async fn async_cross_slot_command_families_fail_before_connecting() {
    let (listener, client) = validation_client();
    let keys = ["{first}:a", "{second}:b"];
    assert_eq!(
        client.delete_many_async(keys).await,
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        client.mget_async::<_, _, u8>(keys).await,
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        client.mget_bytes_async(keys).await,
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        client.mset_async([(keys[0], 1_u8), (keys[1], 2_u8)]).await,
        Err(RedisError::CrossSlot)
    );
    assert_eq!(
        client
            .mset_bytes_async([(keys[0], "one"), (keys[1], "two")])
            .await,
        Err(RedisError::CrossSlot)
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

fn validation_client() -> (std::net::TcpListener, RedisClient) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let config = RedisConfig::cluster([format!("redis://{}/0", listener.local_addr().unwrap())])
        .unwrap()
        .with_pool_size(1)
        .unwrap()
        .with_connection_timeout(std::time::Duration::from_millis(50))
        .unwrap()
        .with_pool_checkout_timeout(std::time::Duration::from_millis(100))
        .unwrap()
        .with_response_timeout(std::time::Duration::from_millis(50))
        .unwrap();
    (listener, RedisClient::new(config).unwrap())
}

#[test]
#[ignore = "requires local Redis Cluster on 127.0.0.1:7000-7002 and explicit AXUTILS_REDIS_CLUSTER_LIVE_TEST=1"]
fn cluster_live_fixture_covers_routing_and_cross_slot_boundaries() {
    require_cluster_live_authorization();
    let client = cluster_client();
    let namespace = format!("axutils:cluster:{}", std::process::id());
    let shared_a = format!("{namespace}:{{same}}:a");
    let shared_b = format!("{namespace}:{{same}}:b");
    let shared_hash = format!("{namespace}:{{same}}:hash");
    let cross_a = format!("{namespace}:{{one}}:a");
    let cross_b = format!("{namespace}:{{two}}:b");

    client.set(&shared_a, 1_u8).expect("cluster set");
    assert_eq!(
        client.get::<_, u8>(&shared_a).expect("cluster get"),
        Some(1)
    );
    client
        .hset(&shared_hash, "field", "value")
        .expect("cluster hset");
    assert_eq!(
        client
            .hget::<_, _, String>(&shared_hash, "field")
            .expect("cluster hget"),
        Some("value".to_owned())
    );
    client
        .set_with_expiry(&shared_a, 2_u8, std::time::Duration::from_secs(10))
        .expect("cluster set with expiry");
    assert!(client.pttl(&shared_a).expect("cluster pttl") > 0);

    let lock_key = format!("{namespace}:{{lock}}:lease");
    let mut lock = client
        .try_lock(&lock_key, std::time::Duration::from_secs(10))
        .expect("cluster lock acquisition")
        .expect("cluster lock should be available");
    assert!(client
        .try_lock(&lock_key, std::time::Duration::from_secs(10))
        .expect("cluster busy lock attempt")
        .is_none());
    assert!(lock.release().expect("cluster lock release"));

    client
        .mset([(shared_a.clone(), 3_u8), (shared_b.clone(), 4_u8)])
        .expect("same-slot mset");
    assert_eq!(
        client
            .mget::<_, _, u8>([shared_a.clone(), shared_b.clone()])
            .expect("same-slot mget"),
        vec![Some(3), Some(4)]
    );
    assert_eq!(
        client.mget::<_, _, u8>([cross_a.clone(), cross_b.clone()]),
        Err(RedisError::CrossSlot)
    );

    for key in [shared_a, shared_b, shared_hash, cross_a, cross_b, lock_key] {
        let _ = client.delete(key).expect("cluster cleanup");
    }
}

#[cfg(feature = "redis-cluster-async")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires local Redis Cluster on 127.0.0.1:7000-7002 and explicit AXUTILS_REDIS_CLUSTER_LIVE_TEST=1"]
async fn cluster_async_fixture_covers_routing() {
    require_cluster_live_authorization();
    let client = cluster_client();
    let key = format!("axutils:cluster:{{async}}:{}", std::process::id());
    client
        .set_async(&key, 1_u8)
        .await
        .expect("cluster async set");
    assert_eq!(
        client
            .get_async::<_, u8>(&key)
            .await
            .expect("cluster async get"),
        Some(1)
    );
    let lock_key = format!("{key}:lock");
    let mut lock = client
        .try_lock_async(&lock_key, std::time::Duration::from_secs(10))
        .await
        .expect("cluster async lock acquisition")
        .expect("cluster async lock should be available");
    assert!(lock
        .renew(std::time::Duration::from_secs(10))
        .await
        .expect("cluster async lock renew"));
    assert!(lock.release().await.expect("cluster async lock release"));
    let _ = client
        .delete_async(&key)
        .await
        .expect("cluster async cleanup");
    let _ = client
        .delete_async(&lock_key)
        .await
        .expect("cluster async lock cleanup");
}

#[test]
fn cluster_live_authorization_requires_exact_opt_in() {
    assert!(!cluster_live_authorized(None));
    assert!(!cluster_live_authorized(Some(std::ffi::OsStr::new(""))));
    assert!(!cluster_live_authorized(Some(std::ffi::OsStr::new("0"))));
    assert!(!cluster_live_authorized(Some(std::ffi::OsStr::new("true"))));
    assert!(cluster_live_authorized(Some(std::ffi::OsStr::new("1"))));
}
