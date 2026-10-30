use mtgish_ast::{corpus, inspect, mtg_types::*};

fn cards() -> Vec<OracleCard> {
    include_str!("fixtures/github_issues.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .collect()
}
fn card(name: &str) -> OracleCard {
    cards()
        .into_iter()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}
fn rules(card: &OracleCard) -> &[Rule] {
    match card {
        OracleCard::Card {
            rules: Some(rules), ..
        } => rules,
        _ => panic!("expected ordinary card"),
    }
}
fn activated(card: &OracleCard) -> &Actions {
    rules(card)
        .iter()
        .find_map(|r| match r {
            Rule::Activated(_, body) | Rule::ActivatedWithModifiers(_, body, _) => {
                Some(body.as_ref())
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn github_repairs_survive_ron_binary_and_json_projection() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            for c in cards() {
                let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
                let (decoded, used): (OracleCard, usize) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
                assert_eq!(used, bytes.len());
                assert_eq!(
                    inspect::value(&c).unwrap(),
                    inspect::value(&decoded).unwrap()
                );
                let ron = ron::ser::to_string(&decoded).unwrap();
                let reparsed: OracleCard = ron::from_str(&ron).unwrap();
                assert_eq!(
                    inspect::value(&c).unwrap(),
                    inspect::value(&reparsed).unwrap()
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn sylvan_library_selects_distinct_drawn_cards_before_each_payment() {
    let c = card("Sylvan Library");
    let Rule::TriggerA(_, body) = &rules(&c)[0] else {
        panic!("expected draw-step trigger")
    };
    let Actions::ActionList(actions) = body.as_ref() else {
        panic!("expected instructions")
    };
    let Action::If(_, then) = &actions[1] else {
        panic!("expected if cards were drawn")
    };
    assert!(
        matches!(&then[0], Action::ChooseNumberCardsInHandOfType(n, filter) if matches!(n.as_ref(), GameNumber::Integer(2)) && matches!(filter.as_ref(), CardsInHand::WasDrawnThisTurn))
    );
    let Action::ActionForEachCardInHand(selected, each) = &then[1] else {
        panic!("expected per-card choices")
    };
    assert!(matches!(
        selected.as_ref(),
        CardsInHand::TheChosenCardsInHand
    ));
    assert!(
        matches!(&each[0], Action::MayCost(cost) if matches!(cost.as_ref(), Cost::PayLife(n) if matches!(n.as_ref(), GameNumber::Integer(4))))
    );
    let Action::Unless(_, unpaid) = &each[1] else {
        panic!("expected fallback on unpaid card")
    };
    assert!(matches!(
        &unpaid[0],
        Action::PutCardFromHandOnTopOfLibrary(CardInHand::EachableCardInHand)
    ));
}

#[test]
fn captains_maneuver_has_two_restricted_targets() {
    let c = card("Captain's Maneuver");
    let Rule::SpellActions(body) = &rules(&c)[0] else {
        panic!("expected spell")
    };
    let Actions::Targeted(targets, _) = body.as_ref() else {
        panic!("expected announced targets")
    };
    let Target::NumberTargetPlayersOrPermanents(n, players, objects) = &targets[0] else {
        panic!("expected restricted targets, excluding battles")
    };
    assert!(matches!(n.as_ref(), GameNumber::Integer(2)));
    assert!(matches!(players.as_ref(), Players::AnyPlayer));
    let Permanents::Or(kinds) = objects.as_ref() else {
        panic!("expected creature or planeswalker")
    };
    assert_eq!(kinds.len(), 2);
    assert!(matches!(
        &kinds[0],
        Permanents::IsCardtype(CardType::Creature)
    ));
    assert!(matches!(
        &kinds[1],
        Permanents::IsCardtype(CardType::Planeswalker)
    ));
}

#[test]
fn skrelv_grants_color_hexproof_and_keeps_toxic() {
    let c = card("Skrelv, Defector Mite");
    let Actions::Targeted(_, body) = activated(&c) else {
        panic!("expected targeted ability")
    };
    let Actions::ActionList(actions) = body.as_ref() else {
        panic!("expected instructions")
    };
    let Action::CreatePermanentLayerEffectUntil(_, changes, _) = &actions[1] else {
        panic!("expected granted abilities")
    };
    assert!(changes.iter().any(|change| matches!(change, LayerEffect::AddAbility(abilities) if abilities.iter().any(|a| matches!(a, Rule::Toxic(1))))));
    assert!(changes.iter().any(|change| matches!(change, LayerEffect::AddAbility(abilities) if abilities.iter().any(|a| matches!(a, Rule::HexproofFrom(Protectable::FromColor(ProtectableColor::TheChosenColor)))))));
}

#[test]
fn untap_choices_and_buyback_reduction_keep_their_direction() {
    let c = card("Peregrine Drake");
    let Rule::TriggerA(_, body) = &rules(&c)[1] else {
        panic!("expected ETB trigger")
    };
    let Actions::ActionList(actions) = body.as_ref() else {
        panic!("must not announce targets")
    };
    assert!(
        matches!(&actions[0], Action::UntapUptoNumberPermanents(n, _) if matches!(n.as_ref(), GameNumber::Integer(5)))
    );
    let c = card("Memory Crystal");
    assert!(
        matches!(&rules(&c)[0], Rule::DecreaseBuybackCosts(mana) if matches!(mana.as_slice(), [ManaSymbol::ManaCostGeneric(2)]))
    );
}
