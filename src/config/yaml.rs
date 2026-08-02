//! `serde-saphyr` 后端：YAML 文本到 [`ConfigValue`] 或调用方类型的转换。
//!
//! 显式设置 `Budget::max_depth` 为调用方在 [`crate::config::ConfigLoader`] 上配置的深度上限
//! （无类型与有类型读取共用同一预算，YAML 是唯一能对两条路径都做到精确、原生深度限制的后端）；
//! 显式设置重复键策略为拒绝；显式关闭 `with_snippet`，避免上游错误渲染携带源码片段。
//! 别名回放预算固定为总计最多 1,000,000 个事件、单个 anchor 最多展开 10,000 次，且回放栈
//! 深度不超过加载器的嵌套深度上限。

use serde::de::DeserializeOwned;

use super::{
    error::ConfigError,
    value::{classify_marker, ErrorMarker},
    ConfigValue,
};

const MAX_ALIAS_REPLAYED_EVENTS: usize = 1_000_000;
const MAX_ALIAS_EXPANSIONS_PER_ANCHOR: usize = 10_000;

pub(crate) fn parse_value(text: &str, max_depth: usize) -> Result<ConfigValue, ConfigError> {
    serde_saphyr::from_str_with_options::<ConfigValue>(text, build_options(max_depth))
        .map_err(|error| map_error(&error, max_depth))
}

pub(crate) fn parse<T: DeserializeOwned>(text: &str, max_depth: usize) -> Result<T, ConfigError> {
    serde_saphyr::from_str_with_options::<T>(text, build_options(max_depth))
        .map_err(|error| map_error(&error, max_depth))
}

fn build_options(max_depth: usize) -> serde_saphyr::Options {
    let mut budget = serde_saphyr::Budget::default();
    budget.max_depth = max_depth;

    let mut options = serde_saphyr::Options::default();
    options.budget = Some(budget);
    options.alias_limits = serde_saphyr::alias_limits! {
        max_total_replayed_events: MAX_ALIAS_REPLAYED_EVENTS,
        max_replay_stack_depth: max_depth,
        max_alias_expansions_per_anchor: MAX_ALIAS_EXPANSIONS_PER_ANCHOR,
    };
    options.duplicate_keys = serde_saphyr::options::DuplicateKeyPolicy::Error;
    options.with_snippet = false;
    options
}

fn map_error(error: &serde_saphyr::Error, max_depth: usize) -> ConfigError {
    if let serde_saphyr::Error::DuplicateMappingKey { key: Some(key), .. } = error {
        return ConfigError::DuplicateKey { key: key.clone() };
    }
    if let serde_saphyr::Error::Budget {
        breach: serde_saphyr::budget::BudgetBreach::Depth { .. },
        ..
    } = error
    {
        return ConfigError::DepthLimitExceeded { limit: max_depth };
    }

    match classify_marker(&error.to_string()) {
        ErrorMarker::DepthLimitExceeded => ConfigError::DepthLimitExceeded { limit: max_depth },
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
    use super::{parse, parse_value, MAX_ALIAS_EXPANSIONS_PER_ANCHOR};
    use crate::ConfigError;
    use serde::Deserialize;

    #[test]
    fn parses_typed_and_untyped_yaml() {
        let text = "server:\n  port: 8080\n  tls: true\n";
        let value = parse_value(text, 64).expect("parse untyped");
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
        let typed: Config = parse(text, 64).expect("parse typed");
        assert_eq!(typed.server.port, 8080);
        assert!(typed.server.tls);
    }

    #[test]
    fn rejects_invalid_syntax_with_location_and_no_snippet() {
        let secret = "s3cr3t-should-not-leak";
        let text = format!("password: \"{secret}\"\n  bad indent: [\n");
        let error = parse_value(&text, 64).expect_err("invalid yaml should fail");
        assert!(matches!(error, ConfigError::Parse { format: "yaml", .. }));
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn empty_document_parses_to_empty_table() {
        let value = parse_value("{}", 64).expect("parse empty mapping");
        assert_eq!(value.as_table().map(|table| table.len()), Some(0));
    }

    #[test]
    fn non_finite_float_literals_are_rejected_for_untyped_reads() {
        // `serde-saphyr`'s default `reject_non_finite_typeless_float` option rejects `.inf`/
        // `.nan` for untyped (typeless) targets rather than silently producing a non-finite
        // `ConfigValue::Float`; this crate keeps that safer default.
        for text in ["a: .inf\n", "a: -.inf\n", "a: .nan\n"] {
            let error = parse_value(text, 64).expect_err("non-finite float should be rejected");
            assert!(matches!(error, ConfigError::Parse { format: "yaml", .. }));
        }
    }

    #[test]
    fn rejects_duplicate_keys() {
        let text = "a: 1\na: 2\n";
        let error = parse_value(text, 64).expect_err("duplicate key should fail");
        assert!(matches!(
            error,
            ConfigError::DuplicateKey { key } if key == "a"
        ));
    }

    #[test]
    fn depth_exactly_at_limit_succeeds_and_one_more_level_fails() {
        let shallow = "a: 1\n";
        assert!(parse_value(shallow, 1).is_ok());

        let nested = "a:\n  b: 1\n";
        let error = parse_value(nested, 1).expect_err("depth 2 should exceed budget 1");
        assert!(matches!(
            error,
            ConfigError::DepthLimitExceeded { limit: 1 }
        ));
        assert!(parse_value(nested, 2).is_ok());
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
        let result = parse_value(text, 64);
        assert!(result.is_err(), "alias bomb should be rejected");
    }

    #[test]
    fn rejects_excessive_expansions_of_one_anchor() {
        let aliases = vec!["*base"; MAX_ALIAS_EXPANSIONS_PER_ANCHOR + 1].join(",");
        let text = format!("base: &base value\nitems: [{aliases}]\n");
        let result = parse_value(&text, 64);
        assert!(
            result.is_err(),
            "one anchor should have a finite expansion limit"
        );
    }
}
