use std::fmt;
use std::io;

use sqlx::Error as BackendError;

/// SQLx 传输失败的稳定分类。
///
/// 该枚举不保存 SQLx 的原始错误、SQL、连接地址、认证信息或数据库响应，适合记录到应用
/// 日志并用于调用方的重试决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SqlxTransportErrorKind {
    /// 连接建立或驱动选择失败。
    Connection,
    /// 连接获取或底层 I/O 超时。
    Timeout,
    /// 协议解析或协议状态错误。
    Protocol,
    /// 数据库服务端返回了失败响应。
    Server,
    /// 底层网络 I/O 失败。
    Network,
    /// 结果行解码失败。
    Decode,
    /// 参数编码失败。
    Encode,
    /// TLS 失败；首版不主动配置 TLS。
    Tls,
    /// 无法进一步分类的 SQLx 传输失败。
    Other,
}

impl fmt::Display for SqlxTransportErrorKind {
    /// 输出固定分类 token，不包含底层错误消息或连接信息。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 将各分类映射为稳定的非敏感文本，供调用方安全展示或记录。
        formatter.write_str(match self {
            Self::Connection => "connection",
            Self::Timeout => "timeout",
            Self::Protocol => "protocol",
            Self::Server => "server",
            Self::Network => "network",
            Self::Decode => "decode",
            Self::Encode => "encode",
            Self::Tls => "tls",
            Self::Other => "other",
        })
    }
}

/// SQLx 配置、runtime、连接池、查询或全局入口错误。
///
/// 该错误只包含固定字段名、稳定分类和本地限制值，不保存完整 URL、用户名、密码、SQL 文本、
/// 数据库错误消息或第三方错误对象；因此 [`std::error::Error::source`] 不会暴露 SQLx 的错误链。
/// `SqlxTransaction` 的原生事务操作仍直接返回 SQLx 自身的错误，不能将其误认为已经过本类型脱敏。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SqlxError {
    /// 配置字段无效。
    InvalidConfig {
        /// 无效配置的固定字段分类，例如 `"url"`、`"tls"` 或 `"max_rows"`；不包含完整
        /// URL、用户名、密码、SQL 文本或数据库错误内容。
        ///
        /// 调用方可按字段名匹配配置错误，但应保留 wildcard 以兼容未来分类。
        field: &'static str,
    },
    /// 异步 API 必须在已有的 Tokio runtime 中调用。
    RuntimeRequired,
    /// 全局 [`crate::utils::SqlxUtils`] 尚未初始化。
    NotInitialized,
    /// 全局 [`crate::utils::SqlxUtils`] 已经成功初始化。
    AlreadyInitialized,
    /// 查询要求至少一行，但结果为空。
    RowNotFound,
    /// 结果行数超过配置的最大值。
    RowLimitExceeded {
        /// 查询结果允许的最大行数，单位为行；只表示本地预算，不包含 SQL 或结果数据。
        ///
        /// 调用方可据此匹配行数预算耗尽并改用分页或更小的查询范围。
        limit: usize,
    },
    /// 连接池获取连接时超时。
    PoolAcquireTimeout,
    /// 连接池已被关闭。
    PoolClosed,
    /// 事务未能开始或事务状态无效。
    TransactionFailed,
    /// SQLx 底层失败的稳定分类。
    Transport(SqlxTransportErrorKind),
}

impl SqlxError {
    /// 判断错误是否属于本库明确识别的基础设施不可用类别。
    ///
    /// 仅连接池获取超时、连接池关闭以及底层传输的超时和网络错误返回 `true`。
    /// 配置、认证/TLS、协议、服务端响应、约束冲突、行不存在、编码和解码错误
    /// 不会被泛化为基础设施不可用；`false` 也不证明基础设施健康。
    /// 此方法只查询已有分类，不执行 I/O，也不保证当前业务操作可以安全重试。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::sqlx::{SqlxError, SqlxTransportErrorKind};
    ///
    /// assert!(SqlxError::PoolClosed.is_infrastructure_unavailable());
    /// assert!(SqlxError::Transport(SqlxTransportErrorKind::Network)
    ///     .is_infrastructure_unavailable());
    /// assert!(!SqlxError::RowNotFound.is_infrastructure_unavailable());
    /// ```
    pub fn is_infrastructure_unavailable(&self) -> bool {
        // 仅列举约定的四类，避免把查询、数据或协议错误误报为基础设施故障。
        matches!(
            self,
            Self::PoolAcquireTimeout
                | Self::PoolClosed
                | Self::Transport(
                    SqlxTransportErrorKind::Timeout | SqlxTransportErrorKind::Network
                )
        )
    }

