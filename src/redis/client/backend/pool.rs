//! 同步池的连接管理、脱敏诊断和连接淘汰；不拥有客户端拓扑选择。

use std::{fmt, time::Duration};

#[cfg(feature = "redis-cluster")]
use ::redis::cluster::{ClusterClient, ClusterConfig, ClusterConnection};
use ::redis::{
    Client as UpstreamClient, Connection, ConnectionLike, RedisError as UpstreamRedisError,
};
use r2d2::{ManageConnection, Pool};

use crate::redis::{RedisError, RedisTransportErrorKind};

/// 单机同步池，checkout 的连接由 ManagedConnection 跟踪是否仍可复用。
pub(super) type SinglePool = Pool<SingleManager>;
/// Cluster 同步池，每个池连接内部管理一个集群拓扑。
#[cfg(feature = "redis-cluster")]
pub(super) type ClusterPool = Pool<ClusterManager>;

/// 单机连接的 r2d2 工厂；所有错误在交给连接池前降为固定分类。
pub(in crate::redis::client) struct SingleManager {
    /// 仅持有端点配置的上游客户端，由 checkout 按需建立连接。
    pub(super) client: UpstreamClient,
    /// 单次建立连接的最长等待时间。
    pub(super) connection_timeout: Duration,
    /// 建立后每次读写的最长阻塞时间。
    pub(super) response_timeout: Duration,
}

/// Cluster 连接的 r2d2 工厂，统一约束集群连接建立和响应超时。
#[cfg(feature = "redis-cluster")]
pub(in crate::redis::client) struct ClusterManager {
    /// 只包含引导节点及本地参数的上游 Cluster client。
    pub(super) client: ClusterClient,
    /// 上游 Cluster 连接建立预算。
    pub(super) connection_timeout: Duration,
    /// 上游 Cluster 命令响应预算。
    pub(super) response_timeout: Duration,
}

/// 标记本库交给 r2d2 的已脱敏错误，使 checkout 超时文本仍可恢复其固定分类。
const MANAGER_ERROR_PREFIX: &str = "axutils-redis-manager:";

/// r2d2 可展示的错误；不持有上游对象或敏感错误链。
#[derive(Debug)]
pub(in crate::redis::client) struct SyncManagerError {
    /// 连接失败的固定分类，不包含 URL、命令或服务器原文。
    kind: RedisTransportErrorKind,
}

impl SyncManagerError {
    /// 从稳定分类构造不含来源错误链的池诊断。
    fn new(kind: RedisTransportErrorKind) -> Self {
        Self { kind }
    }

    /// 将原生连接错误转换为固定分类；不记录或保存第三方诊断原文。
    fn from_upstream(error: &UpstreamRedisError) -> Self {
        let kind = match RedisError::from_upstream(error) {
            RedisError::Transport(kind) => kind,
            RedisError::CrossSlot => RedisTransportErrorKind::Server,
            _ => RedisTransportErrorKind::Other,
        };
        Self::new(kind)
    }
}

impl fmt::Display for SyncManagerError {
    /// 输出可被本库恢复的固定前缀和分类 token。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{MANAGER_ERROR_PREFIX}{}", self.kind)
    }
}

impl std::error::Error for SyncManagerError {}

/// 为原生连接补充本地淘汰标记；超时后的协议状态不能仅凭 socket 开启判断。
pub(in crate::redis::client) struct ManagedConnection<C> {
    /// 由连接池独占借出和归还的上游连接。
    inner: C,
    /// 当前连接不得再放回可用池；设置后不恢复为 false。
    broken: bool,
}

impl<C> ManagedConnection<C> {
    /// 接管已建立连接，初始允许归还连接池。
    fn new(inner: C) -> Self {
        Self {
            inner,
            broken: false,
        }
    }

    /// 标记结果不确定或协议失效的连接，归还时由 r2d2 淘汰。
    pub(in crate::redis::client) fn mark_broken(&mut self) {
        self.broken = true;
    }
}

impl<C: ::redis::ConnectionLike> ::redis::ConnectionLike for ManagedConnection<C> {
    /// 转发单个已编码命令，保持上游协议和响应语义。
    fn req_packed_command(&mut self, cmd: &[u8]) -> ::redis::RedisResult<::redis::Value> {
        self.inner.req_packed_command(cmd)
    }

    /// 转发 pipeline，保留 offset/count 的响应选择语义。
    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        offset: usize,
        count: usize,
    ) -> ::redis::RedisResult<Vec<::redis::Value>> {
        self.inner.req_packed_commands(cmd, offset, count)
    }

    /// 返回底层连接当前数据库编号。
    fn get_db(&self) -> i64 {
        self.inner.get_db()
    }

    /// 查询底层后端是否支持 pipeline，不发送命令。
    fn supports_pipelining(&self) -> bool {
        self.inner.supports_pipelining()
    }

    /// 仅在显式上游调用时转发健康检查；pool checkout 不调用此方法。
    fn check_connection(&mut self) -> bool {
        self.inner.check_connection()
    }

    /// 查询底层连接本地开启状态，不证明其远端健康。
    fn is_open(&self) -> bool {
        self.inner.is_open()
    }
}

