//! `serde-saphyr` 后端：YAML 文本到 [`ConfigValue`] 或调用方类型的转换。
//!
//! 有类型和无类型读取共用 [`crate::config::ConfigLoader`] 的容器深度上限，流式集合
//! （`[]`、`{}`）的扫描和后续结构解析均受此上限约束。拒绝重复键，解析错误省略源码片段。
//! 别名回放预算固定为总计最多 1,000,000 个事件、单个 anchor 最多展开 10,000 次，且回放栈
//! 深度不超过加载器的嵌套深度上限。

use serde::de::DeserializeOwned;
use serde_saphyr::{
    self as yaml, budget::BudgetBreach, granit_parser::ErrorKind, options::DuplicateKeyPolicy,
    Budget, Error as YamlError, ExternalMessageSource, Options,
};

use super::value as config_value;
use super::{error::ConfigError, value::ErrorMarker, ConfigValue};

/// 单次解析允许回放的别名事件总数。
const MAX_ALIAS_REPLAYED_EVENTS: usize = 1_000_000;
/// 单个 anchor 在一次解析中允许展开的次数。
const MAX_ALIAS_EXPANSIONS_PER_ANCHOR: usize = 10_000;

/// 使用与有类型入口相同的 YAML 预算构建配置值树。
pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    parse(text, max_depth)
}

/// 使用 loader 的资源预算反序列化 YAML，并将后端错误映射为公开分类。
pub(crate) fn parse<T: DeserializeOwned>(text: &str, max_depth: usize) -> Result<T, ConfigError> {
    yaml::from_str_with_options::<T>(text, build_options(max_depth))
        .map_err(|error| map_error(&error, max_depth))
}

/// 将 loader 的深度和别名限制叠加到后端的默认资源预算。
fn build_options(max_depth: usize) -> Options {
    // flow 扫描有独立预算，默认 255；两阶段均使用已校验的 loader 上限。
    let mut budget = Budget::default();
    budget.max_depth = max_depth;
    budget.flow_nesting_limit = max_depth;

    // 别名回放单独计费，重复键和错误片段沿用库的拒绝与脱敏策略。
    let mut options = Options::default();
    options.budget = Some(budget);
    options.alias_limits = yaml::alias_limits! {
        max_total_replayed_events: MAX_ALIAS_REPLAYED_EVENTS,
        max_replay_stack_depth: max_depth,
        max_alias_expansions_per_anchor: MAX_ALIAS_EXPANSIONS_PER_ANCHOR,
    };
    options.duplicate_keys = DuplicateKeyPolicy::Error;
    options.with_snippet = false;
    options
}

