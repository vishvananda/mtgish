//! Canonical forms for ordered changes within one effect.
use crate::ir::{Change, ChangeMode, CreatureTypeSet};

/// Coalesce adjacent creature-type additions while preserving their first-seen
/// order. "All" uses one setter form. Adjacent Artifact/Creature additions use
/// text order; other changes and nested effect boundaries remain barriers.
pub fn coalesce_creature_type_additions(changes: Vec<Change>) -> Vec<Change> {
    let mut out = Vec::with_capacity(changes.len());
    for change in changes {
        match change {
            Change::CreatureTypes { types: CreatureTypeSet::All, .. } => {
                out.push(Change::CreatureTypes {
                    mode: ChangeMode::Set,
                    types: CreatureTypeSet::All,
                });
            }
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
    for i in 1..out.len() {
        if matches!(
            (&out[i - 1], &out[i]),
            (Change::AddCardType(crate::mtg_types::CardType::Creature),
             Change::AddCardType(crate::mtg_types::CardType::Artifact))
        ) {
            out.swap(i - 1, i);
        }
    }
    out
}

/// Lose the other abilities, then gain these within the enclosing effect's
/// ordered change list. No operation/effect boundary or new timestamp is added.
pub fn replace_abilities(abilities: Vec<crate::ir::Ability>) -> Vec<Change> {
    use crate::ir::Ability;
    let mut changes = Vec::with_capacity(abilities.len() + 1);
    changes.push(Change::RemoveAllAbilities);
    changes.extend(abilities.into_iter().map(|ability| match ability {
        Ability::Keyword(keyword) => Change::GrantKeyword(keyword),
        other => Change::GrantAbility(Box::new(other)),
    }));
    changes
}

/// The retained direct ability changes of an entry specification. EntersAs is a
/// final set of abilities; grants before the last loss do not survive. Nested
/// change lists are barriers and are deliberately not searched or flattened.
pub fn entering_changes(changes: &[Change]) -> impl Iterator<Item = &Change> {
    let last_loss = changes
        .iter()
        .rposition(|change| matches!(change, Change::RemoveAllAbilities));
    changes
        .iter()
        .enumerate()
        .filter_map(move |(index, change)| {
            let keep = match (last_loss, change) {
                (Some(last), Change::GrantAbility(_) | Change::GrantKeyword(_)) => index > last,
                (Some(last), Change::RemoveAllAbilities) => index == last,
                _ => true,
            };
            keep.then_some(change)
        })
}
