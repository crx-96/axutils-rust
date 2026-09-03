#![cfg(feature = "redis")]

use axutils::{RedisConfig, RedisUtils};
use std::time::Duration;

#[path = "support/redis_server.rs"]
mod redis_server;
use redis_server::{test_config, RedisTestServer};

#[test]
fn cluster_init_checks_connectivity_before_installing_the_global_client() {
    let unavailable = RedisTestServer::start(|_| None);
    let url = format!("redis://{}/0", unavailable.address);
    assert!(RedisUtils::init(test_config(&url)).is_err());
    assert!(!RedisUtils::is_initialized());

    let server = RedisTestServer::start(|command| {
        Some(if command[0] == "PING" {
            b"+PONG\r\n"
        } else {
            b"+OK\r\n"
        })
    });
    let config = RedisConfig::cluster([format!("redis://{}/0", server.address)])
        .unwrap()
        .with_pool_size(1)
        .unwrap()
        .with_connection_timeout(Duration::from_secs(1))
        .unwrap()
        .with_pool_checkout_timeout(Duration::from_secs(2))
        .unwrap()
        .with_response_timeout(Duration::from_secs(1))
        .unwrap();
    RedisUtils::init(config).expect("Cluster 探测成功后初始化");
    assert!(RedisUtils::is_initialized());
    let commands = server.commands.lock().unwrap();
    assert!(commands.iter().any(|name| name == "CLUSTER"));
    assert!(commands.iter().any(|name| name == "PING"));
    drop(commands);
    assert_eq!(RedisUtils::ping().unwrap(), "PONG");
}
