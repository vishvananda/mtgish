//! Exact structural recipes whose rules are already expressed by shared verbs.
use super::*;

fn terms(filter: &ObjectFilter) -> Vec<&ObjectFilter> {
    match filter {
        ObjectFilter::All(all) => all.iter().flat_map(terms).collect(),
        one => vec![one],
    }
}

pub(super) fn creature_anthem(affected: &ObjectSet, changes: &[Change]) -> bool {
    let crate::battlefield_query!(filter) = affected else { return false };
    let mut creature = false;
    let mut controller = false;
    let exact = terms(filter).into_iter().all(|f| match f {
        ObjectFilter::CardType(t) if name(t) == "Creature" => { creature = true; true }
        ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)) => { controller = true; true }
        _ => false,
    });
    exact && creature && controller
        && matches!(changes, [Change::Pump { power: Expr::Constant(1), toughness: Expr::Constant(1) }])
}

fn permanent_types(filter: &ObjectFilter) -> bool {
    let ObjectFilter::Either(all) = filter else { return false };
    let types: BTreeSet<String> = all.iter().filter_map(|f| match f {
        ObjectFilter::CardType(t) => Some(name(t)),
        _ => None,
    }).collect();
    all.len() == 6 && types == ["Artifact", "Battle", "Creature", "Enchantment", "Land", "Planeswalker"].into_iter().map(str::to_string).collect()
}

pub(super) fn graveyard_permanent_target(spec: &TargetSpec) -> Option<String> {
    if !matches!(&spec.kind, TargetKind::Object(d) if d.kind == ObjectKind::Card && d.zones == [EventZone::Graveyard]) {
        return None;
    }
    let mut permanent = false;
    let mut own = false;
    let mut maximum = None;
    let exact = terms(&spec.filter).into_iter().all(|f| match f {
        f if permanent_types(f) => { permanent = true; true }
        ObjectFilter::Property { property: Property::ManaValue, comparison: Comparison::LessOrEqual, value: Expr::Constant(n) } if *n >= 0 && *n <= u32::MAX.into() => { maximum = Some(*n); true }
        ObjectFilter::InZone(ZoneScope::Players { zone: EventZone::Graveyard, players: PlayerSet::One(PlayerRef::AbilityController) }) => { own = true; true }
        _ => false,
    });
    (exact && permanent && own).then_some(maximum).flatten().map(|n| format!("cantrip_mechanics::targets::card_in_your_graveyard_with_mana_value_at_most(q, asker, t, &cantrip_mechanics::targets::PERMANENT_CARD_TYPES, {n})"))
}

/// The two clauses of one printed "attacks or blocks" ability are one trigger.
pub(super) fn combine_attack_block(card: &Card) -> Option<Card> {
    let mut out = card.clone();
    let mut changed = false;
    let mut i = 0;
    while i + 1 < out.abilities.len() {
        let (Ability::Triggered { event: first, .. }, Ability::Triggered { event: second, .. }) = (&out.abilities[i], &out.abilities[i + 1]) else { i += 1; continue };
        let own = |e: &EventSpec, expected: ObjectEvent| matches!(e, EventSpec::Object { event, objects: ObjectFilter::SameObject(ObjectRef::Source) } if std::mem::discriminant(event) == std::mem::discriminant(&expected));
        let pair = own(first, ObjectEvent::Attacks) && own(second, ObjectEvent::Blocks)
            || own(first, ObjectEvent::Blocks) && own(second, ObjectEvent::Attacks);
        if !pair { i += 1; continue; }
        let mut equivalent = out.abilities[i + 1].clone();
        let Ability::Triggered { event, .. } = &mut equivalent else { unreachable!() };
        *event = first.clone();
        if format!("{equivalent:?}") != format!("{:?}", out.abilities[i]) { i += 1; continue; }
        let merged = EventSpec::Either(vec![first.clone(), second.clone()]);
        let Ability::Triggered { event, .. } = &mut out.abilities[i] else { unreachable!() };
        *event = merged;
        out.abilities.remove(i + 1);
        changed = true;
        i += 1;
    }
    changed.then_some(out)
}

