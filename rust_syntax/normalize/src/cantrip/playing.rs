//! Casting and playing cards while something resolves (CR 608.2g).
use super::read::compare;
use super::*;

/// Whether the words ask this of the spell as it would be cast. A split card's
/// halves and an Adventurer's Adventure are judged one at a time (709.3a,
/// 715.3a), so these are read from each way to cast the card, not from the card.
fn of_the_spell(filter: &ObjectFilter) -> bool {
    match filter {
        ObjectFilter::Any | ObjectFilter::CardType(_) => true,
        ObjectFilter::Property {
            property: Property::ManaValue,
            ..
        } => true,
        ObjectFilter::All(parts) | ObjectFilter::Either(parts) => parts.iter().all(of_the_spell),
        ObjectFilter::Not(inner) => of_the_spell(inner),
        _ => false,
    }
}

/// `filter` as what it asks of the spell and what it asks of the card.
fn split(filter: &ObjectFilter) -> (Vec<ObjectFilter>, Vec<ObjectFilter>) {
    match filter {
        ObjectFilter::Any => (Vec::new(), Vec::new()),
        ObjectFilter::All(parts) => {
            let (spell, card) = parts.iter().map(split).unzip::<_, _, Vec<_>, Vec<_>>();
            (spell.concat(), card.concat())
        }
        other if of_the_spell(other) => (vec![other.clone()], Vec::new()),
        other => (Vec::new(), vec![other.clone()]),
    }
}

fn all_of(mut parts: Vec<ObjectFilter>) -> ObjectFilter {
    match parts.len() {
        0 => ObjectFilter::Any,
        1 => parts.remove(0),
        _ => ObjectFilter::All(parts),
    }
}

/// `set` without what its filters ask of the spell, and those filters.
fn peel(set: &ObjectSet) -> (ObjectSet, Vec<ObjectFilter>) {
    match set {
        ObjectSet::Matching { objects, filter } => {
            let (inner, mut spell) = peel(objects);
            let (more, card) = split(filter);
            spell.extend(more);
            if card.is_empty() {
                (inner, spell)
            } else {
                let objects = Box::new(inner);
                let filter = all_of(card);
                (ObjectSet::Matching { objects, filter }, spell)
            }
        }
        ObjectSet::Zone {
            scope,
            kind,
            filter,
        } => {
            let (spell, card) = split(filter);
            let zone = ObjectSet::Zone {
                scope: scope.clone(),
                kind: kind.clone(),
                filter: all_of(card),
            };
            (zone, spell)
        }
        other => (other.clone(), Vec::new()),
    }
}

/// Whether `decider`'s "you may" is only that they cast or play these cards, so
/// choosing one, or declining, is the whole of the question.
pub(super) fn may_only_play(decider: &PlayerRef, body: &Program) -> bool {
    match body.instructions.as_slice() {
        [Instruction::Group(group)] => may_only_play(decider, group),
        [Instruction::Effect { operation, .. }] => {
            let plain = lane_i::plain_play(operation);
            match plain.as_ref().unwrap_or(operation) {
                Operation::PlayCards {
                    actor: PlayerSet::One(actor),
                    ..
                } => format!("{actor:?}") == format!("{decider:?}"),
                _ => false,
            }
        }
        _ => false,
    }
}