impl ManageConnection for SingleManager {
    /// 池借出的单机连接，包含额外的可靠性状态。
    type Connection = ManagedConnection<Connection>;
    /// 连接池可安全记录的固定诊断分类。
    type Error = SyncManagerError;

    /// 在连接和读写时间预算下建立单机连接，任何失败均脱敏返回。
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        // 建立成功后再设置读写超时；配置失败时丢弃局部连接，不交付给池。
        let connection = self
            .client
            .get_connection_with_timeout(self.connection_timeout)
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        connection
            .set_read_timeout(Some(self.response_timeout))
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        connection
            .set_write_timeout(Some(self.response_timeout))
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        Ok(ManagedConnection::new(connection))
    }

    /// checkout 只检查本地开启状态，不发送隐式健康探测命令。
    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        // 已关闭的连接不能借出；远端故障由实际命令报告并决定是否淘汰。
        if connection.is_open() {
            Ok(())
        } else {
            Err(SyncManagerError::new(RedisTransportErrorKind::Connection))
        }
    }

    /// 本地不可靠标记或已关闭状态都会阻止连接再次复用。
    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.broken || !connection.is_open()
    }
}

#[cfg(feature = "redis-cluster")]
impl ManageConnection for ClusterManager {
    /// 池借出的集群连接，包含额外的可靠性状态。
    type Connection = ManagedConnection<ClusterConnection>;
    /// 连接池可安全记录的固定诊断分类。
    type Error = SyncManagerError;

    /// 用一致的连接/响应预算创建集群连接；错误不会泄露节点信息。
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        // 将同一客户端预算应用于上游 Cluster 配置，而非自行管理节点连接。
        let cluster_config = ClusterConfig::new()
            .set_connection_timeout(self.connection_timeout)
            .set_response_timeout(self.response_timeout);
        let connection = self
            .client
            .get_connection_with_config(cluster_config)
            .map_err(|error| SyncManagerError::from_upstream(&error))?;
        Ok(ManagedConnection::new(connection))
    }

    /// checkout 只检查本地开启状态，不发送隐式健康探测命令。
    fn is_valid(&self, connection: &mut Self::Connection) -> Result<(), Self::Error> {
        // 显式 ping 仍由公开命令提供，获取连接只判断本地是否已经关闭。
        if connection.is_open() {
            Ok(())
        } else {
            Err(SyncManagerError::new(RedisTransportErrorKind::Connection))
        }
    }

    /// 本地不可靠标记或已关闭状态都会阻止连接再次复用。
    fn has_broken(&self, connection: &mut Self::Connection) -> bool {
        connection.broken || !connection.is_open()
    }
}

/// 从 r2d2 诊断恢复本库固定类别，未知内容只返回通用池错误。
pub(in crate::redis::client) fn pool_error(error: &r2d2::Error) -> RedisError {
    // 本库 manager 在错误进入 r2d2 前已经脱敏；这里只识别前缀，不向外返回该文本。
    let detail = error.to_string().to_ascii_lowercase();
    if let Some(kind) = detail
        .split_once(MANAGER_ERROR_PREFIX)
        .and_then(|(_, kind)| parse_manager_error_kind(kind.trim()))
    {
        return RedisError::Transport(kind);
    }
    if detail.trim() == "timed out waiting for connection" {
        RedisError::Timeout
    } else {
        RedisError::Pool
    }
}

/// 已关闭或连接/网络/协议/超时错误意味着响应边界不可靠；完整服务端错误可复用连接。
pub(in crate::redis::client) fn should_discard_connection(
    error: &RedisError,
    is_open: bool,
) -> bool {
    !is_open
        || matches!(
            error,
            RedisError::Transport(
                RedisTransportErrorKind::Connection
                    | RedisTransportErrorKind::Network
                    | RedisTransportErrorKind::Protocol
                    | RedisTransportErrorKind::Timeout
            )
        )
}

/// 事务沿用普通连接的淘汰标准，不因完整的服务端命令错误丢弃健康连接。
pub(in crate::redis::client) fn should_discard_transaction_connection(
    error: &UpstreamRedisError,
    is_open: bool,
) -> bool {
    should_discard_connection(&RedisError::from_upstream(error), is_open)
}

#[cfg(feature = "redis-async")]
/// multiplexed 连接没有本地 is_open 探测，只能依据本次错误判断复用可靠性。
pub(in crate::redis::client) fn should_discard_multiplexed_transaction_connection(
    error: &::redis::RedisError,
) -> bool {
    should_discard_transaction_connection(error, true)
}

/// 解析 manager 的固定分类 token；不接受前缀匹配或服务端文本推断。
fn parse_manager_error_kind(value: &str) -> Option<RedisTransportErrorKind> {
    match value {
        "connection" => Some(RedisTransportErrorKind::Connection),
        "authentication" => Some(RedisTransportErrorKind::Authentication),
        "timeout" => Some(RedisTransportErrorKind::Timeout),
        "protocol" => Some(RedisTransportErrorKind::Protocol),
        "server" => Some(RedisTransportErrorKind::Server),
        "network" => Some(RedisTransportErrorKind::Network),
        "other" => Some(RedisTransportErrorKind::Other),
        _ => None,
    }
}
