//! Keywords the engine has by another road, and the final form of those it does
//! not: each planned entry here is the call the card makes once Cantrip has it.
use super::*;
use crate::keyword_actions::KeywordKind;

const M: &str = "cantrip_mechanics";
/// The change an object keeps through every zone for the rest of the game.
const PERPETUAL: &str = "cantrip_mechanics::perpetual::Perpetual";
/// The label of a cost paid as something resolves, where any number can be read.
pub(super) const PAID: &str = "paid";
/// "Remove X storage counters from this land: Add X mana": a mana ability that
/// announces how many counters it removes, unsupported by older Cantrip targets.
pub(super) const MANA_COUNTERS_X: &str = "removing X counters to activate a mana ability";

/// The scalar is this activation's one variable counter-removal component.
pub(super) fn mana_counter_component(cost: &Cost, payment: &PaymentRef, path: &[u32]) -> bool {
    if !matches!(payment, PaymentRef::Activation) {
        return false;
    }
    let [index] = path else { return false };
    let variable = |part: &CostPart| {
        matches!(part, CostPart::RemoveCounters { removals }
        if removals.iter().any(|r| matches!(r.amount, CounterAmount::Count(Count::AnyNumber | Count::Exactly(Expr::AnnouncedX)))))
    };
    if cost.parts.iter().filter(|part| variable(part)).count() != 1 {
        return false;
    }
    matches!(cost.parts.get(*index as usize), Some(CostPart::RemoveCounters { removals })
    if matches!(removals.as_slice(), [CounterRemoval {
        player: PlayerRef::AbilityController,
        recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)), kind: Some(_),
        amount: CounterAmount::Count(Count::AnyNumber | Count::Exactly(Expr::AnnouncedX)), selection: None, ..
    }]))
}

pub(super) fn has_mana_counter_x(cost: &Cost) -> bool {
    cost.parts
        .iter()
        .enumerate()
        .any(|(index, _)| mana_counter_component(cost, &PaymentRef::Activation, &[index as u32]))
}

/// Positive X changes only the amount of these productions, not their possible types.
/// Deliberately exclude conditions, subtraction, and other choices whose outcomes could depend on X.
pub(super) fn x_is_mana_amount(body: &Program, cost: &Cost) -> bool {
    fn amount(n: &Expr, cost: &Cost) -> bool {
        match n {
            Expr::Constant(n) => *n >= 0,
            Expr::AnnouncedX => true,
            Expr::PaidCountersRemoved { payment, component } => {
                mana_counter_component(cost, payment, component)
            }
            Expr::Add(a, b) | Expr::Multiply(a, b) => amount(a, cost) && amount(b, cost),
            _ => false,
        }
    }
    fn production(mana: &ManaProduction, cost: &Cost) -> bool {
        match mana {
            ManaProduction::Amount { amount: n, .. } => amount(n, cost),
            ManaProduction::All(parts) | ManaProduction::ChooseOne(parts) => {
                !parts.is_empty() && parts.iter().all(|part| production(part, cost))
            }
            ManaProduction::Repeat {
                amount: n,
                production: inner,
                ..
            } => amount(n, cost) && production(inner, cost),
            _ => false,
        }
    }
    !body.instructions.is_empty() && body.instructions.iter().all(|instruction| matches!(instruction,
        Instruction::Effect { operation: Operation::AddMana { mana, .. }, .. } if production(mana, cost)))
}

#[cfg(test)]
mod mana_counter_tests {
    use super::*;

    #[test]
    fn counter_payment_reads_require_the_exact_activation_component() {
        let cost = Cost {
            parts: vec![
                CostPart::TapSymbol,
                CostPart::RemoveCounters {
                    removals: vec![CounterRemoval {
                        player: PlayerRef::AbilityController,
                        recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
                        kind: Some(crate::counter::CounterKind::Named(
                            crate::counter::CounterName::ChargeCounter,
                        )),
                        amount: CounterAmount::Count(Count::AnyNumber),
                        selection: None,
                    }],
                },
            ],
        };
        assert!(mana_counter_component(&cost, &PaymentRef::Activation, &[1]));
        for payment in [
            PaymentRef::Casting { ability: 0 },
            PaymentRef::Resolution(ResultId(1)),
            PaymentRef::Branch,
        ] {
            assert!(!mana_counter_component(&cost, &payment, &[1]));
        }
        for path in [vec![], vec![0], vec![1, 0], vec![2]] {
            assert!(!mana_counter_component(
                &cost,
                &PaymentRef::Activation,
                &path
            ));
        }
        let mut fixed = cost.clone();
        if let CostPart::RemoveCounters { removals } = &mut fixed.parts[1] {
            removals[0].amount = CounterAmount::Count(Count::Exactly(Expr::Constant(2)));
        }
        assert!(!has_mana_counter_x(&fixed));
        let mut duplicate = cost.clone();
        duplicate.parts.push(cost.parts[1].clone());
        assert!(!mana_counter_component(
            &duplicate,
            &PaymentRef::Activation,
            &[1]
        ));
    }
}

/// What an optional additional cost sets on the card.
enum Optional {
    Cost(String),
    Kicker(String),
}

/// A cost worked out from the game each time it is asked.
/// A cost's objects that leave nothing to choose, and how many of them it
/// takes: "it" is one (`None`), "the top six cards" exactly six (`Some(6)`).
fn fixed_selection(selection: &CostObjectSelection) -> Option<Option<&Expr>> {
    match (&selection.objects, &selection.choose) {
        (ObjectSet::One(_), None) => Some(None),
        (ObjectSet::Take { count, .. }, Some(Count::Exactly(n)))
            if format!("{count:?}") == format!("{n:?}") =>
        {
            Some(Some(n))
        }
        _ => None,
    }
}

/// Whether a cost is "tap an untapped creature you control" and nothing else.
fn taps_a_creature_you_control(cost: &Cost) -> bool {
    let [CostPart::ChangeTapState {
        selections,
        tapped: true,
        symbol: false,
    }] = cost.parts.as_slice()
    else {
        return false;
    };
    let [CostObjectSelection {
        objects: crate::battlefield_query!(ObjectFilter::AllOf(filters)),
        choose: Some(Count::Exactly(Expr::Constant(1))),
    }] = selections.as_slice()
    else {
        return false;
    };
    let [ObjectFilter::Not(untapped), ObjectFilter::CardType(crate::mtg_types::CardType::Creature), ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController))] =
        filters.as_slice()
    else {
        return false;
    };
    matches!(**untapped, ObjectFilter::Tapped)
}

/// Forecast (CR 702.57): the ability is whole, and the keyword is the requirement.
pub(super) fn forecast(def: String) -> String {
    planned::planned_value("engine: no Forecast", &def, &format!("{def}.forecast()"))
}

/// Keyword actions the shared gate takes an amount for.
pub(super) fn takes_amount(action: KeywordKind) -> bool {
    use KeywordKind as K;
    matches!(
        action,
        K::Blight
            | K::GetTickets
            | K::Awaken
            | K::Endure
            | K::Intensify
            | K::Fateseal
            | K::CollectEvidence
            | K::TakeExtraTurn
    )
}

/// What holds up an ability given to another object, whose keyword is its own requirement.
pub(super) fn grant_gap(ability: &Ability) -> Diagnostic {
    match ability {
        Ability::Mechanic { mechanic, .. } => gap(format!("engine: no {mechanic:?} granted")),
        other => gap(format!("granted {}", form(other))),
    }
}

/// What a tapped-for-mana event's matcher binds: the player, and a bit for each
/// type of mana produced, as `add_one_mana_of_any_type_that_land_produced` reads.
pub(super) const TAPPED_FOR_MANA: &str = "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), Mana::ALL.iter().filter(|&&m| produced.get(m) > 0).fold(0u32, |types, &m| types | 1 << m as u32)];";

/// Whether a triggered mana ability adds what the tap produced.
pub(super) fn adds_what_was_produced(body: &Program) -> bool {
    matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::AddMana {
                mana: ManaProduction::SameTypeAsProduced,
                ..
            },
            ..
        }]
    )
}

/// "Add {U}{U}, {U}{R}, or {R}{R}": the two colors, as a mask.
pub(super) fn filter_land(body: &Program) -> Option<u8> {
    let [Instruction::Effect {
        operation:
            Operation::AddMana {
                players: PlayerSet::One(PlayerRef::AbilityController),
                mana: ManaProduction::ChooseOne(options),
                restriction: None,
            },
        ..
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    let pair = |option: &ManaProduction| -> Option<(usize, usize)> {
        let ManaProduction::All(parts) = option else {
            return None;
        };
        let [ManaProduction::Amount {
            color: a,
            amount: Expr::Constant(1),
        }, ManaProduction::Amount {
            color: b,
            amount: Expr::Constant(1),
        }] = parts.as_slice()
        else {
            return None;
        };
        if matches!(a, ManaColor::Colorless) || matches!(b, ManaColor::Colorless) {
            return None;
        }
        Some((color_index(a).ok()?, color_index(b).ok()?))
    };
    let [first, both, last] = options.as_slice() else {
        return None;
    };
    let ((a, a2), (x, y), (b, b2)) = (pair(first)?, pair(both)?, pair(last)?);
    (a == a2 && b == b2 && a != b && x == a && y == b).then_some((1u8 << a) | (1u8 << b))
}

/// A cost whose size the payer sets as they pay it at resolution.
pub(super) fn payer_sets_the_size(part: &CostPart) -> bool {
    matches!(
        part,
        CostPart::ManaWithX { x: None, .. } | CostPart::ManaAnyNumberOfTimes(_)
    )
}

/// One fixed kind of counter, off one object other than the source.
/// Whether a player action is only "put this on the bottom of its owner's library".
fn moves_self_to_bottom(body: &Program) -> bool {
    matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Move {
                objects: ObjectSet::One(ObjectRef::Source),
                destination: destination @ MovementDestination::Library {position:LibraryPosition::Bottom,..},
                qualifiers,
            },
            ..
        }] if destination.is_plain() && qualifiers.is_empty()
    )
}
/// Collect evidence as the action of a player action, as the cost part it is.
fn evidence_of(body: &Program) -> Option<CostPart> {
    match body.instructions.as_slice() {
        [Instruction::Effect {
            operation:
                Operation::Keyword {
                    action:
                        KeywordAction::CollectEvidence {
                            amount: amount @ Expr::Constant(_),
                        },
                    ..
                },
            ..
        }] => Some(CostPart::Keyword {
            action: CostKeyword::CollectEvidence,
            amount: Some(amount.clone()),
        }),
        _ => None,
    }
}
/// "You may pay {1} and exile it", "you may exile it and collect evidence 4": the
/// one object exiled, and the rest of the price with `Cost` fields of its own. An
/// object with no card to exile can't pay it (Timothar's ruling).
pub(super) fn exile_beside_a_price(cost: &Cost) -> Option<(&ObjectRef, Cost)> {
    let mut exiled = None;
    let mut rest = Vec::new();
    for part in &cost.parts {
        match part {
            CostPart::Move {
                selections,
                destination: destination @ MovementDestination::Exile { .. },
            } if exiled.is_none() && destination.is_plain() => {
                let [CostObjectSelection {
                    objects: ObjectSet::One(object),
                    choose: None,
                }] = selections.as_slice()
                else {
                    return None;
                };
                exiled = Some(object);
            }
            CostPart::Mana(_) | CostPart::Life(_) => rest.push(part.clone()),
            CostPart::PlayerAction { body, .. } => rest.push(evidence_of(body)?),
            _ => return None,
        }
    }
    match (exiled, rest.is_empty()) {
        (Some(object), false) => Some((object, Cost { parts: rest })),
        _ => None,
    }
}
/// "You may discard your hand and draw two cards": each is done as itself, and
/// neither can stop the other.
pub(super) fn discards_hand_and_draws(cost: &Cost) -> bool {
    match cost.parts.as_slice() {
        [CostPart::PlayerAction { body, .. }, CostPart::Draw { .. }] => matches!(
            body.instructions.as_slice(),
            [Instruction::Effect {
                operation: Operation::Discard {
                    amount: Expr::CountObjects(_),
                    random: false,
                    ..
                },
                ..
            }]
        ),
        _ => false,
    }
}
/// "You may untap target tapped creature", "you may untap all lands you control".
pub(super) fn untaps_named_objects(cost: &Cost) -> Option<&ObjectSet> {
    match cost.parts.as_slice() {
        [CostPart::ChangeTapState {
            selections,
            tapped: false,
            symbol: false,
        }] => match selections.as_slice() {
            [CostObjectSelection {
                objects,
                choose: None,
            }] => Some(objects),
            _ => None,
        },
        _ => None,
    }
}
fn removes_from_another(removals: &[CounterRemoval]) -> bool {
    matches!(
        removals,
        [CounterRemoval {
            recipients: Recipients::Objects(ObjectSet::One(object)),
            kind: Some(_),
            amount: CounterAmount::Count(Count::Exactly(Expr::Constant(1..=255))),
            selection: None,
            ..
        }] if !matches!(object, ObjectRef::Source)
    )
}

/// Removing one counter of a fixed kind: done or not, with nothing to divide.
fn removes_one_counter(removals: &[CounterRemoval]) -> bool {
    matches!(
        removals,
        [CounterRemoval {
            recipients: Recipients::Objects(ObjectSet::One(_)),
            kind: Some(_),
            amount: CounterAmount::Count(Count::Exactly(Expr::Constant(1))),
            selection: None,
            ..
        }]
    )
}

/// What the choice of a cost that moves objects is for.
pub(super) fn move_purpose(destination: &Destination) -> &'static str {
    match destination {
        Destination::OwnersHand => "PutIntoHand",
        Destination::OwnersGraveyard => "PutIntoGraveyard",
        Destination::Exile => "Exile",
        Destination::OwnersLibraryTop => "PutOnTop",
        Destination::OwnersLibraryBottom | Destination::OwnersLibraryBottomRandom => "PutOnBottom",
        Destination::Battlefield(_) | Destination::OwnersBattlefield => "PutOntoBattlefield",
        _ => "PutOnLibrary",
    }
}

/// Where a cost finds the objects it moves, and what filters them.
fn cost_zone(objects: &ObjectSet) -> Option<(String, ObjectFilter)> {
    let whose = |players: &PlayerSet| match players {
        PlayerSet::One(PlayerRef::AbilityController) => Some("Yours"),
        PlayerSet::All => Some("Any"),
        _ => None,
    };
    Some(match objects {
        crate::battlefield_query!(filter) => ("CostZone::Battlefield".to_string(), filter.clone()),
        ObjectSet::Zone {
            scope: ZoneScope::Players { zone, players },
            filter,
            ..
        } => (
            format!("CostZone::{zone:?}(Whose::{})", whose(players)?),
            filter.clone(),
        ),
        ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Stack),
            filter,
            ..
        } => ("CostZone::Stack".to_string(), filter.clone()),
        ObjectSet::Take {
            objects,
            count,
            from: SequenceEnd::Top,
        } => match (&**objects, count) {
            (
                ObjectSet::Zone {
                    scope:
                        ZoneScope::Players {
                            zone: EventZone::Library,
                            players,
                        },
                    filter,
                    ..
                },
                Expr::Constant(n),
            ) => (
                format!("CostZone::TopOfLibrary({n}, Whose::{})", whose(players)?),
                filter.clone(),
            ),
            _ => return None,
        },
        ObjectSet::One(ObjectRef::Query { objects, index: 0 }) => match &**objects {
            ObjectSet::Zone {
                scope:
                    ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players,
                    },
                filter,
                ..
            } => (
                format!("CostZone::TopOfGraveyard(Whose::{})", whose(players)?),
                filter.clone(),
            ),
            _ => return None,
        },
        _ => return None,
    })
}

