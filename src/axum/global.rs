//! 默认服务的一次初始化入口；运行仍由实例显式控制。

use std::sync::OnceLock;

use super::{AxumError, AxumServer};

/// 首个成功发布的默认服务，停止后也不允许替换或重新初始化。
static SERVER: OnceLock<AxumServer> = OnceLock::new();

/// 进程内唯一默认 [`AxumServer`] 的生命周期入口。
///
/// server 初始化只成功一次，成功后不能 reset 或 replace。路由构建使用 [`super::AxumApp`]，
/// 服务运行和关闭由 [`AxumUtils::server`] 返回的实例负责。
pub struct AxumUtils;

impl AxumUtils {
    /// 初始化默认服务；并发调用只有一个成功，失败值不占用初始化机会。
    pub fn init(server: AxumServer) -> Result<(), AxumError> {
        // 原子发布已经构建的实例；竞争落败的输入随错误路径释放，不触发监听。
        SERVER
            .set(server)
            .map_err(|_| AxumError::AlreadyInitialized)
    }

    /// 返回是否已经初始化；服务停止或 abandoned 后仍为 `true`。
    pub fn is_initialized() -> bool {
        SERVER.get().is_some()
    }

    /// 返回已初始化的默认服务。
    ///
    /// 未初始化时返回 [`AxumError::NotInitialized`]。返回的实例 clone 共享底层单次运行状态机；
    /// 调用方通过 [`AxumServer`] 直接管理 serve 和 graceful shutdown。
    pub fn server() -> Result<&'static AxumServer, AxumError> {
        SERVER.get().ok_or(AxumError::NotInitialized)
    }
}
