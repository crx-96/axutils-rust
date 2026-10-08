//! Header 容器和安全合并逻辑。

use std::fmt;

use super::HttpError;

/// 单个 header 集合允许的条目总数，重复项也计数。
const MAX_HEADER_ENTRIES: usize = 128;
/// 单个 header 名称的 ASCII 字节上限。
const MAX_HEADER_NAME_BYTES: usize = 256;
/// 单个 header 值的原始字节上限。
const MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;
/// 集合内全部名称和值的总字节上限。
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// 已验证的内部 header 项；不实现 Debug 以避免值被意外展示。
#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct HeaderEntry {
    /// 已转为 ASCII 小写的合法 header token。
    pub(super) name: String,
    /// 已通过控制字符与字节数校验的原始 header 值。
    pub(super) value: Vec<u8>,
}

/// 保留重复项顺序的 HTTP Header 集合。
///
/// Header 名称按 ASCII 不区分大小写处理；值以字节保存，因此不会因强制 UTF-8 转换而
/// 损坏合法的扩展 Header。`Authorization` 和 `Cookie` 不允许通过 `append` 形成重复项，
/// 也不允许在客户端默认 Header 与请求 Header 之间静默合并。
#[derive(Clone, Default, Eq, Hash, PartialEq)]
pub struct HttpHeaders {
    /// 按插入顺序保存的 header 项，同名普通 header 可重复。
    entries: Vec<HeaderEntry>,
    /// 所有名称和值的字节数之和，随增删同步维护。
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
        // 在临时副本中移除旧项并检查新增预算，失败时保留原集合。
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
        // 普通 header 可以重复；敏感项必须唯一，不能靠拼接改变认证或 cookie 语义。
        let normalized = validate_name(name.as_ref())?;
        let value = validate_value(value.as_ref())?;
        if is_sensitive_name(&normalized) && self.contains_normalized(&normalized) {
            return Err(HttpError::DuplicateSensitiveHeader);
        }
        self.push_entry(normalized, value)
    }

    /// 删除所有同名 Header，返回是否删除了条目。
    pub fn remove(&mut self, name: impl AsRef<[u8]>) -> bool {
        // 非法名称不可能已存在，直接返回未删除；合法名称按规范化形式比较。
        let Ok(normalized) = validate_name(name.as_ref()) else {
            return false;
        };
        self.remove_normalized(&normalized)
    }

    /// 检查是否存在同名 Header。
    pub fn contains(&self, name: impl AsRef<[u8]>) -> bool {
        // 非法输入视为不存在，避免为查询接口新增错误返回。
        let Ok(normalized) = validate_name(name.as_ref()) else {
            return false;
        };
        self.contains_normalized(&normalized)
    }

    /// 返回第一个同名 Header 值。
    pub fn get(&self, name: impl AsRef<[u8]>) -> Option<&[u8]> {
        // 只返回第一个同名值；调用方需要全部值时可按顺序遍历。
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

    /// 借用有序内部项，供传输编码和 single-flight key 生成使用。
    pub(super) fn entries(&self) -> &[HeaderEntry] {
        &self.entries
    }

    /// 收集响应 header；保留合法的重复 Set-Cookie 等项，同时执行同样的大小限制。
    pub(super) fn append_internal(
        &mut self,
        name: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<(), HttpError> {
        let normalized = validate_name(name.as_ref())?;
        let value = validate_value(value.as_ref())?;
        self.push_entry(normalized, value)
    }

    /// 用请求普通 header 整组覆盖默认同名项，敏感项冲突则失败。
    pub(super) fn merge(defaults: &Self, request: &Self) -> Result<Self, HttpError> {
        // 在独立集合中应用覆盖，既保留请求重复项顺序，也不修改两侧原始输入。
        let mut merged = defaults.clone();
        let mut replaced = Vec::<String>::new();
        for entry in &request.entries {
            if is_sensitive_name(&entry.name) {
                // 对认证、cookie 等项拒绝默认/请求之间的含混合并。
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

    /// 为跨源请求复制非敏感默认 header，并重建总字节计数。
    pub(super) fn without_sensitive(&self) -> Self {
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

    /// 返回名称和值的累计字节数，不包含传输层分隔符开销。
    pub(super) fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// 在条目数及总字节预算允许时提交一个已校验项，否则保持集合不变。
    fn push_entry(&mut self, name: String, value: Vec<u8>) -> Result<(), HttpError> {
        // 先用饱和计算检查预算，再更新计数和容器，避免部分提交或算术溢出。
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

    /// 检查已经规范化的小写名称，不重复分配或解析名称。
    fn contains_normalized(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == name)
    }

    /// 删除全部同名项并重新计算字节预算；返回是否发生变化。
    fn remove_normalized(&mut self, name: &str) -> bool {
        // 总量最多 128 项，直接重算可以保持重复项删除后的计数不变量清楚。
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
    /// 调试输出仅展示数量和大小，所有名称和值均不回显。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpHeaders")
            .field("len", &self.entries.len())
            .field("total_bytes", &self.total_bytes)
            .finish()
    }
}

/// 标识本库禁止重复/跨源默认传播的敏感 header；输入必须已转为小写。
pub(super) fn is_sensitive_name(name: &str) -> bool {
    matches!(name, "authorization" | "cookie" | "set-cookie")
}

/// 校验 ASCII token 和长度，返回用于不区分大小写比较的小写名称。
fn validate_name(name: &[u8]) -> Result<String, HttpError> {
    // token 校验同时保证 ASCII，因此下面的 UTF-8 转换具有已验证的不变量。
    if name.is_empty()
        || name.len() > MAX_HEADER_NAME_BYTES
        || !name.iter().copied().all(is_token_byte)
    {
        return Err(HttpError::InvalidHeaderName);
    }
    Ok(String::from_utf8(name.to_ascii_lowercase()).expect("HTTP token is ASCII"))
}

/// 限制 header 值大小并拒绝控制字符，保留合法的非 UTF-8 扩展字节。
fn validate_value(value: &[u8]) -> Result<Vec<u8>, HttpError> {
    // 水平 TAB 是允许的 header 空白；CR/LF/NUL/DEL 等均不能进入传输层。
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

/// 判断一个字节是否属于 HTTP 名称 token 的 ASCII 字符集合。
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
