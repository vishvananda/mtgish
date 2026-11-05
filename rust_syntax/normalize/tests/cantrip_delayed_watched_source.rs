#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard};
const FEATURE: &str = "delayed-watched-source";
fn source(name: &str) -> OracleCard {
    let line = include_str!("fixtures/pipeline_generated.ronl").lines().find(|line| line.starts_with(&format!("Card(Name: \"{name}\""))).unwrap();
    ron::from_str(line).unwrap()
}
fn card(name: &str) -> Card { lower::card(&source(name)).unwrap() }
fn custom(rule: &str) -> Card {
    let line = include_str!("fixtures/pipeline_generated.ronl").lines().find(|line| line.starts_with("Card(Name: \"Runesword\"")).unwrap();
    let mut source: OracleCard = ron::from_str(line).unwrap();
    let OracleCard::Card { rules, .. } = &mut source else { panic!() };
    *rules = Some(vec![ron::from_str(rule).unwrap()]); lower::card(&source).unwrap()
}
fn code(card: &Card, features: &[&str]) -> Result<String, String> {
    cantrip::planned::with_features(features, || cantrip::module(card).map_err(|e| e.feature))
}
fn dense(s: &str) -> String { s.chars().filter(|c| !c.is_whitespace()).collect() }
#[test]
fn watched_death_keeps_the_spell_source_and_legal_target_and_carries_the_dead_object() {
    let card = card("Reincarnation");
    let old = code(&card, &[]).unwrap_err();
    assert!(old.contains("separate source and watched object"), "{old}");
    let new = code(&card, &[FEATURE]).unwrap(); syn::parse_file(&new).unwrap(); let d = dense(&new);
    assert!(d.contains("DelayedWhen::ThisTurnWatching(watched),item.object,item.controller"), "{new}");
    assert!(d.contains("t_0_raw.get(0).copied().filter(|t|t_0.contains(t))"), "{new}");
    assert!(d.contains("Some(asker.source)==Some(id)"), "{new}");
    assert!(d.contains("item.subject.object().map(|o:ObjectId|q.state().obj(o).owner)"), "{new}");
    let supported = cantrip::planned::with_additional_features(&[FEATURE], || cantrip::module(&card)).unwrap();
    let printed = cantrip::pr_form::render(&supported, "Reincarnation", "Return the creature to its owner's hand.", cantrip::pr_form::Characteristics::Inline).unwrap();
    assert!(dense(&printed).contains("item.subject.object().map"), "{printed}");
    assert!(!printed.contains(".expect("), "{printed}");
    if let Ok(path) = std::env::var("CANTRIP_REINCARNATION_PRINTED") { std::fs::write(path, &printed).unwrap(); }
    if let Ok(path) = std::env::var("CANTRIP_WATCHED_GENERATED") {
        let source = source("Reincarnation");
        let unified = mtgish_ast::unified::lower(&source).card;
        let generated = cantrip::planned::with_additional_features(&[FEATURE], || cantrip::whole_module(&source, &unified)).unwrap();
        std::fs::write(path, generated).unwrap();
    }
}
#[test]
fn creator_and_original_target_are_distinct_in_a_delayed_body() {
    let maker = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([SacrificePermanent(ThisPermanent)]), UntilEndOfTurn)])))");
    let target = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))");
    for c in [&maker, &target] { assert!(code(c, &[]).is_err()); }
    let a = code(&maker, &[FEATURE]).unwrap(); let b = code(&target, &[FEATURE]).unwrap();
    syn::parse_file(&a).unwrap(); syn::parse_file(&b).unwrap();
    let a_effect = a.split("fn watch_2(g:").nth(1).unwrap().split("fn ability_0(").next().unwrap();
    let b_effect = b.split("fn watch_2(g:").nth(1).unwrap().split("fn ability_0(").next().unwrap();
    assert!(dense(a_effect).contains("Some(asker.source)"), "{a_effect}");
    assert!(dense(a_effect).contains("Asker::new(item.object,item.controller)"), "{a_effect}");
    assert!(dense(b_effect).contains("item.subject.object()"), "{b_effect}");
}
#[test]
fn unsupported_duration_does_not_fall_back_to_the_watched_object_as_source() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { once, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *once = true;
    for features in [&[][..], &[FEATURE][..]] {
        let gap = code(&c, features).unwrap_err(); assert!(gap.contains("separate watched object and unsupported duration"), "{gap}");
    }
}
#[test]
fn a_normal_this_turn_trigger_without_a_separate_watch_is_unchanged() {
    let c = custom("Activated(PayMana([ManaCostGeneric(1)]), ActionList([CreateTriggerUntil(WhenAPlayerCastsASpell(AnyPlayer, AnySpell), ActionList([DrawACard]), UntilEndOfTurn)]))"); let old = code(&c, &[]).unwrap(); let new = code(&c, &[FEATURE]).unwrap();
    assert_eq!(old, new); assert!(old.contains("DelayedWhen::ThisTurn")); assert!(!old.contains("ThisTurnWatching"));
}