impl Compiler {
    /// The test of `w`, a way to cast a card. `numbers` holds what it compares
    /// to, each read once, before anything is cast.
    fn spell_test(
        &self,
        filter: &ObjectFilter,
        id: usize,
        numbers: &mut Vec<String>,
    ) -> Result<String> {
        let joined = |this: &Self, parts: &[ObjectFilter], numbers: &mut Vec<String>, by: &str| {
            let tests = parts
                .iter()
                .map(|part| this.spell_test(part, id, numbers))
                .collect::<Result<Vec<_>>>()?;
            Ok::<String, Diagnostic>(format!("({})", tests.join(by)))
        };
        Ok(match filter {
            ObjectFilter::Any => "true".to_string(),
            ObjectFilter::CardType(t) => {
                format!("g.card(w.face).types.has(CardType::{})", card_type(t)?)
            }
            ObjectFilter::All(parts) => joined(self, parts, numbers, " && ")?,
            ObjectFilter::Either(parts) => joined(self, parts, numbers, " || ")?,
            ObjectFilter::Not(inner) => format!("!{}", self.spell_test(inner, id, numbers)?),
            ObjectFilter::Property {
                comparison, value, ..
            } => {
                numbers.push(self.fetch(self.number(value)?.code));
                let limit = format!("limit_{id}_{}", numbers.len() - 1);
                compare("i64::from(w.mana_value)", comparison, &limit)
            }
            other => return Err(gap(format!("spell cast by its {}", form(other)))),
        })
    }
    /// "You may cast [cards]": offered one at a time while any can be, each cast
    /// now whatever its timing (608.2g). The mana is not asked first, as a
    /// player who finds they can't pay gives the cast up (733.1), and that card
    /// isn't offered again.
    pub(super) fn play_cards(
        &mut self,
        operation: &Operation,
        optional: bool,
        out: &mut String,
    ) -> Result<()> {
        let plain = lane_i::plain_play(operation);
        let Operation::PlayCards {
            actor,
            action,
            cards,
            count,
            cost,
            into_exile,
        } = plain.as_ref().unwrap_or(operation)
        else {
            return Err(gap("play cards"));
        };
        let PlayerSet::One(actor) = actor else {
            return Err(gap(format!("cards played by {}", form(actor))));
        };
        let instead = match cost {
            PlayCost::Free => "Some(ManaCost::ZERO)",
            PlayCost::Normal => "None",
            other => return Err(gap(format!("play cost {}", form(other)))),
        };
        let (most, may) = match count {
            Count::Exactly(Expr::Constant(n)) if *n > 0 => (*n, optional),
            Count::UpTo(Expr::Constant(n)) if *n > 0 => (*n, true),
            Count::AnyNumber => (i64::from(u32::MAX), true),
            other => return Err(gap(format!("play count {}", form(other)))),
        };
        let lands = !matches!(action, CardPlayAction::CastSpell);
        // The engine plays a land for the resolving effect's controller only.
        if lands && !matches!(actor, PlayerRef::AbilityController) {
            return Err(gap("a land played by another player"));
        }
        let (set, spell) = peel(cards);
        if lands && !spell.is_empty() {
            return Err(gap("a land played by what it is as a spell"));
        }
        let id = self.fresh();
        let mut numbers = Vec::new();
        let test = self.spell_test(&all_of(spell), id, &mut numbers)?;
        let caster = self.fetch(self.certain_player(actor)?);
        let pool = self.acted_on(&set)?;
        let exile = if *into_exile { "Spell" } else { "Never" };
        let purpose = if matches!(cost, PlayCost::Free) {
            "CastFree"
        } else {
            "Optional"
        };
        let land_open = if lands {
            " || g.could_play_land_during_resolution(c)"
        } else {
            ""
        };
        writeln!(out, "let caster_{id} = {caster}; let mut pool_{id}: Vec<ObjectId> = {pool}; let mut left_{id}: u32 = {most};").unwrap();
        for (i, number) in numbers.iter().enumerate() {
            writeln!(out, "let limit_{id}_{i}: i64 = i64::from({number});").unwrap();
        }
        writeln!(out, "while left_{id} > 0 {{").unwrap();
        writeln!(out, "let mut open_{id}: Vec<ObjectId> = Vec::new(); for &c in &pool_{id} {{ let ways = g.cast_ways_during_resolution_by(c, caster_{id}); if ways.iter().any(|w| {test}){land_open} {{ open_{id}.push(c); }} }}").unwrap();
        writeln!(out, "let picked_{id} = if open_{id}.is_empty() {{ None }} else if open_{id}.len() > 1 {{ cantrip_mechanics::choose::cards_from(g, caster_{id}, asker.source, Purpose::{purpose}, pool_{id}.clone(), open_{id}.clone(), 1, {may})?.first().copied() }} else if !{may} || cantrip_mechanics::effects::ask_may(g, caster_{id}, open_{id}[0], Purpose::{purpose})? {{ Some(open_{id}[0]) }} else {{ None }};").unwrap();
        writeln!(out, "let Some(card_{id}) = picked_{id} else {{ break; }}; pool_{id}.retain(|&c| c != card_{id});").unwrap();
        writeln!(out, "let ways_{id}: Vec<CastWay> = g.cast_ways_during_resolution_by(card_{id}, caster_{id}).into_iter().filter(|w| {test}).collect();").unwrap();
        writeln!(out, "let done_{id} = if ways_{id}.is_empty() {{ g.play_land_during_resolution(card_{id})?.is_some() }} else {{ let way = cantrip_mechanics::choose::cast_way(g, caster_{id}, card_{id}, ways_{id})?.map(|w| w.way); g.cast_during_resolution_as(card_{id}, CastTerms {{ instead: {instead}, way, caster: Some(caster_{id}), exile_instead: ExileInstead::{exile}, ..CastTerms::default() }})? }};").unwrap();
        writeln!(out, "if done_{id} {{ left_{id} -= 1; }}").unwrap();
        writeln!(out, "}}").unwrap();
        Ok(())
    }
}
