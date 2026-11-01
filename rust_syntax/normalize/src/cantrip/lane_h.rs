//! Rules of the game an object's static ability changes, past the commonest
//! ones in `statics.rs`: each is one of `CardDef`'s tables, or the table Cantrip
//! will have, and the callback a table asks is written out either way.
use super::*;

const PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";
/// The shape of most object tables: `id` is the permanent the rule is about.
const ASKED: &str = "(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool";

/// Whether a filter reads only what a token's characteristics say.
fn chars_only(filter: &ObjectFilter) -> bool {
    use ObjectFilter as F;
    match filter {
        F::Any | F::CardType(_) | F::Subtype(_) | F::Supertype(_) | F::Color(_) => true,
        F::All(items) | F::Either(items) => items.iter().all(chars_only),
        F::Not(inner) => chars_only(inner),
        _ => false,
    }
}
/// "You gain that much plus 1 instead" is that gain made larger, not a second
/// gain that would meet this same effect again (CR 614.5).
pub(super) fn life_amount(
    kind: &str,
    changes: Vec<EventChange>,
    rest: Vec<Instruction>,
) -> (Vec<EventChange>, Vec<Instruction>) {
    if kind == "LifeChange" && matches!(changes.as_slice(), [EventChange::Cancel]) {
        if let [Instruction::Effect {
            operation:
                Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount,
                },
            ..
        }] = rest.as_slice()
        {
            return (vec![EventChange::SetAmount(amount.clone())], Vec::new());
        }
    }
    (changes, rest)
}
/// Whether a filter names what only the resolving ability knows: a lasting
/// rule's callback is told the object it is on and nothing else.
fn names_the_effect(filter: &ObjectFilter) -> bool {
    let text = form(filter);
    ["Source", "AbilityController", "Target", "Event", "Affected", "Chosen", "Result"]
        .iter()
        .any(|word| text.contains(word))
}
/// The words after "for as long as", for the client (`Lasting::text`).
fn lasting_text(duration: &crate::ir::Duration) -> String {
    use crate::ir::Duration as D;
    match duration {
        D::WhileControlled { .. } => "you control {source}".to_string(),
        D::WhileOnBattlefield(_) => "{source} remains on the battlefield".to_string(),
        D::WhileMatches {
            object: ObjectRef::Source,
            filter: ObjectFilter::Tapped,
        } => "{source} remains tapped".to_string(),
        D::WhileMatches {
            filter: ObjectFilter::Tapped,
            ..
        } => "it remains tapped".to_string(),
        D::Either(all) => all
            .iter()
            .map(lasting_text)
            .collect::<Vec<_>>()
            .join(" and "),
        _ => "its condition holds".to_string(),
    }
}
/// "That permanent ... for as long as it ...": the same object as what the effect
/// affects, which is what the effect can read.
pub(super) fn duration_on(
    affected: &ObjectSet,
    duration: &crate::ir::Duration,
) -> crate::ir::Duration {
    use crate::ir::Duration as D;
    let same = |object: &ObjectRef| match affected {
        ObjectSet::One(one) => form(one) == form(object),
        ObjectSet::Targets(slot) => {
            matches!(object, ObjectRef::Target { slot: at, .. } if form(at) == form(slot))
        }
        _ => false,
    };
    match duration {
        D::WhileMatches { object, filter } if same(object) => D::WhileMatches {
            object: ObjectRef::Affected,
            filter: filter.clone(),
        },
        D::Either(all) => D::Either(all.iter().map(|d| duration_on(affected, d)).collect()),
        other => other.clone(),
    }
}
/// The first word of a form, for a name that says which one it is.
fn quoted_word(form: &str) -> String {
    form.chars().take_while(|c| c.is_alphanumeric()).collect()
}
/// The mana a creature pays to attack, as a `ManaCost` a callback returns.
fn generic_mana(n: u32, times: Option<&str>) -> String {
    match times {
        Some(times) => format!(
            "ManaCost {{ generic: u8::try_from(({times}) as i64 * {n}).unwrap_or(u8::MAX), ..ManaCost::ZERO }}"
        ),
        None => format!("ManaCost {{ generic: {n}, ..ManaCost::ZERO }}"),
    }
}

