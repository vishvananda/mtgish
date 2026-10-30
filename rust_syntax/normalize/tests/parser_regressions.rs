use mtgish_ast::{corpus, inspect, mtg_types::*};

fn fixture(name: &str) -> OracleCard {
    include_str!("fixtures/parser_repairs.ronl")
        .lines()
        .map(|line| ron::from_str::<OracleCard>(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap_or_else(|| panic!("missing fixture {name}"))
}

fn rules(card: &OracleCard) -> &[Rule] {
    match card {
        OracleCard::Card {
            rules: Some(rules), ..
        } => rules,
        _ => panic!("expected a regular card with abilities"),
    }
}

fn actions(body: &Actions) -> &[Action] {
    match body {
        Actions::ActionList(body) => body,
        Actions::Targeted(_, body) => actions(body),
        _ => panic!("expected ordinary instructions"),
    }
}

fn activated(card: &OracleCard) -> &[Action] {
    rules(card)
        .iter()
        .find_map(|rule| match rule {
            Rule::Activated(_, body) => Some(actions(body)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn double_kicker_has_two_independent_rust_abilities_in_all_affected_cards() {
    let mut count = 0;
    for line in include_str!("fixtures/kicker.ronl").lines() {
        let card: OracleCard = ron::from_str(line).unwrap();
        let kickers: Vec<_> = rules(&card)
            .iter()
            .filter_map(|rule| match rule {
                Rule::Kicker(cost) => Some(cost.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(kickers.len(), 2, "{:?}", corpus::Identity::of(&card));
        assert!(kickers.iter().all(|cost| matches!(cost, Cost::PayMana(_))));
        count += 1;
    }
    assert_eq!(count, 18);
}

#[test]
fn repaired_parser_output_survives_rust_ron_and_binary_exports() {
    // Match the converter's main-thread stack for the large legacy enum's
    // recursive bincode traversal; Rust test threads otherwise default to 2 MiB.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for line in include_str!("fixtures/parser_repairs.ronl").lines() {
                let card: OracleCard = ron::from_str(line).unwrap();
                let bytes = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
                let (decoded, used): (OracleCard, usize) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
                assert_eq!(used, bytes.len());
                assert_eq!(
                    inspect::value(&card).unwrap(),
                    inspect::value(&decoded).unwrap()
                );
                let exported = ron::ser::to_string(&decoded).unwrap();
                let reparsed: OracleCard = ron::from_str(&exported).unwrap();
                assert_eq!(
                    inspect::value(&card).unwrap(),
                    inspect::value(&reparsed).unwrap()
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn brainstorm_keeps_distinct_draw_and_return_counts_in_rust() {
    let card = fixture("Brainstorm");
    let Rule::SpellActions(body) = &rules(&card)[0] else {
        panic!()
    };
    let body = actions(body);
    assert_eq!(body.len(), 2);
    let Action::DrawNumberCards(draw) = &body[0] else {
        panic!()
    };
    let Action::PutNumCardsFromHandOnTopOfLibraryAnyOrder(put_back) = &body[1] else {
        panic!()
    };
    assert!(matches!(**draw, GameNumber::Integer(3)));
    assert!(matches!(**put_back, GameNumber::Integer(2)));
}

#[test]
fn gideon_and_mechtitan_preserve_every_ability_in_rust() {
    let gideon = fixture("Gideon Blackblade");
    let Action::ChooseAnAbility(choices) = &activated(&gideon)[0] else {
        panic!()
    };
    assert!(matches!(
        choices.as_slice(),
        [Rule::Vigilance, Rule::Lifelink, Rule::Indestructible]
    ));
    let mechtitan = fixture("Mechtitan Core");
    let Action::CreateTokens(tokens, _) = &activated(&mechtitan)[0] else {
        panic!()
    };
    let CreatableToken::NamedTokenWithPT(_, _, _, _, _, _, abilities) = &tokens[0] else {
        panic!()
    };
    assert!(matches!(
        abilities.as_slice(),
        [
            Rule::Flying,
            Rule::Vigilance,
            Rule::Trample,
            Rule::Lifelink,
            Rule::Haste
        ]
    ));
}

#[test]
fn repaired_perpetual_and_type_changes_keep_instruction_boundaries() {
    let teyo = fixture("Teyo, Aegis Adept");
    let body = activated(&teyo);
    assert_eq!(body.len(), 2);
    let Action::CreatePerpetualPermanentEffect(_, first) = &body[0] else {
        panic!()
    };
    let Action::CreatePerpetualPermanentEffect(_, second) = &body[1] else {
        panic!()
    };
    assert!(matches!(first.as_slice(), [PerpetualEffect::SetPower(_)]));
    assert!(matches!(
        second.as_slice(),
        [PerpetualEffect::AddAbility(_)]
    ));
    let clav = fixture("Clavileño, First of the Blessed");
    // Its trigger grants an ability after adding Demon, without removing abilities.
    let Rule::TriggerA(_, body) = &rules(&clav)[0] else {
        panic!()
    };
    let body = actions(body);
    assert_eq!(body.len(), 2);
    let Action::CreatePermanentLayerEffect(_, first) = &body[0] else {
        panic!()
    };
    let Action::CreatePermanentLayerEffect(_, second) = &body[1] else {
        panic!()
    };
    assert!(matches!(
        first.as_slice(),
        [LayerEffect::AddCreatureType(CreatureType::Demon)]
    ));
    assert!(matches!(second.as_slice(), [LayerEffect::AddAbility(_)]));
}
