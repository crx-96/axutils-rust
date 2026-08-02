//! `ConfigValue` -> `serde::Deserializer`，供 INI 与 `.env` 的类型化读取使用。
//!
//! JSON、YAML、TOML 的类型化读取直接使用各自后端原生的 `Deserializer`，不经过本模块；
//! INI 与 `.env` 的值天然都是字符串，本模块在反序列化为布尔、整数或浮点字段时，对字符串
//! 做“宽松解析”：等价于对去除首尾空白后的字符串调用目标类型标准库的 `FromStr`
//! （`bool` 因此只接受 `"true"`/`"false"`）。该差异必须写入 API doc，因为 JSON/YAML/TOML
//! 不做这类字符串到标量的隐式转换。

use std::collections::btree_map;

use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor,
};

use super::{error::ConfigError, value::ConfigValue};

impl de::Error for ConfigError {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        // `#[derive(Deserialize)]` 生成的代码在一些回退路径上只能通过这个通用入口报告错误，
        // 消息文本本身可能包含被拒绝的值；为遵守“错误绝不回显配置值”的边界，这里丢弃消息，
        // 只返回一个通用分类。更精确的分类由下面几个结构化方法（携带字段名，而非配置值）提供。
        ConfigError::TypeMismatch {
            key: String::new(),
            expected: "a compatible value",
        }
    }

    fn missing_field(field: &'static str) -> Self {
        ConfigError::TypeMismatch {
            key: field.to_owned(),
            expected: "a present value",
        }
    }

    fn unknown_field(field: &str, _expected: &'static [&'static str]) -> Self {
        ConfigError::TypeMismatch {
            key: field.to_owned(),
            expected: "a known field",
        }
    }

    fn duplicate_field(field: &'static str) -> Self {
        ConfigError::DuplicateKey {
            key: field.to_owned(),
        }
    }
}

/// 把一个 [`ConfigValue`] 及其所在的键标签包装为 `serde::Deserializer`。
pub(crate) struct ValueDeserializer<'de> {
    value: &'de ConfigValue,
    key: String,
}

impl<'de> ValueDeserializer<'de> {
    pub(crate) fn root(value: &'de ConfigValue) -> Self {
        Self {
            value,
            key: String::new(),
        }
    }

    fn type_mismatch(&self, expected: &'static str) -> ConfigError {
        ConfigError::TypeMismatch {
            key: self.key.clone(),
            expected,
        }
    }
}

macro_rules! deserialize_lenient_int {
    ($method:ident, $visit:ident, $ty:ty, $label:literal) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            match self.value {
                ConfigValue::Integer(value) => <$ty>::try_from(*value)
                    .map_err(|_| self.type_mismatch($label))
                    .and_then(|value| visitor.$visit(value)),
                ConfigValue::String(text) => text
                    .trim()
                    .parse::<$ty>()
                    .map_err(|_| self.type_mismatch($label))
                    .and_then(|value| visitor.$visit(value)),
                _ => Err(self.type_mismatch($label)),
            }
        }
    };
}

macro_rules! deserialize_lenient_float {
    ($method:ident, $visit:ident, $ty:ty, $label:literal) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            match self.value {
                ConfigValue::Integer(value) => visitor.$visit(*value as $ty),
                ConfigValue::Float(value) => visitor.$visit(*value as $ty),
                ConfigValue::String(text) => text
                    .trim()
                    .parse::<$ty>()
                    .map_err(|_| self.type_mismatch($label))
                    .and_then(|value| visitor.$visit(value)),
                _ => Err(self.type_mismatch($label)),
            }
        }
    };
}