    /// 将原生 SQLx 错误收敛为固定分类，不保留其原始内容或错误链。
    pub(crate) fn from_upstream(error: &sqlx::Error) -> Self {
        // 优先识别连接池、结果与事务错误，再将传输失败降为稳定且脱敏的类别。
        match error {
            BackendError::RowNotFound => Self::RowNotFound,
            BackendError::PoolTimedOut => Self::PoolAcquireTimeout,
            BackendError::PoolClosed => Self::PoolClosed,
            BackendError::BeginFailed | BackendError::InvalidSavePointStatement => {
                Self::TransactionFailed
            }
            BackendError::Io(error) => {
                // 只读取 I/O 类型；超时独立于其他网络故障，不复制系统错误文本。
                Self::Transport(if error.kind() == io::ErrorKind::TimedOut {
                    SqlxTransportErrorKind::Timeout
                } else {
                    SqlxTransportErrorKind::Network
                })
            }
            BackendError::Database(_) => Self::Transport(SqlxTransportErrorKind::Server),
            BackendError::Tls(_) => Self::Transport(SqlxTransportErrorKind::Tls),
            BackendError::Protocol(_) => Self::Transport(SqlxTransportErrorKind::Protocol),
            BackendError::Encode(_) => Self::Transport(SqlxTransportErrorKind::Encode),
            BackendError::Decode(_) | BackendError::ColumnDecode { .. } => {
                Self::Transport(SqlxTransportErrorKind::Decode)
            }
            BackendError::Configuration(_) | BackendError::InvalidArgument(_) => {
                Self::Transport(SqlxTransportErrorKind::Connection)
            }
            BackendError::AnyDriverError(_) => Self::Transport(SqlxTransportErrorKind::Other),
            _ => Self::Transport(SqlxTransportErrorKind::Other),
        }
    }
}

impl fmt::Display for SqlxError {
    /// 输出本库错误类别和公开预算，不格式化任何第三方错误对象。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 每个变体只产生固定文本或调用方可见的本地限制值。
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid SQLx configuration field: {field}")
            }
            Self::RuntimeRequired => formatter.write_str("SQLx async API requires a Tokio runtime"),
            Self::NotInitialized => formatter.write_str("SqlxUtils has not been initialized"),
            Self::AlreadyInitialized => formatter.write_str("SqlxUtils is already initialized"),
            Self::RowNotFound => formatter.write_str("no rows returned by the SQLx query"),
            Self::RowLimitExceeded { limit } => {
                write!(formatter, "SQLx query result exceeds the row limit {limit}")
            }
            Self::PoolAcquireTimeout => formatter.write_str("SQLx pool acquire timed out"),
            Self::PoolClosed => formatter.write_str("SQLx connection pool is closed"),
            Self::TransactionFailed => formatter.write_str("SQLx transaction failed"),
            Self::Transport(kind) => write!(formatter, "SQLx transport failed: {kind}"),
        }
    }
}

impl std::error::Error for SqlxError {}

#[cfg(test)]
mod tests {
    use super::{SqlxError, SqlxTransportErrorKind};

    #[test]
    fn error_output_contains_no_sensitive_input() {
        let error = SqlxError::Transport(SqlxTransportErrorKind::Server);
        let display = error.to_string();
        let debug = format!("{error:?}");
        for output in [display, debug] {
            assert!(!output.contains("postgres://"));
            assert!(!output.contains("mysql://"));
            assert!(!output.contains("password"));
            assert!(!output.contains("secret"));
            assert!(!output.contains("SELECT"));
        }
    }
}
