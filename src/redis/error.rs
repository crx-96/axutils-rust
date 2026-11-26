use std::fmt;

use ::redis::{ErrorKind, RedisError as UpstreamRedisError, ServerErrorKind};

/// Redis 传输失败的稳定分类。
///
/// 该枚举不保存 Redis 服务端原始响应、连接地址、认证信息或命令参数，适合记录到应用
/// 日志并用于调用方的重试决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RedisTransportErrorKind {
    /// 建立连接失败。
    Connection,
    /// Redis 拒绝了认证信息。
    Authentication,
    /// 连接建立或命令响应超过了底层时间预算。
    Timeout,
    /// Redis 协议解析或协议状态错误。
    Protocol,
    /// Redis 服务端返回了命令错误。
    Server,
    /// 底层网络 I/O 失败。
    Network,
    /// 无法进一步分类的传输失败。
    Other,
}

impl fmt::Display for RedisTransportErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Connection => "connection",
            Self::Authentication => "authentication",
            Self::Timeout => "timeout",
            Self::Protocol => "protocol",
            Self::Server => "server",
            Self::Network => "network",
            Self::Other => "other",
        })
    }
}

/// Redis 配置、参数、编解码、连接、事务或全局入口错误。
///
/// 错误只包含固定字段名、稳定分类和资源上限，不保存完整 URL、用户名、密码、key、field、
/// 值或第三方错误对象；因此 [`std::error::Error::source`] 不会暴露底层错误链。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RedisError {
    /// 配置字段无效。
    InvalidConfig {
        /// 无效配置的固定字段分类，例如 `"url"`、`"scheme"`、`"nodes"`、`"credentials"`、
        /// `"database"`、`"pool_size"` 或 `"ttl"`；不包含 URL、节点地址、用户名、密码或
        /// 其他配置值。
        ///
        /// 调用方可按字段名匹配配置错误，但不应把字段名误当成可安全记录配置内容的许可。
        field: &'static str,
    },
    /// key 为空或超过当前配置上限。
    InvalidKey,
    /// Hash field 为空或超过当前配置上限。
    InvalidField,
    /// 值、批量参数或事务编码超过限制；`limit` 的单位由具体操作决定。
    ValueTooLarge {
        /// 当前操作允许的输入或批量预算；单位由操作决定，通常是序列化/原始值或批量累计
        /// 字节数，也可能是批量项数。该值不包含 Redis key、value 或服务端响应内容。
        ///
        /// 调用方可读取它向用户报告对应预算并匹配此变体；不要假定所有操作都以字节计量。
        limit: usize,
    },
    /// 多项响应累计字节数超过限制。
    ResponseTooLarge {
        /// 响应累计大小上限，单位为字节；只表示本地预算，不包含响应内容。
        ///
        /// 调用方可用它区分响应预算耗尽与其他传输错误，并据此缩小查询或调整配置。
        limit: usize,
    },
    /// 集合或列表返回项数超过限制。
    CollectionTooLarge {
        /// 集合或列表允许返回的最大项数，单位为项；只表示本地数量预算，不包含返回值。
        ///
        /// 调用方可用它匹配结果数量超限并改用分页或更小的范围。
        limit: usize,
    },
    /// MessagePack 序列化失败。
    Serialize,
    /// MessagePack 反序列化失败。
    Deserialize,
    /// Redis 命令或连接传输失败。
    Transport(RedisTransportErrorKind),
    /// 同步连接池不可用或 checkout 失败。
    Pool,
    /// 本地连接池/预算操作超时。
    Timeout,
    /// 异步 API 必须在 Tokio runtime 中调用。
    RuntimeRequired,
    /// MULTI/EXEC 事务未能可靠完成。
    TransactionFailed,
    /// 当前操作不适用于客户端模式，例如集群事务。
    UnsupportedMode,
    /// 集群多 key 命令跨越了多个 hash slot。
    CrossSlot,
    /// 全局 RedisUtils 尚未初始化。
    NotInitialized,
    /// 全局 RedisUtils 已经成功初始化。
    AlreadyInitialized,
}

impl RedisError {
    pub(crate) fn invalid_config(field: &'static str) -> Self {
        Self::InvalidConfig { field }
    }

    /// 将单条命令或建连错误映射为脱敏的固定分类。
    pub(crate) fn from_upstream(error: &UpstreamRedisError) -> Self {
        if error.code().is_some_and(is_cross_slot_code) {
            return Self::CrossSlot;
        }
        if error.is_timeout() {
            return Self::Transport(RedisTransportErrorKind::Timeout);
        }

        match error.kind() {
            ErrorKind::AuthenticationFailed => {
                Self::Transport(RedisTransportErrorKind::Authentication)
            }
            ErrorKind::Parse | ErrorKind::UnexpectedReturnType => {
                Self::Transport(RedisTransportErrorKind::Protocol)
            }
            ErrorKind::Server(_) | ErrorKind::Extension => {
                Self::Transport(RedisTransportErrorKind::Server)
            }
            ErrorKind::Io => {
                if error.is_io_error() {
                    Self::Transport(RedisTransportErrorKind::Network)
                } else {
                    Self::Transport(RedisTransportErrorKind::Other)
                }
            }
            ErrorKind::ClusterConnectionNotFound => {
                Self::Transport(RedisTransportErrorKind::Connection)
            }
            ErrorKind::RESP3NotSupported => Self::Transport(RedisTransportErrorKind::Protocol),
            ErrorKind::InvalidClientConfig | ErrorKind::Client => {
                Self::Transport(RedisTransportErrorKind::Other)
            }
            _ if error.is_cluster_error() => Self::Transport(RedisTransportErrorKind::Connection),
            _ => Self::Transport(RedisTransportErrorKind::Other),
        }
    }

