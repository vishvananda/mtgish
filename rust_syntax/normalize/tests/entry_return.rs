use mtgish_ast::{
    canonical, human,
    ir::*,
    mtg_types, unified, validate,
    visit::{Visitor, VisitorMut, Walk, WalkMut},
};

#[derive(Default)]
struct ReturnRecipients(Vec<serde_json::Value>);
impl Visitor for ReturnRecipients {
    fn operation(&mut self, operation: &Operation) -> bool {
        if let Operation::Move {
            destination: MovementDestination::Exile { arrival: entry, .. },
            ..
        } = operation
        {
            for state in &entry.states {
                if let ExileState::ExiledForWith { body, .. } = state {
                    for instruction in &body.instructions {
                        if let Instruction::Effect {
                            operation: Operation::PutCounters { placements },
                            ..
                        } = instruction
                        {
                            self.0.extend(
                                placements
                                    .iter()
                                    .map(|p| serde_json::to_value(&p.recipients).unwrap()),
                            );
                        }
                    }
                }
            }
        }
        true
    }
}
fn checked_return(card: &UnifiedCard) {
    validate::unified(card).unwrap();
    let mut found = ReturnRecipients::default();
    card.walk(&mut found);
    assert_eq!(
        found.0,
        vec![
            serde_json::json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"EventSubject"}}})
        ]
    );
}

#[test]
fn lagrella_retains_the_returning_permanent_through_canonicalization_and_authoring() {
    let source: mtg_types::OracleCard =
        ron::from_str(include_str!("fixtures/entry_return.ronl")).unwrap();
    let mut card = unified::lower(&source).card;
    checked_return(&card);
    checked_return(&canonical::mapped_unified(&card).unwrap().card);
    let document = human::document::from_card(&card).unwrap();
    let mut changed = canonical::mapped_unified(&card).unwrap();
    struct ReplaceReturningPermanent;
    impl VisitorMut for ReplaceReturningPermanent {
        fn operation(&mut self, operation: &mut Operation) -> bool {
            if let Operation::PutCounters { placements } = operation {
                for placement in placements {
                    if matches!(
                        &placement.recipients,
                        Recipients::Objects(ObjectSet::One(ObjectRef::EventSubject))
                    ) {
                        placement.recipients =
                            Recipients::Objects(ObjectSet::One(ObjectRef::Source));
                    }
                }
            }
            true
        }
    }
    changed.card.walk_mut(&mut ReplaceReturningPermanent);
    validate::unified(&changed.card).unwrap();
    assert!(document
        .transport(&changed.card, &changed.map)
        .unwrap_err()
        .contains("meaning"));
    checked_return(
        human::document::read(&document.write().unwrap())
            .unwrap()
            .card(),
    );
    let Ability::Triggered {
        announcement, body, ..
    } = &card.faces[0].abilities[0]
    else {
        panic!("missing ETB trigger")
    };
    card.faces[0].abilities[0] = Ability::Spell {
        announcement: announcement.clone(),
        body: body.clone(),
    };
    checked_return(&card);
    checked_return(&canonical::mapped_unified(&card).unwrap().card);
}
