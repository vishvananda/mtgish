//! The IR's shape, read from its Rust source, for tools that walk serialized cards
use std::collections::HashMap;
use syn::{Fields as SynFields, GenericArgument, Item, PathArguments, Type};

#[cfg(test)]
mod contract;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Named(String),
    Vec(Box<Ty>),
    Option(Box<Ty>),
    Tuple(Vec<Ty>),
    /// A type the table does not hold, such as a source enum. Only its serialized
    /// form is checked, by the final deserialization into the real IR type.
    Opaque,
}

#[derive(Debug, Clone)]
pub enum Fields {
    Unit,
    Tuple(Vec<Ty>),
    Named(Vec<(String, Ty)>),
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Struct(Fields),
    Enum {
        /// `#[serde(tag = "kind", content = "args")]`
        adjacent: bool,
        variants: Vec<(String, Fields)>,
    },
}

#[derive(Debug, Default)]
pub struct Schema {
    pub types: HashMap<String, TypeDef>,
}

impl Schema {
    /// The table for this crate's own IR source.
    pub fn of_ir() -> Result<Self, String> {
        Self::from_sources(&crate::declarations::wire_texts())
    }

    pub fn from_sources(sources: &[&str]) -> Result<Self, String> {
        let mut schema = Self::default();
        for source in sources {
            let file = syn::parse_file(source).map_err(|e| e.to_string())?;
            for item in file.items {
                schema.add(&item);
            }
        }
        Ok(schema)
    }

    fn add(&mut self, item: &Item) {
        match item {
            Item::Struct(item) => {
                self.types.insert(
                    item.ident.to_string(),
                    TypeDef::Struct(fields(&item.fields)),
                );
            }
            Item::Enum(item) => {
                let adjacent = item
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("serde") && attribute_has_tag(a));
                let variants = item
                    .variants
                    .iter()
                    .map(|v| (v.ident.to_string(), fields(&v.fields)))
                    .collect();
                self.types.insert(
                    item.ident.to_string(),
                    TypeDef::Enum { adjacent, variants },
                );
            }
            _ => {}
        }
    }

    pub fn variant(&self, ty: &str, name: &str) -> Option<(bool, &Fields)> {
        match self.types.get(ty)? {
            TypeDef::Enum { adjacent, variants } => variants
                .iter()
                .find(|(variant, _)| variant == name)
                .map(|(_, fields)| (*adjacent, fields)),
            TypeDef::Struct(_) => None,
        }
    }
}

fn attribute_has_tag(attribute: &syn::Attribute) -> bool {
    let mut found = false;
    let _ = attribute.parse_nested_meta(|meta| {
        found |= meta.path.is_ident("tag");
        if meta.input.peek(syn::Token![=]) {
            let value = meta.value()?;
            let _: syn::Lit = value.parse()?;
        }
        Ok(())
    });
    found
}

fn fields(fields: &SynFields) -> Fields {
    match fields {
        SynFields::Unit => Fields::Unit,
        SynFields::Unnamed(items) => {
            Fields::Tuple(items.unnamed.iter().map(|f| ty(&f.ty)).collect())
        }
        SynFields::Named(items) => Fields::Named(
            items
                .named
                .iter()
                .filter_map(|f| Some((f.ident.as_ref()?.to_string(), ty(&f.ty))))
                .collect(),
        ),
    }
}

fn generic(segment: &syn::PathSegment) -> Option<&Type> {
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|a| match a {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn ty(value: &Type) -> Ty {
    match value {
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return Ty::Opaque;
            };
            match (segment.ident.to_string().as_str(), generic(segment)) {
                ("Vec", Some(inner)) => Ty::Vec(Box::new(ty(inner))),
                ("Option", Some(inner)) => Ty::Option(Box::new(ty(inner))),
                ("Box", Some(inner)) => ty(inner),
                (name, None) => Ty::Named(name.to_string()),
                _ => Ty::Opaque,
            }
        }
        Type::Tuple(items) => Ty::Tuple(items.elems.iter().map(ty).collect()),
        _ => Ty::Opaque,
    }
}
