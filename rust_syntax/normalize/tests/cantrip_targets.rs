#![cfg(feature = "cantrip")]
//! What the generator writes for targets, modes and choices on real cards, where the
//! form can't be told from the card alone. It reads the corpus and the printed words
//! for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // The source reads "choose up to one" as "choose one": the printed words decide.
    ("Dreamshackle Geist", ".choose(0, 1)"),
    // "Becomes that type" sets; only "in addition to its other types" adds.
    ("Imagecrafter", "EffectKind::SetCreatureType(chosen_"),
    ("Mistform Dreamer", "EffectKind::SetCreatureType(chosen_"),
    ("Mistform Sliver", "EffectKind::AddSubtype(chosen_"),
    // No resolving effect sets land types, and adding one would keep the others.
    ("Tundra Kavu", "no resolving effect sets land types"),
    // Repeats of a color are no color more: any set of them, or all.
    ("Dream Coat", "RequestKind::ChooseColors"),
    ("Dream Coat", "EffectKind::SetColors(colors_"),
    // The defending player is who it would be of each attack.
    ("Orgg", "defending_player_of(q.state(), at)"),
    // The attacking player is the active player.
    ("Goblin Goon", "_block("),
    // What paid for a mana ability, as it last was.
    ("Priest of Yawgmoth", "Event::ZoneChange { old, from, part: false"),
    // A token stayed nowhere, so the exiled creature is read as it last was.
    ("Food Chain", "became.unwrap_or(o)"),
    // Nobody is asked who loses nothing.
    ("Balance", "<= keep_"),
    // The other pile stays in the library for the next step to move.
    ("Riddles in the Dark", "face_down_or_face_up_pile"),
    ("Riddles in the Dark", "return_to_hand_revealed"),
    // "+1/-1 or -1/+1" asks in the card's words and is read as a pair.
    ("Brightling", "labels: &[\"+1/-1\", \"-1/+1\"]"),
    // One payment for all that was chosen, after the choice.
    ("Mudslide", ".mana.times(picked_"),
    // "Targets only a single" holds however many times it was chosen.
    ("Loki, God of Lies", "chosen.iter().all(|t| *t == chosen[0])"),
    // A mandatory removal asks for each counter; "any number" may leave them.
    ("Thornmantle Striker", "g.choose_counter_kind("),
    ("Rhys, the Evermore", "g.remove_counters_up_to("),
    // The copy keeps the original's target if it can't take this one.
    ("Ivy, Gleeful Spellthief", "g.change_a_target_to(Target::Object(copy)"),
    // No card can ask what a spell could target.
    ("Precursor Golem", "a-copy-of-a-spell-for-each-object-it-could-target"),
    // "The discarded card": the last discard of the payment, after its mana.
    ("Wolf of Devil's Breach", ".rev()"),
    ("Wolf of Devil's Breach", "Event::Discarded { card, .. } => Some(card)"),
    // "The blighted creature": the effect says which, where a payment doesn't.
    ("Grub, Notorious Auntie", "blight::may_in_effect("),
    // "Up to two" is the printed words', which the instruction lost.
    ("Fable of the Mirror-Breaker", "discard-of-up-to-a-number-read-as-exactly-that-number"),
    // Emerge's and a kicker's payment, read from the records of the cast.
    ("Adipose Offspring", "original_spell("),
    ("Dralnu's Pet", "original_spell("),
    // How many it sacrificed, kept with the text that reads it.
    ("Wood Elemental", "entering.chosen_number = Some(sacrificed_"),
    // Each opponent discards at once, and what they discarded is read by whose.
    ("Kroxa, Titan of Death's Hunger", "discard_together("),
    // A spell that deals damage once dealt damage that way to what it dealt it to.
    ("Ghired's Belligerence", "permanents_dealt_damage_by(q.history(), asker.source, 0)"),
    // An ability's `by` is its source.
    ("Skophos Maze-Warden", ".contains(&by)"),
    // Kicked or not, the same slot for each opponent.
    ("Mega Flare", "controlled_by_slot_opponent("),
    // A permanent asked of afterwards is one on the battlefield, not a card in a hand.
    ("Herald of Vengeance", "zone.is(ZoneKind::Battlefield)"),
    // The modes of the combat before this one of yours.
    ("Yotian Courier", "Step::BeginCombat"),
    // "You may untap it. Do this only once each turn": asked, and spent by a yes.
    ("Legolas, Counter of Kills", "g.note_done(asker.source"),
    // "When you next cast ... this turn" waits for this turn alone.
    ("Repeated Reverberation", "DelayedWhen::NextThisTurn"),
    // "This turn" of Seraph's trigger is no end of the delayed ability's wait.
    ("Seraph", "DelayedWhen::UntilEvent"),
];

#[test]
fn modules_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let names: Vec<String> = CARDS.iter().map(|(name, _)| name.to_string()).collect();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
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