    /// 识别直接返回或导致事务中止的跨槽错误，其余失败统一保留事务分类。
    pub(crate) fn transaction_failure(error: UpstreamRedisError) -> Self {
        if error.code().is_some_and(is_cross_slot_code) {
            return Self::CrossSlot;
        }
        // 驱动将入队失败包装为 EXECABORT，具体命令的错误码仅保存在子错误列表中。
        if error.kind() == ErrorKind::Server(ServerErrorKind::ExecAbort)
            && error
                .into_server_errors()
                .is_some_and(|errors| errors.iter().any(|(_, err)| is_cross_slot_code(err.code())))
        {
            Self::CrossSlot
        } else {
            Self::TransactionFailed
        }
    }
}

/// 按 ASCII 忽略大小写匹配完整服务端错误码，错误正文不参与分类。
fn is_cross_slot_code(code: &str) -> bool {
    code.eq_ignore_ascii_case("CROSSSLOT")
}

impl fmt::Display for RedisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid Redis configuration field: {field}")
            }
            Self::InvalidKey => formatter.write_str("invalid Redis key"),
            Self::InvalidField => formatter.write_str("invalid Redis hash field"),
            Self::ValueTooLarge { limit } => {
                write!(
                    formatter,
                    "Redis input exceeds limit {limit} (unit depends on operation)"
                )
            }
            Self::ResponseTooLarge { limit } => {
                write!(formatter, "Redis response exceeds limit {limit} bytes")
            }
            Self::CollectionTooLarge { limit } => {
                write!(formatter, "Redis collection exceeds limit {limit} items")
            }
            Self::Serialize => formatter.write_str("Redis value serialization failed"),
            Self::Deserialize => formatter.write_str("Redis value deserialization failed"),
            Self::Transport(kind) => write!(formatter, "Redis transport failed: {kind}"),
            Self::Pool => formatter.write_str("Redis connection pool is unavailable"),
            Self::Timeout => formatter.write_str("Redis local operation timed out"),
            Self::RuntimeRequired => {
                formatter.write_str("Redis async API requires a Tokio runtime")
            }
            Self::TransactionFailed => formatter.write_str("Redis transaction failed"),
            Self::UnsupportedMode => {
                formatter.write_str("Redis operation is unsupported in this mode")
            }
            Self::CrossSlot => formatter.write_str("Redis multi-key operation crosses hash slots"),
            Self::NotInitialized => formatter.write_str("RedisUtils has not been initialized"),
            Self::AlreadyInitialized => formatter.write_str("RedisUtils is already initialized"),
        }
    }
}

impl std::error::Error for RedisError {}

#[cfg(test)]
mod tests {
    use ::redis::{ErrorKind, RedisError as UpstreamRedisError, ServerErrorKind};

    use super::{RedisError, RedisTransportErrorKind};

    #[test]
    fn error_output_contains_no_sensitive_input() {
        let error = RedisError::Transport(RedisTransportErrorKind::Authentication);
        let display = error.to_string();
        let debug = format!("{error:?}");
        for output in [display, debug] {
            assert!(!output.contains("redis://"));
            assert!(!output.contains("password"));
            assert!(!output.contains("secret-key"));
            assert!(!output.contains("secret-value"));
        }
    }

    #[test]
    fn value_limit_display_does_not_assume_bytes() {
        assert_eq!(
            RedisError::ValueTooLarge { limit: 4 }.to_string(),
            "Redis input exceeds limit 4 (unit depends on operation)"
        );
    }

    #[test]
    fn maps_cross_slot_from_upstream_error_code() {
        for upstream in [
            UpstreamRedisError::from((
                ErrorKind::Server(ServerErrorKind::CrossSlot),
                "server error",
            )),
            ::redis::make_extension_error("CrossSlot".to_owned(), None),
        ] {
            assert_eq!(RedisError::from_upstream(&upstream), RedisError::CrossSlot);
            assert_eq!(
                RedisError::transaction_failure(upstream),
                RedisError::CrossSlot
            );
        }
    }

    #[test]
    fn err_mentioning_cross_slot_keeps_its_server_error_classification() {
        let upstream = UpstreamRedisError::from((
            ErrorKind::Server(ServerErrorKind::ResponseError),
            "server error",
            "script mentions CROSSSLOT".to_owned(),
        ));
        assert_eq!(
            RedisError::from_upstream(&upstream),
            RedisError::Transport(RedisTransportErrorKind::Server)
        );
        assert_eq!(
            RedisError::transaction_failure(upstream),
            RedisError::TransactionFailed
        );
    }

    #[test]
    fn maps_cluster_connection_and_resp3_errors_to_stable_transport_kinds() {
        let cluster = UpstreamRedisError::from((
            ErrorKind::ClusterConnectionNotFound,
            "cluster connection unavailable",
        ));
        assert_eq!(
            RedisError::from_upstream(&cluster),
            RedisError::Transport(RedisTransportErrorKind::Connection)
        );

        let resp3 =
            UpstreamRedisError::from((ErrorKind::RESP3NotSupported, "RESP3 is not supported"));
        assert_eq!(
            RedisError::from_upstream(&resp3),
            RedisError::Transport(RedisTransportErrorKind::Protocol)
        );
    }
}
