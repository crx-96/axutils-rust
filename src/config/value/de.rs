//! 无类型配置树的 serde 访问器、容器深度预算和脱敏错误标记。

use std::{collections::BTreeMap, fmt};

use serde::{
    de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor},
    Deserialize,
};

use super::ConfigValue;

/// 深度限制安全网，仅在通过标准 [`serde::Deserialize`] trait 构建 [`ConfigValue`]
/// 时使用（该场景下无法携带调用方在 [`crate::config::ConfigLoader`] 上配置的深度上限）。
/// 数值对应本 crate 允许配置的深度上限的最大值（见 [`crate::config::ConfigLoader::with_max_depth`]）。
const MAX_DEPTH_CEILING: usize = 256;

/// 容器深度超限的内部消息标记，用于穿过后端自定义错误通道。
const DEPTH_MARKER: &str = "\u{0}axutils:config:depth\u{0}";
/// 重复键标记前缀；后续附加键的 UTF-8 字节长度及键名。
const DUPLICATE_KEY_MARKER_PREFIX: &str = "\u{0}axutils:config:duplicate:";
/// 重复键标记终止符；长度字段保证键自身的分隔字符不会截断提取。
const DUPLICATE_KEY_MARKER_SUFFIX: char = '\u{0}';
/// 整数范围错误的内部前缀；后续只附加当前键名，不附加配置值。
const OUT_OF_RANGE_MARKER_PREFIX: &str = "\u{0}axutils:config:range:";
/// 整数范围错误标记的终止符。
const OUT_OF_RANGE_MARKER_SUFFIX: char = '\u{0}';

/// 标记内部深度/范围错误在通用 [`serde::de::Error::custom`] 消息中携带的分类信息。
pub(in crate::config) enum ErrorMarker<'a> {
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
/// 因此使用带 NUL 字节的内部标记定位分类；调用方仍须丢弃未识别的原始后端消息，
/// 不能将这里接收的文本直接作为公开错误输出。
pub(in crate::config) fn classify_marker(message: &str) -> ErrorMarker<'_> {
    // 深度标记没有可变载荷，可直接识别。
    if message.contains(DEPTH_MARKER) {
        return ErrorMarker::DepthLimitExceeded;
    }
    // 按 UTF-8 字节长度提取重复键，并检查边界和终止符，避免错误切片或截断合法键。
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
    // 范围错误只携带最近字段名；找不到完整标记时交回后端通用错误分类。
    if let Some(start) = message.find(OUT_OF_RANGE_MARKER_PREFIX) {
        let rest = &message[start + OUT_OF_RANGE_MARKER_PREFIX.len()..];
        if let Some(end) = rest.find(OUT_OF_RANGE_MARKER_SUFFIX) {
            return ErrorMarker::ValueOutOfRange(&rest[..end]);
        }
    }
    ErrorMarker::None
}

/// 经 serde 自定义错误通道返回深度超限；真实预算由调用方在映射阶段补入。
fn depth_limit_error<E: de::Error>() -> E {
    E::custom(DEPTH_MARKER)
}

/// 将重复键名编码为带长度的内部错误标记，供 JSON 预检与值树构建共同使用。
pub(in crate::config) fn duplicate_key_error_for_deserializer<E: de::Error>(key: &str) -> E {
    E::custom(format!(
        "{DUPLICATE_KEY_MARKER_PREFIX}{}:{key}{DUPLICATE_KEY_MARKER_SUFFIX}",
        key.len()
    ))
}

/// 标记整数超出值树可表示范围，只传递键名而不泄露被拒绝的数值。
fn out_of_range_error<E: de::Error>(key: &str) -> E {
    E::custom(format!(
        "{OUT_OF_RANGE_MARKER_PREFIX}{key}{OUT_OF_RANGE_MARKER_SUFFIX}"
    ))
}

/// 携带剩余深度预算与当前键标签的 [`serde::de::DeserializeSeed`]。
///
/// 用于从 JSON 等提供底层 `Deserializer` 的后端构建 [`ConfigValue`]，深度计数在
/// 每次进入数组/表时递减，为零时返回携带 [`DEPTH_MARKER`] 的错误，由调用方在捕获后映射为
/// [`crate::config::ConfigError::DepthLimitExceeded`]。
pub(in crate::config) struct ConfigValueSeed {
    /// 当前节点还能进入的容器层数；标量不消耗预算。
    remaining_depth: usize,
    /// 最近的表字段名；根节点为空，数组元素沿用所属字段。
    key: String,
}