impl<'de> Deserializer<'de> for ValueDeserializer<'de> {
    type Error = ConfigError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Null => visitor.visit_unit(),
            ConfigValue::Bool(value) => visitor.visit_bool(*value),
            ConfigValue::Integer(value) => visitor.visit_i64(*value),
            ConfigValue::Float(value) => visitor.visit_f64(*value),
            ConfigValue::String(value) => visitor.visit_borrowed_str(value),
            ConfigValue::Array(items) => visitor.visit_seq(ValueSeqAccess {
                iter: items.iter(),
                key: self.key,
            }),
            ConfigValue::Table(table) => visitor.visit_map(ValueMapAccess {
                iter: table.iter(),
                pending: None,
            }),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Bool(value) => visitor.visit_bool(*value),
            ConfigValue::String(text) => text
                .trim()
                .parse::<bool>()
                .map_err(|_| self.type_mismatch("bool"))
                .and_then(|value| visitor.visit_bool(value)),
            _ => Err(self.type_mismatch("bool")),
        }
    }

    deserialize_lenient_int!(deserialize_i8, visit_i8, i8, "i8");
    deserialize_lenient_int!(deserialize_i16, visit_i16, i16, "i16");
    deserialize_lenient_int!(deserialize_i32, visit_i32, i32, "i32");
    deserialize_lenient_int!(deserialize_i64, visit_i64, i64, "i64");
    deserialize_lenient_int!(deserialize_i128, visit_i128, i128, "i128");
    deserialize_lenient_int!(deserialize_u8, visit_u8, u8, "u8");
    deserialize_lenient_int!(deserialize_u16, visit_u16, u16, "u16");
    deserialize_lenient_int!(deserialize_u32, visit_u32, u32, "u32");
    deserialize_lenient_int!(deserialize_u64, visit_u64, u64, "u64");
    deserialize_lenient_int!(deserialize_u128, visit_u128, u128, "u128");
    deserialize_lenient_float!(deserialize_f32, visit_f32, f32, "f32");
    deserialize_lenient_float!(deserialize_f64, visit_f64, f64, "f64");

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::String(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(only), None) => visitor.visit_char(only),
                    _ => Err(self.type_mismatch("char")),
                }
            }
            _ => Err(self.type_mismatch("char")),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::String(value) => visitor.visit_borrowed_str(value),
            _ => Err(self.type_mismatch("string")),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(self.type_mismatch("bytes"))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Null => visitor.visit_unit(),
            _ => Err(self.type_mismatch("unit")),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Array(items) => visitor.visit_seq(ValueSeqAccess {
                iter: items.iter(),
                key: self.key,
            }),
            _ => Err(self.type_mismatch("array")),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::Table(table) => visitor.visit_map(ValueMapAccess {
                iter: table.iter(),
                pending: None,
            }),
            _ => Err(self.type_mismatch("table")),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.value {
            ConfigValue::String(variant) => visitor.visit_enum(StringEnumAccess { variant }),
            _ => Err(self.type_mismatch("enum")),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_any(visitor)
    }
}

struct KeyDeserializer<'de>(&'de str);

impl<'de> Deserializer<'de> for KeyDeserializer<'de> {
    type Error = ConfigError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_borrowed_str(self.0)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_borrowed_str(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum ignored_any
    }
}

struct ValueSeqAccess<'de> {
    iter: std::slice::Iter<'de, ConfigValue>,
    key: String,
}

impl<'de> SeqAccess<'de> for ValueSeqAccess<'de> {
    type Error = ConfigError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed
                .deserialize(ValueDeserializer {
                    value,
                    key: self.key.clone(),
                })
                .map(Some),
            None => Ok(None),
        }
    }
}

struct ValueMapAccess<'de> {
    iter: btree_map::Iter<'de, String, ConfigValue>,
    pending: Option<(&'de str, &'de ConfigValue)>,
}

impl<'de> MapAccess<'de> for ValueMapAccess<'de> {
    type Error = ConfigError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.pending = Some((key.as_str(), value));
                seed.deserialize(KeyDeserializer(key.as_str())).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let Some((key, value)) = self.pending.take() else {
            return Err(ConfigError::TypeMismatch {
                key: String::new(),
                expected: "a map value after its key",
            });
        };
        seed.deserialize(ValueDeserializer {
            value,
            key: key.to_owned(),
        })
    }
}

struct StringEnumAccess<'de> {
    variant: &'de str,
}

