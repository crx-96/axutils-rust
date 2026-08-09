#![cfg(feature = "redis")]

use axutils::{RedisClient, RedisConfig, RedisError};

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
#[ignore = "requires local Redis Cluster on 127.0.0.1:7000-7002"]
fn cluster_live_fixture_covers_routing_and_cross_slot_boundaries() {
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

    for key in [shared_a, shared_b, shared_hash, cross_a, cross_b] {
        let _ = client.delete(key).expect("cluster cleanup");
    }
}

#[cfg(all(feature = "redis", feature = "tokio"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires local Redis Cluster on 127.0.0.1:7000-7002"]
async fn cluster_async_fixture_covers_routing() {
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
    let _ = client
        .delete_async(&key)
        .await
        .expect("cluster async cleanup");
}