impl ConfigValueSeed {
    /// 创建根节点访问器的输入状态，使用调用方提供的容器预算。
    pub(in crate::config) fn root(remaining_depth: usize) -> Self {
        Self {
            remaining_depth,
            key: String::new(),
        }
    }
}

impl<'de> DeserializeSeed<'de> for ConfigValueSeed {
    type Value = ConfigValue;

    /// 将预算与字段上下文移交给访问器，由后端分派真实值类型。
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ConfigValueVisitor {
            remaining_depth: self.remaining_depth,
            key: self.key,
        })
    }
}

/// 供无法暴露底层 `Deserializer`（因此无法使用 `ConfigValueSeed`）的后端使用；
/// 深度上限固定为 `MAX_DEPTH_CEILING`，仅作为防止栈溢出的安全网。当前仅 YAML 后端
/// 使用该实现，真正的可配置深度上限由 `serde-saphyr` 自身的 `Budget::max_depth` 强制。
impl<'de> Deserialize<'de> for ConfigValue {
    /// 在无法注入预算的标准 serde 入口应用固定深度安全网。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ConfigValueSeed::root(MAX_DEPTH_CEILING).deserialize(deserializer)
    }
}

/// 把后端标量和容器构建为值树，并在每层递归维护预算及字段上下文。
struct ConfigValueVisitor {
    /// 当前节点剩余的容器预算，进入表或数组时递减。
    remaining_depth: usize,
    /// 整数范围错误对应的最近字段名；不含配置值。
    key: String,
}

impl<'de> Visitor<'de> for ConfigValueVisitor {
    type Value = ConfigValue;

    /// 提供不含输入内容的 serde 期望类型说明。
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON/YAML value")
    }

    /// 将后端空值保留为 Null。
    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(ConfigValue::Null)
    }

    /// 将缺失的可选值保留为 Null。
    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(ConfigValue::Null)
    }

    /// 去除可选值包装并继续处理内部节点；包装本身不消耗容器预算。
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }

    /// 保留原生布尔值，不进行数值或文本转换。
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(ConfigValue::Bool(value))
    }

    /// 保存已处于值树整数范围内的有符号数。
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(ConfigValue::Integer(value))
    }

    /// 将无符号数检查转换为 i64，溢出时返回带字段名的范围标记。
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    /// 检查宽有符号整数是否可无损保存为 i64。
    fn visit_i128<E: de::Error>(self, value: i128) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    /// 检查宽无符号整数是否可无损保存为 i64。
    fn visit_u128<E: de::Error>(self, value: u128) -> Result<Self::Value, E> {
        i64::try_from(value)
            .map(ConfigValue::Integer)
            .map_err(|_| out_of_range_error(&self.key))
    }

    /// 保存后端允许的浮点值；格式是否允许非有限值由后端负责。
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(ConfigValue::Float(value))
    }

    /// 复制借用字符串，使输出值树不再依赖输入文本生命周期。
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(ConfigValue::String(value.to_owned()))
    }

    /// 接管后端已有字符串，避免再复制一次内容。
    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(ConfigValue::String(value))
    }

    /// 在数组入口扣减预算，逐项递归并保留原顺序。
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        // 先校验当前容器，再读取子项，避免在预算耗尽后继续递归。
        let remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(depth_limit_error)?;

        // 数组元素沿用最近表字段名，错误不包含元素的实际内容。
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(ConfigValueSeed {
            remaining_depth,
            key: self.key.clone(),
        })? {
            items.push(item);
        }
        Ok(ConfigValue::Array(items))
    }

    /// 构建有序表，在读取同名键的值之前拒绝重复键，并将字段名传入子访问器。
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        // 表与数组使用同一容器预算，标量字段不再额外扣减。
        let remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(depth_limit_error)?;

        // 先查重复键，再递归处理该值，保证稳定的拒绝语义且不覆盖已有数据。
        let mut table = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if table.contains_key(&key) {
                return Err(duplicate_key_error_for_deserializer(&key));
            }
            let value = map.next_value_seed(ConfigValueSeed {
                remaining_depth,
                key: key.clone(),
            })?;
            table.insert(key, value);
        }

        Ok(ConfigValue::Table(table))
    }
}
