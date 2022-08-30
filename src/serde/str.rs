//! Implement deserialization support of [`ImmutableString`][crate::ImmutableString] for [`serde`].

use crate::{Position, RhaiError, RhaiResultOf, ERR};
use serde::de::{Deserializer, Visitor};
use std::any::type_name;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Deserializer for `ImmutableString`.
pub struct StringSliceDeserializer<'a> {
    value: &'a str,
}

impl<'a> StringSliceDeserializer<'a> {
    /// Create an `ImmutableStringDeserializer` from an `&str` reference.
    #[must_use]
    pub const fn from_str(value: &'a str) -> Self {
        Self { value }
    }
    /// Shortcut for a type conversion error.
    fn type_error<T>(&self) -> RhaiResultOf<T> {
        Err(
            ERR::ErrorMismatchOutputType(type_name::<T>().into(), "string".into(), Position::NONE)
                .into(),
        )
    }
}

impl<'de> Deserializer<'de> for &mut StringSliceDeserializer<'de> {
    type Error = RhaiError;

    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> RhaiResultOf<V::Value> {
        self.deserialize_str(v)
    }
    fn deserialize_bool<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_i8<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_i16<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_i32<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_i64<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_u8<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_u16<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_u32<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_u64<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_f32<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_f64<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_char<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_str<V: Visitor<'de>>(self, v: V) -> RhaiResultOf<V::Value> {
        // Only allow deserialization into a string.
        v.visit_borrowed_str(self.value)
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> RhaiResultOf<V::Value> {
        self.deserialize_str(visitor)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_option<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_unit<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        v: V,
    ) -> RhaiResultOf<V::Value> {
        self.deserialize_unit(v)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        v: V,
    ) -> RhaiResultOf<V::Value> {
        v.visit_newtype_struct(self)
    }
    fn deserialize_seq<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, v: V) -> RhaiResultOf<V::Value> {
        self.deserialize_seq(v)
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        v: V,
    ) -> RhaiResultOf<V::Value> {
        self.deserialize_seq(v)
    }
    fn deserialize_map<V: Visitor<'de>>(self, _: V) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        v: V,
    ) -> RhaiResultOf<V::Value> {
        self.deserialize_map(v)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _: V,
    ) -> RhaiResultOf<V::Value> {
        self.type_error()
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, v: V) -> RhaiResultOf<V::Value> {
        self.deserialize_str(v)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> RhaiResultOf<V::Value> {
        self.deserialize_any(v)
    }
}
