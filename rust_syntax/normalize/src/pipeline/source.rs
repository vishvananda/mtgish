//! Reversible inspection of the actual Rust source enums. This representation
//! is compiler provenance/intermediate data, never a final semantic escape.
use serde::{
    de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor},
    Deserialize, Serialize,
};
use serde_json::Value;
type Error = de::value::Error;

pub fn decode<'de, T: Deserialize<'de>>(value: &'de Value) -> Result<T, Error> {
    T::deserialize(SourceDeserializer(value))
}
struct SourceDeserializer<'de>(&'de Value);
impl<'de> de::Deserializer<'de> for SourceDeserializer<'de> {
    type Error = Error;
    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        match self.0 {
            Value::Null => v.visit_unit(),
            Value::Bool(b) => v.visit_bool(*b),
            Value::String(s) => v.visit_borrowed_str(s),
            Value::Number(n) => {
                if let Some(n) = n.as_i64() {
                    v.visit_i64(n)
                } else if let Some(n) = n.as_u64() {
                    v.visit_u64(n)
                } else {
                    v.visit_f64(
                        n.as_f64()
                            .ok_or_else(|| de::Error::custom("invalid number"))?,
                    )
                }
            }
            Value::Array(items) => v.visit_seq(Sequence(items.iter())),
            Value::Object(fields) => v.visit_map(Fields {
                iter: fields.iter(),
                value: None,
            }),
        }
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, Error> {
        if self.0.is_null() {
            v.visit_none()
        } else {
            v.visit_some(self)
        }
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        v: V,
    ) -> Result<V::Value, Error> {
        v.visit_newtype_struct(self)
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        _: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Error> {
        let fields = self
            .0
            .as_object()
            .ok_or_else(|| de::Error::custom("expected inspected enum"))?;
        if fields.len() != 3 || fields.get("enum").and_then(Value::as_str) != Some(name) {
            return Err(de::Error::custom(format!("expected source enum {name}")));
        }
        let variant = fields
            .get("variant")
            .and_then(Value::as_str)
            .ok_or_else(|| de::Error::custom("missing variant"))?;
        let args = fields
            .get("args")
            .ok_or_else(|| de::Error::custom("missing arguments"))?;
        v.visit_enum(Enumeration { variant, args })
    }
    serde::forward_to_deserialize_any! {bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf unit unit_struct seq tuple tuple_struct map struct identifier ignored_any}
}
struct Sequence<'de>(std::slice::Iter<'de, Value>);
impl<'de> SeqAccess<'de> for Sequence<'de> {
    type Error = Error;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        self.0
            .next()
            .map(|value| seed.deserialize(SourceDeserializer(value)))
            .transpose()
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}
struct Fields<'de> {
    iter: serde_json::map::Iter<'de>,
    value: Option<&'de Value>,
}
impl<'de> MapAccess<'de> for Fields<'de> {
    type Error = Error;
    fn next_key_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if let Some((key, value)) = self.iter.next() {
            self.value = Some(value);
            seed.deserialize(de::value::StrDeserializer::<Error>::new(key))
                .map(Some)
        } else {
            Ok(None)
        }
    }
    fn next_value_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(SourceDeserializer(
            self.value
                .take()
                .ok_or_else(|| de::Error::custom("value without key"))?,
        ))
    }
}
struct Enumeration<'de> {
    variant: &'de str,
    args: &'de Value,
}
impl<'de> EnumAccess<'de> for Enumeration<'de> {
    type Error = Error;
    type Variant = Self;
    fn variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<(T::Value, Self), Error> {
        let variant = seed.deserialize(de::value::StrDeserializer::<Error>::new(self.variant))?;
        Ok((variant, self))
    }
}
impl<'de> VariantAccess<'de> for Enumeration<'de> {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        if self.args.is_null() {
            Ok(())
        } else {
            Err(de::Error::custom("unit variant has arguments"))
        }
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(SourceDeserializer(self.args))
    }
    fn tuple_variant<V: Visitor<'de>>(self, _: usize, v: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(SourceDeserializer(self.args), v)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        v: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_map(SourceDeserializer(self.args), v)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u32);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Origin {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub pointer: String,
    pub source_type: String,
    pub form: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Face {
    /// Source field path preserves front/back/adventure/half identity. Face
    /// records share one shape; layout-specific behavior is not discarded.
    pub pointer: String,
    pub name: String,
    pub characteristics: std::collections::BTreeMap<String, Value>,
    pub abilities: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceIndex {
    pub layout: String,
    pub faces: Vec<Face>,
    /// Complete original data, including fields without a rewrite. Can be
    /// decoded to the original Rust OracleCard and checked independently.
    pub original: Value,
    pub nodes: Vec<Origin>,
}
impl SourceIndex {
    pub fn new(card: &crate::mtg_types::OracleCard) -> Self {
        let original = crate::inspect::value(card).expect("serializable Rust source");
        let mut result = Self {
            layout: original["variant"].as_str().unwrap().into(),
            faces: vec![],
            original,
            nodes: vec![],
        };
        fn visit(
            value: &Value,
            path: &str,
            parent: Option<NodeId>,
            nodes: &mut Vec<Origin>,
            faces: &mut Vec<Face>,
        ) {
            let mut parent = parent;
            if let (Some(ty), Some(form)) = (
                value.get("enum").and_then(Value::as_str),
                value.get("variant").and_then(Value::as_str),
            ) {
                let id = NodeId(nodes.len().try_into().expect("source tree node IDs"));
                nodes.push(Origin {
                    id: id.clone(),
                    parent: parent.clone(),
                    pointer: path.into(),
                    source_type: ty.into(),
                    form: form.into(),
                });
                parent = Some(id);
            }
            match value {
                Value::Object(fields) => {
                    if let (Some(name), Some(_)) = (
                        fields.get("Name").and_then(Value::as_str),
                        fields.get("Typeline"),
                    ) {
                        let characteristics = fields
                            .iter()
                            .filter(|(k, _)| !matches!(k.as_str(), "Name" | "Rules"))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        faces.push(Face {
                            pointer: path.into(),
                            name: name.into(),
                            characteristics,
                            abilities: fields.get("Rules").map(|_| format!("{path}/Rules")),
                        });
                    }
                    for (key, child) in fields {
                        visit(
                            child,
                            &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                            parent.clone(),
                            nodes,
                            faces,
                        );
                    }
                }
                Value::Array(values) => {
                    for (i, child) in values.iter().enumerate() {
                        visit(child, &format!("{path}/{i}"), parent.clone(), nodes, faces);
                    }
                }
                _ => {}
            }
        }
        visit(
            &result.original,
            "",
            None,
            &mut result.nodes,
            &mut result.faces,
        );
        result
    }
    pub fn at(&self, id: &NodeId) -> &Value {
        self.original
            .pointer(&self.nodes[id.0 as usize].pointer)
            .expect("source provenance")
    }
    pub fn restore(&self) -> Result<crate::mtg_types::OracleCard, Error> {
        decode(&self.original)
    }
}
