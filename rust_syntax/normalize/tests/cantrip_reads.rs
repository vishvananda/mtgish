#![cfg(feature = "cantrip")]
//! What the generator reads of statics, triggers, delayed and reflexive abilities,
//! on the cards whose form isn't obvious from the Oracle text.
//! It reads the corpus for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // "You may have target player mill": you are asked, and they do it.
    ("Cloudhoof Kirin", "ask_may(g, decider_"),
    // "Unless any player pays": the first who does ends the asking, and what no
    // one paid for goes at once, after every payment.
    ("Cleansing", "unpaid_0.push(bound_0)"),
    // All a player was dealt at once, whatever dealt it.
    ("Quartzwood Crasher", ".sums_per_subject()"),
    // Fixed as it triggers, not read back from the spell.
    ("Storm, Windrider", ".members_of("),
    // Without the printed line nothing says these are the spells "you cast".
    ("Sami, Wildcat Captain", "engine: no Affinity granted"),
    // A creature card in hand is no creature: it may still cycle.
    ("Damping Matrix", "q.is_on_battlefield(id)"),
    // Tokens have no event of their own: what was resolving as they entered.
    ("Arcane Artisan", "Event::Entered"),
    // Suspended as the upkeep reads it.
    ("Time Beetle", "fn time_travel_for"),
    // A replacement's `then` has no stack item to be linked by.
    ("Asmodeus the Archfiend", "g.text_linked(asker.source)"),
    // As it enters, the object is the permanent it will be.
    ("Sutured Ghoul", "g.text_linked(asker.source)"),
    // The rest of a prevention, once for each event of damage it stopped.
    ("Deflecting Palm", "then: Some("),
    // A basic landwalk is the keyword anything reading or removing it asks after.
    ("Bog Wraith", "Keyword::Swampwalk"),
    // A snow variant is native keyword metadata, including for references/removal.
    ("Rime Dryad", "Keyword::SnowForestwalk"),
    // "Your last upkeep", in your upkeep, is the one before it.
    ("Wiitigo", "!(upkeeping && r.turn == history.turn)"),
    // One window in the AST for two sentences, "ended" and "the beginning of": without
    // the printed line nothing says which, and no window is written.
    ("Premature Burial", "history window SinceLastTurnOf"),
    ("Concert Kaboomist", "history window SinceLastTurnOf"),
    // A source as it was when it dealt the damage, not as it is now.
    ("Inferno Trap", "dealt.chars(q)"),
    // "Abilities that aren't mana abilities" is all a ban is told.
    ("Karn's Sylex", "matches!(costing, CostFor::Ability)"),
    // Their own or the total: only the printed line says.
    ("Minds Aglow", "an amount read by each player as their own"),
    // "Your choice", which the data says of no one.
    ("Hinder", "counter into OwnersLibraryTopOrBottom"),
    ("Ilharg, the Raze-Boar", "may_put_onto_battlefield_attacking"),
    // The spell that reads a name counts what says it counts as that name.
    ("Muscle Burst", "\"Diligent Farmhand\""),
    ("Diligent Farmhand", "statics: &[]"),
    // Each basic land type, read from the layer before abilities.
    ("Magnigoth Treefolk", "Keyword::landwalk(kind)"),
    // An X in an alternative cost is announced like the mana cost's.
    ("Fall of the Titans", "surge::way(ManaCost { generic: 0, x: 1"),
    ("Pippin, Guard of the Citadel", "EffectKind::ProtectionFromTypes"),
    // "Corrupted": the way's one condition holds the requirement too.
    ("Viral Spawning", ".exile_after().condition("),
    // Sizes known as the payer is asked, and a payer's own permanents.
    ("Archfiend of Spite", "Cost::sacrifice(("),
    ("Reservoir Kraken", "tap_untapped(1, "),
    ("Escape Detection", "freerunning::assassin_dealt_combat_damage"),
    // Holes that stay, each for what the form can't say.
    // Its own ability is told from the others by its printed words.
    ("Withering Wisps", "engine: no limit on activations each turn"),
    // "Permanents you've sacrificed this turn" (#175): yours now, but a count of
    // distinct card types in a history isn't written yet.
    ("Korvold, Gleeful Glutton", "history measure Distinct"),
    // The complete native family mask includes snow and qualified landwalks.
    ("Hammerheim", "EffectKind::LoseKeywords(Keywords::LANDWALKS)"),
    // The creature enters with the Aura attached, so its enchant ability names it.
    (
        "Necromancy",
        "g.put_onto_battlefield_with_aura(o, player_1, asker.source)",
    ),
    ("Headliner Scarlett", "a card a player may look at for a while"),
    ("Bane Alley Broker", "cards a player may look at"),
    ("Matopi Golem", "operation WithReflexive"),
];

/// A half's meld is its partner's instruction: nothing of it is on this card.
#[test]
fn a_passive_meld_half_is_an_ordinary_card() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return;
    }
    let names = vec!["Bruna, the Fading Light".to_string()];
    let modules: BTreeMap<String, String> = cantrip::modules_named(&corpus, &names)
        .unwrap()
        .into_iter()
        .collect();
    let module = modules.get("Bruna, the Fading Light").expect("a module");
    assert!(!module.contains("melds_into"), "{module}");
}

#[test]
fn modules_say_what_their_cards_say() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return;
    }
    let names: Vec<String> = CARDS.iter().map(|(name, _)| name.to_string()).collect();
    let modules: BTreeMap<String, String> = cantrip::modules_named(&corpus, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says) in CARDS {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(
            module.contains(says),
            "{name} should have {says}:\n{module}"
        );
    }
}

/// Searing Blood's delayed damage comes from its spell, while its recipient is the watched creature's controller.
#[test]
fn searing_blood_keeps_the_creating_spell_and_watched_creature_distinct() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() { return; }
    let names = || vec!["Searing Blood".to_string()];
    let old = cantrip::planned::with_features(&[], || cantrip::modules_named(&corpus, &names())).unwrap();
    assert!(old[0].1.contains("delayed ability with separate source and watched object"), "{}", old[0].1);
    let new = cantrip::planned::with_additional_features(&["delayed-watched-source"], || cantrip::modules_named(&corpus, &names())).unwrap();
    let output = &new[0].1;
    assert!(output.contains("DelayedWhen::ThisTurnWatching(watched), item.object"), "{output}");
    assert!(output.contains("item.subject.object().map(|o: ObjectId| q.controller(o))"), "{output}");
    let effect = output.split("fn watch_3(g:").nth(1).unwrap().split("fn ability_0(").next().unwrap();
    assert!(effect.contains("Some(asker.source)"), "{effect}");
    assert!(!output.contains("// gaps:"), "{output}");
}