/// What paid a part of an activation's or a spell's cost, where the engine
/// keeps no list of it: discarded and exiled cards, and revealed ones.
pub(super) fn paid_objects(component: &[u32]) -> String {
    planned::planned_value(
        "engine: no record of what paid a cost",
        "Vec::<ObjectId>::new()",
        &format!("q.paid_objects(item, {component:?}).to_vec()"),
    )
}

/// Whether a flash rider is the sacrifice as the next cleanup step begins.
fn sacrificed_at_cleanup(rider: &Operand) -> bool {
    let words = format!("{rider:?}");
    words.contains("CleanupStepBegins") && words.contains("Sacrifice")
}

/// What a cost is paid for, which decides what the engine can take of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Paying {
    Ability,
    /// A way of casting a spell.
    Way,
    /// A spell's additional cost.
    Additional,
}

impl Paying {
    pub(super) fn of_rule(rule: &CastingRule) -> Paying {
        match rule {
            CastingRule::AdditionalCost { .. }
            | CastingRule::Flash { .. }
            | CastingRule::ReduceForEachPaid { .. } => Paying::Additional,
            _ => Paying::Way,
        }
    }
}

/// Whether the removal is one `cost_part` itself reads.
pub(super) fn plain_removal(removals: &[CounterRemoval]) -> bool {
    match removals {
        [CounterRemoval {
            recipients: Recipients::Objects(objects),
            kind,
            amount: CounterAmount::Count(Count::Exactly(Expr::Constant(n))),
            selection,
            ..
        }] => match objects {
            ObjectSet::One(ObjectRef::Source) => selection.is_none() && (kind.is_some() || *n == 1),
            crate::battlefield_query!(_) => selection.as_ref().is_none_or(|s| {
                *n == 1
                    && matches!(s.count, Count::Exactly(Expr::Constant(1)))
                    && matches!(s.player, PlayerRef::AbilityController)
            }),
            _ => false,
        },
        _ => false,
    }
}

/// Every condition an `All` of filters holds, flattened.
fn each_filter(filter: &ObjectFilter) -> Vec<&ObjectFilter> {
    match filter {
        ObjectFilter::AllOf(items) => items.iter().flat_map(each_filter).collect(),
        other => vec![other],
    }
}

/// "Untapped creatures you control other than this one": what `tap_creatures` taps.
fn other_untapped_creatures(filter: &ObjectFilter) -> bool {
    let items = each_filter(filter);
    let has = |test: &dyn Fn(&ObjectFilter) -> bool| items.iter().any(|f| test(f));
    items.len() == 4
        && has(
            &|f| matches!(f, ObjectFilter::Not(inner) if matches!(**inner, ObjectFilter::Tapped)),
        )
        && has(&|f| {
            matches!(
                f,
                ObjectFilter::CardType(crate::mtg_types::CardType::Creature)
            )
        })
        && has(&|f| {
            matches!(
                f,
                ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController))
            )
        })
        && has(
            &|f| matches!(f, ObjectFilter::Not(inner) if matches!(**inner, ObjectFilter::SameObject(ObjectRef::Source))),
        )
}

/// A set rule a cost's choice is held to: the engine's `select` words.
fn selection_rule(constraint: &SelectionConstraint) -> Result<String> {
    Ok(match constraint {
        SelectionConstraint::DifferentNames => format!("{M}::select::DIFFERENT_NAMES"),
        SelectionConstraint::CommonProperty(property) => {
            format!("{M}::select::sharing(Shared::{property:?})")
        }
        SelectionConstraint::TotalProperty {
            property,
            comparison,
            amount: Expr::Constant(n),
        } => format!("{M}::select::total(Total::{property:?}, Comparison::{comparison:?}, {n})"),
        other => return Err(gap(format!("cost selection rule {}", form(other)))),
    })
}

/// The test and the prompt's word for what a behold names.
fn behold_words(what: &Operand) -> Result<(String, String)> {
    let Operand::Term { form, operands, .. } = what else {
        return Err(gap("cost part Behold"));
    };
    match (form.as_str(), operands.as_slice()) {
        ("IsCreatureType" | "IsPlaneswalkerType", [Operand::Vocabulary { form: subtype, .. }]) => {
            if !crate::cantrip_vocab::SUBTYPES.contains(&subtype.as_str()) {
                return Err(gap(format!("subtype {subtype}")));
            }
            Ok((
                format!(
                    "q.chars(id).has_subtype(q.words(asker.source).subtype(Subtype::{subtype}))"
                ),
                subtype.clone(),
            ))
        }
        ("IsCardtype", [Operand::Vocabulary { form: kind, .. }]) => Ok((
            format!("q.chars(id).is(CardType::{kind})"),
            kind.to_ascii_lowercase(),
        )),
        ("And", [a, b]) => {
            let ((a, one), (b, other)) = (behold_words(a)?, behold_words(b)?);
            Ok((format!("{a} && {b}"), format!("{one} {other}")))
        }
        _ => Err(gap("cost part Behold")),
    }
}

/// "Commander" as a test: Cantrip plays no game that has one.
pub(super) fn commander(id: &str) -> String {
    planned::planned_value(
        "engine: no commander",
        "false",
        &format!("q.is_commander({id})"),
    )
}

/// An `EffectKind` Cantrip does not have yet.
pub(super) fn planned_kind(feature: &str, code: String) -> String {
    planned::planned_value(feature, "EffectKind::Pump(0, 0)", &code)
}

/// How long mana lasts, where that is all its `ManaUse` says, and what else it says.
pub(super) fn mana_lasting(value: Option<&ManaUse>) -> (Option<&'static str>, Option<&ManaUse>) {
    match value {
        Some(ManaUse::Persists(Duration::EndOfTurn)) => (Some("until_end_of_turn"), None),
        Some(ManaUse::Persists(Duration::EndOfCombat)) => (Some("until_end_of_combat"), None),
        other => (None, other),
    }
}

/// The engine's words for the spells a filter names, where it has them.
fn spell_words(filter: &ObjectFilter, chose_as_entered: bool) -> Option<String> {
    use crate::mtg_types::CardType as T;
    fn subtype(f: &ObjectFilter) -> Option<String> {
        match f {
            ObjectFilter::Subtype(s) => {
                let word = name(s);
                crate::cantrip_vocab::SUBTYPES
                    .contains(&word.as_str())
                    .then(|| format!("Subtype::{word}"))
            }
            _ => None,
        }
    }
    fn card_types(f: &ObjectFilter) -> Option<Vec<String>> {
        match f {
            ObjectFilter::CardType(t) => Some(vec![format!("CardType::{}", card_type(t).ok()?)]),
            ObjectFilter::AnyOf(v) => {
                let mut all = Vec::new();
                for f in v {
                    all.extend(card_types(f)?);
                }
                Some(all)
            }
            _ => None,
        }
    }
    if let Some(types) = card_types(filter) {
        return Some(format!(
            "ManaRestriction::Spells(TypeSet::of(&[{}]))",
            types.join(", ")
        ));
    }
    if let ObjectFilter::AnyOf(v) = filter {
        let subtypes = v.iter().map(subtype).collect::<Option<Vec<_>>>()?;
        return Some(format!(
            "ManaRestriction::Subtypes(SubtypeSpells::of_any(&[{}]))",
            subtypes.join(", ")
        ));
    }
    if let ObjectFilter::AllOf(v) = filter {
        let creature = |f: &ObjectFilter| matches!(f, ObjectFilter::CardType(T::Creature));
        if let ([a, b], true) = (v.as_slice(), v.iter().any(creature)) {
            let other = if creature(a) { b } else { a };
            if matches!(other, ObjectFilter::Chosen(ValueKind::CreatureType)) {
                return chose_as_entered.then(|| {
                    "ManaRestriction::Subtypes(SubtypeSpells::CHOSEN_TYPE_CREATURE)".into()
                });
            }
            let word = subtype(other)?;
            return Some(format!(
                "ManaRestriction::Subtypes(SubtypeSpells {{ creature: true, ..SubtypeSpells::of({word}) }})"
            ));
        }
    }
    if matches!(filter, ObjectFilter::Chosen(ValueKind::CreatureType)) && chose_as_entered {
        return Some(
            "ManaRestriction::Subtypes(SubtypeSpells { creature: false, ..SubtypeSpells::CHOSEN_TYPE_CREATURE })"
                .into(),
        );
    }
    None
}

/// The printed price of a keyword: its mana where that is all it is.
fn priced(
    c: &mut Compiler,
    cost: Option<&Cost>,
    mechanic: Mechanic,
    label: &str,
) -> Result<String> {
    match cost.map(|c| c.parts.as_slice()) {
        Some([CostPart::Mana(symbols)]) => mana(symbols),
        _ => c.mechanic_cost(cost, mechanic, label),
    }
}

/// What "as an additional cost, you may sacrifice any number of these, and when you
/// do, copy this spell for each" sacrifices, if `cost` is that.
fn sacrifices_to_copy(cost: &Cost) -> Option<ObjectFilter> {
    let [CostPart::PlayerAction {
        player: PlayerRef::AbilityController,
        body,
    }] = cost.parts.as_slice()
    else {
        return None;
    };
    let mut steps = Vec::new();
    lane_m::flat_groups(&body.instructions, &mut steps);
    let [Instruction::Effect {
        operation:
            Operation::Sacrifice {
                objects: crate::battlefield_query!(filter),
                choose: Some(Count::AnyNumber),
                ..
            },
        ..
    }, Instruction::Effect {
        operation:
            Operation::ReflexiveTrigger {
                condition: None,
                body,
                ..
            },
        ..
    }] = steps.as_slice()
    else {
        return None;
    };
    let creatures = matches!(filter, ObjectFilter::CardType(t) if form(t) == "Creature");
    let copies = matches!(body.instructions.as_slice(), [Instruction::Effect {
        operation: Operation::CopyOnStack { object: ObjectRef::Source, exceptions, card: None, .. },
        ..
    }] if exceptions.is_empty());
    (creatures && copies).then(|| filter.clone())
}