pub(super) fn shared_trigger(event: &EventSpec, body: &Program) -> Option<String> {
    // These shared matchers omit event data. A body that reads any keeps its
    // generated matcher and the precise Frame bindings it needs.
    let reads = format!("{body:?}");
    if ["EventSubject", "EventOther", "EventAmount", "EventGroup", "EventDestination", "EventSource"].iter().any(|s| reads.contains(s)) {
        return None;
    }
    if let EventSpec::Spell { event: SpellEvent::Cast, players: PlayerSet::All, spells: ObjectFilter::Color(color) } = event {
        let color = name(color);
        if !matches!(color.as_str(), "White" | "Blue" | "Black" | "Red" | "Green") { return None; }
        return Some(format!("when_asked(EventMask::SPELL_CAST, cantrip_mechanics::triggers::a_player_casts_a_spell_of_color::<{{ Color::{color} as u8 }}>)"));
    }
    let EventSpec::Either(events) = event else { return None };
    if events.len() != 2 { return None; }
    let own_event = |event: &EventSpec, wanted: ObjectEvent| matches!(event, EventSpec::Object { event, objects: ObjectFilter::SameObject(ObjectRef::Source) } if std::mem::discriminant(event) == std::mem::discriminant(&wanted));
    if (own_event(&events[0], ObjectEvent::Attacks) && own_event(&events[1], ObjectEvent::Blocks))
        || (own_event(&events[0], ObjectEvent::Blocks) && own_event(&events[1], ObjectEvent::Attacks))
    {
        return Some("cantrip_mechanics::triggers::THIS_ATTACKS_OR_BLOCKS".into());
    }
    let own_entry = |e: &EventSpec| matches!(e, EventSpec::ZoneChange { objects: ObjectFilter::SameObject(ObjectRef::Source), from: None, to: EventZone::Battlefield });
    let other = if own_entry(&events[0]) { &events[1] } else if own_entry(&events[1]) { &events[0] } else { return None };
    let EventSpec::ZoneChange { objects, from: None, to: EventZone::Battlefield } = other else { return None };
    let mut another = false;
    let mut controlled = false;
    let mut subtype = None;
    let exact = terms(objects).into_iter().all(|f| match f {
        ObjectFilter::Not(inner) if matches!(**inner, ObjectFilter::SameObject(ObjectRef::Source)) => { another = true; true }
        ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)) => { controlled = true; true }
        ObjectFilter::Subtype(t) => { subtype = Some(name(t)); true }
        _ => false,
    });
    (exact && another && controlled).then_some(subtype).flatten().map(|t| format!("when_asked(EventMask::ZONE_CHANGE, cantrip_mechanics::triggers::this_or_another_of_subtype_you_control_enters::<{{ Subtype::{t} as u16 }}>)"))
}

#[cfg(test)]
mod shared_payment_tests {
    use super::*;

    fn body() -> Program {
        Program { instructions: vec![
            Instruction::Effect { result: ResultId(7), operation: Operation::PayCost {
                player: PlayerRef::AbilityController,
                cost: Cost { parts: vec![CostPart::Mana(vec![ManaSymbol::Generic(1)])] }, optional: true,
            } },
            Instruction::If { condition: Condition::PaymentChosen(ResultId(7)),
                then: Program { instructions: vec![Instruction::Effect { result: ResultId(8),
                    operation: Operation::GainLife { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Constant(1) } }] },
                otherwise: Program { instructions: vec![] },
            },
        ] }
    }

    #[test]
    fn payment_recipe_requires_the_exact_payer_result_amount_and_branches() {
        assert!(pay_to_gain_life(&body()).is_some());
        for variant in 0..6 {
            let mut b = body();
            match variant {
                0 => if let Instruction::Effect { operation: Operation::PayCost { optional, .. }, .. } = &mut b.instructions[0] { *optional = false; },
                1 => if let Instruction::Effect { operation: Operation::PayCost { player, .. }, .. } = &mut b.instructions[0] { *player = PlayerRef::Active; },
                2 => if let Instruction::If { condition, .. } = &mut b.instructions[1] { *condition = Condition::PaymentChosen(ResultId(9)); },
                3 => if let Instruction::If { then, .. } = &mut b.instructions[1] {
                    if let Instruction::Effect { operation: Operation::GainLife { amount, .. }, .. } = &mut then.instructions[0] { *amount = Expr::Constant(2); }
                },
                4 => if let Instruction::If { then, otherwise, .. } = &mut b.instructions[1] { *otherwise = then.clone(); },
                5 => if let Instruction::If { then, .. } = &mut b.instructions[1] { then.instructions.push(then.instructions[0].clone()); },
                _ => unreachable!(),
            }
            assert!(pay_to_gain_life(&b).is_none(), "variant {variant}");
        }
    }

    #[test]
    fn color_trigger_keeps_event_dependent_and_copy_matchers() {
        let event = EventSpec::Spell { event: SpellEvent::Cast, players: PlayerSet::All,
                                      spells: ObjectFilter::Color(ManaColor::Red) };
        assert!(shared_trigger(&event, &body()).is_some());
        let mut reads = body();
        if let Instruction::If { then, .. } = &mut reads.instructions[1] {
            if let Instruction::Effect { operation: Operation::GainLife { amount, .. }, .. } = &mut then.instructions[0] { *amount = Expr::EventAmount; }
        }
        assert!(shared_trigger(&event, &reads).is_none());
        let copied = EventSpec::Spell { event: SpellEvent::Copy, players: PlayerSet::All,
                                       spells: ObjectFilter::Color(ManaColor::Red) };
        assert!(shared_trigger(&copied, &body()).is_none());
    }
}

fn ungroup(mut body: &Program) -> &Program {
    while let [Instruction::Group(inner)] = body.instructions.as_slice() { body = inner; }
    body
}