/// A `ManaRestriction` the wire has a word for, beyond `mana_restriction`'s: each
/// must be one of `ManaRestriction::NAMED`, a `Subtypes` or a `ManaValue`.
fn known_mana_use(value: &ManaUse) -> Option<String> {
    use crate::mtg_types::CardType as T;
    use ObjectFilter as F;
    let subtype = |s: &crate::mtg_types::SubType| -> Option<String> {
        let word = name(s);
        crate::cantrip_vocab::SUBTYPES
            .contains(&word.as_str())
            .then(|| format!("Subtype::{word}"))
    };
    // "Subtype spells" or "subtype or subtype spells".
    let subtypes = |f: &F| -> Option<String> {
        match f {
            F::Subtype(s) => Some(format!("SubtypeSpells::of({})", subtype(s)?)),
            F::Either(items) => {
                let words = items
                    .iter()
                    .map(|f| match f {
                        F::Subtype(s) => subtype(s),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(format!("SubtypeSpells::of_any(&[{}])", words.join(", ")))
            }
            F::All(items) => match items.as_slice() {
                [F::Subtype(s), F::CardType(T::Creature)]
                | [F::CardType(T::Creature), F::Subtype(s)] => Some(format!(
                    "SubtypeSpells {{ creature: true, ..SubtypeSpells::of({}) }}",
                    subtype(s)?
                )),
                _ => None,
            },
            _ => None,
        }
    };
    let chosen_creature = |f: &F| {
        matches!(f, F::All(items) if matches!(items.as_slice(),
            [F::CardType(T::Creature), F::Chosen(ValueKind::CreatureType)]))
    };
    let at_least = |f: &F| match f {
        F::Property {
            property: Property::ManaValue,
            comparison: Comparison::GreaterOrEqual,
            value: Expr::Constant(n),
        } => u8::try_from(*n).ok(),
        _ => None,
    };
    let uncounterable = |effects: &[Operand]| {
        matches!(effects, [Operand::Term { family, form, .. }]
            if family == "SpellEffect" && form == "CantBeCountered")
    };
    let hand = |f: &F| {
        matches!(f, F::Cast { by: None, from: Some(ZoneScope::Players { zone: EventZone::Hand, .. }) })
    };
    Some(match value {
        ManaUse::CastSpell(F::Chosen(ValueKind::CreatureType)) => {
            "ManaRestriction::Subtypes(SubtypeSpells { creature: false, ..SubtypeSpells::CHOSEN_TYPE_CREATURE })".to_string()
        }
        ManaUse::CastSpell(f) if chosen_creature(f) => {
            "ManaRestriction::Subtypes(SubtypeSpells::CHOSEN_TYPE_CREATURE)".to_string()
        }
        ManaUse::CastSpell(F::Cast {
            by: None,
            from: Some(ZoneScope::Shared(EventZone::Exile)),
        }) => "ManaRestriction::SPELLS_FROM_EXILE".to_string(),
        ManaUse::Not(inner)
            if matches!(&**inner, ManaUse::CastSpell(f) if hand(f)) =>
        {
            "ManaRestriction::NOT_SPELLS_FROM_HAND".to_string()
        }
        ManaUse::CastSpell(F::Either(items)) if at_least(items.first()?).is_some() => {
            let [mana_value, F::State(ObjectState::HasX)] = items.as_slice() else {
                return None;
            };
            format!(
                "ManaRestriction::mana_value_at_least_or_x({})",
                at_least(mana_value)?
            )
        }
        ManaUse::CastSpell(f) if at_least(f).is_some() => {
            format!("ManaRestriction::mana_value_at_least({})", at_least(f)?)
        }
        ManaUse::CastSpell(f) => format!("ManaRestriction::Subtypes({})", subtypes(f)?),
        ManaUse::SpentOn {
            spells: F::CardType(T::Creature),
            changes,
            until: Duration::EndOfTurn,
        } if matches!(changes.as_slice(), [Change::GrantKeyword(Keyword::Haste)]) => {
            "ManaRestriction::GainsHaste".to_string()
        }
        ManaUse::All(uses) => match uses.as_slice() {
            [ManaUse::CastSpell(F::Supertype(crate::mtg_types::SuperType::Legendary)), ManaUse::SpentGives {
                spells: F::Any,
                effects,
            }] if uncounterable(effects) => "ManaRestriction::LegendaryUncounterable".to_string(),
            [ManaUse::CastSpell(f), ManaUse::SpentGives {
                spells: F::Any,
                effects,
            }] if chosen_creature(f) && uncounterable(effects) => {
                "ManaRestriction::Subtypes(SubtypeSpells::CHOSEN_TYPE_CREATURE.uncounterable())"
                    .to_string()
            }
            _ => return None,
        },
        ManaUse::Either(uses) => match uses.as_slice() {
            [ManaUse::CastSpell(F::CardType(T::Artifact)), ManaUse::ActivateAbility {
                sources: F::CardType(T::Artifact),
                permanents_only: false,
            }] => "ManaRestriction::ARTIFACT_SPELLS_OR_ABILITIES".to_string(),
            [ManaUse::CastSpell(F::CardType(T::Artifact)), ManaUse::ActivateAbility {
                sources: F::Any,
                permanents_only: false,
            }]
            | [ManaUse::ActivateAbility {
                sources: F::Any,
                permanents_only: false,
            }, ManaUse::CastSpell(F::CardType(T::Artifact))] => {
                "ManaRestriction::ARTIFACT_SPELLS_OR_ANY_ABILITY".to_string()
            }
            [ManaUse::CastSpell(spell), ManaUse::ActivateAbility {
                sources,
                permanents_only: false,
            }] if chosen_creature(spell) && chosen_creature(sources) => {
                "ManaRestriction::Subtypes(SubtypeSpells::CHOSEN_TYPE_CREATURE_SPELLS_OR_ABILITIES)"
                    .to_string()
            }
            [ManaUse::CastSpell(spell), ManaUse::ActivateAbility {
                sources,
                permanents_only,
            }] => {
                let (spell, source) = (subtypes(spell)?, subtypes(sources)?);
                if spell != source || spell.contains("creature") {
                    return None;
                }
                let reach = if *permanents_only {
                    "or_abilities_of_permanents"
                } else {
                    "or_abilities"
                };
                format!("ManaRestriction::Subtypes({spell}.{reach}())")
            }
            _ => return None,
        },
        _ => return None,
    })
}

/// Who casts the spells a filter of spells on the stack is about, and the rest
/// of it: "spells you cast", as "spells you control" before they are on it.
fn spell_caster(filter: &ObjectFilter) -> (PlayerSet, ObjectFilter) {
    let caster = |f: &ObjectFilter| match f {
        ObjectFilter::Cast {
            by: Some(by),
            from: None,
        }
        | ObjectFilter::ControlledBy(by) => Some(by.clone()),
        _ => None,
    };
    if let Some(by) = caster(filter) {
        return (by, ObjectFilter::Any);
    }
    if let ObjectFilter::All(items) = filter {
        if let Some(by) = items.iter().find_map(caster) {
            let rest: Vec<_> = items
                .iter()
                .filter(|f| caster(f).is_none())
                .cloned()
                .collect();
            return (by, ObjectFilter::All(rest));
        }
    }
    (PlayerSet::All, filter.clone())
}
/// The `PayWith` field of a keyword that pays for a spell.
fn pay_with(ability: &Ability) -> Option<&'static str> {
    match ability {
        Ability::Mechanic {
            mechanic: Mechanic::Convoke,
            ..
        } => Some("convoke"),
        Ability::Mechanic {
            mechanic: Mechanic::Improvise,
            ..
        } => Some("improvise"),
        Ability::Mechanic {
            mechanic: Mechanic::Delve,
            ..
        } => Some("delve"),
        _ => None,
    }
}

impl Compiler {
    /// What mana may be spent on: Cantrip's word for it, or the rule it will
    /// read, as a callback on the payment.
    pub(super) fn mana_use(&mut self, value: &ManaUse) -> Result<String> {
        if let Some(code) = known_mana_use(value) {
            return Ok(code);
        }
        let failed = match mana_restriction(value) {
            Ok(code) => return Ok(code),
            Err(failed) => failed,
        };
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let test = self.spending(value);
        (self.reader, self.event_at) = saved;
        let (feature, test, effect) = match test {
            Ok(found) => found,
            Err(gap) if gap.feature.starts_with("mana spending") => return Err(failed),
            Err(gap) => return Err(gap),
        };
        // What a payment is for, past `SpendingFor`, has no word to compile yet.
        let function = if test.contains("ManaPart") {
            format!("|q, asker, spending| {test}")
        } else {
            let function = format!("mana_use_{}", self.functions.len());
            writeln!(self.functions, "#[allow(dead_code, unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, spending: &Spending) -> bool {{ {test} }}").unwrap();
            function
        };
        Ok(planned::planned_value(
            feature,
            "ManaRestriction::ABILITIES",
            &format!("ManaRestriction::Rule(&ManaRule {{ spend: {function}, spent: {effect} }})"),
        ))
    }
    /// A `ManaUse` as a test of the `Spending` in hand: the feature it waits on,
    /// the test, and what the mana does to what it pays for.
    fn spending(&self, value: &ManaUse) -> Result<(&'static str, String, String)> {
        const PREDICATE: &str = "mana spending predicate";
        const EFFECT: &str = "mana spending effect";
        let spell = |c: &Self, f: &ObjectFilter| -> Result<String> {
            Ok(format!(
                "spending.purpose == SpendingFor::Casting && spending.source.is_some_and(|spell| {})",
                c.filter(f, &Candidate::of("spell"))?
            ))
        };
        let many = |items: &[ManaUse], join: &str| -> Result<(&'static str, String, String)> {
            let mut feature = PREDICATE;
            let (mut tests, mut effect) = (Vec::new(), "None".to_string());
            for item in items {
                let (found, test, spent) = self.spending(item)?;
                if found != PREDICATE {
                    feature = found;
                }
                if spent != "None" {
                    effect = spent;
                }
                tests.push(format!("({test})"));
            }
            Ok((feature, tests.join(join), effect))
        };
        Ok(match value {
            ManaUse::CastSpell(f) => (PREDICATE, spell(self, f)?, "None".into()),
            ManaUse::ActivateAbility {
                sources,
                permanents_only,
            } => {
                let of = self.filter(sources, &Candidate::of("source"))?;
                let there = if *permanents_only {
                    "q.state().obj(source).zone.is(ZoneKind::Battlefield) && "
                } else {
                    ""
                };
                (
                    PREDICATE,
                    format!("spending.purpose == SpendingFor::Activating && spending.source.is_some_and(|source| {there}{of})"),
                    "None".into(),
                )
            }
            ManaUse::All(items) => many(items, " && ")?,
            ManaUse::Either(items) => many(items, " || ")?,
            ManaUse::Not(inner) => {
                let (feature, test, effect) = self.spending(inner)?;
                (feature, format!("!({test})"), effect)
            }
            ManaUse::Purpose(purpose) => (
                "mana spending purpose",
                format!("spending.part == ManaPart::{}", quoted_word(&form(purpose))),
                "None".into(),
            ),
            ManaUse::SpentOn { spells, .. } | ManaUse::SpentGives { spells, .. } => (
                EFFECT,
                "true".into(),
                format!(
                    "Some(ManaSpent {{ on: {}, does: {} }})",
                    quoted(&form(spells)),
                    quoted(&form(value))
                ),
            ),
            ManaUse::WhenSpent { spells, .. } => (
                EFFECT,
                "true".into(),
                format!(
                    "Some(ManaSpent {{ on: {}, does: {} }})",
                    quoted(&form(spells)),
                    quoted(&form(value))
                ),
            ),
            other => return Err(gap(format!("mana spending {}", form(other)))),
        })
    }
    /// A duration past `duration`'s: "for as long as", read from the effect (CR
    /// 611.2b), or the word Cantrip will have for it.
    pub(super) fn more_duration(&mut self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        Ok(match duration {
            D::WhileMatches {
                object: ObjectRef::Affected,
                filter:
                    ObjectFilter::Counters {
                        kind: Some(kind),
                        comparison: Comparison::Greater,
                        value: Expr::Constant(0),
                    },
            } => format!("Duration::WhileCounter({})", counter_kind(kind)?),
            D::WhileMatches {
                object: ObjectRef::Affected,
                filter: ObjectFilter::Tapped,
            } => "Duration::WhileTapped".into(),
            D::WhileMatches { .. } | D::WhileControlled { .. } | D::Either(_) => {
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                let test = self.lasting_test(duration);
                (self.reader, self.event_at) = saved;
                let test = test.map_err(|failed| match failed.feature.as_str() {
                    feature if feature.starts_with("duration") => failed,
                    _ => gap(format!("duration {}", form(duration))),
                })?;
                let index = self.lasting.len();
                let function = format!("lasting_{index}");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, e: &ContinuousEffect) -> bool {{ let asker = &Asker::new(e.source, e.controller); {test} }}").unwrap();
                self.lasting.push(format!(
                    "Lasting {{ text: {}, holds: {function} }}",
                    quoted(&lasting_text(duration))
                ));
                self.fetch(format!(
                    "Duration::AsLongAs({}, {index})",
                    read::SOURCE_CARD
                ))
            }
            // Only a declaration of attackers reads this one, and only a rule
            // on it is ever worded "during its controller's next turn".
            D::DuringNextTurn(PlayerRef::ControllerOf(_)) => {
                "Duration::DURING_ITS_CONTROLLERS_NEXT_TURN".into()
            }
            D::UntilNext {
                point: TurnPoint::UpkeepBegins,
                players: PlayerSet::One(player),
            } => planned::planned_value(
                "duration UntilNext upkeep",
                "Duration::Indefinite",
                &format!(
                    "Duration::UntilNextUpkeepOf({})",
                    self.fetch(self.certain_player(player)?)
                ),
            ),
            D::UntilNext {
                point: TurnPoint::EndStepBegins,
                players: PlayerSet::All,
            } => planned::planned_value(
                "duration UntilNext end step",
                "Duration::Indefinite",
                "Duration::UntilNextEndStep",
            ),
            D::UntilPaid { player, cost } => planned::planned_value(
                "duration UntilPaid",
                "Duration::Indefinite",
                &format!(
                    "Duration::UntilPaidBy({}, {})",
                    self.fetch(self.certain_player(player)?),
                    quoted(&form(cost))
                ),
            ),
            other => return Err(gap(format!("duration {}", form(other)))),
        })
    }
    /// The test of a "for as long as": `e` is the effect it is about.
    fn lasting_test(&self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        Ok(match duration {
            D::WhileControlled {
                player: PlayerRef::AbilityController,
                object: ObjectRef::Source,
            } => "is_live(q, e.source) && q.is_on_battlefield(e.source) && q.controller(e.source) == e.controller".to_string(),
            D::WhileMatches { object, filter } => {
                let id = match object {
                    ObjectRef::Source => "e.source",
                    ObjectRef::Affected => "e.affected",
                    other => return Err(gap(format!("duration WhileMatches {}", form(other)))),
                };
                let live = match object {
                    ObjectRef::Source => "is_live(q, e.source) && q.is_on_battlefield(e.source) && ",
                    _ => "",
                };
                self.lasting_filter(live, filter, id)?
            }
            D::WhileOnBattlefield(ObjectRef::Source) => {
                "is_live(q, e.source) && q.is_on_battlefield(e.source)".to_string()
            }
            D::Either(all) => all
                .iter()
                .map(|d| Ok(format!("({})", self.lasting_test(d)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" && "),
            other => return Err(gap(format!("duration {}", form(other)))),
        })
    }
    fn lasting_filter(&self, live: &str, filter: &ObjectFilter, id: &str) -> Result<String> {
        Ok(format!("{live}{}", self.filter(filter, &Candidate::of(id))?))
    }
    /// The events `Proposal` has beyond those `watched` reads itself.
    pub(super) fn more_watched(
        &mut self,
        event: &ReplacementEvent,
    ) -> Result<Option<(Vec<replacements::Watched>, Frame)>> {
        use replacements::Watched;
        let player_frame = Frame {
            player: Some(Carried::Subject),
            amount: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let object_frame = Frame {
            object: Some(Carried::Subject),
            amount: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let one = |watched: Watched, frame: Frame| Ok(Some((vec![watched], frame)));
        match event {
            ReplacementEvent::Player {
                event:
                    event @ (ReplacedPlayerEvent::Draw
                    | ReplacedPlayerEvent::DrawExceptFirstInDrawStep),
                players,
            } => {
                let mut tests =
                    vec![self.matching(player_frame, |c| c.player_in(players, "d.player"))?];
                if let ReplacedPlayerEvent::DrawExceptFirstInDrawStep = event {
                    tests.push(
                        "!cantrip_mechanics::history::first_draw_in_own_draw_step(q, d.player)"
                            .into(),
                    );
                }
                one(
                    Watched {
                        kind: "Draw",
                        variant: "Draw",
                        binding: "d",
                        locals: "let subject = Target::Player(d.player); let data = [1u32, u32::MAX];".into(),
                        tests,
                    },
                    player_frame,
                )
            }
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::DrawCards,
                players,
            } => one(
                Watched {
                    kind: "DrawInstruction",
                    variant: "DrawInstruction",
                    binding: "d",
                    locals: "let subject = Target::Player(d.player); let data = [d.count, u32::MAX];".into(),
                    tests: vec![self.matching(player_frame, |c| c.player_in(players, "d.player"))?],
                },
                player_frame,
            ),
            ReplacementEvent::Player {
                event: event @ (ReplacedPlayerEvent::GainLife | ReplacedPlayerEvent::LoseLife),
                players,
            } => {
                let sign = match event {
                    ReplacedPlayerEvent::GainLife => "l.amount > 0",
                    _ => "l.amount < 0",
                };
                one(
                    Watched {
                        kind: "LifeChange",
                        variant: "LifeChange",
                        binding: "l",
                        locals: "let amount = l.amount.unsigned_abs(); let subject = Target::Player(l.player); let data = [amount, u32::MAX];".into(),
                        tests: vec![
                            sign.to_string(),
                            self.matching(player_frame, |c| c.player_in(players, "l.player"))?,
                        ],
                    },
                    player_frame,
                )
            }
            ReplacementEvent::Tokens {
                tokens,
                controllers,
                creators,
                ..
            } => {
                let token = Candidate {
                    id: "t.token".into(),
                    chars: "q.token_chars(t.token, t.copy)".into(),
                    controller: "t.controller".into(),
                    announced: None,
                };
                if !chars_only(tokens) {
                    return Err(gap("replacement of token creation of what no token shows"));
                }
                let mut tests = vec![
                    "t.count > 0".to_string(),
                    self.matching(player_frame, |c| c.filter(tokens, &token))?,
                ];
                // `TokenCreation` knows no creator but who gets them, and only
                // what a resolving effect or rule says tells those apart.
                for players in [controllers, creators].into_iter().flatten() {
                    tests.push(
                        self.matching(player_frame, |c| c.player_in(players, "t.controller"))?,
                    );
                }
                one(
                    Watched {
                        kind: "TokenCreation",
                        variant: "TokenCreation",
                        binding: "t",
                        locals: "let subject = Target::Player(t.controller); let data = [t.count, u32::MAX];".into(),
                        tests,
                    },
                    player_frame,
                )
            }
            ReplacementEvent::Counters {
                kind,
                objects,
                players: None,
                by,
                effect_only,
            } => {
                let index = kind
                    .as_ref()
                    .map(|kind| Ok::<_, Diagnostic>(format!("{} as usize", counter_kind(kind)?)))
                    .transpose()?;
                let (some, amount) = match &index {
                    Some(index) => (format!("c.counts[{index}] > 0"), format!("c.counts[{index}].max(0) as u32")),
                    None => (
                        "c.counts.iter().any(|&n| n > 0)".to_string(),
                        "c.counts.iter().map(|&n| n.max(0) as u32).sum::<u32>()".to_string(),
                    ),
                };
                let entering = Candidate {
                    controller: "c.controller".into(),
                    ..Candidate::of("id")
                };
                let mut tests = vec![
                    some,
                    "(c.entering || q.is_on_battlefield(id))".to_string(),
                    self.matching(object_frame, |c| c.filter(objects, &entering))?,
                ];
                if let Some(by) = by {
                    tests.push(self.matching(object_frame, |c| c.player_in(by, "c.putter"))?);
                }
                if *effect_only {
                    tests.push("c.effect".to_string());
                }
                self.replaced_counter = index;
                one(
                    Watched {
                        kind: "Counters",
                        variant: "Counters",
                        binding: "c",
                        locals: format!("let id = c.object; let subject = Target::Object(id); let data = [{amount}, u32::MAX];"),
                        tests,
                    },
                    object_frame,
                )
            }
            ReplacementEvent::ProduceMana { sources, by } => {
                let mut tests =
                    vec![self.matching(object_frame, |c| c.filter(sources, &Candidate::of("id")))?];
                if let Some(by) = by {
                    let by = self.matching(object_frame, |c| c.player(by))?;
                    tests.push(format!("Some(m.player) == {by}"));
                }
                one(
                    Watched {
                        kind: "ManaProduction",
                        variant: "ManaProduction",
                        binding: "m",
                        locals: "let id = m.permanent; let amount = m.total(); let subject = Target::Object(id); let data = [amount, u32::MAX];".into(),
                        tests,
                    },
                    object_frame,
                )
            }
            _ => Ok(None),
        }
    }
    /// The changes those events take, as `reshape` writes the others'.
    pub(super) fn more_reshape(
        &mut self,
        kind: &str,
        change: &EventChange,
        frame: Frame,
    ) -> Result<String> {
        let mana_letter = |color: &ManaColor| -> Result<&'static str> {
            Ok(["W", "U", "B", "R", "G", "C"][color_index(color)?])
        };
        Ok(match (kind, change) {
            ("Draw", EventChange::Cancel) => "d.replaced = true;".into(),
            ("DrawInstruction", EventChange::Cancel) => "d.count = 0;".into(),
            ("DrawInstruction", EventChange::SetAmount(n)) => format!(
                "d.count = {};",
                self.matching(frame, |c| c.number(n))?.amount(u32::MAX.into(), "u32")?
            ),
            ("LifeChange", EventChange::Cancel) => "l.amount = 0;".into(),
            ("LifeChange", EventChange::SetAmount(n)) => format!(
                "l.amount = l.amount.signum() * {};",
                self.matching(frame, |c| c.number(n))?.amount(i32::MAX.into(), "i32")?
            ),
            ("TokenCreation", EventChange::Cancel) => "t.count = 0;".into(),
            ("TokenCreation", EventChange::SetAmount(n)) => format!(
                "t.count = {};",
                self.matching(frame, |c| c.number(n))?.amount(u32::MAX.into(), "u32")?
            ),
            ("TokenCreation", EventChange::ReplaceTokens { copies, tokens }) if tokens.is_empty() => {
                format!("t.count = t.count.saturating_mul({copies});")
            }
            ("TokenCreation", EventChange::ReplaceTokens { .. }) => {
                return Err(gap("replacement of token creation with other tokens"))
            }
            ("Counters", EventChange::SetAmount(n)) => {
                let amount = self
                    .matching(frame, |c| c.number(n))?
                    .amount(i16::MAX.into(), "i16")?;
                match self.replaced_counter.clone() {
                    Some(index) => format!("c.counts[{index}] = {amount};"),
                    None => format!(
                        "for i in 0..c.counts.len() {{ if c.counts[i] > 0 {{ let data = [c.counts[i] as u32, u32::MAX]; c.counts[i] = {amount}; }} }}"
                    ),
                }
            }
            ("Counters", EventChange::AddCounters { kind, amount }) => {
                let n = self
                    .matching(frame, |c| c.number(amount))?
                    .amount(i16::MAX.into(), "i16")?;
                let kind = counter_kind(kind)?;
                format!("c.counts[{kind} as usize] = c.counts[{kind} as usize].saturating_add({n});")
            }
            ("Counters", EventChange::Cancel) => "c.counts = [0; CounterKind::COUNT];".into(),
            // "It produces {B} instead of any other type and amount."
            ("ManaProduction", EventChange::AddMana(ManaProduction::Amount { color, amount })) => {
                let n = self
                    .matching(frame, |c| c.number(amount))?
                    .amount(u32::MAX.into(), "u32")?;
                format!(
                    "m.mana = ManaPool::default(); m.mana.0[Mana::{} as usize] = {n};",
                    mana_letter(color)?
                )
            }
            // "... instead of any other type": a land that makes two still does.
            (
                "ManaProduction",
                EventChange::ReplaceMana(ManaProduction::Amount {
                    color,
                    amount: Expr::Constant(1),
                }),
            ) => format!(
                "let total = m.total(); m.mana = ManaPool::default(); m.mana.0[Mana::{} as usize] = total;",
                mana_letter(color)?
            ),
            ("ManaProduction", EventChange::Repeats(n)) => {
                let times = self
                    .matching(frame, |c| c.number(n))?
                    .amount(u32::MAX.into(), "u32")?;
                format!("for mana in m.mana.0.iter_mut() {{ *mana = mana.saturating_mul({times}); }}")
            }
            (_, other) => return Err(gap(format!("event change {}", form(other)))),
        })
    }
    /// A rule a resolving spell or ability makes, as what `EffectKind` says of it
    /// or will: an effect on the object, so it outlasts its source (CR 611.2c).
    pub(super) fn continuous_rule(&mut self, rule: &RuleChange) -> Result<Vec<String>> {
        use RuleChange as R;
        let planned = |feature: &str, code: &str| {
            vec![planned::planned_value(feature, "EffectKind::CantBlock", code)]
        };
        Ok(match rule {
            R::AttackException {
                ignore: AttackCheck::Defender,
                defenders: AttackDefenders::Any,
            } => vec!["EffectKind::CanAttackDespiteDefender".into()],
            R::Prohibit(ObjectAction::Sacrifice) => vec!["EffectKind::CantBeSacrificed".into()],
            R::Prohibit(ObjectAction::Untap) => vec!["EffectKind::CantBecomeUntapped".into()],
            R::Prohibit(ObjectAction::Activate(ActivationFilter::Any)) => {
                vec!["EffectKind::CantActivateAbilities".into()]
            }
            R::Prohibit(ObjectAction::Activate(ActivationFilter::Not(mana)))
                if matches!(**mana, ActivationFilter::Mana) =>
            {
                planned(
                    "continuous rule Prohibit Activate unless mana",
                    "EffectKind::CantActivateNonManaAbilities",
                )
            }
            R::Prohibit(ObjectAction::Regenerate) => planned(
                "continuous rule Prohibit Regenerate",
                "EffectKind::CantBeRegenerated",
            ),
            R::Prohibit(action @ (ObjectAction::BeBlocked(filter) | ObjectAction::Block(filter)))
                if !names_the_effect(filter) =>
            {
                let (subject, other) = match action {
                    ObjectAction::Block(_) => ("blocker", "attacker"),
                    _ => ("attacker", "blocker"),
                };
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                let test = self.filter(filter, &Candidate::of(other));
                (self.reader, self.event_at) = saved;
                let index = self.made_block_rules.len();
                let function = format!("block_rule_{index}");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ {PRELUDE} !({subject} == source && {}) }}", test?).unwrap();
                self.made_block_rules.push(format!(
                    "LastingBlockRule {{ text: {}, allows: {function} }}",
                    quoted(&form(action))
                ));
                vec![format!(
                    "EffectKind::BlockRestriction {{ card: {}, index: {index} }}",
                    read::SOURCE_CARD
                )]
            }
            // With the attacker gone there is nothing to block, and +0/+0 does nothing.
            R::RequireCombat {
                action: ObjectAction::Block(ObjectFilter::SameObject(attacker)),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            } => vec![format!(
                "match {} {{ Some(attacker) => EffectKind::MustBlockAttacker(attacker), None => EffectKind::Pump(0, 0) }}",
                self.object(attacker)?
            )],
            R::RequireCombat {
                action: ObjectAction::Block(ObjectFilter::Any),
                amount: RequirementAmount::Each,
            } => planned(
                "continuous rule RequireCombat Block each",
                "EffectKind::MustBlockEachAttacker",
            ),
            R::UntapStep {
                behavior: UntapBehavior::Skip,
                players: PlayerSet::One(PlayerRef::ControllerOf(controlled)),
            } if matches!(**controlled, ObjectRef::Affected) => planned(
                "continuous rule UntapStep",
                "EffectKind::DoesntUntap",
            ),
            R::RequireCombat {
                action: ObjectAction::Attack(AttackDefenders::Players(PlayerSet::One(player))),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            } => vec![format!(
                "EffectKind::MustAttackAt(Target::Player({}))",
                self.certain_player(player)?
            )],
            R::RequireCombat {
                action: ObjectAction::BeBlocked(ObjectFilter::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            } => planned(
                "continuous rule RequireCombat BeBlocked",
                "EffectKind::MustBeBlocked",
            ),
            R::RequireCombat {
                action: ObjectAction::BeBlocked(ObjectFilter::Any),
                amount: RequirementAmount::Each,
            } => planned(
                "continuous rule RequireCombat BeBlocked by all",
                "EffectKind::AllAbleMustBlock",
            ),
            R::CombatDamage(CombatDamageRule::NoAssignment) => planned(
                "continuous rule CombatDamage NoAssignment",
                "EffectKind::AssignsNoCombatDamage",
            ),
            R::CombatDamage(CombatDamageRule::UseToughness) => planned(
                "continuous rule CombatDamage UseToughness",
                "EffectKind::AssignsDamageByToughness",
            ),
            R::CombatDamage(CombatDamageRule::AsUnblocked { .. }) => planned(
                "continuous rule CombatDamage AsUnblocked",
                "EffectKind::AssignsAsThoughUnblocked",
            ),
            R::BlockCapacity(BlockCapacity::Additional(n)) => {
                let n = self.number(n)?.amount(u32::MAX.into(), "u32")?;
                planned(
                    "continuous rule BlockCapacity",
                    &format!("EffectKind::AdditionalBlocks({n})"),
                )
            }
            R::BlockCapacity(BlockCapacity::Unlimited) => planned(
                "continuous rule BlockCapacity",
                "EffectKind::AdditionalBlocks(u32::MAX)",
            ),
            R::GoadedBy(PlayerRef::AbilityController) => vec!["EffectKind::Goaded".into()],
            other => return Err(gap(format!("continuous rule {}", rule_name(other)))),
        })
    }
    /// A table of `CardDef` that asks of one object.
    fn object_table(
        &mut self,
        table: &'static str,
        function: &str,
        affected: &ObjectSet,
        parts: &mut Parts,
    ) -> Result<()> {
        self.object_test(function, affected)?;
        parts.table(table, function.to_string());
        Ok(())
    }
    /// The table Cantrip will have for `feature`, asked the same way.
    fn object_plan(
        &mut self,
        feature: &str,
        table: &'static str,
        function: &str,
        affected: &ObjectSet,
        parts: &mut Parts,
    ) -> Result<()> {
        self.object_test(function, affected)?;
        self.plan_entry(feature, table, function, parts);
        Ok(())
    }
    fn object_test(&mut self, function: &str, affected: &ObjectSet) -> Result<()> {
        let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn {function}{ASKED} {{ {guard}{test} }}"
        )
        .unwrap();
        Ok(())
    }
    /// A value of `id` under a rule, for a table Cantrip will have.
    fn object_value_plan(
        &mut self,
        feature: &str,
        table: &'static str,
        function: &str,
        affected: &ObjectSet,
        value: &str,
        kind: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> Option<{kind}> {{ ({guard}{test}).then(|| {value}) }}").unwrap();
        self.plan_entry(feature, table, function, parts);
        Ok(())
    }
    /// Who is being attacked, as `Target` `defender` is asked about.
    fn defended(&self, defenders: &AttackDefenders) -> Result<String> {
        let players = |c: &Self, players: &PlayerSet, walkers: bool| -> Result<String> {
            let set = c.players(players)?;
            Ok(format!(
                "(match defender {{ Target::Player(p) => ({set}).contains(&p), Target::Object(id) => {walkers} && q.chars(id).is(CardType::Planeswalker) && ({set}).contains(&q.controller(id)), _ => false }})"
            ))
        };
        match defenders {
            AttackDefenders::Any => Ok("true".into()),
            AttackDefenders::Players(set) => players(self, set, false),
            AttackDefenders::PlayersAndPlaneswalkers(set) => players(self, set, true),
            AttackDefenders::Objects(filter) => Ok(format!(
                "matches!(defender, Target::Object(id) if {})",
                self.filter(filter, &Candidate::of("id"))?
            )),
        }
    }
    /// The mana of a cost to attack, if mana is all it is: the engine's attack
    /// costs are `ManaCost`s, read per creature by `attacker`.
    fn attack_mana(&self, cost: &Cost) -> Result<Option<String>> {
        Ok(match cost.parts.as_slice() {
            [CostPart::Mana(symbols)] => Some(mana(symbols)?),
            [CostPart::Repeated { part, times }] => match part.as_ref() {
                CostPart::Mana(symbols) => match symbols.as_slice() {
                    [ManaSymbol::Generic(n)] => {
                        let times = self.number(times)?.code;
                        Some(generic_mana(*n, Some(&times)))
                    }
                    _ => None,
                },
                _ => None,
            },
            [CostPart::ManaWithX {
                mana: symbols,
                x: Some(x),
            }] if matches!(symbols.as_slice(), [ManaSymbol::X]) => {
                let times = self.number(x)?.code;
                Some(generic_mana(1, Some(&times)))
            }
            _ => None,
        })
    }
    /// What a spell or ability is that can't target: the filter of
    /// `ObjectAction::BeTargeted`, over the `by` of a `TargetingForbid`.
    fn targeting_source(&self, filter: &ObjectFilter) -> Result<String> {
        let candidate = Candidate {
            id: "by.source".into(),
            chars: "(*by_source)".into(),
            controller: "by.controller".into(),
            announced: None,
        };
        Ok(match filter {
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(items) => format!(
                "({})",
                items
                    .iter()
                    .map(|f| self.targeting_source(f))
                    .collect::<Result<Vec<_>>>()?
                    .join(" && ")
            ),
            ObjectFilter::Either(items) => format!(
                "({})",
                items
                    .iter()
                    .map(|f| self.targeting_source(f))
                    .collect::<Result<Vec<_>>>()?
                    .join(" || ")
            ),
            ObjectFilter::Not(inner) => format!("!({})", self.targeting_source(inner)?),
            ObjectFilter::OnStack(StackTrait::Spell) => "by.spell".into(),
            ObjectFilter::AbilityOf(source) => {
                format!("(!by.spell && {})", self.targeting_source(source)?)
            }
            other => self.filter(other, &candidate)?,
        })
    }
    /// The rule tables `rule_table` leaves to this file: true when one took it.
    pub(super) fn more_rules(
        &mut self,
        function: &str,
        affected: &ObjectSet,
        rule: &RuleChange,
        parts: &mut Parts,
    ) -> Result<bool> {
        use RuleChange as R;
        match rule {
            R::AttackException {
                ignore: AttackCheck::Defender,
                defenders: AttackDefenders::Any,
            } => self.object_table("attacks_despite_defender", function, affected, parts)?,
            R::AttackException {
                ignore: AttackCheck::SummoningSickness,
                defenders,
            } => {
                let at = self.defended(defenders)?;
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "attacker")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> bool {{ {guard}{test} && {at} }}").unwrap();
                self.plan_entry(
                    "static rule AttackException SummoningSickness",
                    "attacks_as_though_haste",
                    function,
                    parts,
                );
            }
            R::CombatDamage(CombatDamageRule::UseToughness) => {
                self.object_table("damage_by_toughness", function, affected, parts)?
            }
            R::CombatDamage(CombatDamageRule::AsUnblocked { .. }) => self.object_plan(
                "static rule CombatDamage AsUnblocked",
                "assigns_as_though_unblocked",
                function,
                affected,
                parts,
            )?,
            R::CombatDamage(CombatDamageRule::NoAssignment) => self.object_plan(
                "static rule CombatDamage NoAssignment",
                "assigns_no_combat_damage",
                function,
                affected,
                parts,
            )?,
            R::DamageStaysMarked => {
                self.object_table("damage_stays", function, affected, parts)?
            }
            R::LegendRuleExempt => {
                self.object_table("legend_rule_exempt", function, affected, parts)?
            }
            R::Prohibit(ObjectAction::Sacrifice) => {
                if matches!(affected, ObjectSet::One(ObjectRef::Source))
                    && self.static_guard.is_none()
                {
                    parts.keywords.push("Keyword::CantBeSacrificed".into());
                } else {
                    self.object_table("sacrifice_bans", function, affected, parts)?
                }
            }
            R::Prohibit(ObjectAction::Untap) => {
                self.object_table("cant_become_untapped", function, affected, parts)?
            }
            R::Prohibit(ObjectAction::Attack(
                defenders @ (AttackDefenders::Players(_)
                | AttackDefenders::PlayersAndPlaneswalkers(_)
                | AttackDefenders::Objects(_)),
            )) => {
                let at = self.defended(defenders)?;
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "attacker")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> bool {{ !({guard}{test} && {at}) }}").unwrap();
                parts.attack_rules.push(function.to_string());
            }
            R::CombatCost {
                action: ObjectAction::Attack(defenders),
                payer: PlayerRef::DeclaringPlayer,
                cost,
            } => {
                self.affected_object = Some("attacker");
                let read = self.attack_mana(cost);
                self.affected_object = None;
                let at = self.defended(defenders)?;
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "attacker")?);
                match read? {
                    Some(mana) => {
                        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> Option<ManaCost> {{ ({guard}{test} && {at}).then(|| {mana}) }}").unwrap();
                        parts.table("attack_costs", function.to_string());
                    }
                    None => {
                        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> bool {{ {guard}{test} && {at} }}").unwrap();
                        self.plan_entry(
                            "static rule CombatCost other than mana",
                            "attack_cost_rules",
                            &format!("AttackCost {{ applies: {function}, cost: {} }}", quoted(&form(cost))),
                            parts,
                        );
                    }
                }
            }
            R::CombatCost {
                action: ObjectAction::Block(_) | ObjectAction::BeBlocked(_),
                ..
            } => self.object_plan(
                "static rule CombatCost Block",
                "block_costs",
                function,
                affected,
                parts,
            )?,
            R::RequireCombat {
                action: ObjectAction::BeBlocked(ObjectFilter::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            } => {
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "attacker")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, attacker: ObjectId) -> bool {{ {PRELUDE} {guard}{test} }}").unwrap();
                parts.table("must_be_blocked", function.to_string());
            }
            R::RequireCombat {
                action: ObjectAction::Block(ObjectFilter::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            } => self.object_plan(
                "static rule RequireCombat Block",
                "must_block",
                function,
                affected,
                parts,
            )?,
            R::UntapStep {
                behavior: UntapBehavior::Additional,
                players: PlayerSet::OthersThan(PlayerRef::AbilityController),
            } => self.object_table("untap_in_others_untap_steps", function, affected, parts)?,
            R::UntapStep {
                behavior: UntapBehavior::Optional,
                ..
            } if matches!(affected, ObjectSet::One(ObjectRef::Source))
                && self.static_guard.is_none() =>
            {
                parts.field("may_not_untap", "true".into());
            }
            R::GoadedBy(PlayerRef::AbilityController) => self.object_plan(
                "static rule GoadedBy",
                "goaded_rules",
                function,
                affected,
                parts,
            )?,
            R::BlockCapacity(capacity) => {
                let more = match capacity {
                    BlockCapacity::Additional(n) => self.number(n)?.amount(u32::MAX.into(), "u32")?,
                    BlockCapacity::Unlimited => "u32::MAX".to_string(),
                };
                self.object_value_plan(
                    "static rule BlockCapacity",
                    "additional_blocks",
                    function,
                    affected,
                    &more,
                    "u32",
                    parts,
                )?
            }
            R::BlockedAsThoughLacking(_) => self.object_plan(
                "static rule BlockedAsThoughLacking",
                "blocked_as_though_lacking",
                function,
                affected,
                parts,
            )?,
            R::BlockWhileTapped => {
                self.object_plan(
                    "static rule BlockWhileTapped",
                    "blocks_while_tapped",
                    function,
                    affected,
                    parts,
                )?
            }
            R::MayExert(Some(then))
                if matches!(affected, ObjectSet::One(ObjectRef::Source))
                    && self.static_guard.is_none() =>
            {
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Resolution, EventAt::Nowhere);
                let compiled = self.exert_ability(function, then);
                (self.reader, self.event_at) = saved;
                parts.abilities.push(compiled?);
            }
            R::MayExert(None)
                if matches!(affected, ObjectSet::One(ObjectRef::Source))
                    && self.static_guard.is_none() =>
            {
                self.plan_entry(
                    "static rule MayExert with nothing linked",
                    "abilities",
                    "cantrip_mechanics::exert::may_as_it_attacks(\"You may exert this creature as it attacks.\")",
                    parts,
                );
            }
            R::CombatGroup {
                role,
                participants,
                others_only,
                comparison,
                amount,
            } if !self.keyword_group(rule) => {
                let Expr::Constant(n) = amount else {
                    return Err(gap("static rule CombatGroup of a counted amount"));
                };
                let count = match comparison {
                    Comparison::Greater => format!("{}..=u32::MAX", n + 1),
                    Comparison::GreaterOrEqual => format!("{n}..=u32::MAX"),
                    Comparison::LessOrEqual => format!("0..={n}"),
                    Comparison::Equal => format!("{n}..={n}"),
                    other => return Err(gap(format!("static rule CombatGroup {}", form(other)))),
                };
                let role = match role {
                    CombatRole::Attacker => "Attacker",
                    CombatRole::Blocker => "Blocker",
                    CombatRole::BlockedAttacker => "BlockedAttacker",
                };
                self.affected_object = Some("id");
                let counted = self.filter(participants, &Candidate::of("other"));
                self.affected_object = None;
                let counted = counted?;
                self.object_test(function, affected)?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}_counted(q: &dyn Query, asker: &Asker, id: ObjectId, other: ObjectId) -> bool {{ {counted} }}").unwrap();
                self.plan_entry(
                    &format!("static rule CombatGroup {role}"),
                    "combat_groups",
                    &format!("CombatGroupRule {{ role: CombatRole::{role}, applies: {function}, counted: {function}_counted, others_only: {others_only}, count: {count} }}"),
                    parts,
                );
            }
            R::TargetableDespite {
                sources: ObjectFilter::Any,
                keyword: TargetingKeyword::Hexproof,
            } => {
                let applies = self.targeting_subject(function, affected)?;
                parts.table(
                    "targeting_rules",
                    format!("TargetingRule {{ kind: TargetingRuleKind::IgnoreHexproof, applies: {applies} }}"),
                );
            }
            R::Prohibit(ObjectAction::BeTargeted(filter)) => {
                let saved = std::mem::replace(&mut self.reader, Reader::Layer);
                let forbid = self.targeting_source(filter);
                self.reader = saved;
                let forbid = forbid?;
                let applies = self.targeting_subject(function, affected)?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}_by(q: &dyn LayerQuery, source: ObjectId, by: &Asker, by_source: &Chars) -> bool {{ {PRELUDE} {forbid} }}").unwrap();
                parts.table(
                    "targeting_rules",
                    format!("TargetingRule {{ kind: TargetingRuleKind::Forbid({function}_by), applies: {applies} }}"),
                );
            }
            R::Prohibit(ObjectAction::ReceiveCounters(kind)) => {
                let kinds = match kind {
                    Some(kind) => format!("kind == {}", counter_kind(kind)?),
                    None => "true".to_string(),
                };
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "object")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, object: ObjectId, controller: PlayerId, kind: CounterKind) -> bool {{ {guard}{test} && {kinds} }}").unwrap();
                parts.table("counter_bans", function.to_string());
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    /// "You may exert this as it attacks. When you do, ...": the linked trigger.
    fn exert_ability(&mut self, label: &str, then: &Operand) -> Result<String> {
        let (announcement, body) = match then {
            Operand::Program(body) => (None, body),
            Operand::Ability(ability) => match &**ability {
                Ability::Spell { announcement, body } if announcement.modes.is_none() => {
                    (Some(announcement), body)
                }
                other => return Err(gap(format!("static rule MayExert {}", form(other)))),
            },
            other => return Err(gap(format!("static rule MayExert {}", form(other)))),
        };
        let targets = match announcement {
            Some(announcement) => Some(self.announcement(announcement, label)?),
            None => None,
        };
        let effect = self.effect(body, label)?;
        let text = quoted(&format!(
            "You may exert this creature as it attacks. When you do, {}",
            ability_text(body)
        ));
        let def = format!("cantrip_mechanics::exert::as_it_attacks({effect}, {text})");
        Ok(match targets {
            Some(targets) => format!("{def}.targets({targets})"),
            None => def,
        })
    }
    /// Whether a group restriction is a keyword the layers carry.
    fn keyword_group(&self, rule: &RuleChange) -> bool {
        matches!(
            rule,
            RuleChange::CombatGroup {
                role: CombatRole::BlockedAttacker,
                participants: ObjectFilter::Any | ObjectFilter::CardType(_),
                others_only: false,
                comparison: Comparison::GreaterOrEqual,
                amount: Expr::Constant(3),
            } | RuleChange::CombatGroup {
                role: CombatRole::BlockedAttacker,
                participants: ObjectFilter::Any,
                others_only: false,
                comparison: Comparison::LessOrEqual,
                amount: Expr::Constant(1),
            }
        )
    }
    /// The `applies` of a `TargetingRule`: it sees the layers before abilities.
    fn targeting_subject(&mut self, function: &str, affected: &ObjectSet) -> Result<String> {
        let saved = std::mem::replace(&mut self.reader, Reader::Layer);
        let tests = self
            .guard()
            .and_then(|guard| Ok((guard, self.rule_subject(affected, "subject")?)));
        self.reader = saved;
        let (guard, test) = tests?;
        let applies = format!("{function}_on");
        writeln!(self.functions, "#[allow(unused_variables)] fn {applies}(q: &dyn LayerQuery, source: ObjectId, subject: ObjectId) -> bool {{ {PRELUDE} {guard}{test} }}").unwrap();
        Ok(applies)
    }
    /// Whether `id` is among cards and spells a static ability reaches off the
    /// battlefield, read as `filter` reads it: `None` for any other set.
    pub(super) fn outside_subject(
        &self,
        set: &ObjectSet,
        c: &Candidate,
    ) -> Result<Option<String>> {
        Ok(Some(match set {
            ObjectSet::Zone {
                scope,
                kind: ObjectKind::Spell | ObjectKind::Card | ObjectKind::Any,
                filter,
            } => {
                let zone = self.filter(&ObjectFilter::InZone(scope.clone()), c)?;
                format!("{zone} && {}", self.zone_filter(filter, c)?)
            }
            ObjectSet::AnyZone(ObjectFilter::Any) => "true".to_string(),
            ObjectSet::AnyZone(ObjectFilter::SameObject(ObjectRef::Source)) => {
                format!("source == {}", c.id)
            }
            // "Cards you own that aren't on the battlefield" (CR 400.3): a token
            // or a copy off the battlefield is no card.
            ObjectSet::AnyZone(filter) => {
                let id = &c.id;
                format!(
                    "{} && {{ let o = q.state().obj({id}); !o.zone.is(ZoneKind::Battlefield) && !o.token && !o.card_copy }}",
                    self.zone_filter(filter, c)?
                )
            }
            _ => return Ok(None),
        }))
    }
    /// `filter`, where the layers can also ask how a spell on the stack was cast.
    fn zone_filter(&self, filter: &ObjectFilter, c: &Candidate) -> Result<String> {
        let each = |items: &[ObjectFilter]| -> Result<Vec<String>> {
            items.iter().map(|f| self.zone_filter(f, c)).collect()
        };
        Ok(match filter {
            ObjectFilter::All(items) => format!("({})", each(items)?.join(" && ")),
            ObjectFilter::Either(items) => format!("({})", each(items)?.join(" || ")),
            ObjectFilter::Not(inner) => format!("!({})", self.zone_filter(inner, c)?),
            ObjectFilter::Cast { by, from } if self.reader == Reader::Layer => {
                let id = &c.id;
                let mut tests = vec![format!("q.state().obj({id}).cast_from.is_some()")];
                if let Some(by) = by {
                    tests.push(format!(
                        "q.state().stack.iter().any(|i| i.kind == StackKind::Spell && i.object == {id} && {})",
                        self.player_in(by, "i.controller")?
                    ));
                }
                match from {
                    None => {}
                    Some(ZoneScope::Shared(zone)) => tests.push(format!(
                        "q.state().obj({id}).cast_from == Some({})",
                        zone_kind(zone)?
                    )),
                    Some(ZoneScope::Players { zone, players }) => {
                        tests.push(format!(
                            "q.state().obj({id}).cast_from == Some({})",
                            zone_kind(zone)?
                        ));
                        tests.push(self.player_in(players, &format!("q.state().obj({id}).owner"))?);
                    }
                }
                format!("({})", tests.join(" && "))
            }
            other => self.filter(other, c)?,
        })
    }
    /// The parts of a static ability about spells that are rules of casting
    /// them, not of what they are on the stack: what is left goes to the layers.
    pub(super) fn casting_tables(
        &mut self,
        label: &str,
        affected: &ObjectSet,
        changes: &[Change],
        parts: &mut Parts,
    ) -> Result<Vec<Change>> {
        if let (
            ObjectSet::AnyZone(ObjectFilter::Any),
            [Change::Rule(RuleChange::Prohibit(ObjectAction::Counter))],
        ) = (affected, changes)
        {
            self.plan_entry(
                "static rule Prohibit Counter of abilities",
                "cant_be_countered",
                "|_, _, _| true",
                parts,
            );
            return Ok(Vec::new());
        }
        let ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Stack),
            kind: ObjectKind::Spell,
            filter,
        } = affected
        else {
            return Ok(changes.to_vec());
        };
        let (players, spells) = spell_caster(filter);
        let mut layered = Vec::new();
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let mut compiled = Ok(());
        for (i, change) in changes.iter().enumerate() {
            let label = format!("{label}_{i}");
            compiled = match change {
                Change::SpellCost(modifier) => self.player_rule(
                    &label,
                    &players,
                    &PlayerRule::SpellCost {
                        spells: spells.clone(),
                        modifier: modifier.clone(),
                    },
                    parts,
                ),
                Change::Rule(RuleChange::Prohibit(ObjectAction::Cast)) => self.prohibit(
                    &format!("{label}_rule"),
                    &players,
                    &PlayerAction::CastSpell {
                        from: None,
                        filter: spells.clone(),
                    },
                    parts,
                ),
                Change::GrantAbility(ability) => match pay_with(ability) {
                    Some(way) => self.pay_with_grant(&label, &players, &spells, way, parts),
                    None => {
                        layered.push(change.clone());
                        Ok(())
                    }
                },
                other => {
                    layered.push(other.clone());
                    Ok(())
                }
            };
            if compiled.is_err() {
                break;
            }
        }
        (self.reader, self.event_at) = saved;
        compiled?;
        Ok(layered)
    }
    /// Convoke, delve or improvise on the spells of a static ability.
    fn pay_with_grant(
        &mut self,
        label: &str,
        players: &PlayerSet,
        spells: &ObjectFilter,
        way: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let function = format!("{label}_pay_with");
        let test = self.filter(
            spells,
            &Candidate {
                controller: "caster".into(),
                announced: Some(String::new()),
                ..Candidate::of("spell")
            },
        )?;
        let guard = self.guard()?;
        let member = self.player_in(players, "caster")?;
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, caster: PlayerId, spell: ObjectId) -> PayWith {{ {PRELUDE} if {guard}{member} && {test} {{ PayWith {{ {way}: true, ..PayWith::NONE }} }} else {{ PayWith::NONE }} }}").unwrap();
        parts.table("grants_pay_with", function);
        Ok(())
    }
    /// A keyword ability granted as a keyword, not as an ability to run.
    pub(super) fn granted_keyword(&self, ability: &Ability) -> Option<statics::LayerChange> {
        let Ability::Mechanic { mechanic, .. } = ability else {
            return None;
        };
        let body = match mechanic {
            Mechanic::SplitSecond => {
                "chars.keywords = chars.keywords.with(Keyword::SplitSecond);"
            }
            Mechanic::Retrace => "chars.keywords = chars.keywords.with(Keyword::Retrace);",
            _ => return None,
        };
        Some(statics::LayerChange::of("Ability", body.to_string()))
    }
    /// A rule the layers carry as a keyword of what it affects.
    pub(super) fn layer_rule(&mut self, rule: &RuleChange) -> Result<statics::LayerChange> {
        use RuleChange as R;
        let keyword = |word: &str| {
            statics::LayerChange::of(
                "Ability",
                format!("chars.keywords = chars.keywords.with(Keyword::{word});"),
            )
        };
        Ok(match rule {
            R::CombatGroup {
                role: CombatRole::BlockedAttacker,
                others_only: false,
                comparison: Comparison::GreaterOrEqual,
                amount: Expr::Constant(3),
                ..
            } => keyword("CantBeBlockedExceptByThreeOrMore"),
            R::TapPowerBonus {
                cost: TapPowerCost::Crew,
                bonus: Expr::Constant(2),
            } => keyword("CrewsAsThoughPowerTwoGreater"),
            R::TapPowerBonus {
                cost: TapPowerCost::Saddle,
                bonus: Expr::Constant(2),
            } => keyword("SaddlesAsThoughPowerTwoGreater"),
            R::Prohibit(ObjectAction::Counter) => keyword("CantBeCountered"),
            other => return Err(gap(format!("static rule {}", rule_name(other)))),
        })
    }
}
