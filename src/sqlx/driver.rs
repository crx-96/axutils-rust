use std::sync::Once;

use sqlx::any;

/// 进程内只尝试一次默认 driver 注册；不能覆盖应用已经安装的自定义 driver 列表。
static DEFAULT_DRIVERS: Once = Once::new();

/// 显式连接时安装已编译的 Any drivers；与应用自定义注册冲突时沿用上游 panic 契约。
pub(crate) fn install_default_drivers() {
    // 默认驱动的进程状态由上游管理，Once 只串行化本库的首次连接入口。
    DEFAULT_DRIVERS.call_once(any::install_default_drivers);
}
