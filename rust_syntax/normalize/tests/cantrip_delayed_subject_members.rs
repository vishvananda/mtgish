#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*};
use quote::ToTokens;
use std::{collections::BTreeMap, path::PathBuf};
const FEATURES: &[&str] = &["aura-graveyard-identity", "delayed-subject-members", "snow-mana"];

fn card(name: &str) -> Card {
    let source = include_str!("fixtures/delayed_subject_members.ronl").lines().find(|line| line.contains(&format!("Name: {name:?}"))).unwrap();
    let card = mtgish_ast::lower::card(&ron::from_str(source).unwrap()).unwrap();
    mtgish_ast::validate::card(&card).unwrap(); card
}
fn generate(name: &str) -> String {
    let code = cantrip::planned::with_features(FEATURES, || cantrip::module(&card(name))).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("delayed-capture-generated");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{}.rs", name.replace(' ', "_").to_lowercase())), &code).unwrap();
    code
}
#[test]
fn infinite_authority_captures_the_original_first_creature_across_both_delays() {
    let code = generate("Infinite Authority");
    assert!(code.contains("g.delayed_trigger_with("), "{code}");
    assert!(!code.contains("split(|&id| id == ObjectId::NONE)"), "{code}");
    let file = syn::parse_file(&code).unwrap();
    for item in file.items { if let syn::Item::Fn(f) = item { if f.sig.ident.to_string().starts_with("delayed_") {
        assert!(!f.to_token_stream().to_string().contains("attached_to"), "a delayed callback cannot reread the Aura's live attachment: {code}");
    }}}
}
#[test]
fn gift_of_immortality_captures_the_aura_card_and_the_returned_creature() {
    let code = generate("Gift of Immortality");
    assert!(code.contains("g.delayed_trigger_with("), "{code}");
    assert!(code.contains("found_aura_after_enchanted_left"), "{code}");
    let file = syn::parse_file(&code).unwrap();
    for item in file.items { if let syn::Item::Fn(f) = item { if f.sig.ident.to_string().starts_with("delayed_") {
        assert!(!f.to_token_stream().to_string().contains("found_aura_after_enchanted_left"), "the Aura's first graveyard identity is captured by the creator: {code}");
    }}}
}
#[test]
fn next_of_kin_captures_the_aura_card_and_each_chosen_creature() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("delayed-subject-members-tracked.jsonl");
    cantrip::planned::with_features(FEATURES, || cantrip::planned::tracked(&fixtures.join("delayed_subject_members.ronl"), &fixtures.join("delayed_subject_members.oracle.json"), &output)).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap(); (row["name"].as_str().unwrap().into(), row)
    }).collect();
    let row = &rows["Next of Kin"]; assert!(row["file"].is_null(), "the commander gap still prevents complete generation");
    let code = row["faces"][0]["rust"].as_str().unwrap();
    let gaps = row["faces"][0]["gaps"].as_array().unwrap();
    assert!(!gaps.is_empty()); assert!(gaps.iter().all(|g| g["feature"] == "engine: no commander"), "{gaps:?}");
    assert_eq!(code.matches("g.delayed_trigger_with(").count(), 2, "both actual choices keep their references: {code}");
    syn::parse_file(code).unwrap();
}

#[test]
fn distributed_shares_remain_a_real_object_multiset_in_step_captures() {
    let code = generate("Bounty of the Hunt");
    assert!(code.contains("std::iter::repeat(o).take(share)"), "every divided counter keeps its object: {code}");
    assert!(code.contains("g.delayed_trigger_about("));
    syn::parse_file(&code).unwrap();
}

#[test]
fn the_native_dual_role_feature_is_required_without_forgiving_aura_identity() {
    for name in ["Infinite Authority", "Gift of Immortality", "Next of Kin"] {
        let gaps = cantrip::planned::with_features(&["aura-graveyard-identity"], || cantrip::gaps(&card(name)));
        assert!(gaps.iter().any(|g| g.feature == "step-delayed ability retaining an independent subject and object list"), "{name}: {gaps:?}");
    }
    let gaps = cantrip::planned::with_features(&["delayed-subject-members"], || cantrip::gaps(&card("Gift of Immortality")));
    assert!(gaps.iter().any(|g| g.feature == "Aura graveyard identity after enchanted permanent leaves"), "{gaps:?}");
}