#[test]
fn indefinite_one_shot_can_keep_a_single_watched_object_when_it_is_the_event_subject() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { once, duration, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *once = true; *duration = Duration::Indefinite;
    assert!(code(&c, &[]).is_err());
    let output = code(&c, &[FEATURE]).unwrap(); let d = dense(&output);
    assert!(d.contains("g.delayed_trigger(DelayedWhen::WhenEvent,item.object,item.controller"), "{output}");
    assert!(d.contains("Target::Object(watched)"), "{output}");
    syn::parse_file(&output).unwrap();
}

#[test]
fn watched_object_and_distinct_damage_event_subject_are_not_conflated() {
    let damage = "WhenACreatureDealsCombatDamageToAPermanent(SinglePermanent(Ref_TargetPermanent), IsCardtype(Creature))";
    let payload = custom(&format!("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil({damage}, ActionList([DestroyPermanent(Trigger_ThatOtherCreature)]), UntilEndOfTurn)])))"));
    let mut watched = custom(&format!("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil({damage}, ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))"));
    let Ability::Activated { body, .. } = &mut watched.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event: EventSpec::Damage(d), .. }, .. } = &mut body.instructions[0] else { panic!() };
    d.subject = DamageSubject::Recipient;
    let output = code(&payload, &[FEATURE]).unwrap();
    assert!(output.contains("ThisTurnWatching(watched)"), "{output}");
    syn::parse_file(&output).unwrap();
    let gap = code(&watched, &[FEATURE]).unwrap_err();
    assert!(gap.contains("both watched object and a different event subject") || gap == "target reference", "{gap}");
}

#[test]
fn creator_condition_is_a_source_condition_not_a_watchers_matcher_test() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { intervening_if, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *intervening_if = Some(Condition::ObjectMatches { object: ObjectRef::Source, filter: ObjectFilter::Name("Reincarnation".into()) });
    let output = code(&c, &[FEATURE]).unwrap();
    assert!(output.contains(".condition("), "{output}");
    assert!(dense(&output).contains("Some(asker.source)"), "{output}");
    syn::parse_file(&output).unwrap();
}

#[test]
fn self_watching_one_shot_carries_its_event_subject_and_uses_trigger_semantics() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event, once, duration, .. }, .. } = &mut body.instructions[0] else { panic!() };
    let EventSpec::ZoneChange { objects, .. } = event else { panic!() };
    *objects = ObjectFilter::SameObject(ObjectRef::Source);
    *once = true;
    *duration = Duration::Indefinite;
    let output = code(&c, &[FEATURE]).unwrap();
    assert!(dense(&output).contains("g.delayed_trigger(DelayedWhen::WhenEvent,source,item.controller,AbilityRef::delayed(item.ability.card,0),Target::Object(source))"), "{output}");
    assert!(!output.contains("DelayedWhen::UntilEvent"), "{output}");
    assert!(!output.contains("event matcher guarantees object subject"), "{output}");
    assert!(output.contains("item.subject.object()"), "{output}");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event: EventSpec::ZoneChange { objects, .. }, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *objects = ObjectFilter::Any;
    let gap = code(&c, &[FEATURE]).unwrap_err();
    assert!(gap.contains("one-shot delayed trigger without a proven event subject"), "{gap}");
}

#[test]
fn indefinite_delayed_trigger_watches_a_separate_object_once() {
    let mut c = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))");
    let Ability::Activated { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation, .. } = &mut body.instructions[0] else { panic!() };
    let Operation::CreateTrigger { event, body, .. } = operation.clone() else { panic!() };
    *operation = Operation::CreateDelayedTrigger { event, start: DelayedStart::Immediately, expires: None, intervening_if: None, body };
    assert!(code(&c, &[]).is_err());
    let output = code(&c, &[FEATURE]).unwrap(); let d = dense(&output);
    assert!(d.contains("g.delayed_trigger(DelayedWhen::WhenEvent,item.object,item.controller"), "{output}");
    assert!(d.contains("Target::Object(watched)"), "{output}");
    assert!(d.contains("item.subject.object()"), "{output}");
    syn::parse_file(&output).unwrap();
}

