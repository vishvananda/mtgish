//! What schema 166 keeps apart, each on the source rule of the card that needed it.

use mtgish_ast::{corpus, lower, mtg_types as source, validate};

fn unearth() -> source::OracleCard {
    include_str!("fixtures/scopes.ronl")
        .lines()
        .map(|l| ron::from_str::<source::OracleCard>(l).unwrap())
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == "Unearth")
        })
        .unwrap()
}

/// One rule on a fixture card, through the whole lowering and the validator.
fn lowered(rule: &str) -> String {
    let mut card = unearth();
    let source::OracleCard::Card { rules, .. } = &mut card else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    let card = lower::card(&card).unwrap_or_else(|e| panic!("{e:?}"));
    validate::card(&card).unwrap_or_else(|e| panic!("{e:?}"));
    format!("{card:?}")
}

#[test]
fn a_total_is_not_each_players_own() {
    let text = lowered(
        "SpellActions(ActionList([EachPlayerStartingWithAction(AnyPlayer, You, [PayAnyAmountOfMana]), EachPlayerCreatesTokens(AnyPlayer, [NumberTokens(TheTotalAmountOfManaPaidThisWay, TokenWithPT(PT(1, 1), Colors([White]), [], [Creature], SubtypesList([Soldier]), []))], [])]))",
    );
    assert!(text.contains("PerformedTotal("), "{text}");
}

#[test]
fn a_surveil_says_what_stayed_on_top() {
    let text = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([Surveil(Integer(2)), DrawNumberCards(TheNumberOfCardsSurveilledOnTopOfLibraryThisWay)]))",
    );
    assert!(text.contains("SurveilKeptOnTop("), "{text}");
}

#[test]
fn a_spell_that_is_targeted_is_a_spell() {
    let text = lowered(
        "TriggerA(WhenASpellBecomesTheTargetOfASpellOrAbility(IsCardtype(Creature), ControlledByAPlayer(Opponent)), ActionList([DrawACard]))",
    );
    assert!(text.contains("OnStack(Spell)"), "{text}");
}

#[test]
fn the_exception_to_the_draw_is_the_own_draw_step() {
    let text = lowered(
        "TriggerA(WhenAPlayerDrawsACardExceptTheFirstCardDuringTheirDrawStep(Opponent), ActionList([DrawACard]))",
    );
    assert!(
        text.contains("FirstDrawInOwnDrawStep(EventSubject)"),
        "{text}"
    );
}

#[test]
fn those_counters_are_the_ones_the_event_put() {
    let text = lowered(
        "TriggerA(WhenAPlayerPutsAnyNumberOfCountersOnAPermanent(Opponent, IsCardtype(Creature)), ActionList([PutCounters([DuplicateThoseCounterOnPermanent(ThisPermanent)])]))",
    );
    assert!(text.contains("from: EventPut"), "{text}");
}

#[test]
fn a_card_returned_to_hand_comes_from_your_graveyard() {
    let text = lowered(
        "SpellActions(ActionList([MillNumberCards(Integer(3)), MayAction(PutACardFromGraveyardIntoHand(IsCardtype(Creature)))]))",
    );
    assert!(
        text.contains("scope: Players { zone: Graveyard, players: One(AbilityController) }"),
        "{text}"
    );
}

#[test]
fn every_spell_and_ability_on_the_stack_is_countered() {
    let text = lowered(
        "SpellActions(ActionList([CounterEachSpellAndAbility(ControlledByAPlayer(Opponent))]))",
    );
    assert!(
        text.contains("Zone { scope: Shared(Stack), kind: Any"),
        "{text}"
    );
}

#[test]
fn a_sneak_cost_paid_this_turn_keeps_this_turn() {
    let text = lowered(
        "TriggerA(WhenACreatureDealsCombatDamageToAPlayer(SinglePermanent(ThisPermanent), AnyPlayer), ActionList([If(PermanentPassesFilter(ThisPermanent, SneakCostWasPaidThisTurn), [DrawACard])]))",
    );
    assert!(text.contains("CostPaidThisTurn(Sneak)"), "{text}");
}
