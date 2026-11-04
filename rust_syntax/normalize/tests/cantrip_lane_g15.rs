#![cfg(feature = "cantrip")]
//! Lane g15: forms the generator now writes in Cantrip's idiom, on the cards that
//! waited on them. It reads the corpus and the printed words for these cards
//! alone, and has nothing to say without them.
#[path="support/generated_locals.rs"]
mod generated_locals;
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "If you controlled a Dragon as you cast this spell": a fact kept as it is
    // cast, read from its Cast record, or that of the spell a permanent became.
    ("Draconic Roar", "cast_fact: Some(kept_as_cast)", false),
    ("Orator of Ojutai", "q.state().obj(c.spell).became == Some(asker.source)", false),
    // Cleave {X}{R}{W}: a way with an {X} in its cost.
    ("Lantern Flare", "cantrip_mechanics::cleave::way(ManaCost { generic: 0, x: 1", true),
    // "For as long as you control this creature", its controller read reflexively.
    ("Wall of Stolen Identity", "q.controller(e.source) == e.controller", false),
    // "If this creature would enter and it wasn't cast or no mana was spent to cast
    // it": the spell's cast record, as it would enter.
    ("Primeval Spawn", "cast_of(q, candidate)", false),
    ("Primeval Spawn", "entering.instead = Some(Zone::exile(", false),
    // The greatest damage one source dealt, to each recipient or to all.
    ("Impact Resonance", "let key: u32 = f.data[1];", false),
    ("Dragon Cultist", "let key: u32 = 0;", false),
    // "The total power of Daleks that died this turn", each as it last existed.
    (
        "Genesis of the Daleks",
        "map(|id: ObjectId| i64::from(q.chars(id).power.unwrap_or(0))).sum::<i64>()",
        false,
    ),
    // "Add three mana in any combination of its colors", of the card revealed.
    ("Omnath, Locus of All", "for _ in 0..({ let q: &dyn Query = g; 3i64 }).max(0) {", false),
    ("Omnath, Locus of All", "let revealed_1: Vec<ObjectId> = match paid_1", false),
    // "Those spells": the copy, kept past "if you do".
    ("Spinerock Tyrant", "copies_1.extend(g.copy_spell(spell, copier_1, true)?);", false),
    // "If you gain control of a creature this way, tap it".
    ("Midnight Crusader Shuttle", "(carried_11_0.clone()).first().copied()", false),
    // "For each player, you may cast a card that player milled this way".
    ("The Ur-Sphinx", "pool_2.retain(|&c| g.state().obj(c).owner == owner_1);", false),
    // "Copy that ability": named by its target, as Cantrip knows an ability.
    ("Agrus Kos, Eternal Soldier", "copy::ability_on_stack(g,", false),
    // "May pay {1} or {2}. If that player doesn't": each a may of its own.
    ("Winter's Chill", "if paid_1 {", false),
    ("Winter's Chill", "} else {\nlet payer_4", false),
    // "Mana of the chosen color instead": one made replacement for each color.
    ("Hall of Gemstone", "m.mana.0[Color::Green as usize] += colored;", false),
    (
        "Hall of Gemstone",
        "index: match chosen_0 { Color::White => 0, Color::Blue => 1, Color::Black => 2, Color::Red => 3, Color::Green => 4 }",
        false,
    ),
    // A copy that keeps its own name.
    ("Sakashima the Impostor Avatar", ".named(own.name.unwrap_or(q.card(own.card).name))", false),
];

/// Cards that still wait on a form naming what Cantrip lacks.
const GAPS: &[(&str, &str)] = &[
    ("Blaster, Morale Booster", "engine-no-modular-granted"),
    ("Human-Time Lord Meta-Crisis", "bounded resolution choice"),
    ("The Mystical Archive", "exile outside a stack item that an ability reads as exiled with its source"),
];

#[test]
fn modules_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let mut names: Vec<String> = CARDS
        .iter()
        .map(|(name, ..)| name.to_string())
        .chain(GAPS.iter().map(|(name, _)| name.to_string()))
        .collect();
    names.sort();
    names.dedup();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    let module = |name: &str| {
        modules
            .get(name)
            .unwrap_or_else(|| panic!("no module for {name}"))
            .clone()
    };
    for (name, says, whole) in CARDS {
        let module = module(name);
        assert!(generated_locals::contains(&module,says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
    for (name, gap) in GAPS {
        let module = module(name);
        assert!(module.contains(gap), "{name} should wait on {gap}:\n{module}");
    }
}
