//! Holes of the last round: the code a card will have, stated, where it waited on
//! a word or a read Cantrip has not got. Each arm reads the card's own operands.
use super::*;

/// One of a card's printed card types that Cantrip has no word for. A dungeon has
/// none in Cantrip: `CardDef::dungeon` is what makes one (CR 309).
pub(super) fn printed_type(feature: &str, word: &str) -> Option<String> {
    (word != "Dungeon").then(|| {
        planned::planned_value(feature, "CardType::Land", &format!("CardType::{word}"))
    })
}

/// One of a card's printed subtypes that Cantrip has no word for.
pub(super) fn printed_subtype(word: &str) -> String {
    planned::planned_value(
        &format!("subtype {word}"),
        "Subtype::Plains",
        &format!("Subtype::{word}"),
    )
}

/// `Supertypes` of a card, with the words Cantrip has no field for.
pub(super) fn printed_supertypes(flags: [bool; 3], others: &[String]) -> String {
    let [basic, legendary, snow] = flags;
    let known = format!("basic: {basic}, legendary: {legendary}, snow: {snow}");
    let mut value = format!("Supertypes {{ {known} }}");
    for other in others {
        let field = other.to_lowercase();
        value = planned::planned_value(
            &format!("supertype {other}"),
            &value,
            &format!("Supertypes {{ {known}, {field}: true }}"),
        );
    }
    value
}
