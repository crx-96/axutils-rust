use std::sync::{Mutex, OnceLock};

use tracing::subscriber;
use tracing_subscriber::{
    fmt::writer::{BoxMakeWriter, MakeWriterExt},
    EnvFilter,
};

use super::{LogConfig, LogError};
use crate::telemetry::application as application_trace;

static INITIALIZED: OnceLock<()> = OnceLock::new();
static INITIALIZATION_LOCK: Mutex<()> = Mutex::new(());

/// 面向应用的进程级日志初始化入口。
///
/// 该类型只通过 `utils` 命名空间公开；成功调用 [`Self::init`] 后，全局 subscriber 不能 reset、
/// replace、关闭或重新配置。`init` 使用同步 formatter 和同步 writer，文件日志可能阻塞产生日志的
/// 线程。
pub struct LogUtils;

impl LogUtils {
    /// 安装本 crate 约定的全局 tracing subscriber。
    ///
    /// 路径校验、目录创建、过滤规则解析和 file appender 构造失败不会消耗初始化机会；已有本
    /// crate subscriber 时返回 [`LogError::AlreadyInitialized`]，已有其他全局 subscriber 时返回
    /// [`LogError::GlobalSubscriberAlreadySet`]。不会创建 Tokio runtime、读取 `RUST_LOG` 或提供
    /// 运行时重载。
    pub fn init(config: LogConfig) -> Result<(), LogError> {
        let _lock = INITIALIZATION_LOCK
            .lock()
            .map_err(|_| LogError::InitializationLockPoisoned)?;
        if INITIALIZED.get().is_some() {
            return Err(LogError::AlreadyInitialized);
        }
        if !config.stdout && config.file.is_none() {
            return Err(LogError::InvalidConfig { field: "output" });
        }
        let filter = build_filter(&config)?;
        let writer = match config.file.as_ref() {
            Some(file) if config.stdout => {
                BoxMakeWriter::new(std::io::stdout.and(file.appender()?))
            }
            Some(file) => BoxMakeWriter::new(file.appender()?),
            None => BoxMakeWriter::new(std::io::stdout),
        };
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_target(true)
            .with_writer(writer)
            .finish();

        subscriber::set_global_default(subscriber)
            .map_err(|_| LogError::GlobalSubscriberAlreadySet)?;
        if INITIALIZED.set(()).is_err() {
            return Err(LogError::InitializationStateCorrupted);
        }

        application_trace::record_init();
        Ok(())
    }

    /// 返回本 crate 是否已经成功安装了自己的全局 subscriber。
    ///
    /// 外部应用已经安装的 subscriber 不会被误报为 `true`。
    pub fn is_initialized() -> bool {
        INITIALIZED.get().is_some()
    }
}

fn build_filter(config: &LogConfig) -> Result<EnvFilter, LogError> {
    let mut directives = config.level.as_directive().to_owned();
    if let Some(extra) = config.directives.as_deref() {
        let extra =
            normalize_directives(extra).map_err(|_| LogError::InvalidConfig { field: "filter" })?;
        directives.push(',');
        directives.push_str(&extra);
    }

    EnvFilter::try_new(directives).map_err(|_| LogError::InvalidConfig { field: "filter" })
}

fn normalize_directives(value: &str) -> Result<String, ()> {
    let mut normalized = String::new();
    for (index, raw_directive) in value.split(',').enumerate() {
        let directive = raw_directive.trim();
        if directive.is_empty() {
            return Err(());
        }

        let directive = if let Some((target, level)) = directive.split_once('=') {
            let target = target.trim();
            let level = level.trim();
            if target.is_empty()
                || level.is_empty()
                || target.chars().any(char::is_whitespace)
                || level.chars().any(char::is_whitespace)
            {
                return Err(());
            }
            format!("{target}={level}")
        } else {
            if directive.chars().any(char::is_whitespace) {
                return Err(());
            }
            directive.to_owned()
        };

        if index > 0 {
            normalized.push(',');
        }
        normalized.push_str(&directive);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_directives;

    #[test]
    fn normalizes_directive_separator_and_assignment_whitespace() {
        assert_eq!(
            normalize_directives("  lettre = off, rustls= off  ").unwrap(),
            "lettre=off,rustls=off"
        );
        assert_eq!(normalize_directives("off").unwrap(), "off");
    }

    #[test]
    fn rejects_empty_or_internal_directive_whitespace() {
        for value in ["lettre=off,,rustls=off", "rust ls=off", "rustls=off now"] {
            assert!(normalize_directives(value).is_err(), "accepted {value:?}");
        }
    }
}
