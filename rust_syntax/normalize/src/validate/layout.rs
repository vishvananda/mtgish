//! A known layout has a fixed face/metadata contract. Generic extension data is
//! explicitly namespaced and never inferred from an unrecognized layout word.
use super::*;
pub(super) fn record(card: &UnifiedCard) -> Result {
    let (minimum, maximum, fields): (usize, usize, &[&str]) = match &card.layout {
        Layout::Card
        | Layout::Melded
        | Layout::Planar
        | Layout::Scheme
        | Layout::Conspiracy
        | Layout::Dungeon => (1, 1, &[]),
        Layout::Transforming
        | Layout::ModalDFC
        | Layout::Adventurer
        | Layout::Ominous
        | Layout::Preparer
        | Layout::Room
        | Layout::Split
        | Layout::Flip => (2, 2, &[]),
        Layout::TokenDefinition => (1, 2, &[]),
        Layout::MeldPiece => (1, 1, &["MeldsInto"]),
        Layout::Vanguard => (1, 1, &["HandModifier", "LifeModifier"]),
        Layout::StickerSheet => (0, 0, &["Stickers"]),
        Layout::Extension { namespace, name } => {
            if !namespace.contains('.')
                || !namespace.split('.').all(|part| {
                    !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
                || name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(
                    "layout extension requires a dotted namespace and an identifier name".into(),
                );
            }
            let mut seen = BTreeSet::new();
            if card
                .data
                .iter()
                .any(|field| field.name.is_empty() || !seen.insert(&field.name))
            {
                return Err(
                    "layout extension metadata requires unique, nonempty field names".into(),
                );
            }
            return Ok(());
        }
    };
    if !(minimum..=maximum).contains(&card.faces.len()) {
        return Err(format!(
            "layout {} requires {minimum}..={maximum} faces",
            card.layout
        ));
    }
    let names: BTreeSet<_> = card.data.iter().map(|field| field.name.as_str()).collect();
    if names.len() != card.data.len() || names != fields.iter().copied().collect() {
        return Err(format!(
            "layout {} requires exactly metadata fields {fields:?}",
            card.layout
        ));
    }
    for field in &card.data {
        let valid = match (&card.layout, field.name.as_str(), &field.value) {
            (Layout::Vanguard, "HandModifier" | "LifeModifier", Operand::Integer(_)) => true,
            (Layout::MeldPiece, "MeldsInto", Operand::Text(name)) => !name.is_empty(),
            (Layout::StickerSheet, "Stickers", Operand::Stickers(_)) => true,
            _ => false,
        };
        if !valid {
            return Err(format!(
                "invalid {} metadata field {}",
                card.layout, field.name
            ));
        }
    }
    Ok(())
}
