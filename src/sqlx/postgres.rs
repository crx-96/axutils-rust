//! PostgreSQL 原生错误的只读分类，不承接事务重试策略。

use sqlx::Error as BackendError;

/// 判断 PostgreSQL 操作的原生 SQLx 错误是否报告事务冲突。
///
/// 仅在 `sqlx-postgres` feature 下可用。读取数据库错误的 SQLSTATE，精确识别
/// 序列化冲突 `40001` 和死锁 `40P01`；非数据库错误、没有 SQLSTATE 或其他代码
/// 均返回 `false`。调用方应只将 PostgreSQL 操作的错误传入此分类器；它不检查
/// 具体 driver 类型，也不会解析数据库错误消息。
///
/// 这是错误类别判断，不保证业务可以安全重试，也不自动执行重试。调用方应自行判断
/// 事务和外部副作用是否可重放；如决定重试，应重新执行整个事务。
/// 函数只借用错误且不记录其内容，原生错误仍可能含有 SQL、连接信息或业务参数，
/// 并不会因为经过本函数查询而自动脱敏。
///
/// # Examples
///
/// ```
/// use axutils::sqlx as axutils_sqlx;
/// use sqlx::Error;
///
/// assert!(!axutils_sqlx::is_postgres_transaction_conflict(&Error::RowNotFound));
/// assert!(!axutils_sqlx::is_postgres_transaction_conflict(&Error::PoolClosed));
/// ```
pub fn is_postgres_transaction_conflict(error: &BackendError) -> bool {
    // 仅检查标准错误码；不格式化原始错误，也不把其他失败泛化为可重试事务。
    matches!(
        error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("40001" | "40P01")
    )
}
