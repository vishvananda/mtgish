use mtgish_ast::{ir::*, lower, mtg_types::OracleCard, validate};

fn inspection(source: &str) -> (Card, Program) {
    let source: OracleCard = ron::from_str(source).unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let Ability::Activated { body, .. } = &card.abilities[0] else { panic!() };
    let [Instruction::Effect { operation: Operation::CreateReplacement { body, .. }, .. }]
        = body.instructions.as_slice() else { panic!("{body:?}") };
    let [Instruction::Effect { operation: Operation::ModifyEvent { change: EventChange::Cancel }, .. },
        Instruction::Group(inspection),
        Instruction::Effect { operation: Operation::Draw { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Constant(1) }, .. }]
        = body.instructions.as_slice() else { panic!("cancel, inspect, then an actual draw: {body:?}") };
    let inspection = inspection.clone();
    (card, inspection)
}

#[test]
fn lamp_selects_one_to_leave_and_only_reorders_the_rest() {
    let (_, program) = inspection(include_str!("fixtures/aladdins_lamp.ronl"));
    let [Instruction::Effect { result: seen, operation: Operation::LookAt { .. } },
        Instruction::Effect { result: chosen, operation: Operation::ChooseObjects {
            count: Count::AsManyAsCan(Expr::Constant(1)), .. } },
        Instruction::Effect { operation: Operation::ReorderLibrary {
            objects, end: SequenceEnd::Bottom, order: CardOrder::Random, .. }, .. }]
        = program.instructions.as_slice() else {
            panic!("no put-on-top operation is printed: {program:?}")
        };
    let ObjectSet::InZone { objects, zone: EventZone::Library } = objects else { panic!() };
    let ObjectSet::Difference { objects, excluding } = objects.as_ref() else { panic!() };
    assert!(matches!(objects.as_ref(), ObjectSet::Observed(id) if id.0 == seen.0));
    let ObjectSet::Union(excluded) = excluding.as_ref() else { panic!() };
    assert!(matches!(excluded.as_slice(), [ObjectSet::Selected(id)] if id.0 == chosen.0));
}

#[test]
fn a_different_replacement_sequence_keeps_its_top_reorder() {
    let source = include_str!("fixtures/aladdins_lamp.ronl")
        .replace("PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder", "PutTheRemainingCardsOnTopOfLibraryInAnyOrder");
    let (_, program) = inspection(&source);
    assert!(matches!(&program.instructions[2], Instruction::Effect {
        operation: Operation::ReorderLibrary { end: SequenceEnd::Top, .. }, ..
    }));
    assert_eq!(program.instructions.len(), 4);
}

#[test]
fn telling_times_explicit_put_on_top_still_reorders_the_selected_card() {
    let source: OracleCard = ron::from_str(include_str!("fixtures/inspection.ronl").lines()
        .find(|line| line.starts_with("Card(Name: \"Telling Time\"" )).unwrap()).unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else { panic!() };
    fn top_reorders(program: &Program) -> usize {
        program.instructions.iter().map(|instruction| match instruction {
            Instruction::Group(body) => top_reorders(body),
            Instruction::Effect { operation: Operation::ReorderLibrary { end: SequenceEnd::Top, .. }, .. } => 1,
            _ => 0,
        }).sum()
    }
    assert_eq!(top_reorders(body), 1);
}
