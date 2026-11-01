//! Keywords the engine has by another road, and the final form of those it does
//! not: each planned entry here is the call the card makes once Cantrip has it.
use super::*;

const M: &str = "cantrip_mechanics";
/// The change an object keeps through every zone for the rest of the game.
const PERPETUAL: &str = "cantrip_mechanics::perpetual::Perpetual";
/// The label of a cost paid as something resolves, where any number can be read.
const PAID: &str = "paid";

/// What an optional additional cost sets on the card.
enum Optional {
    Cost(String),
    Kicker(String),
}

/// A cost worked out from the game each time it is asked.
fn reading_cost(code: &str) -> String {
    planned::planned_value(
        "engine: no cost sized by the game",
        "Cost::NONE",
        &format!("Cost::reading(|q, asker, targets| {code})"),
    )
}

/// Forecast (CR 702.57): the ability is whole, and the keyword is the requirement.
pub(super) fn forecast(def: String) -> String {
    planned::planned_value("engine: no Forecast", &def, &format!("{def}.forecast()"))
}

/// Keyword actions the shared gate takes an amount for.
pub(super) fn takes_amount(action: KeywordAction) -> bool {
    use KeywordAction as K;
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
        ObjectSet::Battlefield(filter) => ("CostZone::Battlefield".to_string(), filter.clone()),
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
            ObjectSet::Battlefield(_) => selection.as_ref().is_none_or(|s| {
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
        ObjectFilter::All(items) => items.iter().flat_map(each_filter).collect(),
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
            ObjectFilter::Either(v) => {
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
    if let ObjectFilter::Either(v) = filter {
        let subtypes = v.iter().map(subtype).collect::<Option<Vec<_>>>()?;
        return Some(format!(
            "ManaRestriction::Subtypes(SubtypeSpells::of_any(&[{}]))",
            subtypes.join(", ")
        ));
    }
    if let ObjectFilter::All(v) = filter {
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

impl Compiler {
    /// An event Cantrip does not show to abilities. `in_history` says its record
    /// is already there, so a question of what happened can ask it now.
    pub(super) fn plan_event(
        &self,
        matched: &mut triggers::Matched,
        feature: &str,
        in_history: bool,
    ) -> Result<()> {
        if self.history.is_some() {
            return match in_history {
                true => Ok(()),
                false => Err(gap(feature)),
            };
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
        &self,
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
        Ok(reading_cost(&build(&self.asked_number(n)?)))
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
        let payer = self.fetch(self.certain_player(player)?);
        let ask = |source: &str, subject: &str, cost: &str| {
            format!("g.may_pay(payer_{id}, {source}, {subject}, {cost}, &PAY_QUESTION)?")
        };
        let paid = match cost.parts.as_slice() {
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
        Ok(format!("let payer_{id} = {payer}; let paid_{id} = {paid};"))
    }
    /// "Add one mana of any type that land produced": the trigger's data names
    /// the player and the types, and with none produced nothing is added (106.12a).
    pub(super) fn same_type_as_produced(&mut self, out: &mut String) {
        writeln!(out, "if item.data[1] != 0 {{ {M}::mana::add_one_mana_of_any_type_that_land_produced(g, item)?; }}").unwrap();
    }
    /// A `ColorSet` of the colors among objects, read by a plain function.
    fn colors_of(&self, objects: &ObjectSet) -> Result<String> {
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
            writeln!(out, "if !colors_{id}.is_empty() {{ let colors = {M}::choose_colors(g, player, asker.source, colors_{id}, 1)?; g.add_mana(player, colors[0].mana(), 1); }}").unwrap();
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
            return Err(gap(format!("mana production {}", form(mana))));
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
        let mut fixed = Vec::new();
        let Err(hole) = granted_protection(quality, &mut fixed) else {
            grants.extend(fixed);
            return Ok(());
        };
        let code = match quality {
            Protection::Objects { filter, .. } => match self.filter(filter, &Candidate::of("id")) {
                Ok(test) => format!("ProtectionQuality::Matching(|q, asker, id| {test})"),
                Err(_) => return Err(hole),
            },
            Protection::Players(players) => match self.players(players) {
                Ok(list) => format!("ProtectionQuality::Players(|q, asker| {list})"),
                Err(_) => return Err(hole),
            },
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
            // "Pay {E}{E}. If you can't, ...": whoever can pay does (Greenbelt Rampager).
            CostPart::Energy(n) => {
                let owed = self.fetch(self.number(n)?.amount(255, "u8")?);
                let payer = self.fetch(self.certain_player(player)?);
                writeln!(out, "let owed_{id}: u8 = {owed}; let payer_{id} = {payer}; let paid_{id} = g.state().player(payer_{id}).energy() >= owed_{id}; if paid_{id} {{ g.change_energy(payer_{id}, -i32::from(owed_{id})); }}").unwrap();
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
                let tap = Operation::Tap {
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
            other => {
                let feature = format!("resolution cost {}", form(other));
                return self.stated_payment(&feature, id, player, cost, out);
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
                    format!("CostCount::Reading(|q, asker| {})", self.asked_number(n)?)
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
                ObjectSet::Battlefield(filter),
                CounterAmount::AmongThem(Count::Exactly(Expr::Constant(n))),
            ) if costs::yours(filter) && (0..=255).contains(n) => {
                let filter = self.cost_filter(filter, label)?;
                match &kind {
                    Some(kind) => format!("remove_kind_counters_among({kind}, {n}, {filter})"),
                    None => format!("remove_counters_among({n}, {filter})"),
                }
            }
            (ObjectSet::Battlefield(filter), CounterAmount::AmongThem(count)) => {
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
            (ObjectSet::One(ObjectRef::Granter), CounterAmount::All) => plan(format!(
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
            (ObjectSet::Battlefield(filter), CounterAmount::Count(count)) => {
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
                objects: ObjectSet::One(ObjectRef::Granter),
                choose: None,
            } if !resolving => field("sacrifice_granter: true"),
            CostPart::Sacrifice {
                objects: ObjectSet::One(ObjectRef::AttachedTo(host)),
                choose: None,
            } if !resolving && matches!(**host, ObjectRef::Source) => plan(
                "engine: no cost sacrifices what this is attached to",
                "sacrifice_host: true",
            ),
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
            CostPart::Exert(ObjectRef::Source) if !resolving => "EXERT".into(),
            CostPart::Unattach(ObjectRef::Source) => "UNATTACH_THIS".into(),
            CostPart::Unattach(ObjectRef::Granter) => plan(
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
                    objects: ObjectSet::Battlefield(filter),
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
                let owed = self.fetch(self.number(x)?.amount(255, "u8")?);
                format!(
                    "{{ let base = {}; let owed: u8 = {owed}; Cost::mana(ManaCost {{ generic: base.generic.saturating_add(owed.saturating_mul(base.x)), x: 0, ..base }}) }}",
                    mana(symbols)?
                )
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
                        reading_cost(&format!("({once}).times(({n}).clamp(0, 255) as u8)"))
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
                        objects: ObjectSet::Battlefield(filter),
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
    pub(super) fn activators(&self, def: String, who: Option<&PlayerSet>) -> Result<String> {
        let Some(who) = who else {
            return Ok(def);
        };
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        let test = asked.player_in(who, "player")?;
        Ok(planned::planned_value(
            "engine: no activation by other players",
            &def,
            &format!("{def}.activated_by(|q, asker, player| {test})"),
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
            ActivationRule::OnlyBy(players) => self.activators(def, Some(players))?,
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
            ActivationRule::ManaInstead(cost) => {
                let cost = self.cost(cost, label)?;
                planned::planned_value(
                    "engine: no cost paid rather than each mana",
                    &def,
                    &format!("{def}.instead_of_each_mana({cost})"),
                )
            }
            ActivationRule::MaxPerTurn(n) => {
                let most = self.asked_number(n)?;
                planned::planned_value(
                    "engine: no limit on activations each turn",
                    &def,
                    &format!("{def}.most_each_turn(|q, asker| {most})"),
                )
            }
            ActivationRule::XIs(n) | ActivationRule::XAtMost(n) => {
                let exact = matches!(rule, ActivationRule::XIs(_));
                let bound = self.asked_number(n)?;
                let method = if exact { "x_is" } else { "x_at_most" };
                planned::planned_value(
                    "engine: no rule on an ability's X",
                    &def,
                    &format!("{def}.{method}(|q, asker| {bound})"),
                )
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
                let code = self.optional_cost(cost, label)?;
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
                self.plan_entry(
                    "engine: no flash for an additional cost",
                    "flash_for",
                    &format!("Some({cost})"),
                    parts,
                );
            }
            (CastingRule::ReduceForEachPaid { reductions, cost }, None) => {
                self.reduce_for_each_paid(reductions, cost, label, parts)?;
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
                        format!("ManaSpendRule::ProducedBy(|q, asker, source| {test})")
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
            (other, _) => return Err(gap(format!("casting rule {}", form(other)))),
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
            objects: ObjectSet::Battlefield(filter),
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
                objects: ObjectSet::Battlefield(filter),
                choose: Some(Count::Exactly(Expr::Constant(1)) | Count::AnyNumber),
            }] => {
                let filter = self.cost_filter(filter, label)?;
                Some(format!("Cost {{ variable: Some(VariableCost::SacrificeAnyNumber {{ filter: {filter}, less_each: {less} }}), ..Cost::NONE }}"))
            }
            [CostPart::Move {
                selections,
                destination: Destination::Exile,
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
                        &paid_modes.minimum,
                        &paid_modes.maximum,
                        format!("{:?}", paid_modes.modes) == format!("{:?}", unpaid_modes.modes),
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
            Protection::Each(qualities) => {
                for quality in qualities {
                    match quality {
                        Protection::Objects {
                            kind: ObjectKind::Any,
                            filter: ObjectFilter::Color(c),
                        } => {
                            color(c)?;
                            parts.protection |= 1 << color_index(c)?;
                        }
                        other => self.protection_beyond(other, parts)?,
                    }
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
                    ObjectFilter::Property {
                        property: Property::ManaValue,
                        comparison: Comparison::GreaterOrEqual,
                        value: Expr::Constant(n),
                    },
            } => {
                let statics = format!(
                    "{M}::protection::from_mana_value_at_least::<{n}>({})",
                    text(&format!("mana value {n} or greater"))
                );
                parts.statics.push(statics);
            }
            Protection::Objects { kind, filter } => {
                let test = self.spending_test(filter, "source")?;
                let code = format!(
                    "{M}::protection::from_matching(ObjectKind::{kind:?}, |q, asker, source| {test}, {})",
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
        let mut kinds = Vec::new();
        for change in changes {
            for kind in self.perpetual_kinds(change)? {
                let local = format!("kind_{id}_{}", kinds.len());
                writeln!(code, "let {local} = {};", self.fetch(kind)).unwrap();
                kinds.push(local);
            }
        }
        let applies: String = kinds
            .iter()
            .map(|k| format!("{M}::perpetual::apply(g, asker.source, object, {k}); "))
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
                let way = format!("{M}::beam_me_up::way({}, \"Beam me up\")", mana(symbols)?);
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
                let test = self.cost_filter(filter, &format!("{label}_bands"))?;
                entries.push(("statics", format!("{M}::banding::with_others({test})")));
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
            K::ReadAhead => {
                entries.push(("enters", format!("{M}::read_ahead::enters")));
            }
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
