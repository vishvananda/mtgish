//! Schema-aware inspection for accounting and fingerprints, never an executable
//! AST or a normalization fallback. Serde supplies enum type and variant names,
//! including unit variants that ordinary JSON serialization would turn into
//! ambiguous strings.

use serde::ser::{self, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub fn value<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Value> {
    value.serialize(Inspector)
}

pub fn count(value: &Value, forms: &mut BTreeMap<String, BTreeMap<String, usize>>) {
    match value {
        Value::Object(fields) => {
            if let (Some(ty), Some(variant)) = (
                fields.get("enum").and_then(Value::as_str),
                fields.get("variant").and_then(Value::as_str),
            ) {
                *forms
                    .entry(ty.into())
                    .or_default()
                    .entry(variant.into())
                    .or_default() += 1;
            }
            for child in fields.values() {
                count(child, forms);
            }
        }
        Value::Array(items) => {
            for child in items {
                count(child, forms);
            }
        }
        _ => {}
    }
}

struct Inspector;

/// `{"enum": ty, "variant": name, "args": args}`. Built by hand: `json!` would
/// serialize `args` into a copy, every subtree again at each enum above it.
fn variant(ty: &str, name: &str, args: Value) -> Value {
    let mut fields = Map::new();
    fields.insert("enum".into(), Value::String(ty.into()));
    fields.insert("variant".into(), Value::String(name.into()));
    fields.insert("args".into(), args);
    Value::Object(fields)
}

pub struct Compound {
    sequence: bool,
    items: Vec<Value>,
    fields: Map<String, Value>,
    variant: Option<(&'static str, &'static str)>,
    key: Option<String>,
}

impl Compound {
    fn new(sequence: bool, variant: Option<(&'static str, &'static str)>) -> Self {
        Self {
            sequence,
            items: Vec::new(),
            fields: Map::new(),
            variant,
            key: None,
        }
    }
    fn finish(self) -> Value {
        let value = if self.sequence {
            Value::Array(self.items)
        } else {
            Value::Object(self.fields)
        };
        match self.variant {
            Some((ty, name)) => variant(ty, name, value),
            None => value,
        }
    }
}

macro_rules! scalar {
    ($($method:ident($ty:ty)),* $(,)?) => { $(
        fn $method(self, v: $ty) -> serde_json::Result<Value> { serde_json::to_value(v) }
    )* };
}

impl ser::Serializer for Inspector {
    type Ok = Value;
    type Error = serde_json::Error;
    type SerializeSeq = Compound;
    type SerializeTuple = Compound;
    type SerializeTupleStruct = Compound;
    type SerializeTupleVariant = Compound;
    type SerializeMap = Compound;
    type SerializeStruct = Compound;
    type SerializeStructVariant = Compound;
    scalar!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
        serialize_str(&str)
    );
    fn serialize_bytes(self, v: &[u8]) -> serde_json::Result<Value> {
        serde_json::to_value(v)
    }
    fn serialize_none(self) -> serde_json::Result<Value> {
        Ok(Value::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> serde_json::Result<Value> {
        value(v)
    }
    fn serialize_unit(self) -> serde_json::Result<Value> {
        Ok(Value::Null)
    }
    fn serialize_unit_struct(self, _: &'static str) -> serde_json::Result<Value> {
        Ok(Value::Null)
    }
    fn serialize_unit_variant(
        self,
        ty: &'static str,
        _: u32,
        name: &'static str,
    ) -> serde_json::Result<Value> {
        Ok(variant(ty, name, Value::Null))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        v: &T,
    ) -> serde_json::Result<Value> {
        value(v)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        ty: &'static str,
        _: u32,
        name: &'static str,
        v: &T,
    ) -> serde_json::Result<Value> {
        Ok(variant(ty, name, value(v)?))
    }
    fn serialize_seq(self, _: Option<usize>) -> serde_json::Result<Compound> {
        Ok(Compound::new(true, None))
    }
    fn serialize_tuple(self, _: usize) -> serde_json::Result<Compound> {
        Ok(Compound::new(true, None))
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> serde_json::Result<Compound> {
        Ok(Compound::new(true, None))
    }
    fn serialize_tuple_variant(
        self,
        ty: &'static str,
        _: u32,
        name: &'static str,
        _: usize,
    ) -> serde_json::Result<Compound> {
        Ok(Compound::new(true, Some((ty, name))))
    }
    fn serialize_map(self, _: Option<usize>) -> serde_json::Result<Compound> {
        Ok(Compound::new(false, None))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> serde_json::Result<Compound> {
        Ok(Compound::new(false, None))
    }
    fn serialize_struct_variant(
        self,
        ty: &'static str,
        _: u32,
        name: &'static str,
        _: usize,
    ) -> serde_json::Result<Compound> {
        Ok(Compound::new(false, Some((ty, name))))
    }
}

macro_rules! sequence {
    ($trait:ident, $method:ident) => {
        impl ser::$trait for Compound {
            type Ok = Value;
            type Error = serde_json::Error;
            fn $method<T: Serialize + ?Sized>(&mut self, v: &T) -> serde_json::Result<()> {
                self.items.push(value(v)?);
                Ok(())
            }
            fn end(self) -> serde_json::Result<Value> {
                Ok(self.finish())
            }
        }
    };
}
sequence!(SerializeSeq, serialize_element);
sequence!(SerializeTuple, serialize_element);
sequence!(SerializeTupleStruct, serialize_field);
sequence!(SerializeTupleVariant, serialize_field);

macro_rules! fields {
    ($trait:ident) => {
        impl ser::$trait for Compound {
            type Ok = Value;
            type Error = serde_json::Error;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                v: &T,
            ) -> serde_json::Result<()> {
                self.fields.insert(key.into(), value(v)?);
                Ok(())
            }
            fn end(self) -> serde_json::Result<Value> {
                Ok(self.finish())
            }
        }
    };
}
fields!(SerializeStruct);
fields!(SerializeStructVariant);

impl ser::SerializeMap for Compound {
    type Ok = Value;
    type Error = serde_json::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> serde_json::Result<()> {
        self.key = Some(
            serde_json::to_value(key)?
                .as_str()
                .ok_or_else(|| {
                    <serde_json::Error as ser::Error>::custom("expected string map key")
                })?
                .into(),
        );
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> serde_json::Result<()> {
        let key = self
            .key
            .take()
            .ok_or_else(|| <serde_json::Error as ser::Error>::custom("map value without key"))?;
        self.fields.insert(key, value(v)?);
        Ok(())
    }
    fn end(self) -> serde_json::Result<Value> {
        Ok(self.finish())
    }
}