#[test]
fn matcher_needing_the_creator_as_well_as_the_watched_object_is_an_explicit_gap() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event, .. }, .. } = &mut body.instructions[0] else { panic!() };
    let EventSpec::ZoneChange { objects, .. } = event else { panic!() };
    *objects = ObjectFilter::All(vec![objects.clone(), ObjectFilter::SameObject(ObjectRef::Source)]);
    for features in [&[][..], &[FEATURE][..]] {
        let gap = code(&c, features).unwrap_err();
        assert!(gap.contains("event reading both its creator and watched object"), "{gap}");
    }
}

#[test]
fn timed_one_shot_does_not_emit_an_unbounded_watcher() {
    let mut c = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))");
    let Ability::Activated { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation, .. } = &mut body.instructions[0] else { panic!() };
    let Operation::CreateTrigger { event, body, .. } = operation.clone() else { panic!() };
    *operation = Operation::CreateDelayedTrigger { event, start: DelayedStart::Immediately, expires: Some(Duration::EndOfTurn), intervening_if: None, body };
    for features in [&[][..], &[FEATURE][..]] {
        let gap = code(&c, features).unwrap_err();
        assert!(gap.contains("one-shot delayed trigger with a separate watched object"), "{gap}");
    }
}

#[test]
fn zone_changes_match_the_watched_old_identity_but_carry_the_actual_arrival() {
    let mut c = card("Reincarnation");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event, .. }, .. } = &mut body.instructions[0] else { panic!() };
    let EventSpec::ZoneChange { from, to, .. } = event else { panic!() };
    *from = Some(EventZone::Graveyard); *to = EventZone::Exile;
    let result = code(&c, &[FEATURE]).unwrap();
    assert!(result.contains("let id = *old"), "{result}");
    assert!(!result.contains(".functions_in("), "{result}");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event, .. }, .. } = &mut body.instructions[0] else { panic!() };
    let EventSpec::ZoneChange { to, objects, .. } = event else { panic!() };
    *to = EventZone::Battlefield;
    *objects = ObjectFilter::All(vec![objects.clone(), ObjectFilter::Name("Arriving creature".into())]);
    let arrival = code(&c, &[FEATURE]).unwrap();
    assert!(arrival.contains("*old == asker.source"), "{arrival}");
    assert!(arrival.contains("let id = *new"), "{arrival}");
    assert!(arrival.contains("let subject = Target::Object(id)"), "{arrival}");
    assert!(arrival.contains("Arriving creature"), "{arrival}");
    assert!(!arrival.contains(".functions_in("), "{arrival}");
    let ref_body = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))");
    let Ability::Activated { body: maker_body, .. } = &ref_body.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { body: watched_body, .. }, .. } = &maker_body.instructions[0] else { panic!() };
    let Ability::Spell { body: maker_body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { body: arrival_body, .. }, .. } = &mut maker_body.instructions[0] else { panic!() };
    *arrival_body = watched_body.clone();
    let gap = code(&c, &[FEATURE]).unwrap_err();
    assert!(gap.contains("different event subject") || gap == "target reference", "{gap}");
}

#[test]
fn watched_target_uses_the_allocated_slot_index_not_the_ir_identifier() {
    let mut c = card("Reincarnation");
    let Ability::Spell { announcement, body } = &mut c.abilities[0] else { panic!() };
    announcement.targets[0].slot = Slot(7);
    let Instruction::Effect { operation: Operation::CreateTrigger { event, .. }, .. } = &mut body.instructions[0] else { panic!() };
    let EventSpec::ZoneChange { objects: ObjectFilter::SameObject(ObjectRef::Target { slot, .. }), .. } = event else { panic!() };
    *slot = Slot(7);
    let output = code(&c, &[FEATURE]).unwrap();
    assert!(dense(&output).contains("t_0_raw.get(0).copied().filter(|t|t_0.contains(t))"), "{output}");
    assert!(!output.contains("t_7_raw"), "{output}");
}

