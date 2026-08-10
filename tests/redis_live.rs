#![cfg(feature = "redis")]

use std::{collections::HashMap, fs, path::PathBuf, time::Duration};

use axutils::{RedisClient, RedisConfig, RedisError, RedisUtils};

struct LiveConfig {
    redis_url: String,
    key_prefix: String,
}

const LIVE_CONFIG_PATH: &str = "config/redis-test.toml";

#[test]
#[ignore = "requires config/redis-test.toml and explicit AXUTILS_REDIS_LIVE_TEST=1"]
fn exercises_sync_redis_api_against_local_service() {
    let config = load_live_config();
    let client = RedisClient::new(
        RedisConfig::single(config.redis_url)
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let suffix = format!(
        "{}:{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let key = format!("{}:{}:value", config.key_prefix, suffix);
    let raw_key = format!("{}:{}:counter", config.key_prefix, suffix);
    let hash = format!("{}:{}:hash", config.key_prefix, suffix);
    let raw_hash = format!("{}:{}:raw-hash", config.key_prefix, suffix);
    let list = format!("{}:{}:list", config.key_prefix, suffix);
    let set = format!("{}:{}:set", config.key_prefix, suffix);
    let raw_batch_a = format!("{}:{}:raw-a", config.key_prefix, suffix);
    let raw_batch_b = format!("{}:{}:raw-b", config.key_prefix, suffix);
    let expiring = format!("{}:{}:expiring", config.key_prefix, suffix);
    let wrong_type = format!("{}:{}:wrong-type", config.key_prefix, suffix);
    let nx_key = format!("{}:{}:nx", config.key_prefix, suffix);
    let keys = [
        &key,
        &raw_key,
        &hash,
        &raw_hash,
        &list,
        &set,
        &raw_batch_a,
        &raw_batch_b,
        &expiring,
        &wrong_type,
        &nx_key,
    ];

    assert_eq!(client.get::<_, u32>(&key).expect("missing get"), None);
    client.set(&key, 7_u32).expect("set");
    assert_eq!(client.get::<_, u32>(&key).expect("get"), Some(7));
    client.set_bytes(&raw_key, b"1").expect("raw set");
    assert_eq!(
        client.get_bytes(&raw_key).expect("raw get"),
        Some(b"1".to_vec())
    );
    assert_eq!(client.incr(&raw_key).expect("incr"), 2);
    client
        .mset([(format!("{key}:a"), 1_u8), (format!("{key}:b"), 2_u8)])
        .expect("mset");
    assert_eq!(
        client
            .mget::<_, _, u8>([format!("{key}:a"), format!("{key}:b")])
            .unwrap(),
        vec![Some(1), Some(2)]
    );
    client
        .mset_bytes([
            (raw_batch_a.clone(), [0_u8, 255]),
            (raw_batch_b.clone(), [1_u8, 2]),
        ])
        .expect("raw mset");
    assert_eq!(
        client
            .mget_bytes([raw_batch_a.clone(), raw_batch_b.clone()])
            .expect("raw mget"),
        vec![Some(vec![0, 255]), Some(vec![1, 2])]
    );
    client.hset(&hash, "field", "value").expect("hset");
    assert_eq!(
        client.hget::<_, _, String>(&hash, "field").unwrap(),
        Some("value".to_owned())
    );
    client
        .hset_bytes(&raw_hash, b"raw-field", [0_u8, 255])
        .expect("raw hset");
    assert_eq!(
        client
            .hget_bytes(&raw_hash, b"raw-field")
            .expect("raw hget"),
        Some(vec![0, 255])
    );
    client
        .hset_many_bytes(&raw_hash, [("many-a", [3_u8]), ("many-b", [4_u8])])
        .expect("raw hset many");
    assert_eq!(
        client.hgetall_bytes(&raw_hash).expect("raw hgetall").len(),
        3
    );
    client.lpush(&list, 1_u8).expect("lpush");
    client.rpush(&list, 2_u8).expect("rpush");
    assert_eq!(client.lrange::<_, u8>(&list, 0, -1).unwrap(), vec![1, 2]);
    client.sadd(&set, "member").expect("sadd");
    assert!(client.sismember(&set, "member").unwrap());
    assert!(client.set_bytes_nx(&nx_key, b"first").expect("set nx"));
    assert!(!client
        .set_bytes_nx(&nx_key, b"second")
        .expect("set nx second"));
    client.set_bytes(&expiring, b"ttl").expect("set expiring");
    assert!(client
        .expire(&expiring, Duration::from_secs(10))
        .expect("expire"));
    assert!(client.pttl(&expiring).expect("pttl after expire") > 0);
    assert!(client.persist(&expiring).expect("persist"));
    assert_eq!(client.pttl(&expiring).expect("pttl after persist"), -1);
    assert_eq!(client.ttl("missing:ttl").expect("missing ttl"), -2);
    client
        .set_with_expiry(&key, 8_u8, Duration::from_secs(10))
        .unwrap();
    assert!(client.pttl(&key).unwrap() > 0);
    client
        .transaction(|tx| {
            tx.set(&key, 9_u8)?;
            tx.persist(&key)
        })
        .expect("transaction");
    client
        .set_bytes(&wrong_type, b"not-a-hash")
        .expect("wrong-type setup");
    assert_eq!(
        client.transaction(|tx| tx.hset(&wrong_type, "field", 1_u8)),
        Err(RedisError::TransactionFailed)
    );

    let mut cleanup = keys
        .into_iter()
        .map(|key| key.to_owned())
        .collect::<Vec<_>>();
    cleanup.push(format!("{key}:a"));
    cleanup.push(format!("{key}:b"));
    client.delete_many(cleanup).expect("cleanup");
}

#[test]
#[ignore = "requires config/redis-test.toml and explicit AXUTILS_REDIS_LIVE_TEST=1"]
fn exercises_sync_single_key_lock_ownership_and_ttl() {
    let config = load_live_config();
    let client_a = RedisClient::new(
        RedisConfig::single(config.redis_url.clone())
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let client_b = RedisClient::new(
        RedisConfig::single(config.redis_url)
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let key = format!(
        "{}:lock:{}:{}",
        config.key_prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let results = std::thread::scope(|scope| {
        let barrier_a = std::sync::Arc::clone(&barrier);
        let barrier_b = std::sync::Arc::clone(&barrier);
        let key_a = key.clone();
        let key_b = key.clone();
        let thread_client_a = client_a.clone();
        let thread_client_b = client_b.clone();
        let first = scope.spawn(move || {
            barrier_a.wait();
            match thread_client_a.try_lock(key_a, Duration::from_secs(5))? {
                Some(mut lock) => lock.release(),
                None => Ok(false),
            }
        });
        let second = scope.spawn(move || {
            barrier_b.wait();
            match thread_client_b.try_lock(key_b, Duration::from_secs(5))? {
                Some(mut lock) => lock.release(),
                None => Ok(false),
            }
        });
        barrier.wait();
        [
            first.join().expect("first lock thread should not panic"),
            second.join().expect("second lock thread should not panic"),
        ]
    });
    let successful_releases = results
        .into_iter()
        .map(|result| result.expect("lock command should succeed"))
        .filter(|released| *released)
        .count();
    assert_eq!(successful_releases, 1);

    let mut old = client_b
        .try_lock(&key, Duration::from_millis(120))
        .expect("old lock acquisition")
        .expect("old lock should be available");
    assert!(format!("{old:?}").contains("RedisLockGuard"));
    assert!(!format!("{old:?}").contains(&key));
    assert!(client_a
        .try_lock(&key, Duration::from_secs(1))
        .unwrap()
        .is_none());
    std::thread::sleep(Duration::from_millis(180));

    let mut replacement = client_a
        .try_lock(&key, Duration::from_secs(1))
        .expect("replacement lock acquisition")
        .expect("replacement lock should be available after TTL");
    assert!(!old.release().expect("stale release"));
    assert!(replacement.renew(Duration::from_secs(1)).expect("renew"));
    assert!(replacement.release().expect("release"));

    let mut old_renew = client_b
        .try_lock(&key, Duration::from_millis(120))
        .expect("old renew lock acquisition")
        .expect("old renew lock should be available");
    std::thread::sleep(Duration::from_millis(180));
    let mut replacement = client_a
        .try_lock(&key, Duration::from_secs(1))
        .expect("second replacement lock acquisition")
        .expect("second replacement lock should be available after TTL");
    assert!(!old_renew
        .renew(Duration::from_secs(1))
        .expect("stale renew"));
    assert!(replacement.release().expect("second replacement release"));

    {
        let _drop_guard = client_a
            .try_lock(&key, Duration::from_secs(1))
            .expect("drop lock acquisition")
            .expect("drop lock should be available");
    }
    let mut after_drop = client_b
        .try_lock(&key, Duration::from_secs(1))
        .expect("post-drop lock acquisition")
        .expect("sync Drop should release when Redis is available");
    assert!(after_drop.release().expect("post-drop release"));
}

#[test]
#[ignore = "requires config/redis-test.toml and explicit AXUTILS_REDIS_LIVE_TEST=1"]
fn exercises_redis_utils_lock_forwarding_lifetime() {
    let config = load_live_config();
    RedisUtils::init(
        RedisConfig::single(config.redis_url)
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("RedisUtils live initialization failed"));
    let key = format!(
        "{}:utils-lock:{}:{}",
        config.key_prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );

    let mut lock = RedisUtils::try_lock(&key, Duration::from_secs(1))
        .expect("RedisUtils lock acquisition")
        .expect("RedisUtils lock should be available");
    assert!(lock
        .renew(Duration::from_secs(1))
        .expect("RedisUtils renew"));
    assert!(lock.release().expect("RedisUtils release"));
}

#[cfg(all(feature = "redis", feature = "tokio"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires config/redis-test.toml and explicit AXUTILS_REDIS_LIVE_TEST=1"]
async fn exercises_async_redis_api_against_local_service() {
    let config = load_live_config();
    let client = RedisClient::new(
        RedisConfig::single(config.redis_url)
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let key = format!("{}:{}:async", config.key_prefix, std::process::id());
    client.set_async(&key, 1_u8).await.expect("async set");
    assert_eq!(client.get_async::<_, u8>(&key).await.unwrap(), Some(1));
    client
        .transaction_async(|tx| tx.set(&key, 2_u8))
        .await
        .expect("async transaction");
    assert_eq!(client.get_async::<_, u8>(&key).await.unwrap(), Some(2));
    client.delete_async(&key).await.expect("async cleanup");
}

#[cfg(all(feature = "redis", feature = "tokio"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires config/redis-test.toml and explicit AXUTILS_REDIS_LIVE_TEST=1"]
async fn exercises_async_single_key_lock_and_ttl_fallback() {
    let config = load_live_config();
    let client = RedisClient::new(
        RedisConfig::single(config.redis_url.clone())
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let other = RedisClient::new(
        RedisConfig::single(config.redis_url)
            .unwrap_or_else(|_| panic!("Redis live configuration has invalid redis_url")),
    )
    .unwrap_or_else(|_| panic!("Redis live client construction failed"));
    let key = format!(
        "{}:async-lock:{}:{}",
        config.key_prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );

    let mut lock = client
        .try_lock_async(&key, Duration::from_secs(1))
        .await
        .expect("async lock acquisition")
        .expect("async lock should be available");
    assert!(other
        .try_lock_async(&key, Duration::from_secs(1))
        .await
        .expect("busy lock attempt")
        .is_none());
    assert!(lock
        .renew(Duration::from_secs(1))
        .await
        .expect("async renew"));
    assert!(lock.release().await.expect("async release"));
    assert!(!lock.release().await.expect("repeated async release"));

    let short_lock = client
        .try_lock_async(&key, Duration::from_millis(120))
        .await
        .expect("short async lock acquisition")
        .expect("short async lock should be available");
    drop(short_lock);
    assert!(other
        .try_lock_async(&key, Duration::from_secs(1))
        .await
        .expect("async Drop busy attempt")
        .is_none());
    tokio::time::sleep(Duration::from_millis(180)).await;
    let mut after_ttl = other
        .try_lock_async(&key, Duration::from_secs(1))
        .await
        .expect("async lock after TTL")
        .expect("TTL should release a dropped async guard");
    assert!(after_ttl.release().await.expect("async cleanup"));
}

fn load_live_config() -> LiveConfig {
    if std::env::var("AXUTILS_REDIS_LIVE_TEST").ok().as_deref() != Some("1") {
        panic!("set AXUTILS_REDIS_LIVE_TEST=1 before running Redis live tests");
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(LIVE_CONFIG_PATH);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing Redis live test configuration at {LIVE_CONFIG_PATH}"));
    parse_live_config(&content)
        .unwrap_or_else(|field| panic!("missing or invalid field {field} in {LIVE_CONFIG_PATH}"))
}

fn parse_live_config(content: &str) -> Result<LiveConfig, &'static str> {
    let values = content
        .lines()
        .filter_map(parse_line)
        .collect::<HashMap<_, _>>();
    let redis_url = values
        .get("redis_url")
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or("redis_url")?;
    let key_prefix = values
        .get("key_prefix")
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or("key_prefix")?;
    Ok(LiveConfig {
        redis_url,
        key_prefix,
    })
}

fn parse_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let value = value.trim();
    let value = if let Some(value) = value.strip_prefix('"') {
        let end = value.find('"')?;
        &value[..end]
    } else {
        value.split('#').next()?.trim()
    };
    Some((key.trim().to_owned(), value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{parse_line, parse_live_config};

    #[test]
    fn parses_only_safe_key_value_lines() {
        assert_eq!(
            parse_line(r#"redis_url = "redis://127.0.0.1:6379/0" # local"#),
            Some((
                "redis_url".to_owned(),
                "redis://127.0.0.1:6379/0".to_owned()
            ))
        );
        assert_eq!(parse_line("# password = secret"), None);
    }

    #[test]
    fn rejects_missing_or_empty_live_config_fields() {
        assert!(matches!(
            parse_live_config("redis_url = \"redis://127.0.0.1:6379/0\""),
            Err("key_prefix")
        ));
        assert!(matches!(
            parse_live_config("redis_url = \"redis://127.0.0.1:6379/0\"\nkey_prefix = \"\""),
            Err("key_prefix")
        ));
    }
}