impl Compiler {
    /// An event Cantrip does not show to abilities. `in_history` says its record
    /// is already there, so a question of what happened can ask it now.
    pub(super) fn plan_event(
        &self,
        matched: &mut triggers::Matched,
        feature: &str,
        in_history: bool,
    ) -> Result<()> {
        if self.history.is_some() && in_history {
            return Ok(());
        }
        matched.planned.push(feature.into());
        Ok(())
    }
    /// The trigger of an event that can't fire yet: its matcher is written out as
    /// it will be, and the file compiles with one that never matches.
    pub(super) fn planned_trigger(
        &mut self,
        features: &[String],
        label: &str,
        matcher: &str,
        mask: &str,
    ) -> String {
        let feature = features
            .iter()
            .find(|f| !planned::landed(f))
            .expect("a requirement still open");
        let id = planned::requirement(feature);
        // The body may yet fail: the event is a requirement whatever it does.
        self.gaps.push(gap(feature.clone()));
        writeln!(self.holes, "planned_item!({id:?} => matcher: {matcher});").unwrap();
        writeln!(self.functions, "#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ None }}").unwrap();
        let mask = planned::planned_value(feature, "EventMask(0)", &format!("EventMask({mask})"));
        format!("when_asked({mask}, {label})")
    }
    /// A part of a cost sized by the game: worked out as a resolving cost is
    /// asked, and otherwise by the function the cost will hold.
    fn sized_cost(
        &mut self,
        n: &Expr,
        label: &str,
        field: &str,
        width: &str,
        most: &str,
    ) -> Result<String> {
        let build = |code: &str| {
            format!("Cost {{ {field}: ({code}).clamp(0, {most}) as {width}, ..Cost::NONE }}")
        };
        if label == PAID {
            return Ok(build(&self.fetch(self.number(n)?.code)));
        }
        let code = build(&self.asked_number(n)?);
        Ok(self.reading_cost(&code, label))
    }
    /// A cost worked out from the game each time it is asked: a `fn` the cost
    /// holds, as `cost_of_targets` holds one, of the card's own.
    fn reading_cost(&mut self, code: &str, label: &str) -> String {
        const FEATURE: &str = "engine: no cost sized by the game";
        let prefix = format!("{label}_cost_");
        let at = self.functions.matches(&format!("fn {prefix}")).count();
        let function = format!("{prefix}{at}");
        let params = "q: &dyn Query, asker: &Asker, targets: &[Target]";
        let item = planned::planned_fn(FEATURE, &function, params, "Cost", code);
        writeln!(self.functions, "{item}").unwrap();
        planned::planned_value(FEATURE, "Cost::NONE", &format!("Cost::reading({function})"))
    }
    /// A number a cost's function reads, with no stack item in reach.
    fn asked_number(&self, n: &Expr) -> Result<String> {
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        asked.costing_targets = true;
        Ok(asked.number(n)?.code)
    }
    /// "[player] may pay [cost]": the line that asks, naming the payer and whether
    /// they paid. The engine pays a sacrifice of its `source`, so a cost naming
    /// another object is asked of that object.
    pub(super) fn ask_to_pay(
        &mut self,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
    ) -> Result<String> {
        self.helpers.insert("pay_question");
        let object_subject = self.payment_subject.map(|n| format!("bound_{n}"));
        if object_subject.is_some() {
            self.helpers.insert("pay_for_object_question");
        }
        let payer = self.fetch(self.certain_player(player)?);
        self.notes.acting = Some(format!("Some(payer_{id})"));
        let ask = |source: &str, subject: &str, cost: &str| {
            let (subject, question) = match (&object_subject, subject) {
                (Some(object), "asker.source") => (object.as_str(), "PAY_FOR_OBJECT_QUESTION"),
                _ => (subject, "PAY_QUESTION"),
            };
            format!("g.may_pay(payer_{id}, {source}, {subject}, {cost}, &{question})?")
        };
        let beholds: Vec<&CostPart> = cost
            .parts
            .iter()
            .filter(|part| !matches!(part, CostPart::Mana(symbols) if symbols.is_empty()))
            .collect();
        let paid = match cost.parts.as_slice() {
            // "You may behold an Elf. If you do, ..." (CR 701.4a, 608.2d): nobody is
            // asked with nothing to behold (Elven Passage, Sarkhan); "as this enters"
            // leaves the entering card out (Theorist's Sanctum).
            _ if matches!(
                beholds.as_slice(),
                [CostPart::Behold {
                    count: Expr::Constant(1),
                    ..
                }]
            ) =>
            {
                let [CostPart::Behold { what, .. }] = beholds.as_slice() else {
                    unreachable!("one behold");
                };
                let (test, quality) = behold_words(what)?;
                let function = format!("behold_{}", self.fresh());
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {test} }}").unwrap();
                match self.pending == Pending::Entering {
                    true => format!(
                        "{M}::behold::as_enters(g, entering, {function}, {})?",
                        quoted(&quality)
                    ),
                    false => format!(
                        "{M}::behold::on_resolution(g, payer_{id}, asker.source, {function}, {})?",
                        quoted(&quality)
                    ),
                }
            }
            [CostPart::Choice(_)]
                if lane_k::printed_otherwise(self.line_text(), "for each blight counter") =>
            {
                return Err(gap(
                    "ways to pay chosen for every time first, then all paid at once",
                ));
            }
            // One decision: what the first question leaves unpaid the second offers.
            [CostPart::Choice(costs)] => {
                let mut asks = Vec::new();
                for option in costs {
                    let cost = self.cost(option, PAID)?;
                    asks.push(ask("asker.source", "asker.source", &cost));
                }
                asks.join(" || ")
            }
            [CostPart::Sacrifice {
                objects: ObjectSet::One(object),
                choose: None,
            }] if !matches!(object, ObjectRef::Source) => {
                let object = self.fetch(self.object(object)?);
                format!(
                    "match {object} {{ Some(o) => {}, None => false }}",
                    ask("o", "o", "SACRIFICE_THIS")
                )
            }
            // "Pay {3} and sacrifice it", of the object named some other way.
            [price @ CostPart::Mana(_), CostPart::Sacrifice {
                objects: ObjectSet::One(object),
                choose: None,
            }] if !matches!(object, ObjectRef::Source) => {
                let object = self.fetch(self.object(object)?);
                let price = Cost {
                    parts: vec![price.clone()],
                };
                let price = format!("({}).and(SACRIFICE_THIS)", self.cost(&price, PAID)?);
                format!(
                    "match {object} {{ Some(o) => {}, None => false }}",
                    ask("o", "o", &price)
                )
            }
            [CostPart::RemoveCounters { removals }] if removes_from_another(removals) => {
                let [CounterRemoval {
                    recipients: Recipients::Objects(ObjectSet::One(object)),
                    kind: Some(kind),
                    amount: CounterAmount::Count(Count::Exactly(Expr::Constant(n))),
                    ..
                }] = removals.as_slice()
                else {
                    unreachable!("one kind of counter off one object");
                };
                let object = self.fetch(self.object(object)?);
                let price = format!("remove_counters({}, {n})", read::counter_kind(kind)?);
                format!(
                    "match {object} {{ Some(o) => {}, None => false }}",
                    ask("o", "o", &price)
                )
            }
            [CostPart::ManaCostOf(object)] => {
                let object = self.fetch(self.object(object)?);
                format!(
                    "match {object} {{ Some(o) if g.card(g.obj(o).card).has_cost => {{ let price = of_mana(g.card(g.obj(o).card).cost); {} }} _ => false }}",
                    ask("asker.source", "o", "price")
                )
            }
            // "You may reveal it" of the card a player looked at: only a match can.
            [CostPart::RevealTop(filter)] => {
                let test = self.fetch(self.filter(filter, &Candidate::of("card"))?);
                let top = self.fetch(format!(
                    "q.state().zone(Zone::library(payer_{id})).first().copied()"
                ));
                format!("{{ let mut revealed = false; if let Some(card) = {top} {{ if {test} && {M}::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)? {{ g.reveal(payer_{id}, &[card]); revealed = true; }} }} revealed }}")
            }
            _ => {
                let cost = self.cost(cost, PAID)?;
                ask("asker.source", "asker.source", &cost)
            }
        };
        let able = self.can_pay(id, cost);
        Ok(format!(
            "let payer_{id} = {payer}; let paid_{id} = {able}{paid};"
        ))
    }
    /// "You may pay {1} and exile it": `exile_beside_a_price`. The price is one
    /// payment; the card goes as it is paid, from whoever's graveyard it is in,
    /// and collecting evidence can't take the card itself (Lamplight Phoenix's
    /// ruling), so there the price is asked of that card.
    pub(super) fn pay_and_exile(
        &mut self,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Some((object, price)) = exile_beside_a_price(cost) else {
            return Err(gap("cost part Move"));
        };
        // An ability given later to a token reads the card as linked to it: the
        // grant links it (`exiled_for_grant`).
        let later = format!("{:?}", self.after);
        let for_grant = later.contains("GrantAbility") && later.contains("Earlier { what: Exiled");
        let evidence = price
            .parts
            .iter()
            .any(|part| matches!(part, CostPart::Keyword { .. }));
        let source = if evidence { "o" } else { "asker.source" };
        self.helpers.insert("pay_question");
        let payer = self.fetch(self.certain_player(player)?);
        let object = self.fetch(self.object(object)?);
        let price = self.cost(&price, PAID)?;
        writeln!(out, "let payer_{id} = {payer}; let it_{id}: Option<ObjectId> = {object}; let paid_{id} = match it_{id} {{ Some(o) => g.may_pay(payer_{id}, {source}, o, {price}, &PAY_QUESTION)?, None => false }}; let mut moved_{id}: Vec<ObjectId> = Vec::new(); if let (true, Some(o)) = (paid_{id}, it_{id}) {{ moved_{id}.push(g.exile(o)); }}").unwrap();
        self.performed.insert(result.0, format!("paid_{id}"));
        self.moved.insert(result.0, format!("moved_{id}"));
        self.left_behind(result, format!("moved_{id}.clone()"));
        self.notes.exiled_source = Some(format!("moved_{id}.clone()"));
        if for_grant {
            self.notes.exiled_for_grant = Some(format!("moved_{id}.clone()"));
        }
        Ok(())
    }
    /// "You may discard your hand and draw two cards. If you do": one decision.
    pub(super) fn discard_hand_and_draw(
        &mut self,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let [CostPart::PlayerAction { body, .. }, CostPart::Draw { count }] = cost.parts.as_slice()
        else {
            return Err(gap("cost part PlayerAction"));
        };
        let payer = self.fetch(self.certain_player(player)?);
        let count = self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?);
        // A prohibited draw cannot be offered as part of a resolving cost
        // (118.3, 121.3). Replacement and an empty library do not prohibit it.
        // Determine the whole cost before any component changes the state.
        writeln!(out, "let payer_{id} = {payer}; let draw_count_{id} = {count}; let paid_{id} = !g.cant_draw(payer_{id}, draw_count_{id}) && {M}::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)?; if paid_{id} {{").unwrap();
        self.block(body, out, true)?;
        writeln!(
            out,
            "g.draw(payer_{id}, draw_count_{id}); g.end_sentence();"
        )
        .unwrap();
        writeln!(out, "}}").unwrap();
        self.moved.remove(&result.0);
        self.performed.insert(result.0, format!("paid_{id}"));
        Ok(())
    }
    /// "You may untap ...": "if you do" is that something untapped.
    pub(super) fn may_untap(
        &mut self,
        id: usize,
        player: &PlayerRef,
        objects: &ObjectSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let payer = self.fetch(self.certain_player(player)?);
        writeln!(out, "let payer_{id} = {payer}; let mut paid_{id} = false; if {M}::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)? {{").unwrap();
        let untap = Operation::SetTapped {
            objects: objects.clone(),
            tapped: false,
        };
        self.operation(&untap, result, out)?;
        writeln!(out, "paid_{id} = !{}.is_empty(); }}", self.moved(result)?).unwrap();
        self.moved.remove(&result.0);
        self.performed.insert(result.0, format!("paid_{id}"));
        Ok(())
    }
    /// "You may pay one or more {E}": how much is the question, none is declining,
    /// and it is what "that many" reads.
    pub(super) fn may_pay_energy(
        &mut self,
        id: usize,
        player: &PlayerRef,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let payer = self.fetch(self.certain_player(player)?);
        writeln!(out, "let payer_{id} = {payer}; let most_{id} = u32::from(g.state().player(payer_{id}).energy()); let paid_{id}: u32 = if most_{id} > 0 {{ {M}::choose::number(g, payer_{id}, asker.source, 0, most_{id}, \"Pay how much energy\")? }} else {{ 0 }}; if paid_{id} > 0 {{ g.change_energy(payer_{id}, -(paid_{id} as i32)); }}").unwrap();
        self.performed.insert(result.0, format!("paid_{id}"));
        Ok(())
    }
    /// "You may pay {X}. X can't be greater than ...": X is said first, within the
    /// bound, then paid (107.3f), and X afterwards is the X paid.
    pub(super) fn may_pay_bounded_x(
        &mut self,
        id: usize,
        player: &PlayerRef,
        bound: &Expr,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        self.helpers.insert("pay_question");
        let payer = self.fetch(self.certain_player(player)?);
        let bound = self.fetch(self.number(bound)?.code);
        writeln!(out, "let payer_{id} = {payer}; let most_{id} = ({bound}).clamp(0, 255) as u32; let x_{id} = {M}::choose::number(g, payer_{id}, asker.source, 0, most_{id}, \"Choose X\")?; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, Cost::mana(ManaCost {{ generic: x_{id}, ..ManaCost::ZERO }}), &PAY_QUESTION)?; let bound_{id}: i64 = i64::from(x_{id});").unwrap();
        self.performed.insert(result.0, format!("paid_{id}"));
        self.notes.paid_x = Some(format!("bound_{id}"));
        Ok(())
    }
    /// "Add one mana of any type that land produced": the trigger's data names
    /// the player and the types, and with none produced nothing is added (106.12a).
    pub(super) fn same_type_as_produced(&mut self, out: &mut String) {
        writeln!(out, "if item.data[1] != 0 {{ {M}::mana::add_one_mana_of_any_type_that_land_produced(g, item)?; }}").unwrap();
    }
    /// A `ColorSet` of the colors among objects, read by a plain function.
    pub(super) fn colors_of(&self, objects: &ObjectSet) -> Result<String> {
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        Ok(format!(
            "({}).into_iter().fold(ColorSet::NONE, |all: ColorSet, o: ObjectId| ColorSet(all.0 | q.chars(o).colors.0))",
            asked.objects(objects)?
        ))
    }
    /// "Add one mana of any color among ..." outside a mana ability, and "for
    /// each color among ..." anywhere.
    pub(super) fn colors_among(
        &mut self,
        id: usize,
        players: &PlayerSet,
        objects: &ObjectSet,
        each: bool,
        times: Option<String>,
        out: &mut String,
    ) -> Result<()> {
        let colors = self.fetch(format!(
            "({}).into_iter().fold(ColorSet::NONE, |all: ColorSet, o: ObjectId| ColorSet(all.0 | q.chars(o).colors.0))",
            self.objects(objects)?
        ));
        let players = self.fetch(self.players(players)?);
        writeln!(
            out,
            "let colors_{id} = {colors}; for player in {players} {{"
        )
        .unwrap();
        if each {
            writeln!(
                out,
                "for color in colors_{id}.iter() {{ g.add_mana(player, color.mana(), 1); }}"
            )
            .unwrap();
        } else {
            let times = times.unwrap_or_else(|| "1i64".into());
            writeln!(out, "if !colors_{id}.is_empty() {{ for _ in 0..({times}).max(0) {{ let colors = {M}::choose_colors(g, player, asker.source, colors_{id}, 1)?; g.add_mana(player, colors[0].mana(), 1); }} }}").unwrap();
        }
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// The mana abilities whose colors the game decides as they are offered.
    pub(super) fn mana_beyond(
        &mut self,
        mana: &ManaProduction,
        restriction: Option<&ManaUse>,
        rest: &[Instruction],
        label: &str,
    ) -> Result<Option<String>> {
        if let (ManaProduction::LastNoted { amount }, []) = (mana, rest) {
            if planned::switched_on("noted-values") {
                let (lasting, restriction) = mana_lasting(restriction);
                if lasting.is_some() { return Err(gap("noted mana with extended lifetime")); }
                let only = restriction.map(|r| self.mana_use(r)).transpose()?.map_or("None".into(), |r|format!("Some({r})"));
                writeln!(self.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, _: Option<Color>) -> Result<(), Suspend> {{ {M}::notes::add_last_mana(g, asker, asker.controller, {amount}, {only}) }}").unwrap();
                // Unknown yield advice stays open. The engine's could-produce procedure executes this callback.
                return Ok(Some(format!("add_by({label})")));
            }
        }
        // "Add two mana of different colors" (CR 106.1b): each a color the ones
        // before it aren't, chosen as the ability resolves (Wizards' Rockets asks
        // its colors the same way).
        if let (ManaProduction::DifferentColors(n @ 1..=5), []) = (mana, rest) {
            let (lasting, restriction) = mana_lasting(restriction);
            if lasting.is_some() {
                return Ok(None);
            }
            let rule = restriction.map(|r| self.mana_use(r)).transpose()?;
            let (add, only) = match &rule {
                Some(rule) => (
                    format!("g.add_restricted_mana(asker.controller, color.mana(), 1, {rule});"),
                    format!(".only_for({rule})"),
                ),
                None => (
                    "g.add_mana(asker.controller, color.mana(), 1);".to_string(),
                    String::new(),
                ),
            };
            writeln!(self.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, _: Option<Color>) -> Result<(), Suspend> {{ let mut left = ColorSet(0b11111); for _ in 0..{n} {{ let color = cantrip_mechanics::choose_colors(g, asker.controller, asker.source, left, 1)?[0]; left = ColorSet(left.0 & !ColorSet::of(color).0); {add} }} Ok(()) }}").unwrap();
            return Ok(Some(format!("add_by({label}).yields_any({n}){only}")));
        }
        let among = matches!(
            mana,
            ManaProduction::ColorsAmong { .. }
                | ManaProduction::CouldProduce { .. }
                | ManaProduction::CommanderIdentity
        );
        if !among {
            return Ok(None);
        }
        if !rest.is_empty() {
            return Err(gap("compound mana ability callback"));
        }
        let (lasting, restriction) = mana_lasting(restriction);
        if lasting.is_some() {
            return Ok(None);
        }
        let rule = restriction.map(|r| self.mana_use(r)).transpose()?;
        let add = |color: &str| match &rule {
            Some(rule) => {
                format!("g.add_restricted_mana(asker.controller, {color}, 1, {rule});")
            }
            None => format!("g.add_mana(asker.controller, {color}, 1);"),
        };
        let only = rule
            .as_ref()
            .map_or(String::new(), |rule| format!(".only_for({rule})"));
        let chosen_mana = add("color.mana()");
        let mut any_of = |c: &mut Self| {
            writeln!(c.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(), Suspend> {{ if let Some(color) = chosen {{ {chosen_mana} }} Ok(()) }}").unwrap();
            format!("add_by({label}).choose_color().yields_any(1){only}")
        };
        Ok(Some(match mana {
            ManaProduction::ColorsAmong {
                objects,
                each: true,
            } => {
                let colors = self.colors_of(objects)?;
                writeln!(self.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, _: Option<Color>) -> Result<(), Suspend> {{ let colors = {{ let q: &dyn Query = g; {colors} }}; for color in colors.iter() {{ {chosen_mana} }} Ok(()) }}").unwrap();
                format!("add_by({label}){only}")
            }
            ManaProduction::ColorsAmong {
                objects,
                each: false,
            } => {
                let colors = self.colors_of(objects)?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {label}_colors(q: &dyn Query, asker: &Asker) -> ColorSet {{ {colors} }}").unwrap();
                self.mana_colors = Some((format!(".colors_from({label}_colors)"), None));
                any_of(self)
            }
            // The engine reads what the cost's permanent could produce (106.7).
            ManaProduction::CouldProduce {
                objects,
                colors_only: false,
            } if matches!(
                objects,
                ObjectSet::Earlier { .. } | ObjectSet::One(ObjectRef::Query { .. })
            ) && rule.is_none() =>
            {
                self.mana_colors = Some((".colors_from_sacrifice()".to_string(), None));
                "add_any_color(1)".to_string()
            }
            ManaProduction::CouldProduce {
                objects,
                colors_only,
            } => {
                let mut asked = self.clone();
                (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
                let list = asked.objects(objects)?;
                self.mana_colors = Some((
                    format!(
                        ".colors_from(|q, asker| q.mana_could_produce(&{list}, {colors_only}))"
                    ),
                    Some("engine: no reading of what a permanent could produce"),
                ));
                any_of(self)
            }
            ManaProduction::CommanderIdentity => {
                self.mana_colors = Some((
                    ".colors_from(|q, asker| q.commander_identity(asker.controller))".to_string(),
                    Some("engine: no commander"),
                ));
                any_of(self)
            }
            _ => unreachable!("a production listed above"),
        }))
    }
    /// "Protection from Humans" of no quality the engine names: which sources it
    /// is from, asked of a source as protection is (CR 702.16), is a rule of the
    /// card's `protection_rules`, and the quality, kept by an effect or a layer,
    /// names its card and index, with what the words named as an effect
    /// resolved as its `about`; more of the resolution is a gap of its own.
    fn protection_rule(&mut self, feature: &str, filter: &ObjectFilter) -> Result<Option<String>> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let asked = self.filter(filter, &Candidate::of("id"));
        (self.reader, self.event_at) = saved;
        let test = match asked {
            Err(failed) if failed.feature == "target reference outside a resolving ability" => {
                self.filter(filter, &Candidate::of("id")).ok()
            }
            asked => asked.ok(),
        };
        let Some(test) = test else {
            return Ok(None);
        };
        // A rule reading what the resolution chose but a target can't keep it.
        let (test, about) = self.named_as_resolved(&test)?;
        let index = self.holes.matches("=> protection_rules:").count();
        let function = format!("protection_rule_{index}");
        let params = "q: &dyn Query, asker: &Asker, about: Target, id: ObjectId";
        let body = format!("let source = asker.source; {test}");
        let item = planned::planned_fn(feature, &function, params, "bool", &body);
        writeln!(self.functions, "{item}").unwrap();
        let id = planned::requirement(feature);
        writeln!(
            self.holes,
            "planned_item!({id:?} => protection_rules: {function});"
        )
        .unwrap();
        Ok(Some(format!(
            "ProtectionQuality::Rule {{ card: {}, index: {index}, about: {about} }}",
            self.source_card()
        )))
    }
    /// Protection given by a static or an effect: the qualities the engine has,
    /// and otherwise the test it will have.
    pub(super) fn grant_protection(
        &mut self,
        quality: &Protection,
        grants: &mut Vec<(String, String)>,
    ) -> Result<()> {
        const FEATURE: &str = "engine: no protection from this quality";
        if let Protection::Each(qualities) = quality {
            for quality in qualities {
                self.grant_protection(quality, grants)?;
            }
            return Ok(());
        }
        // "Protection from the chosen color" of a permanent that chose as it entered.
        if let Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Chosen(ValueKind::Color),
        } = quality
        {
            if self.reader == Reader::Layer && self.enters_choosing.contains(&ValueKind::Color) {
                let chosen = self.chosen_value(ValueKind::Color)?;
                grants.push((
                    String::new(),
                    format!("if let Some(chosen) = {chosen} {{ chars.protection = chars.protection.with(chosen); }}"),
                ));
                return Ok(());
            }
        }
        if let Some(grant) = self.protection_read_from(quality)? {
            grants.push(grant);
            return Ok(());
        }
        let mut fixed = Vec::new();
        let Err(hole) = granted_protection(quality, &mut fixed) else {
            grants.extend(fixed);
            return Ok(());
        };
        let code = match quality {
            Protection::Objects { filter, .. } => match self.protection_rule(FEATURE, filter)? {
                Some(rule) => rule,
                None => match self.filter(filter, &Candidate::of("id")) {
                    Ok(test) => format!("ProtectionQuality::Matching(|q, asker, id| {test})"),
                    Err(_) => return Err(hole),
                },
            },
            // An effect has one kind for each player: `continuous` adds them.
            Protection::Players(players) => {
                match self.players(players) {
                    Ok(list) => {
                        let kind = format!("EffectKind::ProtectionFrom(ProtectionQuality::Players(|q, asker| {list}))");
                        grants.push((
                        planned_kind(FEATURE, kind),
                        format!("for player in {list} {{ chars.protection_players |= 1 << player.0; }}"),
                    ));
                        return Ok(());
                    }
                    Err(_) => return Err(hole),
                }
            }
            _ => return Err(hole),
        };
        grants.push((
            planned_kind(FEATURE, format!("EffectKind::ProtectionFrom({code})")),
            format!(
                "{};",
                planned::planned_value(
                    FEATURE,
                    "()",
                    &format!("chars.protection_from.push({code})")
                )
            ),
        ));
        Ok(())
    }
    /// Protection from each color or card type of what the game reads: the kind
    /// a resolving effect adds, read once, and the statement of a layer, where
    /// each object read has been through the layers before abilities. Colors of
    /// a card that moved are the card's, not those it had as "that creature".
    fn protection_read_from(&mut self, quality: &Protection) -> Result<Option<(String, String)>> {
        let Protection::Objects {
            kind: ObjectKind::Any,
            filter,
        } = quality
        else {
            return Ok(None);
        };
        let own_colors = self.reader == Reader::Layer
            && self
                .lines_said()
                .iter()
                .any(|line| line.contains("from each of its colors"));
        let colors = match filter {
            ObjectFilter::SharesColorWith(ObjectRef::Source) if own_colors => "chars.colors".to_string(),
            ObjectFilter::SharesColorWith(object) => format!(
                "{}.map_or(ColorSet::NONE, |o: ObjectId| q.chars(o).colors)",
                self.object(object)?
            ),
            ObjectFilter::SharesColorWithAny(objects) => format!(
                "({}).into_iter().flat_map(|o: ObjectId| q.chars(o).colors.iter()).fold(ColorSet::NONE, ColorSet::with)",
                self.objects(objects)?
            ),
            ObjectFilter::Shares { property: DistinctProperty::CardTypes, with } => {
                let types = format!(
                    "{}.map_or(TypeSet::NONE, |o: ObjectId| q.chars(o).types)",
                    self.object(with)?
                );
                return Ok(Some((
                    format!("EffectKind::ProtectionFromTypes({types})"),
                    format!("chars.protection_types = TypeSet(chars.protection_types.0 | ({types}).0);"),
                )));
            }
            _ => return Ok(None),
        };
        if colors.contains("moved_") {
            return Ok(None);
        }
        Ok(Some((
            format!("EffectKind::Protection({colors})"),
            format!("chars.protection = ColorSet(chars.protection.0 | ({colors}).0);"),
        )))
    }
    /// "You may pay [mana] any number of times": how many is the payer's as they
    /// pay, and is what "that many" reads.
    pub(super) fn pay_any_number_of_times(
        &mut self,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let [CostPart::ManaAnyNumberOfTimes(symbols)] = cost.parts.as_slice() else {
            return Err(gap("pay any number of times"));
        };
        self.helpers.insert("pay_question");
        let payer = self.fetch(self.certain_player(player)?);
        writeln!(out, "let payer_{id} = {payer}; let times_{id} = g.may_pay_times(payer_{id}, asker.source, asker.source, {}, cantrip_api::ANY_NUMBER_OF_TIMES, &PAY_QUESTION)?; let paid_{id} = times_{id} > 0; let bound_{id}: i64 = i64::from(times_{id});", mana(symbols)?).unwrap();
        self.performed.insert(result.0, format!("paid_{id}"));
        self.values.insert(result.0, format!("bound_{id}"));
        Ok(())
    }
    /// The player picks the objects that pay, all of them or none: a cost is
    /// paid whole (118.3).
    pub(super) fn select_whole(
        &mut self,
        player: &PlayerRef,
        objects: &ObjectSet,
        n: &Expr,
        destination: &Destination,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let local = format!("selected_{}", self.fresh());
        let who = self.fetch(self.certain_player(player)?);
        let candidates = self.fetch(self.objects(objects)?);
        let owed = self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?);
        let purpose = move_purpose(destination);
        writeln!(out, "let {local}_player = {who}; let {local}_candidates = {candidates}; let {local}_count = {owed}; let {local} = if {local}_candidates.len() as u64 >= u64::from({local}_count) {{ cantrip_mechanics::choose::select(g, {local}_player, asker.source, Purpose::{purpose}, {local}_candidates, cantrip_mechanics::select::Selection::of(cantrip_mechanics::select::Count::Exactly({local}_count), &[]))? }} else {{ Vec::new() }};").unwrap();
        self.selections.insert(result.0, local);
        Ok(())
    }
    /// The parts of a required instruction `Operation::PayCost` does not read.
    pub(super) fn resolution_cost_beyond(
        &mut self,
        id: usize,
        player: &PlayerRef,
        cost: &Cost,
        part: &CostPart,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        match part {
            // "Pay {3}{U}{U}. If you don't, ...": whether is the payer's (118.12).
            CostPart::Mana(_) => {
                let asked = self.ask_to_pay(id, player, cost)?;
                writeln!(out, "{asked}").unwrap();
            }
            CostPart::Discard {
                count: count @ Expr::Constant(1),
                filter,
                random: false,
            } => {
                let hand = ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Hand,
                        players: PlayerSet::One(player.clone()),
                    },
                    kind: ObjectKind::Card,
                    filter: filter.clone(),
                };
                let one = Count::Exactly(count.clone());
                self.choose(player, &hand, &one, result, "Discard", out)?;
                let picked = self.selections[&result.0].clone();
                writeln!(out, "let discarded_{id} = g.discard_together(&{picked}); let paid_{id} = !discarded_{id}.is_empty();").unwrap();
                self.moved.insert(result.0, format!("discarded_{id}"));
                self.left_behind(result, format!("discarded_{id}.clone()"));
            }
            // "Shuffle a card from your hand into your library. If you do": a card
            // shuffled in, of a hand that had one.
            CostPart::ShuffleFromHand => {
                let hand = ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Hand,
                        players: PlayerSet::One(player.clone()),
                    },
                    kind: ObjectKind::Card,
                    filter: ObjectFilter::Any,
                };
                let one = Count::Exactly(Expr::Constant(1));
                self.choose(player, &hand, &one, result, "PutOnLibrary", out)?;
                let picked = self.selections[&result.0].clone();
                writeln!(out, "let shuffled_{id} = g.put_into_library(&{picked}, LibraryPlace::ShuffledIn); let paid_{id} = !shuffled_{id}.is_empty();").unwrap();
                self.moved.insert(result.0, format!("shuffled_{id}"));
                self.left_behind(result, format!("{picked}.clone()"));
            }
            // "[+2], then ..." in a resolving ability (Comet, Stellar Pup): loyalty
            // counters put on or removed from the source (CR 606.4 by analogy),
            // paid only with as many to remove.
            CostPart::Loyalty(n) => {
                let delta = self.fetch(self.number(n)?.code);
                let stored = Quantity::StoredCounter.kind();
                writeln!(out, "let delta_{id}: i64 = {delta}; let paid_{id} = if delta_{id} >= 0 {{ g.add_counters(asker.source, CounterKind::Loyalty, delta_{id}.min(i64::from({stored}::MAX)) as {stored}); true }} else {{ let owed = delta_{id}.saturating_neg().min(i64::from({stored}::MAX)) as {stored}; g.remove_counters(asker.source, CounterKind::Loyalty, owed) == owed }};").unwrap();
            }
            // "Exile it and the top six cards of your library in a face-down pile.
            // If you do" (Triumph of Saint Katherine): fixed objects, no choice
            // among them, exiled face down together (CR 406.3). Worded as a cost,
            // it is paid whole or not at all (CR 118.3): with any of them missing,
            // nothing moves and "you do" is false.
            CostPart::Move {
                selections,
                destination:
                    MovementDestination::Exile {
                        visibility: ExileVisibility::FaceDown,
                        arrival,
                    },
            } if arrival.counters.is_empty()
                && arrival
                    .states
                    .iter()
                    .all(|s| matches!(s, ExileState::ExilePile { .. }))
                && selections.iter().all(|s| fixed_selection(s).is_some()) =>
            {
                let mut parts = Vec::new();
                for selection in selections {
                    let wanted = fixed_selection(selection).expect("checked");
                    let wanted = match wanted {
                        Some(n) => self.fetch(self.number(n)?.code),
                        None => "1i64".into(),
                    };
                    parts.push(format!(
                        "{{ let part: Vec<ObjectId> = {}; whole_{id} &= part.len() as i64 >= {wanted}; part }}",
                        self.fetch(self.objects(&selection.objects)?)
                    ));
                }
                writeln!(
                    out,
                    "let mut whole_{id} = true; let objects_{id}: Vec<ObjectId> = [{}].concat(); let moved_{id}: Vec<ObjectId> = if whole_{id} {{ g.exile_face_down(&objects_{id}, None) }} else {{ Vec::new() }}; let paid_{id} = whole_{id} && moved_{id}.iter().any(|&o| g.obj(o).zone.is(ZoneKind::Exile));",
                    parts.join(", ")
                )
                .unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.left_behind(result, format!("objects_{id}.clone()"));
            }
            // "Pay {E}{E}. If you can't, ...": whoever can pay does (Greenbelt Rampager).
            CostPart::Energy(n) => {
                let owed = self.fetch(self.number(n)?.amount(255, "u8")?);
                let payer = self.fetch(self.certain_player(player)?);
                writeln!(out, "let owed_{id}: u8 = {owed}; let payer_{id} = {payer}; let paid_{id} = g.state().player(payer_{id}).energy() >= owed_{id}; if paid_{id} {{ g.change_energy(payer_{id}, -i32::from(owed_{id})); }}").unwrap();
            }
            // "Remove a counter from a creature you control. If you do" (Mister
            // Hyde): one counter of any kind off one of them, its player's choice.
            CostPart::RemoveCounters { removals }
                if matches!(
                    removals.as_slice(),
                    [CounterRemoval {
                        recipients: Recipients::Objects(_),
                        kind: None,
                        amount: CounterAmount::Count(Count::Exactly(Expr::Constant(1))),
                        selection: Some(_),
                        ..
                    }]
                ) =>
            {
                let [CounterRemoval {
                    player: remover,
                    recipients: Recipients::Objects(objects),
                    ..
                }] = removals.as_slice()
                else {
                    unreachable!("one counter off one of a set");
                };
                let remover = self.fetch(self.certain_player(remover)?);
                let among = self.fetch(self.objects(objects)?);
                writeln!(out, "let remover_{id} = {remover}; let among_{id}: Vec<ObjectId> = {among}; let paid_{id} = g.remove_a_counter_from_among(remover_{id}, asker.source, &among_{id})?;").unwrap();
            }
            CostPart::RemoveCounters { removals } if removes_one_counter(removals) => {
                self.remove_counters(id, removals, result, out)?;
                writeln!(out, "let paid_{id} = removed_{id} > 0;").unwrap();
            }
            CostPart::ChangeTapState {
                selections,
                tapped: true,
                symbol: false,
            } if matches!(
                selections.as_slice(),
                [CostObjectSelection {
                    objects: ObjectSet::One(_),
                    choose: None
                }]
            ) =>
            {
                let objects = selections[0].objects.clone();
                let tap = Operation::SetTapped {
                    objects,
                    tapped: true,
                };
                self.operation(&tap, result, out)?;
                writeln!(out, "let paid_{id} = !{}.is_empty();", self.moved(result)?).unwrap();
            }
            CostPart::ChangeTapState {
                selections,
                tapped: true,
                symbol: false,
            } if matches!(
                selections.as_slice(),
                [CostObjectSelection {
                    choose: Some(Count::AnyNumber),
                    ..
                }]
            ) =>
            {
                self.tap_any_number(id, player, &selections[0].objects, result, out)?;
            }
            CostPart::Reveal {
                count: Count::Exactly(count) | Count::AsManyAsCan(count),
                filter: ObjectFilter::Any,
                random: true,
            } if self.read_later(result) => {
                let count = self.fetch(self.number(count)?.code);
                let who = self.fetch(self.certain_player(player)?);
                writeln!(out, "let payer_{id} = {who}; let wanted_{id} = ({count}).max(0) as usize; let mut revealed_{id}: Vec<ObjectId> = g.state().zone(Zone::hand(payer_{id})).to_vec(); for i in (1..revealed_{id}.len()).rev() {{ let j = g.random(i as u32 + 1) as usize; revealed_{id}.swap(i, j); }} revealed_{id}.truncate(wanted_{id}); g.reveal(payer_{id}, &revealed_{id}); let paid_{id} = !revealed_{id}.is_empty();").unwrap();
                self.selections.insert(result.0, format!("revealed_{id}"));
                self.before
                    .insert(result.0, format!("revealed_{id}.clone()"));
            }
            // "Put a +1/+1 counter on it for each card in your hand. If you do":
            // that a counter was put (CR 608.2c), on what is still there to have it.
            CostPart::PutCounters(placements)
                if placements.iter().all(|p| {
                    p.selection.is_none() && !matches!(p.recipients, Recipients::Players(_))
                }) =>
            {
                let operation = Operation::PutCounters {
                    placements: placements.clone(),
                };
                self.operation(&operation, result, out)?;
                let Some(applied) = self.selections.get(&result.0).cloned() else {
                    return Err(gap(format!("resolution cost {}", form(part))));
                };
                writeln!(out, "let paid_{id} = !{applied}.is_empty();").unwrap();
            }
            // "Move one or more counters from this onto another target permanent
            // you control. If you do": how many of each kind it has, a question for
            // each (as moving any number of each kind is asked), the last kind
            // asked for at least one while none has moved (CR 122.5, 608.2c).
            CostPart::MoveCounters {
                from,
                to,
                kind: None,
                amount: MovedCounters::AtLeast(Expr::Constant(1)),
            } => {
                let actor = self.fetch(self.certain_player(player)?);
                let sources = self.acted_on(from)?;
                let recipient = self.acted_on(to)?;
                let stored = Quantity::StoredCounter.kind();
                let most_read = if planned::switched_on(quantity::BOUNDED_32) {
                    "max(0)"
                } else {
                    "clamp(0, 255)"
                };
                writeln!(out, "let mut done_{id}: i64 = 0; let actor_{id} = {actor}; let from_{id}: Option<ObjectId> = ({sources}).first().copied(); let to_{id}: Option<ObjectId> = ({recipient}).first().copied(); if let (Some(from), Some(to)) = (from_{id}, to_{id}) {{ let kinds: Vec<CounterKind> = CounterKind::ALL.into_iter().filter(|&kind| g.state().obj(from).counters(kind) > 0).collect(); let last = kinds.len(); for (i, kind) in kinds.into_iter().enumerate() {{ let most = g.state().obj(from).counters(kind).{most_read} as u32; let least = u32::from(done_{id} == 0 && i + 1 == last); if most >= least {{ let amount = cantrip_mechanics::choose::number(g, actor_{id}, asker.source, least, most, \"Move counters\")? as {stored}; done_{id} += i64::from(g.move_counters(from, to, kind, amount)); }} }} }} let paid_{id} = done_{id} > 0;").unwrap();
            }
            other => {
                let feature = format!("resolution cost {}", form(other));
                self.stated_payment(&feature, id, player, cost, out)?;
                self.objects_that_paid(id, result, out);
            }
        }
        Ok(())
    }
    /// What paid an earlier instruction's cost, as it was when it paid.
    pub(super) fn paid_at_resolution(&self, result: &ResultId) -> Result<String> {
        if !self.before.contains_key(&result.0) {
            return Err(gap("cost objects of Resolution"));
        }
        self.transition(result, &ObjectPhase::Before)
    }
    /// A cost that moves objects the engine has no field for.
    pub(super) fn move_cost_beyond(
        &mut self,
        selections: &[CostObjectSelection],
        destination: &Destination,
        label: &str,
    ) -> Result<String> {
        const FEATURE: &str = "engine: no cost moves these objects";
        let to = form(destination);
        let mut costs = Vec::new();
        for CostObjectSelection { objects, choose } in selections {
            if matches!(objects, ObjectSet::One(ObjectRef::Source)) {
                match destination {
                    Destination::Exile => costs.push("EXILE_THIS".to_string()),
                    Destination::OwnersHand => costs.push("RETURN_THIS".to_string()),
                    _ => return Err(gap(format!("move cost {to} from {}", form(objects)))),
                }
                continue;
            }
            let yours = costs::own_graveyard(objects);
            let objects = yours.as_ref().unwrap_or(objects);
            let (from, filter) = cost_zone(objects)
                .ok_or_else(|| gap(format!("move cost {to} from {}", form(objects))))?;
            let filter = self.cost_filter(&filter, label)?;
            let count = match choose {
                None => "CostCount::All".to_string(),
                Some(Count::Exactly(Expr::Constant(n))) => format!("CostCount::Exactly({n})"),
                Some(Count::Exactly(Expr::AnnouncedX)) => "CostCount::Announced".to_string(),
                Some(Count::AnyNumber) => "CostCount::AnyNumber".to_string(),
                Some(Count::AtLeast(Expr::Constant(n))) => format!("CostCount::AtLeast({n})"),
                Some(Count::Exactly(n)) => {
                    let n = self.asked_number(n)?;
                    let params = "q: &dyn Query, asker: &Asker";
                    let count =
                        self.rule_fn(FEATURE, &format!("{label}_moved_count"), params, "i64", &n);
                    format!("CostCount::Reading({count})")
                }
                Some(other) => return Err(gap(format!("move cost counted {}", form(other)))),
            };
            costs.push(planned::planned_value(
                FEATURE,
                "Cost::NONE",
                &format!("Cost {{ moves: Some(MovesObjects {{ from: {from}, to: CostDestination::{to}, count: {count}, filter: {filter} }}), ..Cost::NONE }}"),
            ));
        }
        Ok(costs
            .into_iter()
            .reduce(|a, b| format!("{a}.and({b})"))
            .unwrap_or_else(|| "Cost::NONE".to_string()))
    }
    /// The counters of a cost, beyond the fixed removals `cost_part` reads.
    pub(super) fn counter_removal_beyond(
        &mut self,
        removals: &[CounterRemoval],
        label: &str,
    ) -> Result<String> {
        const FEATURE: &str = "engine: no cost removes counters this way";
        const SHAPE: &str = "counter removal cost shape";
        let plan = |code: String| planned::planned_value(FEATURE, "Cost::NONE", &code);
        let [CounterRemoval {
            recipients: Recipients::Objects(objects),
            kind,
            amount,
            ..
        }] = removals
        else {
            return Err(gap(SHAPE));
        };
        let kind = kind.as_ref().map(read::counter_kind).transpose()?;
        let of_kind = match &kind {
            Some(kind) => format!("Some({kind})"),
            None => "None".to_string(),
        };
        let exactly = |count: &Count| match count {
            Count::Exactly(Expr::Constant(n)) => Some(*n),
            _ => None,
        };
        Ok(match (objects, amount) {
            // "Remove X counters": the X announced is how many (107.3a).
            (
                ObjectSet::One(ObjectRef::Source),
                CounterAmount::Count(Count::Exactly(Expr::AnnouncedX)),
            ) if label != PAID && kind.is_some() => {
                let kind = kind.ok_or_else(|| gap(SHAPE))?;
                // A mana ability announces only its {X} (`activate_mana_as`): no
                // number of counters to remove, nor an X its effect could read.
                if self.paid_for == lane_j::PaidFor::Mana
                    && !planned::switched_on("mana-counter-costs")
                {
                    return Ok(planned::planned_value(
                        MANA_COUNTERS_X,
                        "Cost::NONE",
                        &format!("Cost {{ variable: Some(VariableCost::RemoveAnyNumberOfCounters({kind})), ..Cost::NONE }}"),
                    ));
                }
                format!("Cost {{ variable: Some(VariableCost::RemoveAnyNumberOfCounters({kind})), ..Cost::NONE }}")
            }
            // How many is read as the cost is paid, while the effect resolves.
            (ObjectSet::One(ObjectRef::Source), CounterAmount::Count(Count::Exactly(n)))
                if label == PAID
                    && kind.is_some()
                    && exactly(&Count::Exactly(n.clone())).is_none() =>
            {
                let kind = kind.ok_or_else(|| gap(SHAPE))?;
                let n = self.fetch(self.number(n)?.amount(255, "u8")?);
                format!("Cost {{ remove_counters: Some(({kind}, {n})), ..Cost::NONE }}")
            }
            (
                crate::battlefield_query!(filter),
                CounterAmount::AmongThem(Count::Exactly(Expr::Constant(n))),
            ) if costs::yours(filter) && (0..=255).contains(n) => {
                let filter = self.cost_filter(filter, label)?;
                match &kind {
                    Some(kind) => format!("remove_kind_counters_among({kind}, {n}, {filter})"),
                    None => format!("remove_counters_among({n}, {filter})"),
                }
            }
            (crate::battlefield_query!(filter), CounterAmount::AmongThem(count)) => {
                let filter = self.cost_filter(filter, label)?;
                let size = match count {
                    Count::Exactly(Expr::Constant(n)) => format!("CountersTaken::Exactly({n})"),
                    Count::AnyNumber => "CountersTaken::AnyNumber".to_string(),
                    Count::AtLeast(Expr::Constant(n)) => format!("CountersTaken::AtLeast({n})"),
                    Count::Exactly(Expr::AnnouncedX) => "CountersTaken::Announced".to_string(),
                    _ => return Err(gap(SHAPE)),
                };
                plan(format!(
                    "Cost {{ remove_counters_from: Some(CountersFrom {{ filter: {filter}, kind: {of_kind}, size: {size} }}), ..Cost::NONE }}"
                ))
            }
            (ObjectSet::One(ObjectRef::GrantingObject), CounterAmount::All) => plan(format!(
                "Cost {{ remove_all_counters_from_granter: Some({of_kind}), ..Cost::NONE }}"
            )),
            (ObjectSet::One(ObjectRef::Source), CounterAmount::Count(Count::AnyNumber)) => {
                let kind = kind.ok_or_else(|| gap(SHAPE))?;
                format!("Cost {{ variable: Some(VariableCost::RemoveAnyNumberOfCounters({kind})), ..Cost::NONE }}")
            }
            (ObjectSet::One(ObjectRef::Source), CounterAmount::All) => plan(format!(
                "Cost {{ remove_all_counters: Some({of_kind}), ..Cost::NONE }}"
            )),
            (ObjectSet::One(ObjectRef::Source), CounterAmount::Count(count)) => {
                match (exactly(count), count) {
                    (Some(n), _) => plan(format!(
                        "Cost {{ remove_any_counters: {n}, ..Cost::NONE }}"
                    )),
                    (None, Count::AtLeast(Expr::Constant(n))) => plan(format!(
                        "Cost {{ variable: Some(VariableCost::RemoveAtLeast {{ kind: {of_kind}, least: {n} }}), ..Cost::NONE }}"
                    )),
                    _ => return Err(gap(SHAPE)),
                }
            }
            (crate::battlefield_query!(filter), CounterAmount::Count(count)) => {
                let filter = self.cost_filter(filter, label)?;
                let size = match (exactly(count), count) {
                    (Some(n), _) => format!("CountersTaken::Exactly({n})"),
                    (None, Count::AnyNumber) => "CountersTaken::AnyNumber".to_string(),
                    (None, Count::AtLeast(Expr::Constant(n))) => {
                        format!("CountersTaken::AtLeast({n})")
                    }
                    _ => return Err(gap(SHAPE)),
                };
                plan(format!(
                    "Cost {{ remove_counters_from: Some(CountersFrom {{ filter: {filter}, kind: {of_kind}, size: {size} }}), ..Cost::NONE }}"
                ))
            }
            _ => return Err(gap(SHAPE)),
        })
    }
    /// The parts of a cost `cost_part` does not read: the engine's field where
    /// it has one, and otherwise the field the cost will have.
    pub(super) fn cost_part_beyond(&mut self, part: &CostPart, label: &str) -> Result<String> {
        let resolving = label == PAID;
        let field = |text: &str| format!("Cost {{ {text}, ..Cost::NONE }}");
        let plan = |feature: &str, text: &str| {
            planned::planned_value(
                feature,
                "Cost::NONE",
                &format!("Cost {{ {text}, ..Cost::NONE }}"),
            )
        };
        Ok(match part {
            CostPart::Loyalty(Expr::Multiply(a, b))
                if matches!((&**a, &**b), (Expr::Constant(-1), Expr::AnnouncedX)) =>
            {
                "loyalty_minus_x()".into()
            }
            CostPart::Life(Expr::AnnouncedX) if !resolving && self.paying == Paying::Additional => {
                field("pay_x_life: true")
            }
            CostPart::Life(Expr::AnnouncedX) if !resolving => plan(
                "engine: pay X life outside a spell's additional cost",
                "pay_x_life: true",
            ),
            CostPart::Life(n) => self.sized_cost(n, label, "life", "u32", "i64::from(u32::MAX)")?,
            CostPart::Energy(Expr::AnnouncedX) if !resolving => plan(
                "engine: pay X energy",
                "variable: Some(VariableCost::PayEnergyX)",
            ),
            CostPart::Energy(n) => self.sized_cost(n, label, "energy", "u8", "255")?,
            CostPart::Mill {
                count: Expr::Constant(n),
            } => plan("engine: no mill cost", &format!("mill: {n}")),
            CostPart::Sacrifice {
                objects: ObjectSet::One(ObjectRef::GrantingObject),
                choose: None,
            } if !resolving => field("sacrifice_granter: true"),
            CostPart::Sacrifice {
                objects: ObjectSet::One(ObjectRef::AttachedTo(host)),
                choose: None,
            } if !resolving && matches!(**host, ObjectRef::Source) => {
                let function = format!("{label}_host");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ q.state().obj(asker.source).attached_to == Some(id) }}").unwrap();
                format!("Cost::sacrifice(1, {function})")
            }
            CostPart::Discard { count: Expr::Constant(n), filter: ObjectFilter::Any, random: true }
                if planned::switched_on("random-discard-costs") => {
                let count = u8::try_from(*n).map_err(|_| gap("numeric range for random discard cost"))?;
                field(&format!("discard_at_random: {count}"))
            }
            CostPart::Discard {
                count: Expr::Constant(n),
                filter: ObjectFilter::Any,
                random: true,
            } => plan(
                "engine: no random discard cost",
                &format!("discard_at_random: {n}"),
            ),
            CostPart::Discard {
                count: Expr::AnnouncedX,
                filter: ObjectFilter::Any,
                random: true,
            } => plan(
                "engine: no random discard cost",
                "variable: Some(VariableCost::DiscardXAtRandom)",
            ),
            CostPart::DiscardHand => "DISCARD_HAND".into(),
            // "Put this creature on the bottom of its owner's library".
            CostPart::PlayerAction { body, .. } if !resolving && moves_self_to_bottom(body) => {
                "put_self_into_library(LibraryPlace::Bottom)".into()
            }
            CostPart::Exert(ObjectRef::Source) if !resolving => "EXERT".into(),
            CostPart::Unattach(ObjectRef::Source) => "UNATTACH_THIS".into(),
            CostPart::Unattach(ObjectRef::GrantingObject) => plan(
                "engine: no cost unattaches what gave the ability",
                "unattach_granter: true",
            ),
            CostPart::Teamwork(n) => field(&format!(
                "tap_creatures: Some(TapCreatures {{ power: {n}, greater_with: None }})"
            )),
            CostPart::Keyword {
                action: CostKeyword::Blight,
                amount: Some(Expr::Constant(n)),
            } => format!("{M}::blight::cost({n})"),
            CostPart::Keyword {
                action: CostKeyword::CollectEvidence,
                amount: Some(Expr::Constant(n)),
            } => field(&format!("collect_evidence: {n}")),
            CostPart::Keyword {
                action: CostKeyword::Blight,
                amount: Some(Expr::AnnouncedX),
            } if self.paying == Paying::Additional => {
                field("variable: Some(VariableCost::BlightX)")
            }
            CostPart::Keyword {
                action: CostKeyword::Blight,
                amount: Some(Expr::AnnouncedX),
            } => plan(
                "engine: blight X outside a spell's additional cost",
                "variable: Some(VariableCost::BlightX)",
            ),
            CostPart::Keyword {
                action: CostKeyword::Forage,
                amount: None,
            } => planned::planned_value(
                "engine: no Forage",
                "Cost::NONE",
                &format!("{M}::forage::COST"),
            ),
            CostPart::Behold { count, what } => {
                let Expr::Constant(n) = count else {
                    return Err(gap("cost part Behold"));
                };
                let code = self.behold(*n, what, "cost_of", label)?;
                match self.paying {
                    Paying::Ability => planned::planned_value(
                        "engine: behold outside the casting of a spell",
                        "Cost::NONE",
                        &code,
                    ),
                    _ => code,
                }
            }
            CostPart::BeholdAndExile(what) => {
                let code = self.behold(1, what, "cost_and_exile", label)?;
                match self.paying {
                    Paying::Ability => planned::planned_value(
                        "engine: behold outside the casting of a spell",
                        "Cost::NONE",
                        &code,
                    ),
                    _ => code,
                }
            }
            CostPart::Reveal {
                count,
                filter,
                random: false,
            } => {
                const FEATURE: &str = "engine: no reveal cost";
                match (count, filter) {
                    (
                        Count::Exactly(Expr::Constant(1)),
                        ObjectFilter::SameObject(ObjectRef::Source),
                    ) => plan(FEATURE, "reveal_self: true"),
                    (Count::Exactly(Expr::Constant(n)), filter) => {
                        let filter = self.cost_filter(filter, label)?;
                        plan(FEATURE, &format!("reveal: Some(({n}, {filter}))"))
                    }
                    (Count::AnyNumber, filter) => {
                        let filter = self.cost_filter(filter, label)?;
                        plan(
                            FEATURE,
                            &format!("variable: Some(VariableCost::RevealAnyNumber({filter}))"),
                        )
                    }
                    (Count::Exactly(Expr::CountObjects(_)), ObjectFilter::Any) => {
                        plan(FEATURE, "reveal_hand: true")
                    }
                    _ => return Err(gap("cost part Reveal")),
                }
            }
            CostPart::ChangeTapState {
                selections,
                tapped: false,
                symbol,
            } => match selections.as_slice() {
                [CostObjectSelection {
                    objects: ObjectSet::One(ObjectRef::Source),
                    choose: None,
                }] if *symbol => plan("engine: no untap symbol cost", "untap: true"),
                [CostObjectSelection {
                    objects: crate::battlefield_query!(filter),
                    choose: Some(Count::Exactly(Expr::Constant(n))),
                }] => {
                    let filter = self.cost_filter(filter, label)?;
                    plan(
                        "engine: no cost untaps permanents",
                        &format!("untap_permanents: Some(({n}, {filter}))"),
                    )
                }
                _ => return Err(gap("cost part ChangeTapState")),
            },
            CostPart::ExiledToGraveyard(ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Exile),
                filter,
                ..
            }) => {
                let filter = self.cost_filter(filter, label)?;
                plan(
                    "engine: no cost puts an exiled card into a graveyard",
                    &format!("exiled_to_graveyard: Some({filter})"),
                )
            }
            CostPart::ManaCostOf(ObjectRef::Source) => {
                plan("engine: no cost copies a mana cost", "own_mana_cost: true")
            }
            CostPart::ManaCostOf(ObjectRef::AttachedTo(host))
                if matches!(**host, ObjectRef::Source) =>
            {
                plan("engine: no cost copies a mana cost", "host_mana_cost: true")
            }
            CostPart::Waterbend(symbols) => plan(
                "engine: waterbend as a required additional cost",
                &format!("waterbend: {}", mana(symbols)?),
            ),
            CostPart::ManaWithX {
                mana: symbols,
                x: Some(x),
            } if resolving => {
                let owed = self.fetch(self.number(x)?.amount(u32::MAX.into(), "u32")?);
                format!(
                    "{{ let base = {}; let owed: u32 = {owed}; Cost::mana(ManaCost {{ generic: base.generic.saturating_add(owed.saturating_mul(u32::from(base.x))), x: 0, ..base }}) }}",
                    mana(symbols)?
                )
            }
            CostPart::Choice(_) if self.activation_cost.is_some() => {
                return Err(gap(super::costs::ACTIVATED_COST_CHOICE));
            }
            // "You may reveal a Dragon card from your hand or choose a Dragon you
            // control" (Dragon's Fire): a cost no `Cost` pays (a reveal, card.rs
            // `Cost`) can't be one of the choices, whatever offers the choice.
            CostPart::Choice(costs)
                if costs
                    .iter()
                    .any(|c| c.parts.iter().any(|p| matches!(p, CostPart::Reveal { .. }))) =>
            {
                return Err(gap("engine: no reveal cost"));
            }
            CostPart::Choice(costs) => {
                let mut options = Vec::new();
                for (i, option) in costs.iter().enumerate() {
                    options.push(self.cost(option, &format!("{label}_or_{i}"))?);
                }
                planned::planned_value(
                    "engine: no choice between the costs of an ability",
                    "Cost::NONE",
                    &format!("Cost::either(&[{}])", options.join(", ")),
                )
            }
            CostPart::Repeated { part, times } => {
                let once = Cost {
                    parts: vec![(**part).clone()],
                };
                match (&**part, times) {
                    (CostPart::Mana(symbols), _) if resolving => {
                        let n = self.fetch(self.number(times)?.code);
                        format!(
                            "Cost::mana({}.times(({n}).clamp(0, 255) as u32).unwrap_or(ManaCost {{ generic: 255, ..ManaCost::ZERO }}))",
                            mana(symbols)?
                        )
                    }
                    (_, Expr::Constant(n)) => {
                        let once = self.cost(&once, label)?;
                        (0..*n).fold("Cost::NONE".to_string(), |all, _| {
                            format!("{all}.and({once})")
                        })
                    }
                    _ => {
                        let once = self.cost(&once, label)?;
                        let n = self.asked_number(times)?;
                        self.reading_cost(
                            &format!("({once}).times(({n}).clamp(0, 255) as u8)"),
                            label,
                        )
                    }
                }
            }
            CostPart::Constrained { part, constraints } => {
                if let (
                    CostPart::ChangeTapState {
                        selections,
                        tapped: true,
                        symbol: false,
                    },
                    [SelectionConstraint::TotalProperty {
                        property: Property::Power,
                        comparison: Comparison::GreaterOrEqual,
                        amount: Expr::Constant(power),
                    }],
                ) = (&**part, constraints.as_slice())
                {
                    if let [CostObjectSelection {
                        objects: crate::battlefield_query!(filter),
                        choose: Some(Count::AnyNumber),
                    }] = selections.as_slice()
                    {
                        if other_untapped_creatures(filter) {
                            return Ok(field(&format!(
                                "tap_creatures: Some(TapCreatures {{ power: {power}, greater_with: None }})"
                            )));
                        }
                    }
                }
                let inner = self.cost(
                    &Cost {
                        parts: vec![(**part).clone()],
                    },
                    label,
                )?;
                let rules = constraints
                    .iter()
                    .map(selection_rule)
                    .collect::<Result<Vec<_>>>()?;
                planned::planned_value(
                    "engine: no set rule on what a cost chooses",
                    "Cost::NONE",
                    &format!("({inner}).chosen_under(&[{}])", rules.join(", ")),
                )
            }
            // Tariff: "sacrifices the creature ... unless they pay that creature's mana
            // cost". The data pays the cost of any creature that ties and then has the
            // sacrifice choose among them again, so the creature paid for and the one
            // sacrificed are not one choice.
            CostPart::ObjectAction {
                action: CostObjectAction::PayManaCost,
                ..
            } => {
                return Err(gap(
                    "a mana cost paid for one creature and a sacrifice choosing again",
                ))
            }
            other => return Err(gap(format!("cost part {}", form(other)))),
        })
    }
    /// "Behold [N] [quality]" as a part of a cost.
    fn behold(&mut self, n: i64, what: &Operand, call: &str, label: &str) -> Result<String> {
        let (test, quality) = behold_words(what)?;
        let function = format!("{label}_behold_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {test} }}").unwrap();
        let count = if call == "cost_of" {
            format!("{n}, ")
        } else {
            String::new()
        };
        Ok(format!(
            "{M}::behold::{call}({count}{function}, {})",
            quoted(&quality)
        ))
    }
    /// Boast's test (CR 702.142a).
    pub(super) fn boast(&mut self, label: &str) -> String {
        let function = format!("{label}_boast");
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ q.attacked_this_turn(asker.source) }}").unwrap();
        function
    }
    /// Who may activate an ability (CR 602.2), where it is not its controller.
    /// "Any player may activate this ability" is data (Forge's `AnyPlayer$
    /// True`); any other set of players is a `fn` the card's ability holds, as
    /// `condition` holds one, asked of a player as they would activate it.
    pub(super) fn activators(
        &mut self,
        def: String,
        who: Option<&PlayerSet>,
        label: &str,
    ) -> Result<String> {
        let Some(who) = who else {
            return Ok(def);
        };
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        let test = asked.player_in(who, "player")?;
        const FEATURE: &str = "engine: no activation by other players";
        let rule = match test.as_str() {
            "true" => ".any_player_may_activate()".to_string(),
            _ => {
                let function = format!("{label}_activators");
                let item = planned::planned_fn(
                    FEATURE,
                    &function,
                    "q: &dyn Query, asker: &Asker, player: PlayerId",
                    "bool",
                    &test,
                );
                writeln!(self.functions, "{item}").unwrap();
                format!(".activated_by({function})")
            }
        };
        Ok(planned::planned_value(
            FEATURE,
            &def,
            &format!("{def}{rule}"),
        ))
    }
    /// What an ability's limits say beyond the engine's own methods.
    pub(super) fn activation_rule_beyond(
        &mut self,
        def: String,
        rule: &ActivationRule,
        label: &str,
    ) -> Result<String> {
        Ok(match rule {
            ActivationRule::OnlyBy(players) => self.activators(def, Some(players), label)?,
            ActivationRule::CantBeCopied => planned::planned_value(
                "engine: no ability that can't be copied",
                &def,
                &format!("{def}.cant_be_copied()"),
            ),
            ActivationRule::AnyTimeOfInstant => format!("{def}.only_as_an_instant()"),
            ActivationRule::RestrictMana {
                portion,
                color: spent,
            } => {
                let rule = match spent {
                    Some(spent) => format!("ManaSpendRule::Colors(&[Color::{}])", color(spent)?),
                    None => "ManaSpendRule::ChosenColor".to_string(),
                };
                let portion = match portion {
                    ManaCostPortion::All => "Spent::Whole",
                    ManaCostPortion::X => "Spent::X",
                };
                planned::planned_value(
                    "engine: no rule on the mana spent on a spell",
                    &def,
                    &format!("{def}.mana_spent({portion}, {rule})"),
                )
            }
            // "Tap an untapped creature you control rather than pay that mana"
            // (CR 118.3), as Heirloom Epic prints it.
            ActivationRule::ManaInstead(cost) if taps_a_creature_you_control(cost) => {
                format!("{def}.tap_creatures_for_mana()")
            }
            ActivationRule::ManaInstead(cost) => {
                let cost = self.cost(cost, label)?;
                planned::planned_value(
                    "engine: no cost paid rather than each mana",
                    &def,
                    &format!("{def}.instead_of_each_mana({cost})"),
                )
            }
            // "Activate no more times each turn than ...": this ability of this object,
            // told from the others by its words.
            ActivationRule::MaxPerTurn(n) => {
                let most = self.asked_number(n)?;
                let Some(line) = self.line_text() else {
                    return Ok(planned::planned_value(
                        "engine: no limit on activations each turn",
                        &def,
                        &def,
                    ));
                };
                let function = format!("{label}_more_this_turn");
                let text = quoted(line);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ let made = q.history().select(EventQuery::kind(EventMask::ABILITY_ACTIVATED).this_turn()).filter(|r| matches!(r.event, Event::AbilityActivated {{ source, ability, .. }} if source == asker.source && q.activated_ability(ability).is_some_and(|def| def.text == {text}))).count() as i64; made < i64::from({most}) }}").unwrap();
                format!("{def}.condition({function})")
            }
            ActivationRule::XIs(NumberTest::Compare { comparison: Comparison::Equal, value: n }) | ActivationRule::XAtMost(n) => {
                let exact = matches!(rule, ActivationRule::XIs(_));
                let method = if exact { "x_is" } else { "x_at_most" };
                if planned::switched_on(x_rules::FEATURE) {
                    let callback = self.x_value_callback(n, &format!("{label}_{method}"))?;
                    return Ok(format!("{def}.{method}({callback})"));
                }
                let bound = self.asked_number(n)?;
                const FEATURE: &str = "engine: no rule on an ability's X";
                // The number, asked as the ability is activated: a `fn` it holds.
                let function = format!("{label}_{method}");
                let item = planned::planned_fn(
                    FEATURE,
                    &function,
                    "q: &dyn Query, asker: &Asker",
                    "i64",
                    &bound,
                );
                writeln!(self.functions, "{item}").unwrap();
                planned::planned_value(FEATURE, &def, &format!("{def}.{method}({function})"))
            }
            ActivationRule::XIs(test) => {
                const FEATURE: &str = "engine: numeric predicate on an ability's X";
                let function = format!("{label}_x_matches");
                let predicate = self.number_test("i64::from(x)", test)?;
                let item = planned::planned_fn(FEATURE, &function, "q: &dyn Query, asker: &Asker, x: u16", "bool", &predicate);
                writeln!(self.functions, "{item}").unwrap();
                planned::planned_value(FEATURE, &def, &format!("{def}.x_matches({function})"))
            }
            other => return Err(gap(format!("activation rule {}", form(other)))),
        })
    }
    /// The rules for casting a spell that `casting` does not read.
    pub(super) fn casting_beyond(
        &mut self,
        rule: &CastingRule,
        label: &str,
        parts: &mut Parts,
        condition: Option<&Condition>,
    ) -> Result<()> {
        let sole = |parts: &Parts, field: &'static str| match parts.fields.contains_key(field) {
            true => Err(gap(format!("two rules for {field}"))),
            false => Ok(()),
        };
        match (rule, condition) {
            (
                CastingRule::AdditionalCost {
                    cost,
                    optional: true,
                    repetition: CostRepetition::Once,
                },
                None,
            ) => {
                sole(parts, "optional_cost")?;
                // "You may sacrifice one or more creatures. When you do, copy this spell
                // for each creature sacrificed this way": the number the spell paid.
                if let Some(filter) = sacrifices_to_copy(cost) {
                    let filter = self.cost_filter(&filter, label)?;
                    writeln!(self.functions, "#[allow(unused_variables)] fn {label}_sacrificed(q: &dyn Query, asker: &Asker) -> bool {{ q.obj(asker.source).paid > 0 }}\nfn {label}_copies(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ for _ in 0..g.obj(item.object).paid {{ g.copy_spell(item.object, item.controller, false)?; }} Ok(()) }}").unwrap();
                    parts.field("optional_cost", format!("Some(OptionalCost::new(Cost {{ variable: Some(VariableCost::SacrificeOneOrMore({filter})), ..Cost::NONE }}))"));
                    parts.field(
                        "labels",
                        "&[\"Cast without sacrificing\", \"Sacrifice one or more creatures\"]"
                            .to_string(),
                    );
                    parts.entry("abilities", format!("triggered(cantrip_mechanics::triggers::CAST_THIS_SPELL, {label}_copies, \"When you do, copy this spell for each creature sacrificed this way.\").condition({label}_sacrificed)"));
                    return Ok(());
                }
                let code = self.optional_cost(cost, label)?;
                self.cost_labels(cost, parts);
                match code {
                    Optional::Cost(code) => parts.field("optional_cost", code),
                    Optional::Kicker(code) => parts.field("kicker", code),
                }
            }
            // "This spell costs {2}{R} more for each target beyond the first", and
            // "if it targets a Dragon": a cost that reads the announced targets.
            (
                CastingRule::AdditionalCost {
                    cost,
                    optional: false,
                    repetition,
                },
                condition,
            ) => {
                sole(parts, "cost_of_targets")?;
                let cost = self.cost(cost, label)?;
                let times = match repetition {
                    CostRepetition::Once => cost,
                    CostRepetition::TargetsBeyond(n) => {
                        format!("({cost}).times(targets.len().saturating_sub({n}).min(255) as u8)")
                    }
                };
                let test = match condition {
                    Some(condition) => {
                        let saved = (self.reader, self.event_at);
                        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                        self.costing_targets = true;
                        let test = self.condition(condition);
                        self.costing_targets = false;
                        (self.reader, self.event_at) = saved;
                        test?
                    }
                    None => "true".into(),
                };
                let function = format!("{label}_of_targets");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, targets: &[Target]) -> Cost {{ if {test} {{ {times} }} else {{ Cost::NONE }} }}").unwrap();
                parts.field("cost_of_targets", format!("Some({function})"));
            }
            // Necromancy's rider, which the core keeps: the sacrifice as the next
            // cleanup step begins if the permission was what let it be cast then.
            (CastingRule::FlashWithRider(rider), None) if sacrificed_at_cleanup(rider) => {
                sole(parts, "cast_as_though_flash")?;
                parts.field("cast_as_though_flash", "true".into());
                parts.field("untimely_cleanup_rider", "true".into());
            }
            (
                CastingRule::Flash {
                    additional_cost: Some(cost),
                },
                None,
            ) => {
                let cost = self.cost(cost, label)?;
                // Only a spell cast by its own permission at a time a sorcery
                // couldn't be owes it: not one an effect casts, or another
                // permission lets be cast then. Cantrip marks that for the
                // cleanup rider, which a sorcery has no permanent to act on.
                if self.sorcery && !self.own_face && !parts.fields.contains_key("cost_of_targets") {
                    sole(parts, "cast_as_though_flash")?;
                    parts.field("cast_as_though_flash", "true".into());
                    parts.field("untimely_cleanup_rider", "true".into());
                    let function = format!("{label}_of_flash");
                    writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, targets: &[Target]) -> Cost {{ if q.state().obj(asker.source).marks.untimely_by_own_flash {{ {cost} }} else {{ Cost::NONE }} }}").unwrap();
                    parts.field("cost_of_targets", format!("Some({function})"));
                    return Ok(());
                }
                self.plan_entry(
                    "engine: no flash for an additional cost",
                    "flash_for",
                    &format!("Some({cost})"),
                    parts,
                );
            }
            (CastingRule::ReduceForEachPaid { reductions, cost }, None) => {
                // The spell's own cost (Gorex, "exile any number of creature
                // cards from your graveyard"): what it exiles is exiled with the
                // permanent (CR 607.2q).
                let was = std::mem::replace(&mut self.spell_cost, true);
                let paid = self.reduce_for_each_paid(reductions, cost, label, parts);
                self.spell_cost = was;
                paid?;
            }
            (CastingRule::RestrictMana { portion, filter }, None) => {
                let rule = match filter {
                    ManaPaymentFilter::None => "ManaSpendRule::Nothing".to_string(),
                    ManaPaymentFilter::Colors(colors) => format!(
                        "ManaSpendRule::Colors(&[{}])",
                        colors
                            .iter()
                            .map(|c| color(c).map(|c| format!("Color::{c}")))
                            .collect::<Result<Vec<_>>>()?
                            .join(", ")
                    ),
                    ManaPaymentFilter::ProducedBy(filter) => {
                        let test = self.spending_test(filter, "source")?;
                        let params = "q: &dyn Query, asker: &Asker, source: ObjectId";
                        let feature = "engine: no rule on the mana spent on a spell";
                        let produced = self.rule_fn(
                            feature,
                            &format!("{label}_mana_source"),
                            params,
                            "bool",
                            &test,
                        );
                        format!("ManaSpendRule::ProducedBy({produced})")
                    }
                };
                let portion = match portion {
                    ManaCostPortion::All => "Spent::Whole",
                    ManaCostPortion::X => "Spent::X",
                };
                self.plan_entry(
                    "engine: no rule on the mana spent on a spell",
                    "mana_spent",
                    &format!("Some(({portion}, {rule}))"),
                    parts,
                );
            }
            (other, _) => return self.stated_casting(other, parts),
        }
        Ok(())
    }
    /// An optional additional cost as the card's field. A waterbend is the
    /// kicker the engine has for it.
    fn optional_cost(&mut self, cost: &Cost, label: &str) -> Result<Optional> {
        if let [CostPart::Waterbend(symbols)] = cost.parts.as_slice() {
            return Ok(Optional::Kicker(format!(
                "waterbend_kicker({})",
                mana(symbols)?
            )));
        }
        if let [CostPart::Sacrifice {
            objects: crate::battlefield_query!(filter),
            choose: Some(Count::AnyNumber),
        }] = cost.parts.as_slice()
        {
            let filter = self.cost_filter(filter, label)?;
            return Ok(Optional::Cost(format!("Some(OptionalCost::new(Cost {{ variable: Some(VariableCost::SacrificeOneOrMore({filter})), ..Cost::NONE }}))")));
        }
        let cost = self.cost(cost, label)?;
        Ok(Optional::Cost(format!("Some(OptionalCost::new({cost}))")))
    }
    /// "This spell costs {2} less for each [cost] paid."
    fn reduce_for_each_paid(
        &mut self,
        reductions: &[ManaReduction],
        cost: &Cost,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let [ManaReduction {
            color: None,
            amount: Expr::Constant(less),
        }] = reductions
        else {
            return Err(gap("casting rule ReduceForEachPaid"));
        };
        if parts.fields.contains_key("additional_cost") {
            return Err(gap("two rules for additional_cost"));
        }
        let part = match cost.parts.as_slice() {
            [CostPart::Sacrifice {
                objects: crate::battlefield_query!(filter),
                choose: Some(Count::Exactly(Expr::Constant(1)) | Count::AnyNumber),
            }] => {
                let filter = self.cost_filter(filter, label)?;
                Some(format!("Cost {{ variable: Some(VariableCost::SacrificeAnyNumber {{ filter: {filter}, less_each: {less} }}), ..Cost::NONE }}"))
            }
            [CostPart::Move {
                selections,
                destination: MovementDestination::Exile { .. },
                ..
            }] => match selections.as_slice() {
                [CostObjectSelection {
                    objects:
                        ObjectSet::Zone {
                            scope:
                                ZoneScope::Players {
                                    zone: EventZone::Hand,
                                    players: PlayerSet::One(PlayerRef::AbilityController),
                                },
                            filter,
                            ..
                        },
                    choose: Some(Count::Exactly(Expr::Constant(1)) | Count::AnyNumber),
                }] => {
                    let filter = self.cost_filter(filter, label)?;
                    Some(format!("Cost {{ exile_from_hand_for_less: Some(ExileFromHandForLess {{ filter: {filter}, less_each: {less} }}), ..Cost::NONE }}"))
                }
                _ => None,
            },
            _ => None,
        };
        match part {
            Some(code) => parts.field("additional_cost", format!("Some({code})")),
            None => {
                let cost = self.cost(cost, label)?;
                self.plan_entry(
                    "engine: no cost reduced for each time a cost is paid",
                    "additional_cost",
                    &format!("Some(Cost::for_less({cost}, {less}))"),
                    parts,
                );
            }
        }
        Ok(())
    }
    /// The keywords whose cost branch `cost_branch` does not read.
    pub(super) fn cost_branch_beyond(
        &mut self,
        keyword: &str,
        cost: &Cost,
        paid: &Ability,
        unpaid: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let (
            Ability::Spell {
                announcement: paid_announced,
                body: paid_body,
            },
            Ability::Spell {
                announcement: unpaid_announced,
                body: unpaid_body,
            },
        ) = (paid, unpaid)
        else {
            return Err(gap(format!("cost branch {keyword}")));
        };
        let same_targets =
            format!("{:?}", paid_announced.targets) == format!("{:?}", unpaid_announced.targets);
        match keyword {
            "AdditionalCost" | "Teamwork" => {
                self.paying = Paying::Additional;
                let optional = match cost.parts.as_slice() {
                    [CostPart::Teamwork(n)] => Ok(Optional::Cost(format!(
                        "Some({M}::teamwork::optional({n}))"
                    ))),
                    _ => self.optional_cost(cost, label),
                };
                self.paying = Paying::Ability;
                // What it's paid with is announced before the targets (CR 601.2b-c).
                parts.field("late_targets", "true".into());
                self.cost_labels(cost, parts);
                let was_paid = match optional? {
                    Optional::Cost(code) => {
                        parts.field("optional_cost", code);
                        "item.paid > 0"
                    }
                    Optional::Kicker(code) => {
                        parts.field("kicker", code);
                        "item.kicked > 0"
                    }
                };
                if let (Some(paid_modes), Some(unpaid_modes)) =
                    (&paid_announced.modes, &unpaid_announced.modes)
                {
                    let (Expr::Constant(least), Expr::Constant(most), true) = (
                        &paid_modes.count.minimum,
                        &paid_modes.count.maximum,
                        format!("{:?}", paid_modes.options)
                            == format!("{:?}", unpaid_modes.options),
                    ) else {
                        return Err(gap(format!("cost branch {keyword} that changes its modes")));
                    };
                    let modal = self.modal(unpaid_announced, label, None)?;
                    writeln!(
                        self.functions,
                        "static {modal}_PAID: ModalDef = {modal}.when_paid({least}, {most});"
                    )
                    .unwrap();
                    parts.spell = "Some(SpellDef::MODAL)".into();
                    parts.modal = format!("Some(&{modal}_PAID)");
                    return Ok(());
                }
                if !same_targets {
                    return Err(gap(format!(
                        "cost branch {keyword} with targets of its own"
                    )));
                }
                let targets = self.announcement(unpaid_announced, &format!("{label}_unpaid"))?;
                let effect = self.branch_effect(
                    was_paid,
                    paid_body,
                    paid_announced,
                    unpaid_body,
                    unpaid_announced,
                    label,
                )?;
                parts.spell = format!("spell({targets}, {effect})");
            }
            "Awaken" => {
                let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                    return Err(gap("awaken cost other than mana"));
                };
                let paid_label = format!("{label}_awakened");
                let targets = self.announcement(paid_announced, &paid_label)?;
                let effect = self.effect(paid_body, &paid_label)?;
                self.resolving_slots_of(unpaid_announced);
                let unpaid_targets =
                    self.announcement(unpaid_announced, &format!("{label}_unpaid"))?;
                let unpaid_effect = self.effect(unpaid_body, &format!("{label}_unpaid"))?;
                parts.spell = format!("spell({unpaid_targets}, {unpaid_effect})");
                self.plan_entry(
                    "engine: no Awaken",
                    "ways",
                    &format!(
                        "{M}::awaken::way({}, {targets}, {effect}, \"Awaken\")",
                        mana(symbols)?
                    ),
                    parts,
                );
            }
            other => return Err(gap(format!("cost branch {other}"))),
        }
        Ok(())
    }
    fn resolving_slots_of(&mut self, announced: &Announcement) {
        self.slots.clear();
        for (i, spec) in announced.targets.iter().enumerate() {
            self.slots.insert(spec.slot.0, (i, spec.clone()));
        }
    }
    /// The keyword a granted mechanic is, where it is only a keyword.
    pub(super) fn grants_keyword(&self, ability: &Ability) -> Option<String> {
        let Ability::Mechanic {
            mechanic,
            cost: None,
            amount: None,
            filter: None,
        } = ability
        else {
            return lane_j::granted_keyword(ability).map(String::from);
        };
        use Mechanic as K;
        matches!(
            mechanic,
            K::Banding | K::Decayed | K::Phasing | K::SplitSecond
        )
        .then(|| format!("Keyword::{mechanic:?}"))
    }
    /// What a spell's body says of the card as a whole.
    pub(super) fn spell_keywords(&mut self, parts: &mut Parts) {
        if std::mem::take(&mut self.paradigm) {
            parts.field("paradigm", format!("Some({M}::paradigm::SPELL_ABILITY)"));
            self.delayed.insert(0, format!("{M}::paradigm::TRIGGER"));
        }
    }
    /// "Ward--[cost]": counter it unless the player whose spell or ability
    /// targeted pays, whatever the engine can ask of them while it resolves.
    pub(super) fn ward_beyond(&mut self, cost: &Cost, label: &str) -> Result<String> {
        let saved = (self.reader, self.event_at, self.stack_item);
        (self.reader, self.event_at, self.stack_item) = (Reader::Resolution, EventAt::Item, true);
        let price = self.cost(cost, PAID);
        (self.reader, self.event_at, self.stack_item) = saved;
        let price = price?;
        let function = format!("{label}_ward_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let asker = &Asker::new(item.object, item.controller); let Target::Object(by) = item.subject else {{ return Ok(()); }}; let payer = PlayerId(item.data[0] as u8); if !g.may_pay(payer, item.object, by, {price}, &{M}::COUNTERED_UNLESS_PAID)? {{ g.counter_item(by, item.data[1] as usize); }} Ok(()) }}").unwrap();
        Ok(format!(
            "triggered(when(EventMask::BECAME_TARGET, cantrip_api::intrinsic::ward_matches), {function}, {}).ward()",
            quoted("Ward")
        ))
    }
    /// A protection quality printed on a card, beyond its colors (CR 702.16a).
    pub(super) fn protection_beyond(
        &mut self,
        protection: &Protection,
        parts: &mut Parts,
    ) -> Result<()> {
        const FEATURE: &str = "engine: no protection from this quality";
        let text = |what: &str| quoted(&format!("Protection from {what}"));
        match protection {
            Protection::Objects {
                kind: ObjectKind::Any,
                filter: ObjectFilter::Color(c),
            } => {
                let word = color(c)?;
                parts.statics.push(format!(
                    "{M}::protection::from::<{{ Color::{word} as u8 }}>({})",
                    text(&word.to_lowercase())
                ));
            }
            Protection::Each(qualities) => {
                for quality in qualities {
                    self.protection_beyond(quality, parts)?;
                }
            }
            Protection::Objects {
                kind: ObjectKind::Any,
                filter: ObjectFilter::Subtype(s),
            } => {
                let word = name(s);
                if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                    return Err(gap(format!("subtype {word}")));
                }
                let statics = format!(
                    "{M}::protection::from_subtype::<{{ {M}::lands::sub(Subtype::{word}) }}>({})",
                    text(&word)
                );
                parts.statics.push(statics);
            }
            Protection::Objects {
                kind: ObjectKind::Any,
                filter: ObjectFilter::CardType(t),
            } => {
                let t = card_type(t)?;
                let statics = format!(
                    "{M}::protection::from_type::<{{ CardType::{t} as u8 }}>({})",
                    text(&t)
                );
                parts.statics.push(statics);
            }
            Protection::Objects {
                kind: ObjectKind::Any,
                filter:
                    ObjectFilter::Property { property: Property::ManaValue, test: NumberTest::Compare { comparison: Comparison::GreaterOrEqual, value: Expr::Constant(n) } },
            } => {
                let statics = format!(
                    "{M}::protection::from_mana_value_at_least::<{n}>({})",
                    text(&format!("mana value {n} or greater"))
                );
                parts.statics.push(statics);
            }
            Protection::Objects {
                kind: ObjectKind::Any,
                filter: ObjectFilter::Any,
            } => {
                self.printed_protection("everything", "chars.protection_everything = true;", parts);
            }
            Protection::Objects {
                kind: ObjectKind::Any,
                ..
            } if self.lines_said().iter().any(|line| {
                line.contains("protection from spells that are one or more colors")
            }) =>
            {
                self.printed_protection(
                    "spells that are one or more colors",
                    "chars.protection_colored_spells = true;",
                    parts,
                );
            }
            Protection::Objects { kind, filter } => {
                let test = self.spending_test(filter, "source")?;
                let params = "q: &dyn Query, asker: &Asker, source: ObjectId";
                let quality = self.rule_fn(FEATURE, "protection_quality", params, "bool", &test);
                let code = format!(
                    "{M}::protection::from_matching(ObjectKind::{kind:?}, {quality}, {})",
                    text("a quality")
                );
                self.plan_entry(FEATURE, "statics", &code, parts);
            }
            Protection::Players(players) => {
                let mut asked = self.clone();
                (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
                let test = asked.player_in(players, "player")?;
                let code = format!(
                    "{M}::protection::from_players(|q, asker, player| {test}, {})",
                    text("players")
                );
                self.plan_entry(FEATURE, "statics", &code, parts);
            }
        }
        Ok(())
    }
    /// A quality `Chars` has a field for, printed as a keyword (Progenitus).
    fn printed_protection(&mut self, what: &str, statement: &str, parts: &mut Parts) {
        let apply = format!("printed_protection_{}", parts.statics.len());
        writeln!(
            self.functions,
            "fn {apply}(_: &dyn LayerQuery, _: ObjectId, chars: &mut Chars) {{ {statement} }}"
        )
        .unwrap();
        let text = quoted(&format!("Protection from {what}"));
        parts.statics.push(format!(
            "static_ability(Layer::Ability, {M}::statics::itself, {apply}, {text})"
        ));
    }
    /// A filter asked of a spell or source where mana is being spent.
    fn spending_test(&self, filter: &ObjectFilter, id: &str) -> Result<String> {
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        asked.filter(filter, &Candidate::of(id))
    }
    /// A `ManaUse` as a `ManaRestriction`: the engine's own words, and otherwise
    /// the test its mana will carry.
    pub(super) fn mana_use_composed(&self, value: &ManaUse) -> Result<String> {
        const FEATURE: &str = "engine: mana spent by what it may pay for";
        let planned =
            |code: String| planned::planned_value(FEATURE, "ManaRestriction::ABILITIES", &code);
        if let Ok(code) = mana_restriction(value) {
            return Ok(code);
        }
        let many = |c: &Self, uses: &[ManaUse]| -> Result<String> {
            let codes = uses
                .iter()
                .map(|u| c.mana_use_composed(u))
                .collect::<Result<Vec<_>>>()?;
            Ok(codes.join(", "))
        };
        Ok(match value {
            // The engine knows one union, and fails the build on any other.
            ManaUse::Either(uses) => {
                let codes = many(self, uses)?;
                let code = format!("ManaRestriction::AnyOf(&[{codes}])");
                if codes == "ManaRestriction::ARTIFACT_SPELLS, ManaRestriction::ABILITIES" {
                    code
                } else {
                    planned(code)
                }
            }
            ManaUse::All(uses) => {
                planned(format!("ManaRestriction::AllOf(&[{}])", many(self, uses)?))
            }
            ManaUse::Not(inner) => planned(format!(
                "ManaRestriction::Not(&{})",
                self.mana_use_composed(inner)?
            )),
            ManaUse::CastSpell(filter) => match spell_words(
                filter,
                self.enters_choosing.contains(&ValueKind::CreatureType),
            ) {
                Some(code) => code,
                None => planned(format!(
                    "ManaRestriction::SpellsMatching(|q, asker, spell| {})",
                    self.spending_test(filter, "spell")?
                )),
            },
            ManaUse::ActivateAbility {
                sources,
                permanents_only,
            } => planned(format!(
                "ManaRestriction::AbilitiesMatching(|q, asker, source| {}, AbilitiesOf::{})",
                self.spending_test(sources, "source")?,
                if *permanents_only {
                    "Permanents"
                } else {
                    "Sources"
                }
            )),
            ManaUse::Purpose(purpose) => planned(format!(
                "ManaRestriction::Purpose(ManaPurpose::{})",
                form(purpose)
            )),
            ManaUse::Persists(duration) => planned(format!(
                "ManaRestriction::Lasts(Duration::{})",
                form(duration)
            )),
            ManaUse::SpentOn { spells, .. }
            | ManaUse::SpentGives { spells, .. }
            | ManaUse::WhenSpent { spells, .. } => planned(format!(
                "ManaRestriction::Rider({}, |q, asker, spell| {})",
                quoted(&ron::to_string(value).unwrap_or_default()),
                self.spending_test(spells, "spell")?
            )),
            ManaUse::Primitive { name, .. } => {
                return Err(gap(format!("primitive mana use {name}")))
            }
        })
    }
    /// A token definition a planned keyword will list in `EXTRA`.
    fn plan_extra(&mut self, feature: &str, path: String) {
        if planned::landed(feature) {
            self.extras.push(path);
            return;
        }
        let id = planned::requirement(feature);
        writeln!(self.holes, "planned_item!({id:?} => extras: {path});").unwrap();
    }
    /// Changes an object keeps for the rest of the game (Alchemy's "perpetually").
    pub(super) fn perpetual_effect(
        &mut self,
        objects: &ObjectSet,
        changes: &[Change],
        out: &mut String,
    ) -> Result<()> {
        const FEATURE: &str = "engine: no perpetual effects";
        let objects = match self.objects(objects) {
            Ok(code) => self.fetch(code),
            // The objects come from an instruction with a hole of its own.
            Err(failed) if failed.feature == FOLLOWS_GAP => return Err(gap(FEATURE)),
            Err(failed) => return Err(failed),
        };
        let id = self.fresh();
        let mut code = format!("let objects_{id}: Vec<ObjectId> = {objects};\n");
        // What each change reads of "it" (Teyo's "equal to its toughness") is
        // of the object it is on, so each is made for that object.
        let saved = self.affected_object.replace("object");
        let mut kinds = Vec::new();
        for change in changes {
            match self.perpetual_kinds(change) {
                Ok(made) => kinds.extend(made),
                Err(failed) => {
                    self.affected_object = saved;
                    return Err(failed);
                }
            }
        }
        self.affected_object = saved;
        let applies: String = kinds
            .into_iter()
            .enumerate()
            .map(|(i, kind)| {
                let kind = self.fetch(kind);
                format!("let kind_{id}_{i} = {kind}; {M}::perpetual::apply(g, asker.source, object, kind_{id}_{i}); ")
            })
            .collect();
        write!(code, "for &object in &objects_{id} {{ {applies}}}").unwrap();
        self.plan_code(FEATURE, &code, out);
        Ok(())
    }
    fn perpetual_kinds(&mut self, change: &Change) -> Result<Vec<String>> {
        match change {
            Change::GrantAbility(ability) => Ok(vec![self.perpetual_grant(ability)?]),
            Change::Incorporate(symbols) => Ok(vec![format!(
                "{PERPETUAL}::Incorporate({})",
                mana(symbols)?
            )]),
            other => Ok(self
                .effect_kinds(other)?
                .into_iter()
                .map(|kind| format!("{PERPETUAL}::Effect({kind})"))
                .collect()),
        }
    }
    /// An ability an object keeps: one of `delayed` where it is one ability, and
    /// otherwise all that its rules add to a card, as a definition of their own.
    fn perpetual_grant(&mut self, ability: &Ability) -> Result<String> {
        if matches!(ability, Ability::Casting(CastingRule::SpendAsThoughAnyType)) {
            return Ok(format!("{PERPETUAL}::SpendAsThoughAnyType"));
        }
        let id = self.fresh();
        let grown = self.grown_parts(ability, &format!("perpetual_{id}"))?;
        if let [def] = grown.abilities.as_slice() {
            if grown.only_abilities() {
                self.delayed.push(def.clone());
                let index = self.delayed.len() - 1;
                return Ok(format!(
                    "{PERPETUAL}::Effect(EffectKind::AddAbility(AbilityRef::delayed({}, {index})))",
                    self.source_card()
                ));
            }
        }
        let enters = self.enters(&grown.enters);
        let name = format!("PERPETUAL_{id}");
        writeln!(
            self.functions,
            "static {name}: CardDef = {};",
            grown.card(&enters)
        )
        .unwrap();
        Ok(format!("{PERPETUAL}::Rules(&{name})"))
    }
    /// A keyword that isn't one of the many `mechanic` lists by the same shape.
    pub(super) fn mechanic_beyond(
        &mut self,
        mechanic: Mechanic,
        cost: Option<&Cost>,
        amount: Option<&Expr>,
        filter: Option<&ObjectFilter>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        use Mechanic as K;
        let feature = format!("engine: no {mechanic:?}");
        let fixed = || -> Result<u8> {
            amount
                .and_then(costs::constant_u8)
                .ok_or_else(|| gap(feature.clone()))
        };
        // What Cantrip has: the card lists it as the mechanic says.
        match mechanic {
            K::AnnihilatorX => {
                let amount = amount.ok_or_else(|| gap(feature.clone()))?;
                let ability = self.annihilator_x(amount, label)?;
                parts.abilities.push(ability);
                return Ok(());
            }
            K::StartYourEngines => {
                parts.field("start_your_engines", "true".into());
                return Ok(());
            }
            K::Storied => {
                parts.field("storied", "true".into());
                return Ok(());
            }
            K::Aftermath => {
                parts.field("aftermath", "true".into());
                return Ok(());
            }
            K::Fuse => {
                parts.field("fuse", "true".into());
                return Ok(());
            }
            K::Daybound | K::Nightbound => {
                parts.keywords.push(format!("Keyword::{mechanic:?}"));
                return Ok(());
            }
            K::Station => {
                parts
                    .abilities
                    .push(format!("{M}::station::ability(\"Station\")"));
                return Ok(());
            }
            K::Hideaway => {
                let n = amount
                    .and_then(costs::constant_u8)
                    .ok_or_else(|| gap("Hideaway without a fixed number"))?;
                let function = format!("{label}_hideaway");
                writeln!(self.functions, "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {M}::hideaway(g, item, {n}) }}").unwrap();
                parts.abilities.push(format!(
                    "triggered({M}::triggers::THIS_ENTERS, {function}, {})",
                    quoted(&format!("Hideaway {n}"))
                ));
                return Ok(());
            }
            K::BeamMeUp => {
                let Some([CostPart::Mana(symbols)]) = cost.map(|c| c.parts.as_slice()) else {
                    return Err(gap("BeamMeUp cost other than mana"));
                };
                let text = quoted(&self.said_word("Beam me up".into()));
                let way = format!("{M}::beam_me_up::way({}, {text})", mana(symbols)?);
                parts.table("ways", way);
                return Ok(());
            }
            _ => {}
        }
        // The rest is the keyword's own module, once Cantrip has it.
        let word = name(&mechanic);
        let module = |word: &str| {
            let mut snake = String::new();
            for (i, c) in word.chars().enumerate() {
                if c.is_ascii_uppercase() && i > 0 {
                    snake.push('_');
                }
                snake.push(c.to_ascii_lowercase());
            }
            snake
        };
        let path = module(&word);
        let word = self.said_word(word);
        let trigger = |n: u8| format!("{M}::{path}::trigger::<{n}>({word:?})");
        let plain = format!("{M}::{path}::TRIGGER");
        let mut entries: Vec<(&'static str, String)> = Vec::new();
        match mechanic {
            K::Bushido
            | K::Soulshift
            | K::Rampage
            | K::Afflict
            | K::Renown
            | K::Poisonous
            | K::Frenzy
            | K::Fabricate
            | K::Afterlife => entries.push(("abilities", trigger(fixed()?))),
            K::Exploit
            | K::DoubleTeam
            | K::Soulbond
            | K::Conspire
            | K::Haunt
            | K::Gravestorm
            | K::Demonstrate => entries.push(("abilities", plain)),
            K::Absorb => entries.push((
                "preventions",
                format!("{M}::absorb::prevention::<{}>()", fixed()?),
            )),
            K::Devour | K::Tribute | K::Amplify | K::Bloodthirst | K::VanishingEnters => {
                let (path, n) = (path.trim_end_matches("_enters"), fixed()?);
                entries.push(("enters", format!("{M}::{path}::enters::<{n}>")));
            }
            K::BloodthirstX => {
                entries.push(("enters", format!("{M}::bloodthirst::enters_x")));
            }
            K::Vanishing => {
                entries.push((
                    "abilities",
                    format!("{M}::vanishing::upkeep(\"Vanishing\")"),
                ));
                entries.push(("abilities", format!("{M}::vanishing::LAST_COUNTER")));
            }
            K::Graft => {
                entries.push(("enters", format!("{M}::graft::enters::<{}>", fixed()?)));
                entries.push(("abilities", format!("{M}::graft::TRIGGER")));
            }
            K::Blitz | K::Prowl | K::Disturb | K::Mutate | K::MoreThanMeetsTheEye => {
                let price = priced(self, cost, mechanic, label)?;
                entries.push(("ways", format!("{M}::{path}::way({price}, {word:?})")));
            }
            K::Encore | K::Specialize | K::Recover | K::Fortify | K::SpecializeFromGraveyard => {
                let price = priced(self, cost, mechanic, label)?;
                entries.push((
                    "abilities",
                    format!("{M}::{path}::ability({price}, {word:?})"),
                ));
            }
            K::Reinforce => {
                let price = priced(self, cost, mechanic, label)?;
                let n = fixed()?;
                entries.push((
                    "abilities",
                    format!("{M}::reinforce::ability::<{n}>({price}, {word:?})"),
                ));
            }
            K::Champion => {
                let filter = filter.ok_or_else(|| gap(feature.clone()))?;
                let test = self.cost_filter(filter, &format!("{label}_champion"))?;
                entries.push(("abilities", format!("{M}::champion::enters({test})")));
                entries.push(("abilities", format!("{M}::champion::LEAVES")));
            }
            K::BandsWithOthers => {
                let filter = filter.ok_or_else(|| gap(feature.clone()))?;
                // Printed qualities are closed descriptions: legendary creatures or creatures of a fixed name.
                // The holder is not a lexical source for the quality, so no choosing/granting context is invented.
                fn closed(filter: &ObjectFilter) -> bool {
                    match filter {
                        ObjectFilter::AllOf(parts) => parts.iter().all(closed),
                        ObjectFilter::CardType(_) | ObjectFilter::Supertype(_) | ObjectFilter::Name(_) => true,
                        _ => false,
                    }
                }
                if !closed(filter) { return Err(gap("bands with other of a contextual quality")); }
                let body = self.filter(filter, &Candidate::of("id"))?;
                let test = self.rule_fn(&feature, &format!("{label}_bands"), "q: &dyn Query, id: ObjectId", "bool", &body);
                entries.push(("abilities", format!("{M}::banding::with_others({test}, {word:?})")));
            }
            K::UmbraArmor => {
                entries.push(("replacements", format!("{M}::umbra_armor::REPLACEMENT")));
            }
            K::LivingMetal => {
                entries.push(("statics", format!("{M}::living_metal::ON_YOUR_TURN")));
            }
            K::StartingIntensity => {
                entries.push((
                    "enters",
                    format!("{M}::intensity::starting::<{}>", fixed()?),
                ));
            }
            K::JumpStart => {
                entries.push(("ways", format!("{M}::jump_start::way(\"Jump-start\")")));
            }
            // A core keyword: it is on the card with the rest of its keywords, and the rules do the rest (CR 702.155).
            K::ReadAhead => return Ok(()),
            K::Undaunted => {
                entries.push(("own_cost_changes", format!("{M}::undaunted::cost")));
            }
            K::Compleated => {
                entries.push(("enters", format!("{M}::compleated::enters")));
            }
            K::HiddenAgenda | K::DoubleAgenda => {
                entries.push(("opening_hand", format!("{M}::{path}::OPENING_HAND")));
            }
            other => return Err(gap(format!("engine: no {other:?}"))),
        }
        for (list, code) in entries {
            self.plan_entry(&feature, list, &code, parts);
        }
        match mechanic {
            K::Afterlife => self.plan_extra(&feature, format!("{M}::afterlife::SPIRIT")),
            K::Fabricate => self.plan_extra(&feature, format!("{M}::fabricate::SERVO")),
            _ => {}
        }
        Ok(())
    }
}
