//! 配置文件解析后的无类型值树。

use std::{collections::BTreeMap, fmt};

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

#[cfg(feature = "config-toml")]
use toml::value::Datetime;

/// 深度限制安全网，仅在通过标准 [`serde::Deserialize`] trait 构建 [`ConfigValue`]
/// 时使用（该场景下无法携带调用方在 [`crate::config::ConfigLoader`] 上配置的深度上限）。
/// 数值对应本 crate 允许配置的深度上限的最大值（见 [`crate::config::ConfigLoader::with_max_depth`]）。
const MAX_DEPTH_CEILING: usize = 256;

/// `toml`（经由 `toml_datetime`）在 `deserialize_any` 时把日期时间值包装为一个只有这一个
/// 字段的“伪表”来传递原始字符串表示，而不是直接调用 `visit_str`；本 crate 的 TOML 后端据此
/// 识别并还原为 [`ConfigValue::String`]，其余后端不做这一特殊处理，避免把恰好使用同名键的
/// 普通表误判为日期时间。
#[cfg(feature = "config-toml")]
pub(crate) const TOML_DATETIME_FIELD: &str = "$__toml_private_datetime";

const DEPTH_MARKER: &str = "\u{0}axutils:config:depth\u{0}";
const DUPLICATE_KEY_MARKER_PREFIX: &str = "\u{0}axutils:config:duplicate:";
const DUPLICATE_KEY_MARKER_SUFFIX: char = '\u{0}';
const OUT_OF_RANGE_MARKER_PREFIX: &str = "\u{0}axutils:config:range:";
const OUT_OF_RANGE_MARKER_SUFFIX: char = '\u{0}';

/// 标记内部深度/范围错误在通用 [`serde::de::Error::custom`] 消息中携带的分类信息。
pub(crate) enum ErrorMarker<'a> {
    /// 超过深度上限。
    DepthLimitExceeded,
    /// 同一作用域内出现重复键，附带重复键名。
    DuplicateKey(&'a str),
    /// 整数超出 `i64` 可表示范围，附带触发字段的键名（可能为空）。
    ValueOutOfRange(&'a str),
    /// 不是本 crate 注入的标记，调用方应按后端自身的错误处理。
    None,
}

/// 从后端错误的 `Display`/`message()` 文本中识别本 crate 注入的深度/范围标记。
///
/// 各解析后端可能会在自定义错误消息前后追加位置信息（例如 `"... at line 1 column 6"`），
/// 因此标记两侧都使用 NUL 字节（`\u{0}`）包裹，NUL 字节几乎不可能出现在真实错误文本中，
/// 可以在任意包裹文本中稳定提取。
pub(crate) fn classify_marker(message: &str) -> ErrorMarker<'_> {
    if message.contains(DEPTH_MARKER) {
        return ErrorMarker::DepthLimitExceeded;
    }
    if let Some(start) = message.find(DUPLICATE_KEY_MARKER_PREFIX) {
        let rest = &message[start + DUPLICATE_KEY_MARKER_PREFIX.len()..];
        if let Some(separator) = rest.find(':') {
            let key_start = separator + 1;
            if let Ok(key_length) = rest[..separator].parse::<usize>() {
                if let Some(key_end) = key_start.checked_add(key_length) {
                    if key_end <= rest.len()
                        && rest.is_char_boundary(key_end)
                        && rest[key_end..].starts_with(DUPLICATE_KEY_MARKER_SUFFIX)
                    {
                        return ErrorMarker::DuplicateKey(&rest[key_start..key_end]);
                    }
                }
            }
        }
    }
    if let Some(start) = message.find(OUT_OF_RANGE_MARKER_PREFIX) {
        let rest = &message[start + OUT_OF_RANGE_MARKER_PREFIX.len()..];
        if let Some(end) = rest.find(OUT_OF_RANGE_MARKER_SUFFIX) {
            return ErrorMarker::ValueOutOfRange(&rest[..end]);
        }
    }
    ErrorMarker::None
}

fn depth_limit_error<E: de::Error>() -> E {
    E::custom(DEPTH_MARKER)
}

pub(crate) fn duplicate_key_error_for_deserializer<E: de::Error>(key: &str) -> E {
    E::custom(format!(
        "{DUPLICATE_KEY_MARKER_PREFIX}{}:{key}{DUPLICATE_KEY_MARKER_SUFFIX}",
        key.len()
    ))
}

