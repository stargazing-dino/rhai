//! Implement serialization support of [`Dynamic`][crate::Dynamic] for [`serde`].

use crate::{Dynamic, Position, RhaiError, RhaiResult, RhaiResultOf, ERR};
use serde::ser::{
    Error, SerializeMap, SerializeSeq, SerializeStruct, SerializeTuple, SerializeTupleStruct,
};
use serde::{Serialize, Serializer};
use std::fmt;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;

/// Serializer for [`Dynamic`][crate::Dynamic] which is kept as a reference.
struct DynamicSerializer {
    /// Buffer to hold a temporary key.
    _key: Dynamic,
    /// Buffer to hold a temporary value.
    _value: Dynamic,
}

impl DynamicSerializer {
    /// Create a [`DynamicSerializer`] from a [`Dynamic`][crate::Dynamic] value.
    #[must_use]
    pub const fn new(_value: Dynamic) -> Self {
        Self {
            _key: Dynamic::UNIT,
            _value,
        }
    }
}

/// Serialize a Rust type that implements [`serde::Serialize`] into a [`Dynamic`][crate::Dynamic].
///
/// # Example
///
/// ```
/// # fn main() -> Result<(), Box<rhai::EvalAltResult>> {
/// # #[cfg(not(feature = "no_index"))]
/// # #[cfg(not(feature = "no_object"))]
/// # #[cfg(not(feature = "no_float"))]
/// # {
/// use rhai::{Dynamic, Array, Map, INT};
/// use rhai::serde::to_dynamic;
/// use serde::Serialize;
///
/// #[derive(Debug, serde::Serialize, PartialEq)]
/// struct Point {
///     x: f64,
///     y: f64
/// }
///
/// #[derive(Debug, serde::Serialize, PartialEq)]
/// struct MyStruct {
///     a: i64,
///     b: Vec<String>,
///     c: bool,
///     d: Point
/// }
///
/// let x = MyStruct {
///     a: 42,
///     b: vec![ "hello".into(), "world".into() ],
///     c: true,
///     d: Point { x: 123.456, y: 999.0 }
/// };
///
/// // Convert the 'MyStruct' into a 'Dynamic'
/// let value = to_dynamic(x)?;
///
/// assert!(value.is::<Map>());
///
/// let map = value.cast::<Map>();
/// let point = map["d"].read_lock::<Map>().unwrap();
/// assert_eq!(*point["x"].read_lock::<f64>().unwrap(), 123.456);
/// assert_eq!(*point["y"].read_lock::<f64>().unwrap(), 999.0);
/// # }
/// # Ok(())
/// # }
/// ```
pub fn to_dynamic<T: Serialize>(value: T) -> RhaiResult {
    let mut s = DynamicSerializer::new(Dynamic::UNIT);
    value.serialize(&mut s)
}

impl Error for RhaiError {
    fn custom<T: fmt::Display>(err: T) -> Self {
        ERR::ErrorRuntime(err.to_string().into(), Position::NONE).into()
    }
}

