/// Counter moves and placements and a highest-reveal. Steps that need results of
/// their own are in `lower_search.rs`.
fn reduce_search_action(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Cost as Paid, Object, Players, Term};
    if let Some(form) = name.strip_prefix("PutCountersAction::") {
        return reduce_search_counters(actor, form, operands);
    }
    Some(match (name.strip_prefix("Action::")?, operands) {
        ("MoveCounters", [Term { family, form, operands }]) if family == "MoveCountersAction" => {
            return reduce_counter_move(actor, form, operands)
        }
        ("PutOrRemoveCounters", [Term { form, operands, .. }])
            if form == "DuplicateOrRemoveACounterOfEachKindOnPermanent" =>
        {
            let [Object(object)] = operands.as_slice() else {
                return None;
            };
            Operation::EachCounterKind {
                actor: actor.clone(),
                among: ObjectSet::One(object.clone()),
                options: vec![
                    CounterStep::PutThatKind(object.clone()),
                    CounterStep::RemoveThatKind(object.clone()),
                ],
                optional: false,
            }
        }
        ("PlayersRevealTopCardOfLibraryAndFindHighestManaValue", [Players(players)]) => {
            Operation::Reveal {
                objects: ObjectSet::LibraryTops(players.clone()),
            }
        }
        (
            "Reflexive_CastGraveyardCardWithAdditionalCostIntoExile_WhenYouCastThatSpell",
            [Object(card), cost @ Paid(_)],
        ) => Operation::PlayWith {
            actor: actor.clone(),
            action: CardPlayAction::CastSpell,
            cards: ObjectSet::One(card.clone()),
            count: Count::AsManyAsCan(Expr::Constant(1)),
            terms: PlayTerms {
                cost: PlayCost::Additional(cost.clone()),
                into_exile: true,
                ..play_terms()
            },
            group: None,
        },
        _ => return None,
    })
}

fn reduce_counter_move(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Object, Objects};
    let (kind, from, to, amount) = match (form, operands) {
        (
            "ACounterOfEachTypeFromPermanentOntoNewPermanentThatNewPermanentDoesntHave",
            [Object(from), Object(to)],
        ) => (
            None,
            from,
            ObjectSet::One(to.clone()),
            MovedCounters::OneOfEachKindMissing,
        ),
        (
            "DistributeAnyNumberOfCountersOfTypeFromPermanentOntoNewPermanents",
            [kind, Object(from), Objects(to)],
        ) => (
            Some(counter_kind(kind)?),
            from,
            to.clone(),
            MovedCounters::AnyNumber,
        ),
        _ => return None,
    };
    Some(Operation::MoveCounters {
        actor: actor.clone(),
        from: ObjectSet::One(from.clone()),
        to,
        kind,
        amount,
    })
}

fn reduce_search_counters(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{List, Object, Objects};
    let one = |object: &ObjectRef| Recipients::Objects(ObjectSet::One(object.clone()));
    Some(match (form, operands) {
        // The kinds are the ones `lacking` doesn't have, so it must be what gets one.
        (
            "ACounterOfTypeAtRandomThatPermanentDoesHaveOnPermanent",
            [List(kinds), Object(lacking), Object(object)],
        ) if format!("{lacking:?}") == format!("{object:?}") => Operation::PutChosenCounters {
            objects: ObjectSet::One(object.clone()),
            choice: CounterChoice::RandomMissing(counter_kinds(kinds)?),
        },
        (
            "ADuplicateOfEachKindOfCounterAmoungPermanentsOrACounterOfTypeOnPermanent",
            [Objects(among), kind, Object(object)],
        ) => Operation::EachCounterKind {
            actor: actor.clone(),
            among: among.clone(),
            options: vec![
                CounterStep::Put(counter_kind(kind)?, object.clone()),
                CounterStep::PutThatKind(object.clone()),
            ],
            optional: true,
        },
        (
            "ForEachPermanentMayDuplicateACounterOfPermanentOnPermanent",
            [Objects(objects), Object(from), Object(to)],
        ) => Operation::EachObject {
            objects: objects.clone(),
            action: Box::new(Operation::DuplicateCounters {
                from: one(from),
                to: one(to),
                copies: CounterCopies::UpToOne,
                kind: None,
            }),
        },
        _ => return None,
    })
}

/// The cards revealed by the highest-mana-value reveal that tie for the highest.
fn highest_revealed() -> ObjectSet {
    let revealed = || ObjectSet::Earlier {
        what: EarlierObjects::Revealed,
        by: None,
    };
    ObjectSet::Matching {
        objects: Box::new(revealed()),
        filter: ObjectFilter::Extreme {
            among: Box::new(revealed()),
            property: Property::ManaValue,
            highest: true,
        },
    }
}

fn reduce_search_condition(form: &str, operands: &[Operand]) -> Option<Condition> {
    match (form, operands) {
        ("PlayersRevealTopCardOfLibraryAndFindHighestManaValue_HasASingleWinner", []) => {
            Some(Condition::Compare {
                left: Expr::CountObjects(Box::new(highest_revealed())),
                comparison: Comparison::Equal,
                right: Expr::Constant(1),
            })
        }
        _ => None,
    }
}

fn reduce_search_cost(name: &str, operands: &[Operand]) -> Option<CostPart> {
    use Operand::{Object, Term};
    let ("Cost::MoveCounters", [Term { family, form, operands }]) = (name, operands) else {
        return None;
    };
    match (family.as_str(), form.as_str(), operands.as_slice()) {
        (
            "MoveCountersCost",
            "AtLeastOneCounterFromPermanentOntoNewPermanent",
            [Object(from), Object(to)],
        ) => Some(CostPart::MoveCounters {
            from: ObjectSet::One(from.clone()),
            to: ObjectSet::One(to.clone()),
            kind: None,
            amount: MovedCounters::AtLeast(Expr::Constant(1)),
        }),
        _ => None,
    }
}