/// 保留重复键、深度和数值范围分类，普通解析错误仅返回位置。
fn map_error(error: &YamlError, max_depth: usize) -> ConfigError {
    // 别名包装记录引用位置。按内层原因分类，普通解析错误仍使用外层位置。
    let mut cause = error;
    while let YamlError::AliasError { error: inner, .. } = cause {
        cause = inner;
    }

    let marker = match cause {
        YamlError::DuplicateMappingKey { key: Some(key), .. } => ErrorMarker::DuplicateKey(key),
        // 容器预算、别名回放栈和流式扫描器分别检查 loader 的同一深度上限。
        YamlError::Budget {
            breach: BudgetBreach::Depth { .. },
            ..
        }
        | YamlError::AliasReplayStackDepthExceeded { .. } => ErrorMarker::DepthLimitExceeded,
        YamlError::ExternalMessage { source, .. } => match source.as_ref() {
            ExternalMessageSource::Parser(scan)
                if scan.kind() == &ErrorKind::RecursionLimitExceeded =>
            {
                ErrorMarker::DepthLimitExceeded
            }
            _ => ErrorMarker::None,
        },
        // Display 会转义 NUL。只从 serde 自定义消息的原始字段读取完整标记。
        YamlError::Message { msg, .. } if msg.starts_with('\0') && msg.ends_with('\0') => {
            config_value::classify_marker(msg)
        }
        _ => ErrorMarker::None,
    };

    match marker {
        ErrorMarker::DepthLimitExceeded => ConfigError::DepthLimitExceeded { limit: max_depth },
        ErrorMarker::DuplicateKey(key) => ConfigError::DuplicateKey {
            key: key.to_owned(),
        },
        ErrorMarker::ValueOutOfRange(key) => ConfigError::ValueOutOfRange {
            key: key.to_owned(),
        },
        ErrorMarker::None => {
            let location = error.location();
            ConfigError::Parse {
                format: "yaml",
                line: location
                    .map(|location| location.line() as usize)
                    .filter(|line| *line != 0),
                column: location
                    .map(|location| location.column() as usize)
                    .filter(|column| *column != 0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fmt::Debug, thread};

    use super::{self as yaml_config, MAX_ALIAS_EXPANSIONS_PER_ANCHOR};
    use crate::config::{ConfigError, ConfigFormat, ConfigLoader, ConfigValue};
    use serde::{de::DeserializeOwned, Deserialize};

    #[test]
    fn parses_typed_and_untyped_yaml() {
        let text = "server:\n  port: 8080\n  tls: true\n";
        let value = yaml_config::parse_value(text, 64).expect("parse untyped");
        assert_eq!(
            value.get("server.port").and_then(|v| v.as_i64()),
            Some(8080)
        );

        #[derive(Deserialize)]
        struct Server {
            port: u16,
            tls: bool,
        }
        #[derive(Deserialize)]
        struct Config {
            server: Server,
        }
        let typed: Config = yaml_config::parse(text, 64).expect("parse typed");
        assert_eq!(typed.server.port, 8080);
        assert!(typed.server.tls);
    }

    #[test]
    fn untyped_integer_out_of_range_reports_only_the_key() {
        let error = ConfigLoader::new()
            .parse_value("count: 9223372036854775808", ConfigFormat::Yaml)
            .unwrap_err();
        assert!(matches!(&error, ConfigError::ValueOutOfRange { key } if key == "count"));
        assert!(!error.to_string().contains("9223372036854775808"));
    }

    #[test]
    fn alias_at_a_deeper_use_site_reports_the_loader_depth_limit() {
        let text = "base: &base [1]\nnested:\n  items: *base";
        let loader = ConfigLoader::new().with_max_depth(2).unwrap();
        assert!(matches!(
            loader.parse_value(text, ConfigFormat::Yaml),
            Err(ConfigError::DepthLimitExceeded { limit: 2 })
        ));
    }

    #[test]
    fn uppercase_hex_prefix_is_text_unless_an_integer_is_requested() {
        let loader = ConfigLoader::new();
        let format = ConfigFormat::Yaml;
        assert_eq!(
            loader.parse_value("0X1F", format).unwrap().as_str(),
            Some("0X1F")
        );
        for scalar in ["0x1F", "!!int 0X1F"] {
            assert_eq!(
                loader.parse_value(scalar, format).unwrap().as_i64(),
                Some(31)
            );
        }
        assert_eq!(loader.parse::<i64>("0X1F", format).unwrap(), 31);
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_snippet() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("password: \"{secret}\"\n  bad indent: [\n");
        let error = yaml_config::parse_value(&text, 64).expect_err("invalid yaml should fail");
        assert!(matches!(error, ConfigError::Parse { format: "yaml", .. }));
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn typed_collections_distinguish_empty_collections_from_null() {
        #[derive(Debug, Deserialize)]
        struct Field<T> {
            value: T,
        }

        fn check<T: DeserializeOwned + Default + Debug + PartialEq>(empty: &str) {
            let loader = ConfigLoader::new();
            let format = ConfigFormat::Yaml;
            let text = format!("value: {empty}");
            let required = loader.parse::<Field<T>>(&text, format).unwrap();
            let optional = loader.parse::<Field<Option<T>>>(&text, format).unwrap();
            assert_eq!(required.value, T::default());
            assert_eq!(optional.value, Some(T::default()));

            for text in ["value: null", "value:", "value: ~"] {
                let error = loader.parse::<Field<T>>(text, format).unwrap_err();
                assert!(matches!(error, ConfigError::Parse { format: "yaml", .. }));
                let optional = loader.parse::<Field<Option<T>>>(text, format).unwrap();
                assert_eq!(optional.value, None);
                let untyped = loader.parse_value(text, format).unwrap();
                assert_eq!(untyped.get("value"), Some(&ConfigValue::Null));
            }
        }

        #[derive(Debug, Default, Deserialize, PartialEq)]
        struct Settings {
            retries: Option<u8>,
        }

        check::<Vec<u8>>("[]");
        check::<BTreeMap<String, u8>>("{}");
        check::<Settings>("{}");
    }

    #[test]
    fn flow_sequences_respect_the_maximum_loader_depth() {
        // 为调试构建的递归 serde 访问器预留栈空间，输入只取公开预算的相邻边界。
        thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let loader = ConfigLoader::new().with_max_depth(256).unwrap();
                let format = ConfigFormat::Yaml;
                for depth in [255, 256, 257] {
                    let text = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
                    let untyped = loader.parse_value(&text, format);
                    let typed = loader.parse::<Vec<ConfigValue>>(&text, format);
                    if depth <= 256 {
                        untyped.expect("flow sequence within loader depth");
                        typed.expect("typed flow sequence within loader depth");
                    } else {
                        for error in [untyped.unwrap_err(), typed.unwrap_err()] {
                            assert!(matches!(
                                error,
                                ConfigError::DepthLimitExceeded { limit: 256 }
                            ));
                        }
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn explicit_empty_mapping_parses_to_empty_table() {
        let value = yaml_config::parse_value("{}", 64).expect("parse empty mapping");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn non_finite_float_literals_are_rejected_for_untyped_reads() {
        // `serde-saphyr`'s default `reject_non_finite_typeless_float` option rejects `.inf`/
        // `.nan` for untyped (typeless) targets rather than silently producing a non-finite
        // `ConfigValue::Float`; this crate keeps that safer default.
        for text in ["a: .inf\n", "a: -.inf\n", "a: .nan\n"] {
            let error = yaml_config::parse_value(text, 64)
                .expect_err("non-finite float should be rejected");
            assert!(matches!(error, ConfigError::Parse { format: "yaml", .. }));
        }
    }

    #[test]
    fn rejects_duplicate_keys() {
        let text = "a: 1\na: 2\n";
        let error = yaml_config::parse_value(text, 64).expect_err("duplicate key should fail");
        assert!(matches!(
            error,
            ConfigError::DuplicateKey { key } if key == "a"
        ));
    }

    #[test]
    fn depth_exactly_at_limit_succeeds_and_one_more_level_fails() {
        let shallow = "a: 1\n";
        assert!(yaml_config::parse_value(shallow, 1).is_ok());

        let nested = "a:\n  b: 1\n";
        let error =
            yaml_config::parse_value(nested, 1).expect_err("depth 2 should exceed budget 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(yaml_config::parse_value(nested, 2).is_ok());
    }

    #[test]
    fn rejects_alias_expansion_that_exceeds_budget() {
        // Classic "billion laughs" style alias bomb: each layer aliases the previous layer nine
        // times, causing exponential event replay under the default budget.
        let text = "a: &a [1,1,1,1,1,1,1,1,1]\n\
                     b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]\n\
                     c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]\n\
                     d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]\n\
                     e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]\n\
                     f: [*e,*e,*e,*e,*e,*e,*e,*e,*e]\n";
        let result = yaml_config::parse_value(text, 64);
        assert!(result.is_err(), "alias bomb should be rejected");
    }

    #[test]
    fn rejects_excessive_expansions_of_one_anchor() {
        let aliases = vec!["*base"; MAX_ALIAS_EXPANSIONS_PER_ANCHOR + 1].join(",");
        let text = format!("base: &base value\nitems: [{aliases}]\n");
        let result = yaml_config::parse_value(&text, 64);
        assert!(
            result.is_err(),
            "one anchor should have a finite expansion limit"
        );
    }
}