fn out_of_range_error<E: de::Error>(key: &str) -> E {
    E::custom(format!(
        "{OUT_OF_RANGE_MARKER_PREFIX}{key}{OUT_OF_RANGE_MARKER_SUFFIX}"
    ))
}

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
    /// [`crate::config::ConfigError::Parse`]），不会产生非有限的 `Float`；JSON/TOML 语法本身不支持
    /// 这类字面量。
    Float(f64),
    /// 字符串；TOML 的日期时间、YAML 的时间戳等无原生对应类型的标量统一保留为原始字符串。
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
        match self {
            Self::Table(value) => Some(value),
            _ => None,
        }
    }
}

/// 携带剩余深度预算与当前键标签的 [`serde::de::DeserializeSeed`]。
///
/// 用于从 JSON/TOML 等提供底层 `Deserializer` 的后端构建 [`ConfigValue`]，深度计数在
/// 每次进入数组/表时递减，为零时返回携带 [`DEPTH_MARKER`] 的错误，由调用方在捕获后映射为
/// [`crate::config::ConfigError::DepthLimitExceeded`]。
pub(crate) struct ConfigValueSeed {
    pub(crate) remaining_depth: usize,
    pub(crate) key: String,
    pub(crate) detect_toml_datetime: bool,
}

impl ConfigValueSeed {
    pub(crate) fn root(remaining_depth: usize) -> Self {
        Self {
            remaining_depth,
            key: String::new(),
            detect_toml_datetime: false,
        }
    }

    /// 与 [`ConfigValueSeed::root`] 相同，但额外识别 `toml`/`toml_datetime` 用于传递日期时间
    /// 原始表示的伪表字段（见 [`TOML_DATETIME_FIELD`]），仅供 `src/config/toml.rs` 使用。
    #[cfg(feature = "config-toml")]
    pub(crate) fn root_for_toml(remaining_depth: usize) -> Self {
        Self {
            remaining_depth,
            key: String::new(),
            detect_toml_datetime: true,
        }
    }
}

impl<'de> DeserializeSeed<'de> for ConfigValueSeed {
    type Value = ConfigValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ConfigValueVisitor {
            remaining_depth: self.remaining_depth,
            key: self.key,
            detect_toml_datetime: self.detect_toml_datetime,
        })
    }
}

/// 供无法暴露底层 `Deserializer`（因此无法使用 `ConfigValueSeed`）的后端使用；
/// 深度上限固定为 `MAX_DEPTH_CEILING`，仅作为防止栈溢出的安全网。当前仅 YAML 后端
/// 使用该实现，真正的可配置深度上限由 `serde-saphyr` 自身的 `Budget::max_depth` 强制。
impl<'de> serde::Deserialize<'de> for ConfigValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ConfigValueSeed::root(MAX_DEPTH_CEILING).deserialize(deserializer)
    }
}

struct ConfigValueVisitor {
    remaining_depth: usize,
    key: String,
    detect_toml_datetime: bool,
}

impl<'de> Visitor<'de> for ConfigValueVisitor {
    type Value = ConfigValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON/YAML/TOML value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(ConfigValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(ConfigValue::Null)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(ConfigValue::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(ConfigValue::Integer(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    fn visit_u128<E: de::Error>(self, value: u128) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(ConfigValue::Float(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(ConfigValue::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(ConfigValue::String(value))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(depth_limit_error)?;

        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(ConfigValueSeed {
            remaining_depth,
            key: self.key.clone(),
            detect_toml_datetime: self.detect_toml_datetime,
        })? {
            items.push(item);
        }
        Ok(ConfigValue::Array(items))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(depth_limit_error)?;

        let mut table = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if table.contains_key(&key) {
                return Err(duplicate_key_error_for_deserializer(&key));
            }
            let value = map.next_value_seed(ConfigValueSeed {
                remaining_depth,
                key: key.clone(),
                detect_toml_datetime: self.detect_toml_datetime,
            })?;
            table.insert(key, value);
        }

        // `toml_datetime` exposes a datetime as a one-field pseudo-table. Consume the
        // complete map before converting it so a user table containing the marker together
        // with another field cannot cause the already-read fields to be discarded.
        #[cfg(feature = "config-toml")]
        if self.detect_toml_datetime && table.len() == 1 {
            if let Some(ConfigValue::String(raw)) = table.remove(TOML_DATETIME_FIELD) {
                if raw.parse::<Datetime>().is_ok() {
                    return Ok(ConfigValue::String(raw));
                }
                table.insert(TOML_DATETIME_FIELD.to_owned(), ConfigValue::String(raw));
            }
        }
        Ok(ConfigValue::Table(table))
    }
}
