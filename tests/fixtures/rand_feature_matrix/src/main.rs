#[cfg(feature = "rand")]
fn compile_rand_api() {
    use axutils::random_utils::RandomUtils as ModuleRandomUtils;
    use axutils::utils::random_utils::RandomUtils as NestedRandomUtils;
    use axutils::utils::RandomUtils as UtilsRandomUtils;
    use axutils::{LetterCase, RandomUtils};

    let root = RandomUtils::alphabetic_string(4, LetterCase::Lower).expect("root path");
    let module = ModuleRandomUtils::numeric_string(4).expect("module path");
    let utils = UtilsRandomUtils::alphanumeric_string(4).expect("utils path");
    let nested = NestedRandomUtils::integer(1..=1).expect("nested path");
    assert_eq!(nested, 1);
    assert_eq!(root.len(), 4);
    assert_eq!(module.len(), 4);
    assert_eq!(utils.len(), 4);
}

#[cfg(any(feature = "redis", feature = "redis-tokio"))]
fn compile_redis_without_random_utils() {
    let config = axutils::RedisConfig::single("redis://example.com:6379/0")
        .expect("redis feature should expose RedisConfig");
    let _ = axutils::RedisClient::new(config).expect("redis client construction is local");
}

#[cfg(feature = "negative-no-rand-root")]
fn main() {
    let _ = axutils::RandomUtils::numeric_string;
}

#[cfg(feature = "negative-no-rand-module")]
fn main() {
    let _ = axutils::random_utils::RandomUtils::numeric_string;
}

#[cfg(feature = "negative-no-rand-utils")]
fn main() {
    let _ = axutils::utils::RandomUtils::numeric_string;
}

#[cfg(feature = "negative-redis-random-utils")]
fn main() {
    let _ = axutils::RandomUtils::numeric_string;
}

#[cfg(not(any(
    feature = "negative-no-rand-root",
    feature = "negative-no-rand-module",
    feature = "negative-no-rand-utils",
    feature = "negative-redis-random-utils",
)))]
fn main() {
    #[cfg(feature = "rand")]
    compile_rand_api();
    #[cfg(any(feature = "redis", feature = "redis-tokio"))]
    compile_redis_without_random_utils();
}
