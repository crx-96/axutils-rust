// JWT 标准时间 claim 使用系统 Unix 时钟。
//
// 公共 decode API 不提供时钟注入；测试通过 crate 内部的固定时间入口验证边界，应用若需
// 可控时钟应在调用前自行生成/校验 token 或隔离业务时间策略。

use std::time::{SystemTime, UNIX_EPOCH};

use super::JwtError;

pub(crate) fn now_seconds() -> Result<u64, JwtError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| JwtError::InvalidClaim { claim: "clock" })
}