/// The whole resolution is the existing optional fixed-mana payment recipe.
/// A different payer, amount, condition, or either extra branch stays generated.
pub(super) fn pay_to_gain_life(body: &Program) -> Option<&Cost> {
    let [
        Instruction::Effect { result, operation: Operation::PayCost { player: PlayerRef::AbilityController, cost, optional: true } },
        Instruction::If { condition: Condition::PaymentChosen(paid), then, otherwise },
    ] = ungroup(body).instructions.as_slice() else { return None };
    if result.0 != paid.0 || !otherwise.instructions.is_empty()
        || !cost.parts.iter().all(|p| matches!(p, CostPart::Mana(symbols) if !symbols.iter().any(|s| matches!(s, ManaSymbol::X))))
    { return None; }
    matches!(ungroup(then).instructions.as_slice(), [Instruction::Effect {
        operation: Operation::GainLife { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Constant(1) }, ..
    }]).then_some(cost)
}

fn basic_plains(filter: &ObjectFilter) -> bool {
    let all = terms(filter);
    all.len() == 2
        && all.iter().any(|f| matches!(f, ObjectFilter::Supertype(t) if name(t) == "Basic"))
        && all.iter().any(|f| matches!(f, ObjectFilter::Subtype(t) if name(t) == "Plains"))
}

impl Compiler {
    pub(super) fn sequence_recipe(&mut self, body: &Program, out: &mut String) -> Result<bool> {
        let steps = &ungroup(body).instructions;
        if let [
            Instruction::Effect { result: found, operation: Operation::Search { player: PlayerRef::AbilityController, library: PlayerRef::AbilityController, top: None, choices, also, either: false, outside: false } },
            Instruction::Effect { operation: Operation::Reveal { objects: ObjectSet::Selected(revealed) }, .. },
            Instruction::Effect { operation: Operation::Move { objects: ObjectSet::Selected(moved), destination: Destination::OwnersHand, face_down: false, .. }, .. },
            Instruction::Effect { operation: Operation::Shuffle { players: PlayerSet::One(PlayerRef::AbilityController), excluding: None }, .. },
        ] = steps.as_slice() {
            if also.is_empty() && found.0 == revealed.0 && found.0 == moved.0
                && matches!(choices.as_slice(), [choice] if basic_plains(&choice.filter) && matches!(choice.count, Count::UpTo(Expr::Constant(1))) && choice.constraints.is_empty())
            {
                out.push_str("cantrip_mechanics::library::find_to_hand(g, asker.controller, asker.source, cantrip_mechanics::select::BASIC_PLAINS_CARD, true)?;\n");
                return Ok(true);
            }
        }
        let [
            Instruction::Effect { operation: Operation::LookAt { player: PlayerRef::AbilityController, objects: looked }, .. },
            Instruction::Effect { result: paid, operation: Operation::PayCost { player: PlayerRef::AbilityController, cost, optional: true } },
            Instruction::If { condition: Condition::Not(condition), then, otherwise },
        ] = steps.as_slice() else { return Ok(false) };
        if !matches!(&**condition, Condition::PaymentChosen(p) if p.0 == paid.0) || !otherwise.instructions.is_empty()
            || !matches!(looked, ObjectSet::Take { count: Expr::Constant(1), from: SequenceEnd::Top, objects } if matches!(&**objects, ObjectSet::Zone { scope: ZoneScope::Players { zone: EventZone::Library, players: PlayerSet::One(PlayerRef::AbilityController) }, filter: ObjectFilter::Any, .. }))
        { return Ok(false); }
        let [CostPart::PlayerAction { player: PlayerRef::AbilityController, body: action }] = cost.parts.as_slice() else { return Ok(false) };
        let [Instruction::Effect { operation: Operation::MoveTopCard { actor: PlayerSet::One(PlayerRef::AbilityController), cards: ObjectFilter::CardType(t), reveal: false, destination: Destination::OwnersBattlefield, entry }, .. }] = ungroup(action).instructions.as_slice() else { return Ok(false) };
        if name(t) != "Land" { return Ok(false); }
        let [Instruction::Effect { operation: Operation::Move { objects: fallback, destination: Destination::OwnersHand, face_down: false, .. }, .. }] = ungroup(then).instructions.as_slice() else { return Ok(false) };
        if format!("{fallback:?}") != format!("{looked:?}") { return Ok(false); }
        let Some((entry, _)) = lane_i::entry_of(entry, &Destination::OwnersBattlefield) else { return Ok(false) };
        if !entry.states.is_empty() || !entry.counters.is_empty() { return Ok(false); }
        let objects = "cantrip_mechanics::top_of_library(g, asker.controller, 1)";
        let tapped = entry.tapped;
        writeln!(out, "let looked = {objects}; g.look_at(asker.controller, &looked); g.end_sentence(); for original in looked {{ let mut entered = false; if g.chars(original).is(CardType::Land) && cantrip_mechanics::effects::ask_may(g, asker.controller, asker.source, Purpose::Optional)? {{ let arrived = g.put_onto_battlefield(original, g.obj(original).owner, {tapped})?; entered = g.is_on_battlefield(arrived); }} g.end_sentence(); if !entered {{ let card = g.found_after_move(original).unwrap_or(original); g.return_to_hand(&[card]); }} }}").unwrap();
        Ok(true)
    }
}
