#[cfg(unix)]
use std::{
    future::{poll_fn, Future},
    pin::pin,
    task::Poll,
};

use super::TokioError;
use tokio::signal;

/// 跨平台 OS shutdown 原因。
///
/// # Examples
/// ```rust
/// # use axutils::tokio::*;
/// # use axutils::tokio::*;
/// # #[cfg(feature="tokio")] {
/// let reason = TokioShutdownReason::CtrlC;
/// assert_eq!(reason, TokioShutdownReason::CtrlC);
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TokioShutdownReason {
    /// 所有平台的 Ctrl+C。
    CtrlC,
    /// Unix SIGTERM。
    #[cfg(unix)]
    SigTerm,
}

/// 等待 Ctrl+C，Unix 同时等待 SIGTERM；注册失败返回 Signal。
///
/// # Examples
/// ```rust,no_run
/// # use axutils::tokio::*;
/// # use axutils::tokio::wait_for_shutdown;
/// # async fn example() -> Result<(), TokioError> {
/// let _reason = wait_for_shutdown().await?;
/// # Ok(()) }
/// ```
pub async fn wait_for_shutdown() -> Result<TokioShutdownReason, TokioError> {
    #[cfg(unix)]
    {
        use ::tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).map_err(TokioError::Signal)?;
        let ctrl_c = signal::ctrl_c();
        let mut ctrl_c = pin!(ctrl_c);
        poll_fn(|cx| {
            if let Poll::Ready(result) = ctrl_c.as_mut().poll(cx) {
                return Poll::Ready(
                    result
                        .map(|()| TokioShutdownReason::CtrlC)
                        .map_err(TokioError::Signal),
                );
            }
            if let Poll::Ready(Some(())) = term.poll_recv(cx) {
                return Poll::Ready(Ok(TokioShutdownReason::SigTerm));
            }
            Poll::Pending
        })
        .await
    }
    #[cfg(not(unix))]
    {
        signal::ctrl_c().await.map_err(TokioError::Signal)?;
        Ok(TokioShutdownReason::CtrlC)
    }
}
