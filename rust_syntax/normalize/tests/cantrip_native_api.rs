#![cfg(feature = "cantrip")]
//! Compatibility probes are generated from source, retaining the native API's semantics.
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};
fn card(name: &str) -> Card {
    include_str!("fixtures/native_api_20261111.ronl").lines()
        .map(|s| ron::from_str::<OracleCard>(s).unwrap())
        .find(|s| corpus::Identity::of(s).unwrap().names.iter().any(|n| n == name))
        .map(|s| lower::card(&s).unwrap()).unwrap()
}
fn generated(name: &str) -> String { cantrip::module(&card(name)).unwrap_or_else(|e| panic!("{name}: {e:?}")) }
fn dense(s: &str) -> String { s.chars().filter(|c| !c.is_whitespace()).collect() }
#[test]
fn riot_and_job_select_use_the_core_flag_and_shared_token() {
    let riot = generated("Rampaging Rendhorn");
    assert!(riot.contains("riot: true"), "{riot}");
    assert!(!riot.contains("riot::enters"), "{riot}");
    let job = generated("Monk's Fist");
    assert!(job.contains("tokens::HERO_1_1_COLORLESS"), "{job}");
}
#[test]
fn added_native_fields_keep_ordinary_costs_and_permissions() {
    let emerge = dense(&generated("Drownyard Behemoth"));
    assert!(emerge.contains("excluded:None"), "{emerge}");
    let alternative = dense(&generated("Fist of Suns"));
    assert!(alternative.contains("kind:WayKind::Other"), "{alternative}");
    assert!(alternative.contains("..ManaCost::ZERO"), "{alternative}");
    for name in ["Squee, the Immortal", "Grolnok, the Omnivore"] {
        let code = dense(&generated(name));
        assert!(code.contains("from:ZoneKind::Exile"), "{name}: {code}");
        assert!(code.contains("any_color:false"), "{name}: {code}");
        assert!(code.contains("instead:None"), "{name}: {code}");
    }
}
#[test]
fn watched_delayed_trigger_keeps_source_and_watched_subject_separate() {
    let code = dense(&generated("Reincarnation"));
    assert!(code.contains("DelayedWhen::WatchingThisTurn,item.object,item.controller"), "{code}");
    assert!(code.contains("Target::Object(watched)"), "{code}");
    assert!(!code.contains("ThisTurnWatching("), "{code}");
}
#[test]
fn migrated_search_and_free_cast_calls_evaluate_queries_before_mutably_borrowing_game() {
    let search = dense(&generated("Grand Master of Flowers"));
    assert!(search.contains("search_zone_and_or_library(g,"), "{search}");
    assert!(search.contains("&[zone_"), "{search}");
    let cast = dense(&generated("Mnemonic Deluge"));
    assert!(cast.contains("letcaster_"), "{cast}");
    assert!(cast.contains("letcastable_"), "{cast}");
    assert!(cast.contains("cast_any_number_free(g,caster_"), "{cast}");
}
#[test]
fn sacrificed_permanent_count_does_not_count_the_new_objects_of_a_merged_permanent() {
    // The same native receipt supplies the scalar count and all new objects.
    // A merged permanent is one sacrificed permanent but can yield two cards.
    let mut card = card("Wrath of God");
    let Ability::Spell { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Effect { result, .. } = body.instructions[0].clone() else { panic!() };
    body.instructions.push(Instruction::Effect { result: ResultId(50), operation: Operation::Draw { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Performed(result) } });
    let code = cantrip::module(&card).unwrap();
    assert!(code.contains(".count())"), "{code}");
    assert!(code.contains(".into_objects();"), "{code}");
    assert!(code.contains("done_permanent_count_destroyed_0 as i64"), "{code}");
    assert!(!code.contains("destroyed_0.len()"), "{code}");
}
#[test]
fn cannot_block_is_an_open_rule_that_applies_to_later_creatures() {
    let code = generated("Falter");
    assert!(code.contains("LastingRuleKind::CantBlock"), "{code}");
    assert!(code.contains("g.add_lasting_rule("), "{code}");
    assert!(!code.contains("CreaturesCantBlock("), "{code}");
    assert!(!code.contains("g.creatures_cant_block("), "{code}");
}

#[test]
fn optional_player_guards_preserve_chooser_and_both_piles() {
    for name in ["Boneyard Parley", "Death or Glory", "Split the Spoils"] {
        let code = generated(name);
        syn::parse_file(&code).unwrap();
        assert!(code.contains("carried_"), "{code}");
        // Both piles must survive, rather than only the chosen pile.
        assert!(code.contains("other_") && code.contains("pile_"), "{code}");
    }
    let code = generated("Allure of the Unknown");
    assert!(!code.contains("Some(selected_1_player) } { let caster_"), "{code}");
    assert!(code.contains("Some(selected_") && code.contains("carried_"), "{code}");
    let code = generated("Gollum, Scheming Guide");
    assert!(code.contains("carried_filter_"), "{code}");
    assert!(code.contains("unwrap_or(u32::MAX)"), "an absent chooser is not a guessed nonland: {code}");
    assert!(code.contains("_ => false"), "{code}");
}

#[test]
fn complex_search_loyalty_and_granted_layer_continuations_match_native_types() {
    let search = generated("Emergent Ultimatum");
    assert!(search.contains("choice.chooser"), "{search}");
    assert!(search.contains("let searcher_") && search.contains("let search_count_"), "{search}");
    let loyalty = generated("Nissa, Steward of Elements");
    assert!(loyalty.contains("i64::from(entering.x).min(2147483647) as i32"), "{loyalty}");
    assert!(loyalty.contains("slot.saturating_add(more)"), "{loyalty}");
    let toughness = generated("V.A.T.S.");
    assert!(toughness.contains("-> i64 { i64::from(q.chars(id).toughness"), "{toughness}");
    for name in ["Rune of Speed", "Rune of Might", "Dancer's Chakrams"] {
        let source: OracleCard = include_str!("fixtures/native_api_20261111.ronl").lines()
            .map(|s| ron::from_str::<OracleCard>(s).unwrap())
            .find(|s| corpus::Identity::of(s).unwrap().names.iter().any(|n| n == name)).unwrap();
        let lowered = mtgish_ast::unified::lower(&source);
        // Dancer's commander predicate remains a separate, explicit gap.
        let whole = cantrip::whole(&source, &lowered.card).unwrap();
        let code = &whole.faces[0].rust;
        assert!(code.contains("AbilityDef::Static(static_ability("), "{code}");
        assert!(code.contains(".continues())"), "the continuation belongs to StaticDef: {code}");
    }
}
