//! A player and an object can be bound at the same numeric label. Their reads
//! are distinguished by the semantic reference type, not by that label alone.
pub(crate) fn namespace(owner: &str) -> Option<&'static str> {
    match owner {
        "Instruction::ForEachPlayer"
        | "Condition::ExistsPlayer"
        | "PlayerSet::Matching"
        | "PlayerRef::Bound" => Some("player_binding"),
        "Instruction::ForEachObject" | "ObjectRef::Bound" | "Information::Captured" => {
            Some("object_binding")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
    fn contains_binding(ty: &Ty) -> bool {
        match ty {
            Ty::Named(name) => name == "Binding",
            Ty::Vec(inner) | Ty::Option(inner) => contains_binding(inner),
            Ty::Tuple(items) => items.iter().any(contains_binding),
            Ty::Opaque => false,
        }
    }
    fn check(owner: &str, fields: &Fields) -> usize {
        let types: Vec<_> = match fields {
            Fields::Named(fields) => fields.iter().map(|(_, ty)| ty).collect(),
            Fields::Tuple(items) => items.iter().collect(),
            Fields::Unit => vec![],
        };
        let count = types.into_iter().filter(|ty| contains_binding(ty)).count();
        if count > 0 {
            assert!(
                namespace(owner).is_some(),
                "new binding position needs a semantic role: {owner}"
            );
        }
        count
    }
    #[test]
    fn every_declared_loop_binding_position_has_a_role() {
        let mut count = 0;
        for (name, ty) in Schema::of_ir().unwrap().types {
            match ty {
                TypeDef::Struct(fields) => count += check(&name, &fields),
                TypeDef::Enum { variants, .. } => {
                    for (variant, fields) in variants {
                        count += check(&format!("{name}::{variant}"), &fields)
                    }
                }
            }
        }
        assert!(count > 0);
    }
}
