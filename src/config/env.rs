//! 自实现的 `.env`（dotenv）解析器。
//!
//! 首期固定支持的语法（详见 API doc）：`KEY=VALUE`，键名限定为
//! `[A-Za-z_][A-Za-z0-9_]*`；值支持无引号、单引号、双引号三种形式，仅双引号形式处理
//! `\n`/`\r`/`\t`/`\\`/`\"`/`\$` 转义并支持 `${VAR}` 插值。插值优先在当前文件中已解析的键里
//! 查找，找不到时按调用方配置决定是否回退到进程环境变量，两者都没有则返回
//! [`ConfigError::UndefinedVariable`]，不会静默替换为空字符串。这些语义与 `dotenv`/`dotenvy`
//! 存在已知差异，本 crate 不声称与其完全兼容。

use std::collections::BTreeMap;

use super::{error::ConfigError, value::ConfigValue};

pub(crate) fn parse_value(
    text: &str,
    allow_env_fallback: bool,
) -> Result<ConfigValue, ConfigError> {
    let mut scanner = Scanner::new(text);
    let mut table: BTreeMap<String, String> = BTreeMap::new();

    loop {
        skip_blank_and_comment_lines(&mut scanner);
        if scanner.eof() {
            break;
        }

        let line = scanner.line;
        consume_optional_export(&mut scanner);
        let key = parse_key(&mut scanner, line)?;
        skip_horizontal_ws(&mut scanner);
        expect_char(&mut scanner, '=', line)?;
        skip_horizontal_ws(&mut scanner);

        let value = parse_value_token(&mut scanner, &table, allow_env_fallback)?;

        if table.contains_key(&key) {
            return Err(ConfigError::DuplicateKey { key });
        }
        table.insert(key, value);

        finish_line(&mut scanner, line)?;
    }

    Ok(ConfigValue::Table(
        table
            .into_iter()
            .map(|(key, value)| (key, ConfigValue::String(value)))
            .collect(),
    ))
}

struct Scanner<'a> {
    text: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            line: 1,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.text.len()
    }

    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut chars = self.text[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }
}

fn parse_error(line: usize) -> ConfigError {
    ConfigError::Parse {
        format: "env",
        line: Some(line),
        column: None,
    }
}

fn skip_horizontal_ws(scanner: &mut Scanner) {
    while matches!(scanner.peek(), Some(' ') | Some('\t')) {
        scanner.bump();
    }
}

fn skip_blank_and_comment_lines(scanner: &mut Scanner) {
    loop {
        skip_horizontal_ws(scanner);
        match scanner.peek() {
            Some('\n') => {
                scanner.bump();
            }
            Some('\r') => {
                scanner.bump();
                if scanner.peek() == Some('\n') {
                    scanner.bump();
                }
            }
            Some('#') => {
                while !matches!(scanner.peek(), None | Some('\n')) {
                    scanner.bump();
                }
            }
            _ => break,
        }
    }
}

fn consume_optional_export(scanner: &mut Scanner) {
    if let Some(after_keyword) = scanner.text[scanner.pos..].strip_prefix("export") {
        if matches!(after_keyword.chars().next(), Some(' ') | Some('\t')) {
            for _ in 0.."export".chars().count() {
                scanner.bump();
            }
            skip_horizontal_ws(scanner);
        }
    }
}