impl Serializer for &mut DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;
    type SerializeSeq = DynamicSerializer;
    type SerializeTuple = DynamicSerializer;
    type SerializeTupleStruct = DynamicSerializer;
    #[cfg(not(feature = "no_object"))]
    #[cfg(not(feature = "no_index"))]
    type SerializeTupleVariant = TupleVariantSerializer;
    #[cfg(any(feature = "no_object", feature = "no_index"))]
    type SerializeTupleVariant = serde::ser::Impossible<Dynamic, RhaiError>;
    type SerializeMap = DynamicSerializer;
    type SerializeStruct = DynamicSerializer;
    #[cfg(not(feature = "no_object"))]
    type SerializeStructVariant = StructVariantSerializer;
    #[cfg(feature = "no_object")]
    type SerializeStructVariant = serde::ser::Impossible<Dynamic, RhaiError>;

    fn serialize_bool(self, v: bool) -> RhaiResultOf<Self::Ok> {
        Ok(v.into())
    }

    fn serialize_i8(self, v: i8) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        return self.serialize_i64(i64::from(v));
        #[cfg(feature = "only_i32")]
        return self.serialize_i32(i32::from(v));
    }

    fn serialize_i16(self, v: i16) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        return self.serialize_i64(i64::from(v));
        #[cfg(feature = "only_i32")]
        return self.serialize_i32(i32::from(v));
    }

    fn serialize_i32(self, v: i32) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        return self.serialize_i64(i64::from(v));
        #[cfg(feature = "only_i32")]
        return Ok(v.into());
    }

    fn serialize_i64(self, v: i64) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        {
            Ok(v.into())
        }
        #[cfg(feature = "only_i32")]
        if v > i32::MAX as i64 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i32(v as i32)
        }
    }

    fn serialize_i128(self, v: i128) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        if v > i64::MAX as i128 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i64(v as i64)
        }
        #[cfg(feature = "only_i32")]
        if v > i32::MAX as i128 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i32(v as i32)
        }
    }

    fn serialize_u8(self, v: u8) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        return self.serialize_i64(i64::from(v));
        #[cfg(feature = "only_i32")]
        return self.serialize_i32(i32::from(v));
    }

    fn serialize_u16(self, v: u16) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        return self.serialize_i64(i64::from(v));
        #[cfg(feature = "only_i32")]
        return self.serialize_i32(i32::from(v));
    }

    fn serialize_u32(self, v: u32) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        {
            self.serialize_i64(i64::from(v))
        }
        #[cfg(feature = "only_i32")]
        if v > i32::MAX as u32 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i32(v as i32)
        }
    }

    fn serialize_u64(self, v: u64) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        if v > i64::MAX as u64 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i64(v as i64)
        }
        #[cfg(feature = "only_i32")]
        if v > i32::MAX as u64 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i32(v as i32)
        }
    }

    fn serialize_u128(self, v: u128) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "only_i32"))]
        if v > i64::MAX as u128 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i64(v as i64)
        }
        #[cfg(feature = "only_i32")]
        if v > i32::MAX as u128 {
            Ok(Dynamic::from(v))
        } else {
            self.serialize_i32(v as i32)
        }
    }

    fn serialize_f32(self, v: f32) -> RhaiResultOf<Self::Ok> {
        #[cfg(any(not(feature = "no_float"), not(feature = "decimal")))]
        return Ok(Dynamic::from(v));

        #[cfg(feature = "no_float")]
        #[cfg(feature = "decimal")]
        {
            use rust_decimal::Decimal;
            use std::convert::TryFrom;

            Decimal::try_from(v)
                .map(|v| v.into())
                .map_err(Error::custom)
        }
    }

    fn serialize_f64(self, v: f64) -> RhaiResultOf<Self::Ok> {
        #[cfg(any(not(feature = "no_float"), not(feature = "decimal")))]
        return Ok(Dynamic::from(v));

        #[cfg(feature = "no_float")]
        #[cfg(feature = "decimal")]
        {
            use rust_decimal::Decimal;
            use std::convert::TryFrom;

            Decimal::try_from(v)
                .map(|v| v.into())
                .map_err(Error::custom)
        }
    }

    fn serialize_char(self, v: char) -> RhaiResultOf<Self::Ok> {
        Ok(v.into())
    }

    fn serialize_str(self, v: &str) -> RhaiResultOf<Self::Ok> {
        Ok(v.into())
    }

    fn serialize_bytes(self, _v: &[u8]) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_index"))]
        return Ok(Dynamic::from_blob(_v.to_vec()));

        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "BLOB's are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_none(self) -> RhaiResultOf<Self::Ok> {
        Ok(Dynamic::UNIT)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> RhaiResultOf<Self::Ok> {
        value.serialize(&mut *self)
    }

    fn serialize_unit(self) -> RhaiResultOf<Self::Ok> {
        Ok(Dynamic::UNIT)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> RhaiResultOf<Self::Ok> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> RhaiResultOf<Self::Ok> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> RhaiResultOf<Self::Ok> {
        value.serialize(&mut *self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_object"))]
        {
            let content = to_dynamic(_value)?;
            make_variant(_variant, content)
        }
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_seq(self, _len: Option<usize>) -> RhaiResultOf<Self::SerializeSeq> {
        #[cfg(not(feature = "no_index"))]
        return Ok(DynamicSerializer::new(crate::Array::new().into()));
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "arrays are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_tuple(self, len: usize) -> RhaiResultOf<Self::SerializeTuple> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> RhaiResultOf<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> RhaiResultOf<Self::SerializeTupleVariant> {
        #[cfg(not(feature = "no_object"))]
        #[cfg(not(feature = "no_index"))]
        return Ok(TupleVariantSerializer {
            variant: _variant,
            array: crate::Array::with_capacity(_len),
        });
        #[cfg(any(feature = "no_object", feature = "no_index"))]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "tuples are not supported with 'no_index' or 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_map(self, _len: Option<usize>) -> RhaiResultOf<Self::SerializeMap> {
        #[cfg(not(feature = "no_object"))]
        return Ok(DynamicSerializer::new(crate::Map::new().into()));
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> RhaiResultOf<Self::SerializeStruct> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> RhaiResultOf<Self::SerializeStructVariant> {
        #[cfg(not(feature = "no_object"))]
        return Ok(StructVariantSerializer {
            variant: _variant,
            map: crate::Map::new(),
        });
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }
}

impl SerializeSeq for DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, _value: &T) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_index"))]
        {
            let value = _value.serialize(&mut *self)?;
            let arr = self._value.downcast_mut::<crate::Array>().unwrap();
            arr.push(value);
            Ok(())
        }
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "arrays are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }

    // Close the sequence.
    fn end(self) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_index"))]
        return Ok(self._value);
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "arrays are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }
}

impl SerializeTuple for DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, _value: &T) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_index"))]
        {
            let value = _value.serialize(&mut *self)?;
            let arr = self._value.downcast_mut::<crate::Array>().unwrap();
            arr.push(value);
            Ok(())
        }
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "tuples are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_index"))]
        return Ok(self._value);
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "tuples are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }
}

