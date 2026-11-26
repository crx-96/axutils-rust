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

/// 合并默认级别与追加规则，向调用方只返回固定的过滤配置错误。
fn build_filter(config: &LogConfig) -> Result<EnvFilter, LogError> {
    // 追加的裸级别覆盖默认级别，各 target 的优先级由 EnvFilter 处理。
    let mut directives = config.level.as_directive().to_owned();
    if let Some(extra) = config.directives.as_deref() {
        let extra =
            normalize_directives(extra).map_err(|_| LogError::InvalidConfig { field: "filter" })?;
        directives.push(',');
        directives.push_str(&extra);
    }

    EnvFilter::try_new(directives).map_err(|_| LogError::InvalidConfig { field: "filter" })
}

/// 规范化分隔符周围的空白，并拒绝会被 EnvFilter 宽松解释的级别拼写。
fn normalize_directives(value: &str) -> Result<String, ()> {
    // 保留 directive 顺序，使同一条规则的后续设置仍能覆盖前一条。
    let mut normalized = String::new();
    for (index, raw_directive) in value.split(',').enumerate() {
        let directive = raw_directive.trim();
        // 外层级别位于选择器之后。最后一个等号右侧若还有 `]`，该等号属于 span，
        // 整个选择器应原样交给 EnvFilter 解析，省略级别时由它采用 trace。
        let assignment = directive
            .rsplit_once('=')
            .filter(|(_, suffix)| !suffix.contains(']'));
        let level_or_selector = assignment.map_or(directive, |(_, level)| level.trim_start());

        // 空级别、裸 warning 和数字级别别名在 EnvFilter 中有宽松解释，本接口拒绝这些拼写。
        if level_or_selector.is_empty()
            || level_or_selector.chars().any(char::is_whitespace)
            || level_or_selector.eq_ignore_ascii_case("warning")
            || level_or_selector.parse::<usize>().is_ok_and(|n| n <= 5)
        {
            return Err(());
        }

        if index > 0 {
            normalized.push(',');
        }
        if let Some((selector, _)) = assignment {
            // 只清除外层等号两侧的空白，选择器内部仍按原有约定拒绝空白。
            let selector = selector.trim_end();
            if selector.is_empty() || selector.chars().any(char::is_whitespace) {
                return Err(());
            }
            normalized.push_str(selector);
            normalized.push('=');
        }
        normalized.push_str(level_or_selector);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{build_filter, normalize_directives, LogConfig, LogError};
    use tracing_subscriber::EnvFilter;

    #[test]
    fn rejects_invalid_level_spellings_as_filter_errors() {
        for directive in [
            "warning",
            "WARNING",
            "2",
            "off,app=2",
            "app=warning",
            "app[work]=02",
            "app[work{count=2}]=",
            "[work{count=2}] =   ",
            "app[work{count=2}]=5",
            "[work{count=2}] = 02",
            "app[work{count = 2}]=warn",
            "app[work{count=2}] = wa rn",
        ] {
            let config = LogConfig::new().with_directives(directive);
            assert!(
                matches!(
                    build_filter(&config),
                    Err(LogError::InvalidConfig { field: "filter" })
                ),
                "accepted {directive:?}"
            );
        }
    }

    #[test]
    fn preserves_named_levels_targets_and_span_selectors() {
        for level in ["off", "error", "warn", "INFO", "debug", "trace"] {
            for directive in [level.to_owned(), format!("app={level}")] {
                let actual = build_filter(&LogConfig::new().with_directives(&directive)).unwrap();
                let expected = EnvFilter::try_new(format!("info,{directive}")).unwrap();
                assert_eq!(actual.to_string(), expected.to_string());
            }
        }
        for (directive, expected) in [
            ("off,app::worker", "off,app::worker=trace"),
            ("404", "info,404=trace"),
            ("warning=debug", "info,warning=debug"),
            ("app[work{count=2}]=warn", "info,app[work{count=2}]=warn"),
            ("app[work{count=2}]", "info,app[work{count=2}]=trace"),
            ("  [work{count=0}]  ", "info,[work{count=0}]=trace"),
            (
                " off , app[work{count=2}] = WARN , [work{count=0}] ",
                "off,app[work{count=2}]=warn,[work{count=0}]=trace",
            ),
        ] {
            let actual = build_filter(&LogConfig::new().with_directives(directive)).unwrap();
            assert_eq!(
                actual.to_string(),
                EnvFilter::try_new(expected).unwrap().to_string()
            );
        }
    }

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