#[test]
fn an_object_matched_by_a_history_filter_is_not_confused_with_the_watched_source() {
    let damage = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenACreatureDealsCombatDamageToAPermanent(SinglePermanent(Ref_TargetPermanent), IsCardtype(Creature)), ActionList([DrawACard]), UntilEndOfTurn)])))");
    let Ability::Activated { body, .. } = &damage.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event: damage_event, .. }, .. } = &body.instructions[0] else { panic!() };
    let mut c = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenAPermanentLeavesTheBattlefield(SinglePermanent(Ref_TargetPermanent)), ActionList([Exile([Permanent(Ref_TargetPermanent)], [])]), UntilEndOfTurn)])))");
    let Ability::Activated { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { event: EventSpec::Object { objects, .. }, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *objects = ObjectFilter::Condition(Box::new(Condition::Compare {
        left: Expr::EventHistory { event: Box::new(damage_event.clone()), window: HistoryWindow::ThisTurn, measure: HistoryMeasure::Occurrences },
        comparison: Comparison::Greater, right: Expr::Constant(0),
    }));
    let gap = code(&c, &[FEATURE]).unwrap_err();
    assert!(gap.contains("different event subject") || gap == "target reference", "{gap}");
}

fn damage_with_condition(condition: Condition) -> Card {
    let mut c = custom("Activated(PayMana([ManaCostGeneric(1)]), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntil(WhenACreatureDealsCombatDamageToAPermanent(SinglePermanent(Ref_TargetPermanent), IsCardtype(Creature)), ActionList([DrawACard]), UntilEndOfTurn)])))");
    let Ability::Activated { body, .. } = &mut c.abilities[0] else { panic!() };
    let Instruction::Effect { operation: Operation::CreateTrigger { intervening_if, .. }, .. } = &mut body.instructions[0] else { panic!() };
    *intervening_if = Some(condition);
    c
}
fn named(object: ObjectRef, name: &str) -> Condition {
    Condition::ObjectMatches { object, filter: ObjectFilter::Name(name.into()) }
}
#[test]
fn creator_and_event_dependent_conditions_do_not_run_as_watcher() {
    let creator = named(ObjectRef::Source, "creator");
    let event_conditions = [
        named(ObjectRef::Damage(DamageSubject::Source), "creator"),
        named(ObjectRef::EventSubject, "creator"),
        Condition::PlayerIsActive(PlayerRef::ControllerOf(Box::new(ObjectRef::Damage(DamageSubject::Source)))),
        Condition::Compare { left: Expr::EventAmount, comparison: Comparison::Greater, right: Expr::Constant(0) },
        Condition::ExistsPlayer { players: PlayerSet::All, binding: Binding(0), condition: Box::new(Condition::All(vec![Condition::PlayerIsActive(PlayerRef::Bound(Binding(0))), named(ObjectRef::Damage(DamageSubject::Source), "creator")])) },
    ];
    for event_condition in event_conditions {
        let c = damage_with_condition(Condition::All(vec![creator.clone(), event_condition]));
        let gap = code(&c, &[FEATURE]).unwrap_err();
        assert!(gap.contains("condition reading both its creator and event"), "{gap}");
    }
    let original_target = damage_with_condition(Condition::All(vec![creator, named(ObjectRef::Target { slot: Slot(0), index: 0 }, "creator")]));
    let gap = code(&original_target, &[FEATURE]).unwrap_err();
    assert!(gap.contains("target reference") || gap.contains("condition reading both its creator and event"), "{gap}");
}
#[test]
fn creator_literal_named_event_and_event_only_conditions_keep_their_contexts() {
    let creator = code(&damage_with_condition(named(ObjectRef::Source, "Event Horizon")), &[FEATURE]).unwrap();
    assert!(creator.contains(".condition("), "{creator}");
    syn::parse_file(&creator).unwrap();
    let bound = Condition::ExistsPlayer { players: PlayerSet::All, binding: Binding(0), condition: Box::new(Condition::PlayerIsActive(PlayerRef::Bound(Binding(0)))) };
    let bound_creator = code(&damage_with_condition(Condition::All(vec![named(ObjectRef::Source, "creator"), bound])), &[FEATURE]).unwrap();
    assert!(bound_creator.contains(".condition("), "{bound_creator}");
    syn::parse_file(&bound_creator).unwrap();
    let event = code(&damage_with_condition(named(ObjectRef::EventSubject, "victim")), &[FEATURE]).unwrap();
    assert!(event.contains("subject.object()") && event.contains("item.subject"), "{event}");
    syn::parse_file(&event).unwrap();
    let watched = code(&damage_with_condition(named(ObjectRef::Target { slot: Slot(0), index: 0 }, "victim")), &[FEATURE]).unwrap_err();
    assert!(watched.contains("target reference") || watched.contains("watched object"), "{watched}");
}
