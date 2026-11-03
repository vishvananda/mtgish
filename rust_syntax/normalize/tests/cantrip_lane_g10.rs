#![cfg(feature = "cantrip")]
//! Forms lane g10 writes: the snow mana spent to cast a spell, choices at random
//! and of a defender, attachments to a host chosen after them, damage dealt
//! together, the players and the permissions of a walk, and reads that sit inside
//! another hole's code. It reads the corpus and the printed words for these cards
//! alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // Snow mana of its own colors spent to cast it: snow sources told apart as
    // they pay (CR 106.4, 601.2h).
    ("Boreal Outrider", "mana_told_apart: &[snow_mana]", false),
    ("Boreal Outrider", "q.chars(source).supertypes.snow && ", false),
    // "Each creature dealt damage this way": the history's damage by the spell.
    ("Demonfire", "permanents_dealt_damage_by(q.history(), asker.source, 0)", false),
    // A color at random from those no permanent of theirs has.
    ("Opulent Clomper", "g.choose_at_random(pool_", false),
    // The defending player its controller chooses among those it attacks.
    ("Tahngarth, First Mate", "combat::choose_among(", false),
    // The Equipment first, then a host for them, a lone one taken without asking.
    ("Gilgamesh, Master-at-Arms", "Purpose::Attach", false),
    // The land type chosen as it resolves, read from its local.
    ("Vision Charm", ".is_some_and(|chosen|", false),
    // A read of a choice whose own instruction has a hole follows that hole.
    ("Rite of Ruin", "depends on an instruction with a gap", false),
    ("Swift Warkite", "depends on an instruction with a gap", false),
    // "Each legendary card in your graveyard gains 'You may play this card from
    // your graveyard'": a permission of each card to its owner (CR 611.2c).
    (
        "Kethis, the Hidden Hand",
        "g.allow_play(card, owner, PlayGrant { lands: true, expiry: Expiry::EndOfTurn, ..PlayGrant::CAST });",
        true,
    ),
    // Each player adds mana in turn: a pool no permanent sees.
    ("Yurlok of Scorch Thrash", "g.add_mana(player, Mana::B", false),
    // A walk's reveal carried out of its "if that player is there" block.
    ("Mind Grind", "carried_", false),
    // Damage dealt by several parts at once is one batch (CR 120.3).
    ("The Great Aerie", "g.damage_together(&together_", false),
];

#[test]
fn modules_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let mut names: Vec<String> = CARDS.iter().map(|(name, ..)| name.to_string()).collect();
    names.sort();
    names.dedup();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says, whole) in CARDS {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
}
