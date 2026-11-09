use mtgish_ast::{canonical, corpus, ir::*, lower, mtg_types as source, validate};
use serde_json::Value;

fn card(line: &str) -> Card {
    let source: source::OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    card
}
fn halfdane() -> Card {
    card(
        include_str!("fixtures/next_upkeep_end.ronl")
            .lines()
            .next()
            .unwrap(),
    )
}
fn duration(card: &mut Card) -> &mut Duration {
    let Ability::Triggered { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { duration, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!("expected one continuous effect")
    };
    duration
}
#[test]
fn halfdane_preserves_the_complete_program_and_uses_the_end_of_the_next_upkeep() {
    corpus::on_large_stack(|| {
        let line = include_str!("fixtures/next_upkeep_end.ronl")
            .lines()
            .next()
            .unwrap();
        let mut actual = halfdane();
        assert!(matches!(
            duration(&mut actual),
            Duration::UntilEndOfNextUpkeep(PlayerRef::AbilityController)
        ));
        let before = card(&line.replace("UntilEndOfNextUpkeep(You)", "UntilNextUpkeep(You)"));
        *duration(&mut actual) = Duration::UntilNext {
            point: TurnPoint::UpkeepBegins,
            players: PlayerSet::One(PlayerRef::AbilityController),
        };
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(before).unwrap()
        );
    });
}
#[test]
fn all_next_upkeep_callers_roundtrip_and_keep_beginning_and_end_distinct() {
    corpus::on_large_stack(|| {
        for line in include_str!("fixtures/next_upkeep_end.ronl").lines() {
            let source: source::OracleCard = ron::from_str(line).unwrap();
            let bytes = bincode::encode_to_vec(&source, bincode::config::standard()).unwrap();
            let (decoded, used): (source::OracleCard, usize) =
                bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
            assert_eq!(used, bytes.len());
            assert_eq!(
                ron::to_string(&source).unwrap(),
                ron::to_string(&decoded).unwrap()
            );
            let actual = card(line);
            let bytes = bincode::encode_to_vec(&actual, bincode::config::standard()).unwrap();
            let (decoded, used): (Card, usize) =
                bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
            assert_eq!(used, bytes.len());
            validate::card(&decoded).unwrap();
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(decoded).unwrap()
            );
            let mut canonical = serde_json::to_value(&actual.abilities).unwrap();
            canonical::abilities(&mut canonical).unwrap();
            let parsed: Vec<Ability> = serde_json::from_value(canonical.clone()).unwrap();
            validate::abilities(&parsed).unwrap();
            let mut twice = canonical.clone();
            canonical::abilities(&mut twice).unwrap();
            assert_eq!(canonical, twice);
            if actual.name != "Halfdane" {
                assert!(line.contains("UntilNextUpkeep(You)"));
                assert!(!serde_json::to_string(&actual)
                    .unwrap()
                    .contains("UntilEndOfNextUpkeep"));
                assert!(serde_json::to_string(&actual)
                    .unwrap()
                    .contains("UpkeepBegins"));
            }
        }
    });
}
#[test]
fn exact_upkeep_end_validates_its_player_and_is_not_an_alias_for_upkeep_beginning() {
    corpus::on_large_stack(|| {
        let mut c = halfdane();
        *duration(&mut c) = Duration::UntilEndOfNextUpkeep(PlayerRef::Bound(LoopBinding(999)));
        assert!(validate::card(&c).is_err());
        let mut end: Value = serde_json::to_value(halfdane().abilities).unwrap();
        let mut begin = end.clone();
        begin[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["duration"] = serde_json::json!({"UntilNext":{"point":"UpkeepBegins","players":{"kind":"One","args":{"kind":"AbilityController"}}}});
        canonical::abilities(&mut end).unwrap();
        canonical::abilities(&mut begin).unwrap();
        assert_ne!(end, begin);
    });
}
#[test]
#[cfg(feature = "cantrip")]
fn generator_refuses_an_exact_duration_that_the_engine_cannot_represent() {
    corpus::on_large_stack(|| {
        let error = mtgish_ast::cantrip::module(&halfdane()).unwrap_err();
        assert!(error.feature.contains("UntilEndOfNextUpkeep"), "{error:?}");
    });
}
