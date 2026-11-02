#![cfg(feature = "cantrip")]
//! Forms the Pioneer card lane found wrong, or never run by any game: what each
//! card's module now says, and what it no longer says. It reads the corpus and the
//! printed words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use mtgish_ast::cantrip::pr_form::{render, Characteristics};
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // The flashback permission tests only the type the card has.
    ("Memory Deluge", "fn ability_1_require(q: &dyn Query, asker: &Asker) -> bool { (q.chars(asker.source).is(CardType::Instant)) }"),
    ("Otherworldly Gaze", "fn ability_1_require(q: &dyn Query, asker: &Asker) -> bool { (q.chars(asker.source).is(CardType::Instant)) }"),
    // A spell's instructions each end before the next (CR 608.2c).
    ("Teferi's Protection", "cantrip_api::instruction(g, |g| { let players_0_0"),
    ("Anger of the Gods", "cantrip_api::instruction(g, |g| { for bound_1002 in dealt_0.clone() {"),
    // "Put them on the bottom of your library in a random order" (CR 401.4).
    ("Getaway Barrel", "g.bottom_in_random_order("),
    // A way to cast granted to a card in a graveyard asks about the card (CR 715.4):
    // Bonecrusher Giant is a creature card, though its Stomp is an instant.
    ("Iroh, Grand Lotus", "q.card(q.obj(card).card).is(CardType::Instant) || q.card(q.obj(card).card).is(CardType::Sorcery)"),
    ("Underworld Breach", "!(q.card(q.obj(card).card).is(CardType::Land))"),
    // A permission to cast spells reads the face cast (CR 715.3a): Lurrus can't cast
    // an Adventure.
    ("Lurrus of the Dream-Den", "q.chars(card).is(CardType::Artifact)"),
    // "Its controller's graveyard" is the target's controller's.
    ("Drown in the Loch", "Some(id).map(|o: ObjectId| q.controller(o)) == Some(q.state().obj(card_in_zone).owner)"),
    // "You may put it onto the battlefield" is asked only when the top card is one.
    ("Risen Reef", "let paid_1 = offered_1 && cantrip_mechanics::effects::ask_may("),
    ("Nissa, Steward of Elements", "if offered_2 && cantrip_mechanics::effects::ask_may("),
    // "Exile it ... lose life equal to its mana value": the card exiled, not the new top.
    ("Sorin the Mirthless", "let amount_5 = { let q: &dyn Query = g; ((objects_4.iter().copied().filter(|&o| q.state().obj(o).became.is_some())"),
    // "Entered the battlefield from your graveyard" is of the event that fired it.
    ("Prized Amalgam", "match event { Event::ZoneChange { new, from, to, .. } if *new == candidate && to.is(ZoneKind::Battlefield) => Some(*from)"),
    // "If it's a creature" is the copy as it enters (CR 707.2).
    ("Spark Double", "entering.copy.as_ref().is_some_and(|c| c.extra_types.has(CardType::Creature) || !c.types_stated() && q.card(c.card).is(CardType::Creature))"),
    // One search across graveyard, hand and library (CR 701.23a).
    ("Unmoored Ego", "let searches_1 = g.may_search(searcher_1, owner_1); let mut shown_1: Vec<ObjectId> = match searches_1 { true => cantrip_mechanics::library::searched_cards(g, owner_1), false => Vec::new() }; shown_1.extend(g.state().zone(Zone::graveyard(owner_1)).iter().copied());shown_1.extend(g.state().zone(Zone::hand(owner_1)).iter().copied());"),
    ("Unmoored Ego", "cantrip_mechanics::select::Count::UpTo({ let q: &dyn Query = g; (4i64)"),
    // "Up to one": always asked.
    ("Enigmatic Incarnation", "let taken = cantrip_mechanics::choose::select(g, player, asker.source, Purpose::Sacrifice, eligible,"),
];

/// Forms these cards no longer hold.
const GONE: &[(&str, &str)] = &[
    // The last mode of a choice is the one left: it is not tested.
    ("Aether Gust", "if ability_0_mode_1_target_0(g, asker, chosen) { return ability_0_mode_1(g, item); }"),
    // Two types tested of a card that has one.
    ("Memory Deluge", "CardType::Sorcery"),
    ("Turn the Earth", "CardType::Sorcery"),
    // "If it's the first time" is of the event that fired it, and stays so.
    ("Botanical Brawler", ".count() <= 1"),
    // Who cast it is fixed once it is cast.
    ("Light-Paws, Emperor's Voice", "if !{ let q: &dyn Query = g; item.subject.object().is_some_and(|candidate: ObjectId| cast_of("),
    // Only cards with a way to cast are offered; no land is played.
    ("Torrential Gearhulk", "play_land_during_resolution"),
    ("Torrential Gearhulk", "open_0.len() > 1"),
    ("Emergent Ultimatum", "play_land_during_resolution"),
    // "Up to" never takes all without asking.
    ("Enigmatic Incarnation", "AsManyAsCan(n) if"),
    ("Unmoored Ego", "left_1"),
    ("Unmoored Ego", "Zone::hand(owner_1)).to_vec()"),
];

fn modules(names: &[&str]) -> Option<BTreeMap<String, String>> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return None;
    }
    let mut names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    names.sort();
    names.dedup();
    Some(cantrip::modules_printed(&corpus, &oracle, &names).unwrap().into_iter().collect())
}

#[test]
fn modules_say_what_their_cards_say() {
    let names: Vec<&str> = CARDS.iter().map(|(n, _)| *n).chain(GONE.iter().map(|(n, _)| *n)).collect();
    let Some(modules) = modules(&names) else { return };
    for (name, says) in CARDS {
        let module = modules.get(*name).unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(says), "{name} should have {says}:\n{module}");
        assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
    }
    for (name, gone) in GONE {
        let module = modules.get(*name).unwrap_or_else(|| panic!("no module for {name}"));
        assert!(!module.contains(gone), "{name} should no longer have {gone}:\n{module}");
    }
}

/// Careful Cultivation and Jugan Defends the Temple create one Human Monk token, which
/// Cantrip's mechanics hold once.
#[test]
fn the_human_monk_is_one_token() {
    let cultivation = "Careful Cultivation";
    let Some(modules) = modules(&[cultivation]) else { return };
    let printed = "Create a 1/1 green Human Monk creature token with \"{T}: Add {G}.\"";
    let file = render(&modules[cultivation], cultivation, printed, Characteristics::Oracle).unwrap();
    assert!(file.contains("g.card_id(&tokens::HUMAN_MONK)"), "{file}");
    assert!(!file.contains("pub mod token_"), "{file}");
}
