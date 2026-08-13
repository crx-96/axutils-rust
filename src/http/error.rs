//! HTTP 错误类型。

use std::fmt;

/// 可归类的传输层错误类型。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum HttpTransportErrorKind {
    /// DNS、连接建立或连接复用失败。
    Connection,
    /// 连接或请求超过了时间预算。
    Timeout,
    /// TLS 握手或证书验证失败。
    Tls,
    /// HTTP 协议、解析或响应格式失败。
    Protocol,
    /// 其他未分类的本地传输错误。
    Other,
}

/// HTTP 客户端错误。
///
/// 错误值不会保存 URL、请求头、请求体、响应体或第三方库的错误文本，因而可以安全地
/// 复制到日志和共享的 single-flight 结果中。需要诊断底层库时，应在应用边界自行记录
/// 受控的上下文，而不要把敏感请求内容放入错误消息。
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HttpError {
    /// 配置字段无效。
    InvalidConfig { field: &'static str },
    /// 请求字段无效。
    InvalidRequest { field: &'static str },
    /// URL 不是 HTTP 或 HTTPS 绝对地址，或不能作为安全的相对地址解析。
    InvalidUrl,
    /// Header 名称不是合法的 HTTP token。
    InvalidHeaderName,
    /// Header 值包含控制字符或超出单值限制。
    InvalidHeaderValue,
    /// Header 数量或总大小超过限制。
    HeaderLimitExceeded,
    /// 不允许合并敏感重复 Header。
    DuplicateSensitiveHeader,
    /// 请求体超过限制。
    RequestBodyTooLarge { limit: usize },
    /// 响应体超过限制。
    ResponseTooLarge { limit: usize },
    /// 响应体不是有效 UTF-8。
    InvalidUtf8,
    /// 使用 Serde JSON 序列化请求体失败。
    JsonSerialize,
    /// 使用 Serde URL 编码查询参数失败。
    QuerySerialize,
    /// 使用 Serde JSON 反序列化响应体失败。
    JsonDeserialize,
    /// 传输失败；`attempts` 包含已经完成或发起的网络尝试次数。
    Transport {
        /// 便于调用方按连接、超时或 TLS 分类处理。
        kind: HttpTransportErrorKind,
        /// 已使用的网络尝试次数。
        attempts: u32,
        /// 是否已经达到 `RetryPolicy::max_retries()` 总尝试次数。
        ///
        /// 不可重试的方法、提前到达请求 deadline 或单次等待超时本身不会把该字段置为
        /// `true`，除非当时的尝试次数已经达到预算。
        exhausted: bool,
    },
    /// 异步入口需要在 Tokio runtime 中调用。
    RuntimeRequired,
    /// 同步入口不能在 Tokio runtime 中调用。
    BlockingInAsyncRuntime,
    /// 全局 HTTP 工具尚未初始化。
    NotInitialized,
    /// 全局 HTTP 工具已经初始化。
    AlreadyInitialized,
    /// single-flight 的 leader 在发布结果前取消或异常退出。
    CoalescedRequestCancelled,
    /// follower 等待共享结果时超过了自己的时间预算。
    CoalescedWaitTimeout,
    /// HTTP 后端客户端构造失败。
    ClientBuild,
}

impl fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig { field } => {
                write!(formatter, "invalid HTTP configuration: {field}")
            }
            Self::InvalidRequest { field } => write!(formatter, "invalid HTTP request: {field}"),
            Self::InvalidUrl => formatter.write_str("invalid HTTP URL"),
            Self::InvalidHeaderName => formatter.write_str("invalid HTTP header name"),
            Self::InvalidHeaderValue => formatter.write_str("invalid HTTP header value"),
            Self::HeaderLimitExceeded => formatter.write_str("HTTP header limits exceeded"),
            Self::DuplicateSensitiveHeader => {
                formatter.write_str("sensitive HTTP header cannot be duplicated")
            }
            Self::RequestBodyTooLarge { limit } => {
                write!(formatter, "HTTP request body exceeds {limit} bytes")
            }
            Self::ResponseTooLarge { limit } => {
                write!(formatter, "HTTP response body exceeds {limit} bytes")
            }
            Self::InvalidUtf8 => formatter.write_str("HTTP response body is not valid UTF-8"),
            Self::JsonSerialize => formatter.write_str("failed to serialize HTTP JSON body"),
            Self::QuerySerialize => formatter.write_str("failed to serialize HTTP query"),
            Self::JsonDeserialize => {
                formatter.write_str("failed to deserialize HTTP JSON response")
            }
            Self::Transport {
                kind,
                attempts,
                exhausted,
            } => write!(
                formatter,
                "HTTP transport failed ({kind:?}, attempts={attempts}, exhausted={exhausted})"
            ),
            Self::RuntimeRequired => formatter.write_str("HTTP async API requires a Tokio runtime"),
            Self::BlockingInAsyncRuntime => {
                formatter.write_str("blocking HTTP API cannot run in a Tokio runtime")
            }
            Self::NotInitialized => formatter.write_str("global HTTP client is not initialized"),
            Self::AlreadyInitialized => {
                formatter.write_str("global HTTP client is already initialized")
            }
            Self::CoalescedRequestCancelled => {
                formatter.write_str("coalesced HTTP request was cancelled")
            }
            Self::CoalescedWaitTimeout => {
                formatter.write_str("coalesced HTTP request wait timed out")
            }
            Self::ClientBuild => formatter.write_str("failed to build HTTP client"),
        }
    }
}

impl std::error::Error for HttpError {}
