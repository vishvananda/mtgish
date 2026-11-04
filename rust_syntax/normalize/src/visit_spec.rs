//! Build-time typed IR traversal. Fields and variants come from ir.rs itself.
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use syn::{Fields, GenericArgument, Item, PathArguments, Type};

const HOOKS: &[(&str, &str)] = &[
    ("Ability", "ability"),
    ("Instruction", "instruction"),
    ("Operation", "operation"),
    ("ObjectFilter", "object_filter"),
    ("ObjectSet", "object_set"),
    ("ObjectRef", "object_ref"),
    ("Expr", "expr"),
];

fn children(
    expr: proc_macro2::TokenStream,
    ty: &Type,
    names: &BTreeSet<String>,
) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(path) => {
            let path = &path.path;
            if path.segments.len() != 1 {
                return quote!();
            }
            let segment = &path.segments[0];
            if names.contains(&segment.ident.to_string()) {
                return quote!(Walk::walk(#expr, visitor););
            }
            let PathArguments::AngleBracketed(args) = &segment.arguments else {
                return quote!();
            };
            let inner = args
                .args
                .iter()
                .filter_map(|arg| match arg {
                    GenericArgument::Type(ty) => Some(ty),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [inner] = inner.as_slice() else {
                panic!("unknown IR container {}", segment.ident)
            };
            let body = children(quote!(child), inner, names);
            if body.is_empty() {
                return quote!();
            }
            match segment.ident.to_string().as_str() {
                "Box" => children(quote!((#expr).as_ref()), inner, names),
                "Option" => quote!(if let Some(child) = (#expr).as_ref() { #body }),
                "Vec" => quote!(for child in (#expr).iter() { #body }),
                other => panic!("unknown IR container {other}"),
            }
        }
        Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                let index = syn::Index::from(index);
                children(quote!(&(#expr).#index), ty, names)
            })
            .collect(),
        Type::Array(array) => {
            let body = children(quote!(child), &array.elem, names);
            if body.is_empty() {
                quote!()
            } else {
                quote!(for child in (#expr).iter() { #body })
            }
        }
        other => panic!("unsupported IR field type: {}", quote!(#other)),
    }
}

/// Deterministic declaration-order traversal. External parser vocabularies and
/// scalar/string fields are leaves; all locally declared IR children are visited.
pub fn generate(source: &str) -> String {
    let file = syn::parse_file(source).expect("valid Rust IR declarations");
    let names: BTreeSet<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(item) => Some(item.ident.to_string()),
            Item::Enum(item) => Some(item.ident.to_string()),
            _ => None,
        })
        .collect();
    file.items.iter().filter_map(|item| {
        let (name, body) = match item {
            Item::Struct(item) => {
                let body = item.fields.iter().enumerate().map(|(index, field)| {
                    let member = field.ident.as_ref().map(|ident| quote!(#ident)).unwrap_or_else(|| { let index=syn::Index::from(index);quote!(#index) });
                    children(quote!(&self.#member), &field.ty, &names)
                }).collect::<proc_macro2::TokenStream>();
                (&item.ident, body)
            }
            Item::Enum(item) => {
                let arms = item.variants.iter().map(|variant| {
                    let variant_name=&variant.ident;
                    match &variant.fields {
                        Fields::Unit => quote!(Self::#variant_name => {}),
                        Fields::Named(fields) => {
                            let mut bindings=Vec::new();let mut bodies=Vec::new();
                            for field in &fields.named {
                                let field_name=field.ident.as_ref().unwrap();
                                let body=children(quote!(#field_name),&field.ty,&names);
                                if !body.is_empty() {bindings.push(field_name);bodies.push(body);}
                            }
                            quote!(Self::#variant_name { #(#bindings,)* .. } => { #(#bodies)* })
                        }
                        Fields::Unnamed(fields) => {
                            let mut bindings=Vec::new();let mut bodies=Vec::new();
                            for (index,field) in fields.unnamed.iter().enumerate() {
                                let field_name=format_ident!("field_{index}");let body=children(quote!(#field_name),&field.ty,&names);
                                bindings.push(if body.is_empty() {quote!(_)} else {quote!(#field_name)});bodies.push(body);
                            }
                            quote!(Self::#variant_name(#(#bindings),*) => { #(#bodies)* })
                        }
                    }
                });
                (&item.ident,quote!(match self {#(#arms,)*}))
            }
            _ => return None,
        };
        let hook=HOOKS.iter().find(|(ty,_)| *ty==name.to_string()).map(|(_,method)| {
            let method=format_ident!("{method}");quote!(if !visitor.#method(self) {return;})
        });
        Some(quote!(#[allow(unused_variables)] impl Walk for #name {fn walk<V: Visitor>(&self,visitor:&mut V){ #hook #body }}))
    }).collect::<proc_macro2::TokenStream>().to_string()
}