fn parse_key(scanner: &mut Scanner, line: usize) -> Result<String, ConfigError> {
    let start = scanner.pos;
    if !matches!(scanner.peek(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_') {
        return Err(parse_error(line));
    }
    scanner.bump();
    while matches!(scanner.peek(), Some(ch) if ch.is_ascii_alphanumeric() || ch == '_') {
        scanner.bump();
    }
    Ok(scanner.text[start..scanner.pos].to_owned())
}

fn is_valid_key_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn expect_char(scanner: &mut Scanner, expected: char, line: usize) -> Result<(), ConfigError> {
    if scanner.peek() == Some(expected) {
        scanner.bump();
        Ok(())
    } else {
        Err(parse_error(line))
    }
}

fn finish_line(scanner: &mut Scanner, line: usize) -> Result<(), ConfigError> {
    skip_horizontal_ws(scanner);
    match scanner.peek() {
        None => Ok(()),
        Some('\n') => {
            scanner.bump();
            Ok(())
        }
        Some('\r') => {
            scanner.bump();
            if scanner.peek() == Some('\n') {
                scanner.bump();
            }
            Ok(())
        }
        _ => Err(parse_error(line)),
    }
}

fn parse_value_token(
    scanner: &mut Scanner,
    table: &BTreeMap<String, String>,
    allow_env_fallback: bool,
) -> Result<String, ConfigError> {
    match scanner.peek() {
        Some('"') => parse_double_quoted(scanner, table, allow_env_fallback),
        Some('\'') => parse_single_quoted(scanner),
        _ => Ok(parse_unquoted(scanner)),
    }
}

fn parse_unquoted(scanner: &mut Scanner) -> String {
    let start = scanner.pos;
    while !matches!(scanner.peek(), None | Some('\n') | Some('\r')) {
        scanner.bump();
    }
    let raw = &scanner.text[start..scanner.pos];
    let content = match raw.find(" #") {
        Some(index) => &raw[..index],
        None => raw,
    };
    content.trim_end_matches([' ', '\t']).to_owned()
}

fn parse_single_quoted(scanner: &mut Scanner) -> Result<String, ConfigError> {
    let line = scanner.line;
    scanner.bump(); // opening '
    let start = scanner.pos;
    loop {
        match scanner.peek() {
            Some('\'') => {
                let content = scanner.text[start..scanner.pos].to_owned();
                scanner.bump(); // closing '
                return Ok(content);
            }
            Some(_) => {
                scanner.bump();
            }
            None => return Err(parse_error(line)),
        }
    }
}

fn parse_double_quoted(
    scanner: &mut Scanner,
    table: &BTreeMap<String, String>,
    allow_env_fallback: bool,
) -> Result<String, ConfigError> {
    let line = scanner.line;
    scanner.bump(); // opening "
    let mut result = String::new();

    loop {
        match scanner.peek() {
            Some('"') => {
                scanner.bump();
                return Ok(result);
            }
            None => return Err(parse_error(line)),
            Some('\\') => {
                scanner.bump();
                match scanner.peek() {
                    Some('n') => {
                        result.push('\n');
                        scanner.bump();
                    }
                    Some('r') => {
                        result.push('\r');
                        scanner.bump();
                    }
                    Some('t') => {
                        result.push('\t');
                        scanner.bump();
                    }
                    Some('\\') => {
                        result.push('\\');
                        scanner.bump();
                    }
                    Some('"') => {
                        result.push('"');
                        scanner.bump();
                    }
                    Some('$') => {
                        result.push('$');
                        scanner.bump();
                    }
                    Some(other) => {
                        // Unrecognized escape sequences are not defined by the doc; keep both
                        // characters literally rather than silently dropping the backslash.
                        result.push('\\');
                        result.push(other);
                        scanner.bump();
                    }
                    None => return Err(parse_error(line)),
                }
            }
            Some('$') if scanner.peek2() == Some('{') => {
                let interpolation_line = scanner.line;
                scanner.bump(); // $
                scanner.bump(); // {
                let name_start = scanner.pos;
                while !matches!(scanner.peek(), None | Some('}')) {
                    scanner.bump();
                }
                if scanner.peek() != Some('}') {
                    return Err(parse_error(interpolation_line));
                }
                let name = scanner.text[name_start..scanner.pos].to_owned();
                if !is_valid_key_name(&name) {
                    return Err(parse_error(interpolation_line));
                }
                scanner.bump(); // }

                result.push_str(&resolve_variable(
                    &name,
                    table,
                    allow_env_fallback,
                    interpolation_line,
                )?);
            }
            Some(ch) => {
                result.push(ch);
                scanner.bump();
            }
        }
    }
}

fn resolve_variable(
    name: &str,
    table: &BTreeMap<String, String>,
    allow_env_fallback: bool,
    line: usize,
) -> Result<String, ConfigError> {
    if let Some(value) = table.get(name) {
        return Ok(value.clone());
    }
    if allow_env_fallback {
        if let Ok(value) = std::env::var(name) {
            return Ok(value);
        }
    }
    Err(ConfigError::UndefinedVariable {
        key: name.to_owned(),
        line,
    })
}

/// 序列化所有会读写进程环境变量的测试，避免与同一测试二进制内并行运行的其他测试竞争。
#[cfg(test)]
pub(crate) mod env_test_lock {
    pub(crate) static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

#[cfg(test)]
mod tests {
    use super::{env_test_lock::LOCK, parse_value};
    use crate::ConfigError;

    fn table_of(
        value: &crate::ConfigValue,
    ) -> &std::collections::BTreeMap<String, crate::ConfigValue> {
        value.as_table().expect("env should parse to a table")
    }

    #[test]
    fn empty_and_comment_only_text_parses_to_empty_table() {
        for text in ["", "\n\n", "# just a comment\n# another\n"] {
            let value = parse_value(text, false).expect("parse should succeed");
            assert_eq!(table_of(&value).len(), 0);
        }
    }

    #[test]
    fn parses_unquoted_single_and_double_quoted_values() {
        let text =
            "UNQUOTED = hello world  \nSINGLE='raw $NOT_INTERP\\n'\nDOUBLE=\"line1\\nline2\"\n";
        let value = parse_value(text, false).expect("parse should succeed");
        let table = table_of(&value);
        assert_eq!(
            table.get("UNQUOTED").and_then(|v| v.as_str()),
            Some("hello world")
        );
        assert_eq!(
            table.get("SINGLE").and_then(|v| v.as_str()),
            Some("raw $NOT_INTERP\\n")
        );
        assert_eq!(
            table.get("DOUBLE").and_then(|v| v.as_str()),
            Some("line1\nline2")
        );
    }

    #[test]
    fn strips_full_line_and_inline_comments() {
        let text = "# full line comment\nKEY=value # trailing comment\n";
        let value = parse_value(text, false).expect("parse should succeed");
        assert_eq!(
            table_of(&value).get("KEY").and_then(|v| v.as_str()),
            Some("value")
        );
    }

    #[test]
    fn supports_export_prefix() {
        let text = "export KEY=value\n";
        let value = parse_value(text, false).expect("parse should succeed");
        assert_eq!(
            table_of(&value).get("KEY").and_then(|v| v.as_str()),
            Some("value")
        );
    }

    #[test]
    fn rejects_invalid_key_names() {
        for text in ["1KEY=value\n", "KE-Y=value\n", "=value\n"] {
            assert!(matches!(
                parse_value(text, false),
                Err(ConfigError::Parse { format: "env", .. })
            ));
        }
    }

    #[test]
    fn rejects_duplicate_keys() {
        let text = "KEY=1\nKEY=2\n";
        assert!(matches!(
            parse_value(text, false),
            Err(ConfigError::DuplicateKey { key }) if key == "KEY"
        ));
    }

    #[test]
    fn interpolation_prefers_file_value_over_process_env() {
        let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let env_key = "AXUTILS_CONFIG_ENV_TEST_INTERP_FILE_PRIORITY";
        std::env::set_var(env_key, "from-process-env");
        let text = format!("BASE=file-value\nDERIVED=\"${{BASE}}\"\nENVVAR=\"${{{env_key}}}\"\n");
        let value = parse_value(&text, true).expect("parse should succeed");
        let table = table_of(&value);
        assert_eq!(
            table.get("DERIVED").and_then(|v| v.as_str()),
            Some("file-value")
        );
        assert_eq!(
            table.get("ENVVAR").and_then(|v| v.as_str()),
            Some("from-process-env")
        );
        std::env::remove_var(env_key);
    }

    #[test]
    fn interpolation_errors_when_fallback_disabled() {
        let _guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let env_key = "AXUTILS_CONFIG_ENV_TEST_INTERP_FALLBACK_DISABLED";
        std::env::set_var(env_key, "should-not-be-used");
        let text = format!("DERIVED=\"${{{env_key}}}\"\n");
        let error = parse_value(&text, false).expect_err("fallback disabled should error");
        assert!(matches!(
            error,
            ConfigError::UndefinedVariable { key, .. } if key == env_key
        ));
        std::env::remove_var(env_key);
    }

    #[test]
    fn undefined_variable_is_never_silently_empty() {
        let text = "DERIVED=\"${DOES_NOT_EXIST_ANYWHERE_AXUTILS}\"\n";
        let error = parse_value(text, true).expect_err("undefined variable should error");
        assert!(matches!(error, ConfigError::UndefinedVariable { .. }));
    }

    #[test]
    fn rejects_interpolation_names_that_are_not_valid_keys() {
        for text in ["VALUE=\"${}\"\n", "VALUE=\"${BAD-NAME}\"\n"] {
            assert!(matches!(
                parse_value(text, false),
                Err(ConfigError::Parse { format: "env", .. })
            ));
        }
    }

    #[test]
    fn only_references_keys_defined_earlier_in_the_file() {
        let text = "DERIVED=\"${LATER}\"\nLATER=value\n";
        let error = parse_value(text, false).expect_err("forward reference should error");
        assert!(matches!(
            error,
            ConfigError::UndefinedVariable { key, .. } if key == "LATER"
        ));
    }

    #[test]
    fn backslash_dollar_escapes_interpolation() {
        let text = "KEY=\"\\$NOT_INTERPOLATED\"\n";
        let value = parse_value(text, false).expect("parse should succeed");
        assert_eq!(
            table_of(&value).get("KEY").and_then(|v| v.as_str()),
            Some("$NOT_INTERPOLATED")
        );
    }
}
