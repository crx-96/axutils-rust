//! Redis 失效队列的脱敏事件适配；不安装 subscriber，也不参与调度。

use crate::redis::RedisError;

use super::redis as redis_trace;

/// 记录固定事件分类与条目数；可选错误只提取库内稳定分类，不输出键或 Redis 响应。
pub(crate) fn record(event: &'static str, count: usize, error: Option<&RedisError>) {
    // 所有事件名由库实现提供；调用方不能将业务文案或缓存键注入日志字段。
    ::tracing::warn!(
        target: "axutils::redis",
        operation = "cache_invalidation",
        event,
        count,
        error_kind = error.map(redis_trace::error_kind),
    );
}