impl<'de> EnumAccess<'de> for StringEnumAccess<'de> {
    type Error = ConfigError;
    type Variant = UnitOnlyVariantAccess;

    fn variant_seed<S>(self, seed: S) -> Result<(S::Value, Self::Variant), Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        let value = seed.deserialize(KeyDeserializer(self.variant))?;
        Ok((value, UnitOnlyVariantAccess))
    }
}

struct UnitOnlyVariantAccess;

impl<'de> VariantAccess<'de> for UnitOnlyVariantAccess {
    type Error = ConfigError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        Err(ConfigError::TypeMismatch {
            key: String::new(),
            expected: "a unit enum variant",
        })
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ConfigError::TypeMismatch {
            key: String::new(),
            expected: "a unit enum variant",
        })
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ConfigError::TypeMismatch {
            key: String::new(),
            expected: "a unit enum variant",
        })
    }
}

pub(crate) fn deserialize<T: serde::de::DeserializeOwned>(
    value: &ConfigValue,
) -> Result<T, ConfigError> {
    T::deserialize(ValueDeserializer::root(value))
}

#[cfg(test)]
mod tests {
    use super::deserialize;
    use crate::{ConfigError, ConfigValue};
    use serde::Deserialize;
    use std::collections::BTreeMap;

    fn string_table(entries: &[(&str, &str)]) -> ConfigValue {
        ConfigValue::Table(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), ConfigValue::String(v.to_string())))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Server {
        port: u16,
        tls: bool,
        ratio: f64,
        name: String,
    }

    #[test]
    fn leniently_parses_strings_into_scalars() {
        let value = string_table(&[
            ("port", "8080"),
            ("tls", "true"),
            ("ratio", "0.5"),
            ("name", "primary"),
        ]);
        let server: Server = deserialize(&value).expect("typed deserialize should succeed");
        assert_eq!(
            server,
            Server {
                port: 8080,
                tls: true,
                ratio: 0.5,
                name: "primary".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_unparsable_scalar_with_key_context() {
        let value = string_table(&[
            ("port", "not-a-number"),
            ("tls", "true"),
            ("ratio", "0.5"),
            ("name", "primary"),
        ]);
        let error = deserialize::<Server>(&value).expect_err("invalid port should fail");
        assert!(matches!(
            error,
            ConfigError::TypeMismatch { key, expected: "u16" } if key == "port"
        ));
    }

    #[test]
    fn missing_field_reports_field_name_without_leaking_other_values() {
        let secret = "s3cr3t";
        let value = string_table(&[("tls", "true"), ("ratio", "0.5"), ("name", secret)]);
        let error = deserialize::<Server>(&value).expect_err("missing port should fail");
        assert!(!error.to_string().contains(secret));
        assert!(matches!(
            error,
            ConfigError::TypeMismatch { ref key, expected: "a present value" } if key == "port"
        ));
    }

    #[test]
    fn bool_rejects_non_canonical_spellings() {
        let value = string_table(&[
            ("port", "8080"),
            ("tls", "yes"),
            ("ratio", "0.5"),
            ("name", "primary"),
        ]);
        let error = deserialize::<Server>(&value).expect_err("non-canonical bool should fail");
        assert!(matches!(
            error,
            ConfigError::TypeMismatch { key, expected: "bool" } if key == "tls"
        ));
    }

    #[derive(Deserialize, Debug, PartialEq)]
    enum Level {
        Low,
        High,
    }

    #[test]
    fn deserializes_unit_enum_variant_from_string() {
        let value = ConfigValue::String("High".to_owned());
        let level: Level = deserialize(&value).expect("enum deserialize should succeed");
        assert_eq!(level, Level::High);
    }

    #[test]
    fn deserializes_nested_table_for_struct_field() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Nested {
            server: Server,
        }
        let mut root = BTreeMap::new();
        root.insert(
            "server".to_owned(),
            string_table(&[
                ("port", "1"),
                ("tls", "false"),
                ("ratio", "1.5"),
                ("name", "n"),
            ]),
        );
        let nested: Nested =
            deserialize(&ConfigValue::Table(root)).expect("nested deserialize should succeed");
        assert_eq!(nested.server.port, 1);
    }
}
