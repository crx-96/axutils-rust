use super::TokioError;
use std::time::Duration;
use tokio::runtime::Builder as RuntimeBuilder;
const MAX_BLOCKING_THREADS: usize = 4_096;
const MAX_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(300);

/// Tokio runtime scheduler 类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokioRuntimeFlavor {
    /// Tokio 多线程 work-stealing scheduler。
    MultiThread,
    /// 只在调用 `block_on` 的线程推进任务的当前线程 scheduler。
    CurrentThread,
}

/// 显式 runtime 构建配置；构造和 builder 不启动线程或执行 I/O。
#[derive(Clone, Debug)]
pub struct TokioConfig {
    flavor: TokioRuntimeFlavor,
    worker_threads: Option<usize>,
    max_blocking_threads: usize,
    thread_name: Option<String>,
    enable_io: bool,
    enable_time: bool,
    shutdown_timeout: Duration,
}
impl Default for TokioConfig {
    fn default() -> Self {
        Self {
            flavor: TokioRuntimeFlavor::MultiThread,
            worker_threads: None,
            max_blocking_threads: 512,
            thread_name: Some("axutils-runtime".into()),
            enable_io: true,
            enable_time: true,
            shutdown_timeout: Duration::from_secs(30),
        }
    }
}
impl TokioConfig {
    /// 创建有限默认配置。
    ///
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// let config = TokioConfig::new();
    /// assert_eq!(config.flavor(), TokioRuntimeFlavor::MultiThread);
    /// # }
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// 选择 scheduler 类型；不会立即构建 runtime。
    ///
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// let config = TokioConfig::new().with_flavor(TokioRuntimeFlavor::CurrentThread);
    /// assert_eq!(config.flavor(), TokioRuntimeFlavor::CurrentThread);
    /// # }
    /// ```
    pub fn with_flavor(mut self, v: TokioRuntimeFlavor) -> Self {
        self.flavor = v;
        self
    }

    /// 设置多线程 worker 数；`None` 使用 Tokio 默认，显式值必须为 1..=1024。
    ///
    /// CurrentThread 与显式 worker 数组合会在 `builder` 返回错误。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// let error = TokioConfig::new().with_worker_threads(Some(0)).unwrap_err();
    /// assert!(matches!(error, TokioError::InvalidConfig{field:"worker_threads"}));
    /// # }
    /// ```
    pub fn with_worker_threads(mut self, v: Option<usize>) -> Result<Self, TokioError> {
        if v.is_some_and(|n| n == 0 || n > 1_024) {
            return Err(TokioError::InvalidConfig {
                field: "worker_threads",
            });
        }
        self.worker_threads = v;
        Ok(self)
    }

    /// 设置 blocking pool 线程上限，范围 1..=4096；不会限制单个 closure 工作量。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// assert!(TokioConfig::new().with_max_blocking_threads(4096).is_ok());
    /// # }
    /// ```
    pub fn with_max_blocking_threads(mut self, v: usize) -> Result<Self, TokioError> {
        if !(1..=MAX_BLOCKING_THREADS).contains(&v) {
            return Err(TokioError::InvalidConfig {
                field: "max_blocking_threads",
            });
        }
        self.max_blocking_threads = v;
        Ok(self)
    }

    /// 设置可选线程名；非空名称最多 64 字节且不得包含 NUL。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// let config = TokioConfig::new().with_thread_name(None).unwrap();
    /// assert_eq!(config.thread_name(), None);
    /// # }
    /// ```
    pub fn with_thread_name(mut self, v: Option<String>) -> Result<Self, TokioError> {
        if v.as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 64 || s.contains('\0'))
        {
            return Err(TokioError::InvalidConfig {
                field: "thread_name",
            });
        }
        self.thread_name = v;
        Ok(self)
    }

    /// 控制 IO driver；关闭后使用 Tokio IO API可能 panic。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert!(!TokioConfig::new().with_io_enabled(false).io_enabled()); }
    /// ```
    pub fn with_io_enabled(mut self, v: bool) -> Self {
        self.enable_io = v;
        self
    }

    /// 控制 time driver；关闭后使用 timer API 可能 panic。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert!(!TokioConfig::new().with_time_enabled(false).time_enabled()); }
    /// ```
    pub fn with_time_enabled(mut self, v: bool) -> Self {
        self.enable_time = v;
        self
    }

    /// 设置 `TokioUtils::run` 的 shutdown timeout，范围 >0 且 <=300 秒。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] {
    /// assert!(TokioConfig::new().with_shutdown_timeout(std::time::Duration::ZERO).is_err());
    /// # }
    /// ```
    pub fn with_shutdown_timeout(mut self, v: Duration) -> Result<Self, TokioError> {
        if v.is_zero() || v > MAX_SHUTDOWN_TIMEOUT {
            return Err(TokioError::InvalidConfig {
                field: "shutdown_timeout",
            });
        }
        self.shutdown_timeout = v;
        Ok(self)
    }

    /// 返回 scheduler 类型。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { let _ = TokioConfig::new().flavor(); }
    /// ```
    pub fn flavor(&self) -> TokioRuntimeFlavor {
        self.flavor
    }
    /// 返回显式 worker 数。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert_eq!(TokioConfig::new().worker_threads(), None); }
    /// ```
    pub fn worker_threads(&self) -> Option<usize> {
        self.worker_threads
    }
    /// 返回 blocking pool 线程上限。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert_eq!(TokioConfig::new().max_blocking_threads(), 512); }
    /// ```
    pub fn max_blocking_threads(&self) -> usize {
        self.max_blocking_threads
    }
    /// 返回线程名。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert_eq!(TokioConfig::new().thread_name(), Some("axutils-runtime")); }
    /// ```
    pub fn thread_name(&self) -> Option<&str> {
        self.thread_name.as_deref()
    }
    /// 返回是否启用 IO driver。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert!(TokioConfig::new().io_enabled()); }
    /// ```
    pub fn io_enabled(&self) -> bool {
        self.enable_io
    }
    /// 返回是否启用 time driver。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert!(TokioConfig::new().time_enabled()); }
    /// ```
    pub fn time_enabled(&self) -> bool {
        self.enable_time
    }
    /// 返回 runtime shutdown 预算。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { assert_eq!(TokioConfig::new().shutdown_timeout(), std::time::Duration::from_secs(30)); }
    /// ```
    pub fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    /// 创建配置完成的 Tokio Builder，不启动线程。
    ///
    /// CurrentThread 与显式 worker 数组合返回 `InvalidConfig`。
    /// # Examples
    /// ```rust
    /// # use axutils::tokio::*;
    /// # use axutils::tokio::*;
    /// # #[cfg(feature="tokio")] { let _builder = TokioConfig::new().builder().unwrap(); }
    /// ```
    pub fn builder(&self) -> Result<RuntimeBuilder, TokioError> {
        if self.flavor == TokioRuntimeFlavor::CurrentThread && self.worker_threads.is_some() {
            return Err(TokioError::InvalidConfig {
                field: "worker_threads",
            });
        }
        let mut b = match self.flavor {
            TokioRuntimeFlavor::MultiThread => RuntimeBuilder::new_multi_thread(),
            TokioRuntimeFlavor::CurrentThread => RuntimeBuilder::new_current_thread(),
        };
        if let Some(n) = self.worker_threads {
            b.worker_threads(n);
        }
        b.max_blocking_threads(self.max_blocking_threads);
        if let Some(n) = &self.thread_name {
            b.thread_name(n);
        }
        if self.enable_io {
            b.enable_io();
        }
        if self.enable_time {
            b.enable_time();
        }
        Ok(b)
    }
}
