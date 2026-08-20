use crate::tokio::{wait_for_shutdown, TokioConfig, TokioError, TokioShutdownReason};
use ::tokio::{
    runtime::{Handle, Runtime},
    sync::mpsc,
    task::JoinHandle,
};
use std::{
    future::Future,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    time::Duration,
};

/// 无状态 Tokio facade；普通方法使用调用方 runtime，只有显式 build/run 创建 runtime。
pub struct TokioUtils;
impl TokioUtils {
    /// 返回当前 Handle；线程不在 runtime/EnterGuard context 时返回 RuntimeRequired。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] { assert!(matches!(axutils::TokioUtils::try_current_handle(), Err(axutils::TokioError::RuntimeRequired))); }
    /// ```
    pub fn try_current_handle() -> Result<Handle, TokioError> {
        Handle::try_current().map_err(|_| TokioError::RuntimeRequired)
    }

    /// 判断当前线程是否位于 runtime context；不保证 owner 将长期存活。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] { assert!(!axutils::TokioUtils::has_runtime()); }
    /// ```
    pub fn has_runtime() -> bool {
        Handle::try_current().is_ok()
    }

    /// 在当前 runtime 登记 Send future，缺少 context 返回 RuntimeRequired。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] {
    /// let value=axutils::TokioUtils::run(&axutils::TokioConfig::new(),async{axutils::TokioUtils::spawn(async{1}).unwrap().await.unwrap()}).unwrap(); assert_eq!(value,1);
    /// # }
    /// ```
    pub fn spawn<F>(f: F) -> Result<JoinHandle<F::Output>, TokioError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        Ok(Self::try_current_handle()?.spawn(f))
    }

    /// 在显式 Handle 登记 Send future；返回原生 JoinHandle。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] {
    /// let value=axutils::TokioUtils::run(&axutils::TokioConfig::new(),async{let h=tokio::runtime::Handle::current();axutils::TokioUtils::spawn_on(&h,async{2}).await.unwrap()}).unwrap();assert_eq!(value,2);
    /// # }
    /// ```
    pub fn spawn_on<F>(h: &Handle, f: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        h.spawn(f)
    }

    /// 在当前 blocking pool 登记 closure；开始执行后不能靠 future timeout 或丢弃 handle 强停。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] {
    /// let value=axutils::TokioUtils::run(&axutils::TokioConfig::new(),async{axutils::TokioUtils::spawn_blocking(||3).unwrap().await.unwrap()}).unwrap();assert_eq!(value,3);
    /// # }
    /// ```
    pub fn spawn_blocking<F, T>(f: F) -> Result<JoinHandle<T>, TokioError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        Ok(Self::try_current_handle()?.spawn_blocking(f))
    }

    /// 为 future 设置等待预算；elapsed 映射为 Timeout，丢弃 future 只具备其自身取消语义。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] {
    /// let result=axutils::TokioUtils::run(&axutils::TokioConfig::new(),async{axutils::TokioUtils::timeout(std::time::Duration::ZERO,std::future::pending::<()>()).await}).unwrap();assert!(matches!(result,Err(axutils::TokioError::Timeout)));
    /// # }
    /// ```
    pub async fn timeout<F>(d: Duration, f: F) -> Result<F::Output, TokioError>
    where
        F: Future,
    {
        ::tokio::time::timeout(d, f)
            .await
            .map_err(|_| TokioError::Timeout)
    }

    /// 创建容量 1..=1,000,000 的 Tokio mpsc channel。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] { assert!(matches!(axutils::TokioUtils::bounded_mpsc::<u8>(0),Err(axutils::TokioError::InvalidConfig{field:"channel_capacity"}))); }
    /// ```
    pub fn bounded_mpsc<T>(n: usize) -> Result<(mpsc::Sender<T>, mpsc::Receiver<T>), TokioError> {
        if !(1..=1_000_000).contains(&n) {
            return Err(TokioError::InvalidConfig {
                field: "channel_capacity",
            });
        }
        Ok(mpsc::channel(n))
    }

    /// 在 runtime context 外构建拥有型 Runtime；嵌套时返回 NestedRuntime。
    ///
    /// 调用方必须在 async context 外 drop，或使用原生 shutdown API。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] { let runtime=axutils::TokioUtils::build_runtime(&axutils::TokioConfig::new()).unwrap();runtime.shutdown_background(); }
    /// ```
    pub fn build_runtime(c: &TokioConfig) -> Result<Runtime, TokioError> {
        if Self::has_runtime() {
            return Err(TokioError::NestedRuntime);
        }
        c.builder()?.build().map_err(TokioError::RuntimeBuild)
    }

    /// 构建 runtime 并 block_on future；正常或 unwind 路径都先执行有限 shutdown，随后恢复 panic。
    /// # Examples
    /// ```rust
    /// # #[cfg(feature="tokio")] { assert_eq!(axutils::TokioUtils::run(&axutils::TokioConfig::new(),async{4}).unwrap(),4); }
    /// ```
    pub fn run<F>(c: &TokioConfig, f: F) -> Result<F::Output, TokioError>
    where
        F: Future,
    {
        let rt = Self::build_runtime(c)?;
        let r = catch_unwind(AssertUnwindSafe(|| rt.block_on(f)));
        rt.shutdown_timeout(c.shutdown_timeout());
        match r {
            Ok(v) => Ok(v),
            Err(p) => resume_unwind(p),
        }
    }

    /// 等待 Ctrl+C，Unix 同时等待 SIGTERM；注册失败返回 Signal。
    /// # Examples
    /// ```rust,no_run
    /// # async fn example()->Result<(),axutils::TokioError>{ let _=axutils::TokioUtils::wait_for_shutdown().await?;Ok(()) }
    /// ```
    pub async fn wait_for_shutdown() -> Result<TokioShutdownReason, TokioError> {
        wait_for_shutdown().await
    }
}
