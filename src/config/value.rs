//! 配置文件解析后的无类型值树及只读访问接口。

use std::collections::BTreeMap;

mod de;

pub(super) use de::{
    classify_marker, duplicate_key_error_for_deserializer, ConfigValueSeed, ErrorMarker,
};

/// 配置文件解析后的无类型值树。
///
/// `ConfigValue` 可能包含配置文件中的敏感信息（例如密码或令牌）；它派生 [`Debug`]
/// 仅为方便调试，调用方不应把整棵树原样写入日志或遥测系统。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ConfigValue {
    /// 空值（JSON/YAML 的 `null`）。
    Null,
    /// 布尔值。
    Bool(bool),
    /// 64 位有符号整数；解析阶段发现的、超出该范围的整数会返回
    /// [`crate::config::ConfigError::ValueOutOfRange`]，不会静默转换为浮点数。**已知限制**：JSON 后端
    /// （不启用 `serde_json` 的 `arbitrary_precision` feature）对超过 `u64::MAX`
    /// （约 1.8×10¹⁹）且不含小数点/指数的纯整数字面量，会在其自身词法阶段就退化为浮点数，
    /// 本 crate 在这种情况下无法检测到精度丢失；`i64::MAX` 到 `u64::MAX` 之间的整数字面量
    /// 不受影响，仍会被正确拒绝。
    Integer(i64),
    /// 64 位浮点数。YAML 后端的无类型读取默认拒绝 `.inf`/`.nan` 等非有限值（返回
    /// [`crate::config::ConfigError::Parse`]），不会产生非有限的 `Float`。JSON 不支持这类字面量；
    /// TOML 的显式 `inf`/`nan` 保留为对应的非有限浮点值，普通数值溢出仍返回解析错误。
    Float(f64),
    /// 字符串；TOML 的日期时间、YAML 的时间戳等无原生对应类型的标量以字符串表示。
    String(String),
    /// 数组。
    Array(Vec<ConfigValue>),
    /// 表（JSON 对象、YAML 映射、TOML 表、INI section 或 `.env` 的扁平键值集合）。
    ///
    /// 使用 [`BTreeMap`] 而非哈希表，保证键序确定、可重现，并避免哈希碰撞类拒绝服务面。
    Table(BTreeMap<String, ConfigValue>),
}

impl ConfigValue {
    /// 按点号分隔的路径依次做字段查找，例如 `"server.tls.port"`。
    ///
    /// 只做逐段的表字段查找，不支持数组下标、通配符或表达式；路径穿过非表节点，或某一段
    /// 键不存在时返回 `None`。键名本身包含点号时无法通过该方法访问，请改用
    /// [`ConfigValue::as_table`] 直接按键查找。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{config::{ConfigFormat, ConfigValue}, utils::ConfigUtils};
    ///
    /// let value =
    ///     ConfigUtils::parse_value(r#"{"server": {"port": 8080}}"#, ConfigFormat::Json).unwrap();
    /// assert_eq!(value.get("server.port").and_then(ConfigValue::as_i64), Some(8080));
    /// assert!(value.get("server.missing").is_none());
    /// ```
    pub fn get(&self, path: &str) -> Option<&ConfigValue> {
        // 逐层访问表字段；路径任一段缺失或遇到非表节点时停止，不尝试数组索引或类型转换。
        let mut current = self;
        for segment in path.split('.') {
            current = current.as_table()?.get(segment)?;
        }
        Some(current)
    }

    /// 返回值的类型名称：`"null"`、`"bool"`、`"integer"`、`"float"`、`"string"`、
    /// `"array"` 或 `"table"`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// assert_eq!(ConfigValue::Bool(true).kind(), "bool");
    /// assert_eq!(ConfigValue::Null.kind(), "null");
    /// ```
    pub fn kind(&self) -> &'static str {
        // 返回稳定的小写分类名，供调用方描述值类型而不输出实际配置内容。
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Table(_) => "table",
        }
    }

    /// 值为 [`ConfigValue::Bool`] 时返回其内容，否则返回 `None`；不做类型转换。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// assert_eq!(ConfigValue::Bool(true).as_bool(), Some(true));
    /// assert_eq!(ConfigValue::Integer(1).as_bool(), None);
    /// ```
    pub fn as_bool(&self) -> Option<bool> {
        // 仅接受原生布尔变体，字符串或数字不会被隐式转换。
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// 值为 [`ConfigValue::Integer`] 时返回其内容，否则返回 `None`；不做类型转换。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// assert_eq!(ConfigValue::Integer(42).as_i64(), Some(42));
    /// assert_eq!(ConfigValue::Float(1.5).as_i64(), None);
    /// ```
    pub fn as_i64(&self) -> Option<i64> {
        // 只读取整数变体，不截断浮点数或解析字符串。
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// 值为 [`ConfigValue::Float`] 时返回其内容，否则返回 `None`；不做类型转换，
    /// 整数值不会被隐式加宽为浮点数。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// assert_eq!(ConfigValue::Float(1.5).as_f64(), Some(1.5));
    /// assert_eq!(ConfigValue::Integer(1).as_f64(), None);
    /// ```
    pub fn as_f64(&self) -> Option<f64> {
        // 仅返回浮点变体，避免把整数转换为可能失去精度的浮点数。
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// 值为 [`ConfigValue::String`] 时返回其内容，否则返回 `None`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// assert_eq!(ConfigValue::String("x".to_owned()).as_str(), Some("x"));
    /// assert_eq!(ConfigValue::Bool(true).as_str(), None);
    /// ```
    pub fn as_str(&self) -> Option<&str> {
        // 借用原字符串；其他变体不做格式化，也不分配额外内容。
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// 值为 [`ConfigValue::Array`] 时返回其内容，否则返回 `None`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::config::ConfigValue;
    ///
    /// let array = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
    /// assert_eq!(array.as_array().map(<[_]>::len), Some(1));
    /// assert_eq!(ConfigValue::Bool(true).as_array(), None);
    /// ```
    pub fn as_array(&self) -> Option<&[ConfigValue]> {
        // 仅开放只读切片，元素及其内存仍由当前值树持有。
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    /// 值为 [`ConfigValue::Table`] 时返回其内容，否则返回 `None`。
    ///
    /// # Examples
    ///
    /// ```
    /// use axutils::{config::ConfigFormat, utils::ConfigUtils};
    ///
    /// let value = ConfigUtils::parse_value(r#"{"a": 1}"#, ConfigFormat::Json).unwrap();
    /// assert_eq!(value.as_table().map(|table| table.len()), Some(1));
    /// ```
    pub fn as_table(&self) -> Option<&BTreeMap<String, ConfigValue>> {
        // 借用有序表供字段枚举，不复制键和值。
        match self {
            Self::Table(value) => Some(value),
            _ => None,
        }
    }
}