fn capturing(counts: &[i64], order: &[usize], player: bool) -> Card {
    let mut c = card("Gift of Immortality");
    let mut targets: Vec<_> = counts.iter().enumerate().map(|(i, &n)| TargetSpec {
        slot: TargetSlot(i as u16), kind: TargetKind::Object(ObjectDomain { kind: ObjectKind::Permanent, zones: vec![EventZone::Battlefield] }),
        count: Count::UpTo(Expr::Constant(n)), filter: ObjectFilter::Any, players: None, distribute: None, constraints: vec![], chooser: None,
    }).collect();
    let mut instructions: Vec<_> = order.iter().enumerate().map(|(i, &target)| Instruction::Effect { result: ResultId(i as u32), operation: Operation::SetTapped { objects: ObjectSet::Targets(TargetSlot(target as u16)), tapped: true } }).collect();
    if player {
        let slot = TargetSlot(targets.len() as u16);
        targets.push(TargetSpec { slot: slot.clone(), kind: TargetKind::Player, count: Count::Exactly(Expr::Constant(1)), filter: ObjectFilter::Any, players: None, distribute: None, constraints: vec![], chooser: None });
        instructions.push(Instruction::Effect { result: ResultId(100), operation: Operation::Draw { players: PlayerSet::One(PlayerRef::Target { slot, index: 0 }), amount: Expr::Constant(1) } });
    }
    c.abilities = vec![Ability::Spell { announcement: Announcement { targets, modes: None }, body: Program { instructions: vec![Instruction::Effect { result: ResultId(101), operation: Operation::CreateDelayedTrigger {
        event: EventSpec::Turn { point: TurnPoint::EndStepBegins, players: PlayerSet::All }, start: DelayedStart::Immediately, expires: None, intervening_if: None, body: Program { instructions },
    }}] } }];
    mtgish_ast::validate::card(&c).unwrap(); c
}
fn custom(c: &Card) -> Result<String, cantrip::Diagnostic> {
    cantrip::planned::with_features(FEATURES, || cantrip::module(c))
}
fn subject_expression(code: &str) -> String {
    let dense: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    dense.split("letsubject=").nth(1).unwrap().split(";g.delayed_trigger_with").next().unwrap().into()
}
#[test]
fn capture_order_does_not_change_which_proven_singular_reference_uses_the_subject() {
    let a = custom(&capturing(&[1, 1], &[0, 1], false)).unwrap();
    let b = custom(&capturing(&[1, 1], &[1, 0], false)).unwrap();
    assert_eq!(subject_expression(&a), subject_expression(&b));
    assert!(subject_expression(&a).contains("t_0"));
}
#[test]
fn only_a_proven_single_group_can_share_storage_with_a_plural_group() {
    let code = custom(&capturing(&[2, 1], &[0, 1], false)).unwrap();
    assert!(subject_expression(&code).contains("t_1"));
    assert!(!subject_expression(&code).contains("t_0"));
    for c in [capturing(&[2, 2], &[0, 1], false), capturing(&[1, 1, 1], &[0, 1, 2], false)] {
        let gap = custom(&c).unwrap_err();
        assert_eq!(gap.feature, "step-delayed ability carrying more than one plural object group or excess independent references");
    }
}
#[test]
fn a_player_subject_can_accompany_one_real_object_list() {
    let code = custom(&capturing(&[2], &[0], true)).unwrap();
    assert!(subject_expression(&code).contains("Target::Player"));
    assert!(code.contains("q.event_objects(item).to_vec()"));
    let gap = custom(&capturing(&[1, 1], &[0, 1], true)).unwrap_err();
    assert_eq!(gap.feature, "step-delayed ability carrying more than one plural object group or excess independent references");
}

#[test]
fn an_unknown_outer_reference_is_a_gap_instead_of_an_empty_capture() {
    let mut c = capturing(&[1], &[0], false);
    let Ability::Spell { body, .. } = &mut c.abilities[0] else { unreachable!() };
    let Instruction::Effect { operation: Operation::CreateDelayedTrigger { body, .. }, .. } = &mut body.instructions[0] else { unreachable!() };
    let Instruction::Effect { operation: Operation::SetTapped { objects, .. }, .. } = &mut body.instructions[0] else { unreachable!() };
    *objects = ObjectSet::Targets(TargetSlot(7));
    assert!(mtgish_ast::validate::card(&c).is_err());
    assert!(custom(&c).is_err(), "unknown creator reference cannot silently become None");
}

#[test]
fn the_step_feature_does_not_land_event_or_reflexive_list_capture() {
    for reflexive in [false, true] {
        let mut c = capturing(&[2], &[0], false);
        let Ability::Spell { body, .. } = &mut c.abilities[0] else { unreachable!() };
        let Instruction::Effect { operation, .. } = &mut body.instructions[0] else { unreachable!() };
        let Operation::CreateDelayedTrigger { body, .. } = operation else { unreachable!() };
        let body = body.clone(); let announcement = Announcement { targets: vec![], modes: None };
        *operation = if reflexive {
            Operation::ReflexiveTrigger { performed: None, condition: None, announcement, body }
        } else {
            Operation::CreateTrigger { event: EventSpec::Turn { point: TurnPoint::EndStepBegins, players: PlayerSet::All }, duration: Duration::EndOfTurn, announcement, body, once: false, intervening_if: None }
        };
        mtgish_ast::validate::card(&c).unwrap();
        let gaps = |features| cantrip::planned::with_features(features, || cantrip::gaps(&c).into_iter().map(|g| g.feature).collect::<Vec<_>>());
        let before = gaps(&[]); assert!(!before.is_empty()); assert_eq!(gaps(FEATURES), before);
    }
}
