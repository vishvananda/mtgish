/// An event qualified by the activation it happens during, as Forge's `WhileKeyword$`.
fn reduce_last_event(form: &str, operands: &[Operand]) -> Option<EventSpec> {
    use Operand::{Objects, Players};
    Some(match (form, operands) {
        (
            "WhenAPermanentIsExiledFromTheBattlefieldWhileAPlayerIsActivatingACraftAbility",
            [Objects(set), Players(players)],
        ) => EventSpec::WhileActivating {
            event: Box::new(zone_change(
                objects_filter(set),
                Some(EventZone::Battlefield),
                EventZone::Exile,
            )),
            players: players.clone(),
            abilities: ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Craft)),
        },
        _ => return None,
    })
}

/// Stack objects counted by what they made players do, as Forge's `NumPiledGuessedSA`.
fn reduce_last_number(form: &str, operands: &[Operand]) -> Option<Expr> {
    let player = |event, players: &PlayerSet| EventSpec::Player {
        event,
        players: players.clone(),
        objects: None,
    };
    Some(match (form, operands) {
        (
            "TheNumberOfSpellsOrAbilitiesThatCausedAnyNumberOfPlayersToGuessOrToGroupCardsOrPermanentsIntoAPileThisTurn",
            [spells, Operand::Players(players)],
        ) => Expr::EventHistory {
            event: Box::new(caused(
                stack_filter(spells)?,
                EventSpec::AnyOf(vec![
                    player(PlayerEvent::Guesses, players),
                    player(PlayerEvent::GroupsIntoPiles, players),
                ]),
            )),
            window: HistoryWindow::ThisTurn,
            measure: HistoryMeasure::DistinctCauses,
        },
        _ => return None,
    })
}

/// A replacement that keeps the event and changes its outcome.
fn reduce_last_replacement_action(
    actor: &PlayerSet,
    form: &str,
    operands: &[Operand],
) -> Option<Operation> {
    Some(match (form, operands) {
        (
            "RollThatManyAndMayExchangeOneWithPermanentsBasePowerOrBaseToughness",
            [Operand::Object(object)],
        ) => Operation::ModifyEvent {
            change: EventChange::ExchangeDieResult {
                decider: actor_of(actor),
                object: object.clone(),
                properties: vec![Property::BasePower, Property::BaseToughness],
            },
        },
        _ => return None,
    })
}
