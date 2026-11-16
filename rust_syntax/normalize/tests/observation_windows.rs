//! A live top-card query can use an observation only inside its stable window.
use mtgish_ast::{ir, mtg_types::OracleCard, unified, validate, visit::{Visitor, Walk}};

fn moves(actions: &str, targeted: bool) -> Vec<ir::ObjectSet> {
    let actions = if targeted {
        format!("Targeted([TargetPlayer(AnyPlayer)],ActionList([{actions}]))")
    } else {
        format!("ActionList([{actions}])")
    };
    let source: OracleCard = ron::from_str(&format!(
        "Card(Name:\"Observation window\",Typeline:(Supertypes:[],Cardtypes:[Sorcery],Subtypes:[]),Rules:[SpellActions({actions})])"
    )).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    struct Moves(Vec<ir::ObjectSet>);
    impl Visitor for Moves {
        fn operation(&mut self, operation: &ir::Operation) -> bool {
            if let ir::Operation::Move { objects, .. } = operation {
                self.0.push(objects.clone());
            }
            true
        }
    }
    let mut moves = Moves(vec![]);
    lowered.card.walk(&mut moves);
    assert_eq!(moves.0.len(), 1);
    moves.0
}

#[test]
fn pure_condition_keeps_the_revealed_card() {
    let moves = moves(
        "RevealTopCardOfLibrary,If(ACardWasRevealedThisWay(AnyCard),[PutTopOfLibraryInHand])",
        false,
    );
    assert!(matches!(moves[0], ir::ObjectSet::Observed(_)));
}

#[test]
fn a_draw_invalidates_the_top_card_identity_proof() {
    let moves = moves("RevealTopCardOfLibrary,DrawACard,PutTopOfLibraryInHand", false);
    assert!(matches!(moves[0], ir::ObjectSet::Take { .. }));
}

#[test]
fn another_players_library_does_not_share_the_observation() {
    let moves = moves(
        "RevealTopCardOfLibrary,PlayerAction(Ref_TargetPlayer,PutTopOfLibraryInHand)",
        true,
    );
    assert!(matches!(moves[0], ir::ObjectSet::Take { .. }));
}
