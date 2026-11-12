use mtgish_ast::{ir::*,mtg_types::OracleCard,unified,validate};
#[test]
fn glyph_delusion_grants_both_abilities_without_a_duration() {
    let source:OracleCard=ron::from_str(include_str!("fixtures/glyph_delusion_duration.ronl")).unwrap();
    let lowered=unified::lower(&source);assert!(lowered.diagnostics.is_empty());validate::unified(&lowered.card).unwrap();
    let Ability::Spell {body,..}=&lowered.card.faces[0].abilities[0] else {panic!()};
    let Instruction::Effect {operation:Operation::CreateContinuousEffect {changes,duration,..},..}=&body.instructions[1] else {panic!("{body:?}")};
    assert!(matches!(duration,Duration::Indefinite));assert_eq!(changes.len(),2);
    assert!(changes.iter().all(|c|matches!(c,Change::GrantAbility(_))));
    #[cfg(feature="cantrip")]
    {
        let generated=mtgish_ast::cantrip::whole(&source,&lowered.card).unwrap();let face=&generated.faces[0];
        assert!(face.gaps.is_empty(),"{:?}",face.gaps);assert!(face.rust.contains("Duration::Indefinite"));
        assert!(face.rust.contains("Keyword::DoesNotUntap"));assert!(face.rust.contains("AbilityDef::Static"));
        assert!(!face.rust.contains("granted_static("));syn::parse_file(&face.rust).unwrap();
    }
}

#[path="support/source_json.rs"]
mod source_json;
#[test]
fn corrected_source_representations_agree() {
    source_json::assert_fixture_matches(include_str!("fixtures/glyph_delusion_duration.ronl"));
}
