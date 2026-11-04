//! Canonical forms for ordered changes within one effect.
use crate::ir::{Change, ChangeMode, CreatureTypeSet};

/// Coalesce adjacent creature-type additions while preserving their first-seen
/// order. Other changes and nested effect boundaries are barriers. A type is a
/// vocabulary word, so its enum discriminant is its complete identity.
pub fn coalesce_creature_type_additions(changes: Vec<Change>) -> Vec<Change> {
    let mut out = Vec::with_capacity(changes.len());
    for change in changes {
        match change {
            Change::CreatureTypes {
                mode: ChangeMode::Add,
                types: CreatureTypeSet::Listed(types),
            } => {
                if !matches!(
                    out.last(),
                    Some(Change::CreatureTypes {
                        mode: ChangeMode::Add,
                        types: CreatureTypeSet::Listed(_)
                    })
                ) {
                    out.push(Change::CreatureTypes {
                        mode: ChangeMode::Add,
                        types: CreatureTypeSet::Listed(vec![]),
                    });
                }
                let Some(Change::CreatureTypes {
                    types: CreatureTypeSet::Listed(kept),
                    ..
                }) = out.last_mut()
                else {
                    unreachable!()
                };
                for ty in types {
                    if !kept
                        .iter()
                        .any(|old| std::mem::discriminant(old) == std::mem::discriminant(&ty))
                    {
                        kept.push(ty);
                    }
                }
            }
            Change::Continuing {
                changes: inner,
                until,
            } => out.push(Change::Continuing {
                changes: self::coalesce_creature_type_additions(inner),
                until,
            }),
            Change::Copiable(inner) => out.push(Change::Copiable(
                self::coalesce_creature_type_additions(inner),
            )),
            other => out.push(other),
        }
    }
    out
}
