use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-control-mana/orcish_squatters.rs"] mod orcish_squatters;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-control-mana/naked_singularity.rs"] mod naked_singularity;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-control-mana/reality_twist.rs"] mod reality_twist;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter()
        .filter(|c| !["Orcish Squatters", "Naked Singularity", "Reality Twist"].contains(&c.name))
        .chain([&orcish_squatters::CARD, &naked_singularity::CARD, &reality_twist::CARD])))
}
fn play(name: &str, text: &str) -> cantrip_script::run::ScriptResult {
    run_on(&GameScript::parse(text,name).unwrap(),table()).unwrap_or_else(|e| panic!("{}",e.report))
}
fn choice(name: &str, text: &str, second: &str) {
    let result=play(name,text);
    let RequestKind::ChooseReplacement { hints, .. }=&result.game.pending().expect("handover prompt").kind else { panic!("not a replacement choice") };
    assert_eq!(hints.len(),2);
    assert_ne!(hints[0],hints[1],"the player must be able to distinguish the replacements");
    assert!(hints.contains(&"Plains produce {R}") && hints.contains(&second),"{hints:?}");
}
#[test] fn lost_and_regained_control_does_not_restart_squatters_duration() { play("interrupted",include_str!("fixtures/early-control-mana/orcish-squatters-control-interrupted-before-resolution.cantrip")); }
#[test] fn losing_control_before_resolution_prevents_control() { play("lost",include_str!("fixtures/early-control-mana/orcish-squatters-loses-control-before-trigger-resolves.cantrip")); }
#[test] fn uninterrupted_control_gains_the_land() { play("unbroken",include_str!("fixtures/early-control-mana/orcish-squatters-steals-land-when-unblocked.cantrip")); }
#[test] fn naked_singularity_names_each_applicable_rule() { choice("naked",include_str!("fixtures/early-control-mana/naked-singularity-choice.cantrip"),"Islands produce {G}"); }
#[test] fn reality_twist_names_each_applicable_rule() { choice("reality",include_str!("fixtures/early-control-mana/reality-twist-choice.cantrip"),"Swamps produce {G}"); }
#[test] fn naked_singularity_replacement_keeps_its_mana_result() { play("naked-result",include_str!("fixtures/early-control-mana/naked-singularity-result.cantrip")); }
#[test] fn reality_twist_replacement_keeps_its_mana_result() { play("reality-result",include_str!("fixtures/early-control-mana/reality-twist-result.cantrip")); }
