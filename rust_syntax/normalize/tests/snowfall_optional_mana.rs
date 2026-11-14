use mtgish_ast::{ir::*, lower, mtg_types::OracleCard, validate};

fn blue_amount(mana: &ManaProduction) -> i64 {
    match mana {
        ManaProduction::Amount { color: ManaColor::Blue, amount: Expr::Constant(n) } => *n,
        ManaProduction::All(parts) => parts.iter().map(blue_amount).sum(),
        other => panic!("an alternative must add only the printed blue mana: {other:?}"),
    }
}

fn offered(program: &Program) -> &ManaProduction {
    let [Instruction::Optional { decider, body }] = program.instructions.as_slice() else {
        panic!("the Island's controller must be allowed to decline: {program:?}")
    };
    assert!(matches!(decider, PlayerRef::ControllerOf(object)
        if matches!(object.as_ref(), ObjectRef::EventSubject)));
    let [Instruction::Effect { operation: Operation::AddMana { players, mana, restriction }, .. }]
        = body.instructions.as_slice() else { panic!("{body:?}") };
    assert!(matches!(players, PlayerSet::One(player) if matches!(player,
        PlayerRef::ControllerOf(object) if matches!(object.as_ref(), ObjectRef::EventSubject))));
    assert!(matches!(restriction, Some(ManaUse::Purpose(ManaPurpose::CumulativeUpkeep))));
    mana
}

#[test]
fn optional_snow_self_replacement_retains_the_original_one_blue_choice() {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/snowfall_optional_mana.ronl").trim(),
    ).unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let Ability::Triggered { body, .. } = &card.abilities[1] else { panic!() };
    let [Instruction::If { condition, then, otherwise }] = body.instructions.as_slice() else { panic!() };
    assert!(matches!(condition, Condition::ObjectMatches {
        object: ObjectRef::EventSubject, filter: ObjectFilter::Supertype(_)
    }));
    let ManaProduction::ChooseOne(alternatives) = offered(then) else {
        panic!("snow permits the original blue or the optional two-blue replacement")
    };
    assert_eq!(alternatives.iter().map(blue_amount).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(blue_amount(offered(otherwise)), 1);
}