impl SerializeTupleStruct for DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, _value: &T) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_index"))]
        {
            let value = _value.serialize(&mut *self)?;
            let arr = self._value.downcast_mut::<crate::Array>().unwrap();
            arr.push(value);
            Ok(())
        }
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "tuples are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_index"))]
        return Ok(self._value);
        #[cfg(feature = "no_index")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "tuples are not supported with 'no_index'".into(),
            Position::NONE,
        )
        .into());
    }
}

impl SerializeMap for DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, _key: &T) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_object"))]
        {
            self._key = _key.serialize(&mut *self)?;
            Ok(())
        }
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, _value: &T) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_object"))]
        {
            let key = std::mem::take(&mut self._key)
                .into_immutable_string()
                .map_err(|typ| {
                    ERR::ErrorMismatchDataType("string".into(), typ.into(), Position::NONE)
                })?;
            let value = _value.serialize(&mut *self)?;
            let map = self._value.downcast_mut::<crate::Map>().unwrap();
            map.insert(key.into(), value);
            Ok(())
        }
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn serialize_entry<K: ?Sized + Serialize, T: ?Sized + Serialize>(
        &mut self,
        _key: &K,
        _value: &T,
    ) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_object"))]
        {
            let key: Dynamic = _key.serialize(&mut *self)?;
            let key = key.into_immutable_string().map_err(|typ| {
                ERR::ErrorMismatchDataType("string".into(), typ.into(), Position::NONE)
            })?;
            let value = _value.serialize(&mut *self)?;
            let map = self._value.downcast_mut::<crate::Map>().unwrap();
            map.insert(key.into(), value);
            Ok(())
        }
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_object"))]
        return Ok(self._value);
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }
}

impl SerializeStruct for DynamicSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> RhaiResultOf<()> {
        #[cfg(not(feature = "no_object"))]
        {
            let value = _value.serialize(&mut *self)?;
            let map = self._value.downcast_mut::<crate::Map>().unwrap();
            map.insert(_key.into(), value);
            Ok(())
        }
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        #[cfg(not(feature = "no_object"))]
        return Ok(self._value);
        #[cfg(feature = "no_object")]
        return Err(ERR::ErrorMismatchDataType(
            "".into(),
            "object maps are not supported with 'no_object'".into(),
            Position::NONE,
        )
        .into());
    }
}

#[cfg(not(feature = "no_object"))]
#[cfg(not(feature = "no_index"))]
struct TupleVariantSerializer {
    variant: &'static str,
    array: crate::Array,
}

#[cfg(not(feature = "no_object"))]
#[cfg(not(feature = "no_index"))]
impl serde::ser::SerializeTupleVariant for TupleVariantSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> RhaiResultOf<()> {
        let value = to_dynamic(value)?;
        self.array.push(value);
        Ok(())
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        make_variant(self.variant, self.array.into())
    }
}

#[cfg(not(feature = "no_object"))]
struct StructVariantSerializer {
    variant: &'static str,
    map: crate::Map,
}

#[cfg(not(feature = "no_object"))]
impl serde::ser::SerializeStructVariant for StructVariantSerializer {
    type Ok = Dynamic;
    type Error = RhaiError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> RhaiResultOf<()> {
        let value = to_dynamic(value)?;
        self.map.insert(key.into(), value);
        Ok(())
    }

    fn end(self) -> RhaiResultOf<Self::Ok> {
        make_variant(self.variant, self.map.into())
    }
}

#[cfg(not(feature = "no_object"))]
fn make_variant(variant: &'static str, value: Dynamic) -> RhaiResult {
    let mut map = crate::Map::new();
    map.insert(variant.into(), value);
    Ok(map.into())
}
