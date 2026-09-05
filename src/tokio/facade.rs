use std::{
    future::Future,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    time::Duration,
};

use ::tokio::{
    runtime::{Handle, Runtime},
    sync::mpsc,
    task::JoinHandle,
    time::timeout as await_timeout,
};

use super::{wait_for_shutdown, TokioConfig, TokioError, TokioShutdownReason};

/// 无状态 Tokio facade；普通方法使用调用方 runtime，只有显式 build/run 创建 runtime。
pub struct TokioUtils;

impl TokioUtils {
    /// 返回当前 Handle；线程不在 runtime/EnterGuard context 时返回 RuntimeRequired。
    pub fn try_current_handle() -> Result<Handle, TokioError> {
        Handle::try_current().map_err(|_| TokioError::RuntimeRequired)
    }

    /// 判断当前线程是否位于 runtime context；不保证 owner 将长期存活。
    pub fn has_runtime() -> bool {
        Handle::try_current().is_ok()
    }

    /// 在当前 runtime 登记 Send future，缺少 context 返回 RuntimeRequired。
    pub fn spawn<F>(f: F) -> Result<JoinHandle<F::Output>, TokioError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        Ok(Self::try_current_handle()?.spawn(f))
    }

    /// 在显式 Handle 登记 Send future；返回原生 JoinHandle。
    pub fn spawn_on<F>(handle: &Handle, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        handle.spawn(future)
    }

    /// 在当前 blocking pool 登记 closure；开始执行后不能靠 timeout 或丢弃 handle 强停。
    pub fn spawn_blocking<F, T>(f: F) -> Result<JoinHandle<T>, TokioError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        Ok(Self::try_current_handle()?.spawn_blocking(f))
    }

    /// 为 future 设置等待预算；elapsed 映射为 Timeout，超时后通过丢弃 future 取消它。
    ///
    /// # Panics
    ///
    /// 与 [`tokio::time::timeout`] 相同：在没有 Tokio runtime 或没有启用 time driver 的 runtime
    /// 中创建并 poll 返回的 future 会 panic；该状态不会映射为 [`TokioError`]。
    pub async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, TokioError>
    where
        F: Future,
    {
        await_timeout(duration, future)
            .await
            .map_err(|_| TokioError::Timeout)
    }

    /// 创建容量 1..=1,000,000 的 Tokio mpsc channel。
    pub fn bounded_mpsc<T>(
        capacity: usize,
    ) -> Result<(mpsc::Sender<T>, mpsc::Receiver<T>), TokioError> {
        if !(1..=1_000_000).contains(&capacity) {
            return Err(TokioError::InvalidConfig {
                field: "channel_capacity",
            });
        }
        Ok(mpsc::channel(capacity))
    }

    /// 在 runtime context 外构建拥有型 Runtime；嵌套时返回 NestedRuntime。
    pub fn build_runtime(config: &TokioConfig) -> Result<Runtime, TokioError> {
        if Self::has_runtime() {
            return Err(TokioError::NestedRuntime);
        }
        config.builder()?.build().map_err(TokioError::RuntimeBuild)
    }

    /// 构建 runtime 并 block_on future；正常或 unwind 路径都先执行有限 shutdown，随后恢复 panic。
    pub fn run<F>(config: &TokioConfig, future: F) -> Result<F::Output, TokioError>
    where
        F: Future,
    {
        let runtime = Self::build_runtime(config)?;
        let result = catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)));
        runtime.shutdown_timeout(config.shutdown_timeout());
        match result {
            Ok(value) => Ok(value),
            Err(payload) => resume_unwind(payload),
        }
    }

    /// 等待 Ctrl+C，Unix 同时等待 SIGTERM；注册失败返回 Signal。
    pub async fn wait_for_shutdown() -> Result<TokioShutdownReason, TokioError> {
        wait_for_shutdown().await
    }
}
