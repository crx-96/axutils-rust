//! Header 容器和安全合并逻辑。

use std::fmt;

use super::HttpError;

const MAX_HEADER_ENTRIES: usize = 128;
const MAX_HEADER_NAME_BYTES: usize = 256;
const MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;
const MAX_HEADER_BYTES: usize = 64 * 1024;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct HeaderEntry {
    pub(crate) name: String,
    pub(crate) value: Vec<u8>,
}

/// 保留重复项顺序的 HTTP Header 集合。
///
/// Header 名称按 ASCII 不区分大小写处理；值以字节保存，因此不会因强制 UTF-8 转换而
/// 损坏合法的扩展 Header。`Authorization` 和 `Cookie` 不允许通过 `append` 形成重复项，
/// 也不允许在客户端默认 Header 与请求 Header 之间静默合并。
#[derive(Clone, Default, Eq, Hash, PartialEq)]
pub struct HttpHeaders {
    entries: Vec<HeaderEntry>,
    total_bytes: usize,
}

impl HttpHeaders {
    /// 创建空 Header 集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建带有预分配容量的 Header 集合。
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity.min(MAX_HEADER_ENTRIES)),
            total_bytes: 0,
        }
    }

    /// 返回 Header 条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 返回是否没有 Header。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 设置 Header；同名的旧条目会被全部替换。
    pub fn set(
        &mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<(), HttpError> {
        let normalized = validate_name(name.as_ref())?;
        let value = validate_value(value.as_ref())?;
        let mut next = self.clone();
        next.remove_normalized(&normalized);
        next.push_entry(normalized, value)?;
        *self = next;
        Ok(())
    }

    /// 追加一个 Header 条目，并保留其相对顺序。
    pub fn append(
        &mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<(), HttpError> {
        let normalized = validate_name(name.as_ref())?;
        let value = validate_value(value.as_ref())?;
        if is_sensitive_name(&normalized) && self.contains_normalized(&normalized) {
            return Err(HttpError::DuplicateSensitiveHeader);
        }
        self.push_entry(normalized, value)
    }

    /// 删除所有同名 Header，返回是否删除了条目。
    pub fn remove(&mut self, name: impl AsRef<[u8]>) -> bool {
        let Ok(normalized) = validate_name(name.as_ref()) else {
            return false;
        };
        self.remove_normalized(&normalized)
    }

    /// 检查是否存在同名 Header。
    pub fn contains(&self, name: impl AsRef<[u8]>) -> bool {
        let Ok(normalized) = validate_name(name.as_ref()) else {
            return false;
        };
        self.contains_normalized(&normalized)
    }

    /// 返回第一个同名 Header 值。
    pub fn get(&self, name: impl AsRef<[u8]>) -> Option<&[u8]> {
        let normalized = validate_name(name.as_ref()).ok()?;
        self.entries
            .iter()
            .find(|entry| entry.name == normalized)
            .map(|entry| entry.value.as_slice())
    }

    /// 按插入顺序遍历 Header 名称和值。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.value.as_slice()))
    }

    pub(crate) fn entries(&self) -> &[HeaderEntry] {
        &self.entries
    }

    pub(crate) fn append_internal(
        &mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<(), HttpError> {
        let normalized = validate_name(name.as_ref())?;
        let value = validate_value(value.as_ref())?;
        self.push_entry(normalized, value)
    }

    pub(crate) fn merge(defaults: &Self, request: &Self) -> Result<Self, HttpError> {
        let mut merged = defaults.clone();
        let mut replaced = Vec::<String>::new();
        for entry in &request.entries {
            if is_sensitive_name(&entry.name) {
                if defaults.contains_normalized(&entry.name) {
                    return Err(HttpError::DuplicateSensitiveHeader);
                }
                if merged.contains_normalized(&entry.name) {
                    return Err(HttpError::DuplicateSensitiveHeader);
                }
            } else if !replaced.iter().any(|name| name == &entry.name) {
                merged.remove_normalized(&entry.name);
                replaced.push(entry.name.clone());
            }
            merged.push_entry(entry.name.clone(), entry.value.clone())?;
        }
        Ok(merged)
    }

    pub(crate) fn without_sensitive(&self) -> Self {
        let entries = self
            .entries
            .iter()
            .filter(|entry| !is_sensitive_name(&entry.name))
            .cloned()
            .collect::<Vec<_>>();
        let total_bytes = entries
            .iter()
            .map(|entry| entry.name.len() + entry.value.len())
            .sum();
        Self {
            entries,
            total_bytes,
        }
    }

    pub(crate) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn push_entry(&mut self, name: String, value: Vec<u8>) -> Result<(), HttpError> {
        if self.entries.len() >= MAX_HEADER_ENTRIES
            || self
                .total_bytes
                .saturating_add(name.len())
                .saturating_add(value.len())
                > MAX_HEADER_BYTES
        {
            return Err(HttpError::HeaderLimitExceeded);
        }
        self.total_bytes += name.len() + value.len();
        self.entries.push(HeaderEntry { name, value });
        Ok(())
    }

    fn contains_normalized(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == name)
    }

    fn remove_normalized(&mut self, name: &str) -> bool {
        let original_len = self.entries.len();
        self.entries.retain(|entry| entry.name != name);
        if original_len != self.entries.len() {
            self.total_bytes = self
                .entries
                .iter()
                .map(|entry| entry.name.len() + entry.value.len())
                .sum();
            true
        } else {
            false
        }
    }
}

impl fmt::Debug for HttpHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpHeaders")
            .field("len", &self.entries.len())
            .field("total_bytes", &self.total_bytes)
            .finish()
    }
}

pub(crate) fn is_sensitive_name(name: &str) -> bool {
    matches!(name, "authorization" | "cookie" | "set-cookie")
}

fn validate_name(name: &[u8]) -> Result<String, HttpError> {
    if name.is_empty()
        || name.len() > MAX_HEADER_NAME_BYTES
        || !name.iter().copied().all(is_token_byte)
    {
        return Err(HttpError::InvalidHeaderName);
    }
    Ok(String::from_utf8(name.to_ascii_lowercase()).expect("HTTP token is ASCII"))
}

fn validate_value(value: &[u8]) -> Result<Vec<u8>, HttpError> {
    if value.len() > MAX_HEADER_VALUE_BYTES
        || value
            .iter()
            .copied()
            .any(|byte| byte < 0x20 && byte != b'\t' || byte == 0x7f)
    {
        return Err(HttpError::InvalidHeaderValue);
    }
    Ok(value.to_vec())
}

fn is_token_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'0'..=b'9'
            | b'a'..=b'z'
            | b'A'..=b'Z'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
    )
}
