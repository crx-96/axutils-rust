use std::time::{SystemTime, UNIX_EPOCH};

use super::JwtError;

pub(crate) fn now_seconds() -> Result<u64, JwtError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| JwtError::InvalidClaim { claim: "clock" })
}
