use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, validate};
fn card(name: &str) -> Card {
    let source: OracleCard = include_str!("fixtures/choices.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let card = lower::card(&source).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&card).unwrap();
    card
}
#[test]
fn repaired_cards_keep_choices_out_of_announcement() {
    for (name, count) in [
        ("Great Whale", 7),
        ("Palinchron", 7),
        ("Peregrine Drake", 5),
    ] {
        let card = card(name);
        let (announcement, body) = card
            .abilities
            .iter()
            .find_map(|a| match a {
                Ability::Triggered {
                    announcement, body, ..
                } => Some((announcement, body)),
                _ => None,
            })
            .unwrap();
        assert!(announcement.targets.is_empty());
        let Instruction::Group(group) = &body.instructions[0] else {
            panic!()
        };
        let Instruction::Effect {
            result,
            operation:
                Operation::ChooseObjects {
                    count: Count::UpTo(Expr::Constant(n)),
                    ..
                },
        } = &group.instructions[0]
        else {
            panic!()
        };
        assert_eq!(*n, count);
        assert!(
            matches!(&group.instructions[1], Instruction::Effect {operation:Operation::SetTapped{objects:ObjectSet::Selected(selected),tapped:false},..} if selected.0==result.0)
        );
        let roundtrip: Card = ron::from_str(&ron::ser::to_string(&card).unwrap()).unwrap();
        validate::card(&roundtrip).unwrap();
    }
    card("Abigale, Eloquent First-Year");
    let silk = serde_json::to_string(&card("Silkguard")).unwrap();
    assert!(silk.contains("Modified"));
}
#[test]
fn choices_must_refer_to_a_declared_selection_result() {
    let mut card = card("Peregrine Drake");
    let Ability::Triggered { body, .. } = &mut card.abilities[1] else {
        panic!()
    };
    let Instruction::Group(group) = &mut body.instructions[0] else {
        panic!()
    };
    let Instruction::Effect { operation, .. } = &mut group.instructions[0] else {
        panic!()
    };
    *operation = Operation::Draw {
        players: PlayerSet::One(PlayerRef::AbilityController),
        amount: Expr::Constant(1),
    };
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("object selection"));
}
