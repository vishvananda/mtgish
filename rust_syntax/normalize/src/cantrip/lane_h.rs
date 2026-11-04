//! Rules of the game an object's static ability changes, past the commonest
//! ones in `statics.rs`: each is one of `CardDef`'s tables, or the table Cantrip
//! will have, and the callback a table asks is written out either way.
use super::*;

const PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";
/// The shape of most object tables: `id` is the permanent the rule is about.
const ASKED: &str = "(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool";

/// How long a permission on a card lasts.
pub(super) fn play_expiry(duration: &crate::ir::Duration) -> Result<String> {
    use crate::ir::Duration as D;
    Ok(match duration {
        D::EndOfTurn => "Expiry::EndOfTurn".to_string(),
        D::UntilEndOfNextTurn(PlayerRef::AbilityController) => {
            "Expiry::EndOfYourNextTurn".to_string()
        }
        D::UntilNext {
            point: TurnPoint::EndStepBegins,
            players: PlayerSet::One(PlayerRef::AbilityController),
        } => "Expiry::UntilYourNextEndStep".to_string(),
        D::WhileControlled {
            player: PlayerRef::AbilityController,
            object: ObjectRef::Source,
        } => "Expiry::WhileControls(asker.source)".to_string(),
        D::Indefinite | D::WhileExiled(_) | D::WhileAnyExiled(_) => "Expiry::Never".to_string(),
        other => return Err(gap(format!("play permission lasting {}", form(other)))),
    })
}

/// A plain keyword as a card says it, "first strike": none for one with a
/// quality of its own (protection, ward).
pub(super) fn keyword_words(keyword: &Keyword) -> Option<String> {
    match keyword {
        Keyword::Protection(_) | Keyword::Ward(_) => None,
        other => Some(camel_words(&form(other))),
    }
}

/// "Except it enters with an additional +1/+1 counter on it if it's a creature" (Spark Double, ruling 2019-05-03): "it"
/// is the copy as it enters, whose types are the copiable values it took (CR 707.2), not what an effect makes the original
/// now (a land animated into a creature copies as a land). `code` tests the chosen object's types with `q.chars(id)`; each
/// such test reads `copied`, an `Option<&Copiable>` of the values, instead.
fn copy_types_as_it_enters(code: &str, copied: &str) -> String {
    const READ: &str = "q.chars(id).is(CardType::";
    let mut out = String::with_capacity(code.len());
    let mut rest = code;
    while let Some(at) = rest.find(READ) {
        let after = &rest[at + READ.len()..];
        let Some(end) = after.find(')') else { break };
        let kind = &after[..end];
        out.push_str(&rest[..at]);
        out.push_str(&format!(
            "{copied}.is_some_and(|c| c.extra_types.has(CardType::{kind}) || !c.types_stated() && q.card(c.card).is(CardType::{kind}))"
        ));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `FirstStrike` as a card says it.
fn camel_words(word: &str) -> String {
    let mut out = String::new();
    for (i, c) in word.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// "If this would be put into a graveyard from anywhere, reveal it and shuffle
/// it into its owner's library instead": the words Cantrip has a constant for.
pub(super) fn reveal_and_shuffle_instead(
    active: &ReplacementActive,
    event: &ReplacementEvent,
    body: &Program,
) -> bool {
    let instructions = match body.instructions.as_slice() {
        [Instruction::Group(inner)] => inner.instructions.as_slice(),
        other => other,
    };
    matches!(active, ReplacementActive::Everywhere)
        && matches!(
            event,
            ReplacementEvent::ZoneChange {
                objects: ObjectFilter::SameObject(ObjectRef::Source),
                from: None,
                to: EventZone::Graveyard,
            }
        )
        && matches!(
            instructions,
            [
                Instruction::Effect {
                    operation: Operation::Reveal {
                        objects: ObjectSet::One(ObjectRef::EventSubject),
                        ..
                    },
                    ..
                },
                Instruction::Effect {
                    operation: Operation::ModifyEvent {
                        change: EventChange::SetDestination(Destination::OwnersLibraryShuffled),
                    },
                    ..
                },
            ]
        )
}

/// A play permission that says nothing but where from.
pub(super) fn plain_terms() -> PlayTerms {
    PlayTerms {
        limit: None,
        during: None,
        cost: PlayCost::Normal,
        spend_as: None,
        into_exile: false,
        condition: None,
        effects: Vec::new(),
        flash: false,
        may_look: false,
        then: None,
        face_down: None,
    }
}

/// The generic mana a cost change names: `{2}` as a term, alone or in a list.
fn generic_amount(amount: &Operand) -> Option<u8> {
    match amount {
        Operand::List(items) => match items.as_slice() {
            [item] => generic_amount(item),
            _ => None,
        },
        Operand::Term { form, operands, .. }
            if form == "CostReduceGeneric" || form == "ManaCostGeneric" =>
        {
            match operands.as_slice() {
                [Operand::Integer(n)] => u8::try_from(*n).ok(),
                _ => None,
            }
        }
        _ => None,
    }
}

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
/// What the client calls one choice of what a permanent enters as.
fn entry_label(changes: &[Change]) -> String {
    let mut words = Vec::new();
    for change in changes {
        let Change::Copiable(list) = change else {
            continue;
        };
        for change in list {
            match change {
                Change::SetBasePowerToughness {
                    power: Expr::Constant(power),
                    toughness: Expr::Constant(toughness),
                } => words.push(format!("{power}/{toughness}")),
                Change::AddCardType(kind) => words.push(name(kind).to_lowercase()),
                Change::GrantKeyword(word) => {
                    words.push(format!("with {}", form(word).to_lowercase()))
                }
                Change::CreatureTypes {
                    types: CreatureTypeSet::Listed(types),
                    ..
                } => words.extend(types.iter().map(name)),
                _ => {}
            }
        }
    }
    words.join(" ")
}
/// Whether a program says what the permanent enters as: `CardDef::enters_as_copy`,
/// chosen before every other replacement of how it enters (CR 616.1c).
pub(super) fn enters_as_copy(program: &Program) -> bool {
    fn states(state: &EntryState) -> bool {
        match state {
            EntryState::CopyOf { .. } => true,
            EntryState::Changes(changes) => all_copiable(changes),
            EntryState::ChooseChanges(options) => options.iter().all(|o| all_copiable(o)),
            _ => false,
        }
    }
    program
        .instructions
        .iter()
        .any(|instruction| match instruction {
            Instruction::Effect {
                operation:
                    Operation::ModifyEvent {
                        change: EventChange::Entry(state),
                    },
                ..
            } => states(state),
            Instruction::Optional { body, .. } | Instruction::Group(body) => enters_as_copy(body),
            Instruction::If {
                then, otherwise, ..
            } => enters_as_copy(then) || enters_as_copy(otherwise),
            _ => false,
        })
}
/// Whether an entry's changes are all copiable values (CR 707.2).
fn all_copiable(changes: &[Change]) -> bool {
    !changes.is_empty() && changes.iter().all(|c| matches!(c, Change::Copiable(_)))
}
/// Whether "you may have it enter as a copy" is what this optional body says.
pub(super) fn asks_to_copy(body: &Program) -> bool {
    let mut flat = Vec::new();
    lane_m::flat_groups(&body.instructions, &mut flat);
    matches!(
        flat.first(),
        Some(Instruction::Effect {
            operation: Operation::ModifyEvent {
                change: EventChange::Entry(EntryState::CopyOf { .. }),
            },
            ..
        })
    )
}
/// Whether a filter names what only the resolving ability knows: a lasting
/// rule's callback is told the object it is on and nothing else.
fn names_the_effect(filter: &ObjectFilter) -> bool {
    let text = form(filter);
    [
        "Source",
        "AbilityController",
        "Target",
        "Event",
        "Affected",
        "Chosen",
        "Result",
    ]
    .iter()
    .any(|word| text.contains(word))
}
/// The words after "for as long as", for the client (`Lasting::text`).
fn lasting_text(duration: &crate::ir::Duration) -> String {
    use crate::ir::Duration as D;
    match duration {
        D::WhileControlled {
            player: PlayerRef::ControllerOf(_),
            ..
        } => "they control it".to_string(),
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
        // "For as long as they control it", they being whoever controls it
        // as the effect begins: Vislor Turlough.
        D::WhileControlled {
            player: PlayerRef::ControllerOf(of),
            object,
        } if same(object) && form(&**of) == form(object) => D::WhileControlled {
            player: PlayerRef::ControllerOf(Box::new(ObjectRef::Affected)),
            object: ObjectRef::Affected,
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
            "ManaCost {{ generic: u32::try_from(({times}) as i64 * {n}).unwrap_or(u32::MAX), ..ManaCost::ZERO }}"
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
        matches!(
            f,
            F::Cast {
                by: None,
                from: Some(ZoneScope::Players {
                    zone: EventZone::Hand,
                    ..
                })
            }
        )
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

/// A mana rule made as the effect resolves reads its subtype through the words of
/// the object's text (612.2).
pub(super) fn rule_through_words(rule: &str) -> String {
    const PLAIN: &str = "SubtypeSpells::of(Subtype::";
    let mut read = String::new();
    let mut rest = rule;
    while let Some(at) = rest.find(PLAIN) {
        let (before, tail) = rest.split_at(at);
        let tail = &tail[PLAIN.len()..];
        let end = tail.find(')').unwrap_or(tail.len());
        write!(
            read,
            "{before}SubtypeSpells::of(g.words(asker.source).subtype(Subtype::{}))",
            &tail[..end]
        )
        .unwrap();
        rest = &tail[(end + 1).min(tail.len())..];
    }
    read.push_str(rest);
    read
}

impl Compiler {
    /// What mana may be spent on: Cantrip's word for it, or the rule it will
    /// read, as a callback on the payment.
    pub(super) fn mana_use(&mut self, value: &ManaUse) -> Result<String> {
        if let Some(code) = known_mana_use(value) {
            let printed_any = self.line_text().is_some_and(|line| {
                line.contains("Spend this mana") && !line.contains("creature spell")
            });
            if printed_any && code.contains("CHOSEN_TYPE_CREATURE_SPELLS_OR_ABILITIES") {
                return Err(gap("a chosen type mana rule that names creatures it doesn't print"));
            }
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
            Err(gap) if gap.feature.starts_with("mana spending") => {
                let _ = failed;
                return self.mana_use_composed(value);
            }
            Err(gap) => return Err(gap),
        };
        // What a payment is for, past `SpendingFor`, has no word to compile yet.
        // A rule the mana carries is a function of the card's; one reading
        // what a payment is for past `SpendingFor` is an item of the hole,
        // having no word to compile yet.
        let function = format!("mana_use_{}", self.functions.len());
        let item = format!("#[allow(dead_code, unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, spending: &Spending) -> bool {{ {test} }}");
        match test.contains("ManaPart") {
            true => writeln!(self.functions, "{}", planned::planned_item(feature, &item)).unwrap(),
            false => writeln!(self.functions, "{item}").unwrap(),
        }
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
            D::WhileMatches { .. }
            | D::WhileItMatches(_)
            | D::WhileControlled { .. }
            | D::Either(_) => {
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                let test = self.lasting_test(duration);
                (self.reader, self.event_at) = saved;
                if test.is_err() && !matches!(duration, D::Either(_)) {
                    if let D::WhileMatches { object, filter } = duration {
                        if let Ok(code) = self.lasting_about(duration, object, filter) {
                            return Ok(code);
                        }
                    }
                    return self.duration_p(duration);
                }
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
                    self.source_card()
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
            other => self.duration_p(other)?,
        })
    }
    /// "For as long as" about something its ability named as it resolved (CR
    /// 611.2b): the object it names, or what the test of the effect's own
    /// object reads ("remains attached to that creature"). The game keeps that
    /// as data, the `about` of `Duration::AsLongAsAbout`, and asks the card's
    /// `lasting_about` test of it; it keeps no closure.
    fn lasting_about(
        &mut self,
        duration: &crate::ir::Duration,
        object: &ObjectRef,
        filter: &ObjectFilter,
    ) -> Result<String> {
        let feature = format!("duration {}", form(duration));
        let (id, live, named) = match object {
            ObjectRef::Source => (
                "e.source",
                "is_live(q, e.source) && q.is_on_battlefield(e.source) && ",
                None,
            ),
            ObjectRef::Affected => ("e.affected", "", None),
            other => ("id", "", Some(self.fetch(self.object(other)?))),
        };
        let test = self.filter(filter, &Candidate::of(id))?;
        let (test, about) = match named {
            Some(named) => {
                if lane_o::reads_the_resolution(&test) {
                    return Err(gap(&feature));
                }
                (
                    format!("about.object().is_some_and(|id| {test})"),
                    format!("{named}.map(Target::Object).unwrap_or(Target::None)"),
                )
            }
            None => {
                // An object the ability remembers, as its item holds it.
                let mut test = test;
                let mut held = None;
                for n in 0..8 {
                    let read = format!("Some(ObjectId(item.data[{n}])).filter(|o| *o != ObjectId::NONE)");
                    if test.contains(&read) {
                        if held.is_some() {
                            return Err(gap(&feature));
                        }
                        test = test.replace(&read, "about.object()");
                        held = Some(format!("{read}.map(Target::Object).unwrap_or(Target::None)"));
                    }
                }
                let (test, about) = self.named_as_resolved(&test)?;
                let about = match (held, about.as_str()) {
                    (None, _) => about,
                    (Some(held), "Target::None") => held,
                    _ => return Err(gap(&feature)),
                };
                (format!("{live}{test}"), about)
            }
        };
        let index = self.made_lasting_about.len();
        let function = format!("lasting_about_{index}");
        let item = format!("#[allow(unused_variables)] fn {function}(q: &dyn Query, e: &ContinuousEffect, about: Target) -> bool {{ let asker = &Asker::new(e.source, e.controller); {test} }}");
        match planned::landed(&feature) {
            true => writeln!(self.functions, "{item}").unwrap(),
            false => writeln!(self.functions, "planned_item!({:?} => {item});", planned::requirement(&feature)).unwrap(),
        }
        self.made_lasting_about.push((
            feature.clone(),
            format!(
                "LastingAbout {{ text: {}, holds: {function} }}",
                quoted(&lasting_text(duration))
            ),
        ));
        Ok(planned::planned_value(
            &feature,
            "Duration::Indefinite",
            &self.fetch(format!(
                "Duration::AsLongAsAbout({}, {index}, {about})",
                self.source_card()
            )),
        ))
    }
    /// The test of a "for as long as": `e` is the effect it is about.
    fn lasting_test(&self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        Ok(match duration {
            D::WhileControlled {
                player: PlayerRef::AbilityController,
                object: ObjectRef::Source,
            } => "is_live(q, e.source) && q.is_on_battlefield(e.source) && q.controller(e.source) == e.controller".to_string(),
            // "When you do, ... for as long as you control this creature" (Wall of
            // Stolen Identity): the reflexive ability's "you" comes out as its
            // event subject's controller, but the card says you, the player the
            // effect is for.
            D::WhileControlled {
                player: PlayerRef::ControllerOf(of),
                object: ObjectRef::Source,
            } if matches!(**of, ObjectRef::EventSubject)
                && self.card_prints("for as long as you control this") =>
            {
                "is_live(q, e.source) && q.is_on_battlefield(e.source) && q.controller(e.source) == e.controller".to_string()
            }
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
            // The controller it had as the effect began is the player of the
            // latest control effect on it then (CR 613.1b, a control effect
            // ending or a later one is a change, CR 611.2b); with none, the
            // effect's controller's.
            D::WhileControlled {
                player: PlayerRef::ControllerOf(of),
                object: ObjectRef::Affected,
            } if matches!(**of, ObjectRef::Affected) => "is_live(q, e.affected) && q.is_on_battlefield(e.affected) && Some(q.controller(e.affected)) == Some(q.state().effects.iter().filter(|f| f.affected == e.affected && f.timestamp <= e.timestamp).filter_map(|f| match f.kind { EffectKind::Control(p) => Some((f.timestamp, p)), _ => None }).max_by_key(|&(t, _)| t).map_or(e.controller, |(_, p)| p))".to_string(),
            // "For as long as that creature is enchanted": it, the affected
            // object (Rootwater Matriarch).
            D::WhileItMatches(filter) => self.lasting_filter("", filter, "e.affected")?,
            D::Either(all) => all
                .iter()
                .map(|d| Ok(format!("({})", self.lasting_test(d)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" && "),
            other => return Err(gap(format!("duration {}", form(other)))),
        })
    }
    fn lasting_filter(&self, live: &str, filter: &ObjectFilter, id: &str) -> Result<String> {
        Ok(format!(
            "{live}{}",
            self.filter(filter, &Candidate::of(id))?
        ))
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
                    event @ (ReplacedPlayerEvent::Draw | ReplacedPlayerEvent::DrawExceptFirstInDrawStep),
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
                        locals:
                            "let subject = Target::Player(d.player); let data = [1u32, u32::MAX];"
                                .into(),
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
                    locals:
                        "let amount = d.count; let subject = Target::Player(d.player); let data = [d.count, u32::MAX];"
                            .into(),
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
                    entry_from: false,
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
                    Some(index) => (
                        format!("c.counts[{index}] > 0"),
                        format!("c.counts[{index}].max(0) as u32"),
                    ),
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
            _ => self.watched_rest(event),
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
            _ => return self.reshape_rest(kind, change, frame),
        })
    }
    /// A rule a resolving spell or ability makes, as what `EffectKind` says of it
    /// or will: an effect on the object, so it outlasts its source (CR 611.2c).
    pub(super) fn continuous_rule(&mut self, rule: &RuleChange) -> Result<Vec<String>> {
        use RuleChange as R;
        let planned = |feature: &str, code: &str| {
            vec![planned::planned_value(
                feature,
                "EffectKind::CantBlock",
                code,
            )]
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
            R::Prohibit(ObjectAction::Regenerate) if planned::switched_on("regeneration-bans") => {
                vec!["EffectKind::CantBeRegenerated".into()]
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
                // A block rule's source is the object the effect is on.
                let event = (self.frame, self.event_object.clone());
                if self.rule_on_event_object {
                    self.frame.object = Some(Carried::Subject);
                    self.event_object = Some(ObjectRef::Source);
                }
                // "Only by creatures in the same sector" (Space Beleren): the
                // object the effect is on is the one the rule is asked about.
                let affected = self.affected_object.replace(subject);
                let test = self.filter(filter, &Candidate::of(other));
                self.affected_object = affected;
                (self.frame, self.event_object) = event;
                (self.reader, self.event_at) = saved;
                if test.as_ref().is_ok_and(|code| self.reads_a_local(code)) {
                    return Err(gap("a lasting block rule reading what its ability chose"));
                }
                let index = self.made_block_rules.len();
                let function = format!("block_rule_{index}");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ {PRELUDE} !({subject} == source && {}) }}", test?).unwrap();
                self.made_block_rules.push(format!(
                    "LastingBlockRule {{ text: {}, allows: {function} }}",
                    quoted(&self.block_rule_words().unwrap_or_else(|| form(action)))
                ));
                vec![format!(
                    "EffectKind::BlockRestriction {{ card: {}, index: {index} }}",
                    self.source_card()
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
            other => return self.stated_rule_kind(other),
        })
    }
    /// "You may play it this turn": a permission on the cards themselves, each a
    /// new object once it moves (CR 400.7), so it is theirs however long it lasts.
    /// False if this rule is no such permission.
    pub(super) fn play_for_a_while(
        &mut self,
        id: usize,
        i: usize,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<bool> {
        let plain = plain_terms();
        let (permission, terms) = match rule {
            PlayerRule::PlayPermissionWith { permission, terms } => (&**permission, terms),
            other => (other, &plain),
        };
        let PlayerRule::PlayPermission {
            cards,
            from,
            action,
            filter,
        } = permission
        else {
            return Ok(false);
        };
        let lands = match action {
            CardPlayAction::Either => true,
            CardPlayAction::CastSpell => false,
            other => return Err(gap(format!("play permission to {}", form(other)))),
        };
        // "Until that player's next end step, they may play it" (Casey & Raph):
        // the holder's own next end step, whoever the holder is.
        let expiry = match duration {
            crate::ir::Duration::UntilNext {
                point: TurnPoint::EndStepBegins,
                players: until,
            } if matches!(players, PlayerSet::One(_))
                && format!("{until:?}") == format!("{players:?}") =>
            {
                "Expiry::UntilYourNextEndStep".to_string()
            }
            // "For as long as it remains exiled" read as until it is cast: a
            // permission for that card, which ends as it leaves exile (CR 400.7).
            crate::ir::Duration::UntilCast(_) => "Expiry::Never".to_string(),
            // "They may play those cards until the end of their next turn": the
            // next turn of the player the permission is for.
            crate::ir::Duration::UntilEndOfNextTurn(whose)
                if matches!(players, PlayerSet::One(p) if form(p) == form(whose)) =>
            {
                "Expiry::EndOfYourNextTurn".to_string()
            }
            other => play_expiry(other)?,
        };
        if terms.during.is_some() || terms.flash || terms.may_look {
            return Err(gap("play permission on other terms"));
        }
        if terms.then.is_some() || terms.face_down.is_some() {
            return Err(gap("play permission on other terms"));
        }
        if let Some(effect) = terms.effects.first() {
            return Err(gap(played_effect_hole(effect)));
        }
        let mut grant = format!("lands: {lands}, expiry: {expiry}, ");
        match &terms.cost {
            PlayCost::Normal => {}
            PlayCost::Free => grant.push_str("instead: Some(ManaCost::ZERO), "),
            PlayCost::Using(CostedKeyword::Adventure) => grant.push_str("adventure_only: true, "),
            PlayCost::Alternate(Operand::Cost(price)) => match price.parts.as_slice() {
                [CostPart::Mana(symbols)] => {
                    write!(grant, "instead: Some({}), ", mana(symbols)?).unwrap()
                }
                [CostPart::LifeEqualToOwnManaValue] => {
                    grant.push_str("life_for_mana_value: true, ")
                }
                _ => return Err(gap("play permission for another cost")),
            },
            PlayCost::Additional(Operand::Cost(price)) => match price.parts.as_slice() {
                [CostPart::Mana(symbols)] => match symbols.as_slice() {
                    [ManaSymbol::Generic(n)] if u8::try_from(*n).is_ok() => {
                        write!(grant, "more: {n}, ").unwrap()
                    }
                    _ => return Err(gap("play permission for another cost")),
                },
                _ => return Err(gap("play permission for another cost")),
            },
            _ => return Err(gap("play permission for another cost")),
        }
        if terms.into_exile {
            grant.push_str("exile_instead: ExileInstead::Spell, ");
        }
        let mut planned_feature: Option<&str> = None;
        match &terms.spend_as {
            None => {}
            Some(SpendAs::AnyType) => grant.push_str("any_type: true, "),
            Some(SpendAs::AnyColor) => {
                grant.push_str("any_color: true, ");
                planned_feature = Some("play permission with mana as though any color");
            }
            Some(other) => {
                write!(grant, "spend_as: SpendAs::{}, ", form(other)).unwrap();
                planned_feature = Some("play permission with one mana as though any color");
            }
        }
        match &terms.condition {
            None => {}
            Some(Condition::Controls {
                player: PlayerRef::AbilityController,
                filter: ObjectFilter::Subtype(subtype),
                minimum: 1,
            }) => write!(
                grant,
                "when: Some(PlayCondition::ControlsSubtype(Subtype::{})), ",
                name(subtype)
            )
            .unwrap(),
            Some(condition) => {
                let label = format!("play_{id}_{i}_when");
                let test = self.activation_condition(condition, &label)?;
                // The game keeps the grant as data: the condition is a rule of
                // the card's `play_conditions`, the grant naming its index, as
                // `EffectKind::CantCast` names one of `lasting_spell_rules`.
                const FEATURE: &str = "play permission on a condition";
                let index = self.holes.matches("=> play_conditions:").count();
                let id = planned::requirement(FEATURE);
                writeln!(self.holes, "planned_item!({id:?} => play_conditions: {test});").unwrap();
                let card = match self.stack_item {
                    true => "item.ability.card",
                    false => "g.card_id(&CARD)",
                };
                write!(grant, "when: Some(PlayCondition::Rule {{ card: {card}, index: {index} }}), ").unwrap();
                planned_feature = Some(FEATURE);
            }
        }
        let grant = format!("PlayGrant {{ {grant}..PlayGrant::CAST }}");
        let players = self.fetch(self.players(players)?);
        let exiled_with = matches!(filter, ObjectFilter::Any)
            && matches!(
                cards,
                ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Exile),
                    filter: ObjectFilter::ExiledWith(ObjectRef::Source),
                    ..
                }
            );
        if matches!(duration, crate::ir::Duration::UntilCast(_))
            && (exiled_with || terms.limit.is_some())
        {
            return Err(gap("play permission lasting UntilCast"));
        }
        let one = match terms.limit {
            None => false,
            Some(PlayLimit::Total(1)) if exiled_with => true,
            Some(PlayLimit::Total(_)) if !self.stack_item || !exiled_with => {
                let count = match terms.limit {
                    Some(PlayLimit::Total(n)) => n,
                    _ => 1,
                };
                let chosen = ObjectSet::Matching {
                    objects: Box::new(cards.clone()),
                    filter: filter.clone(),
                };
                let cards = self.fetch(self.objects(&chosen)?);
                self.plan_code(
                    "play permission for a number of cards",
                    &format!("let cards = {cards}; let players = {players}; for &player in &players {{ g.allow_play_some(&cards, player, {grant}, {count}); }}"),
                    out,
                );
                return Ok(true);
            }
            Some(_) => return Err(gap("play permission for a number of cards")),
        };
        if exiled_with && self.stack_item {
            let code = format!("let players_{id}_{i} = {players}; let linked_{id}_{i} = g.linked(item); cantrip_api::instruction(g, |g| {{ for &player in &players_{id}_{i} {{ g.allow_play_exiled_with(linked_{id}_{i}, player, {grant}, {one}); }} }});");
            match planned_feature {
                Some(feature) => self.plan_code(feature, &code, out),
                None => writeln!(out, "{code}").unwrap(),
            }
            return Ok(true);
        }
        if cards.battlefield_filter().is_none() && matches!(cards, ObjectSet::Zone { .. }) {
            // Cards that come there later are named too: no list of them today has it.
            return Err(gap("play permission on an open set of cards"));
        }
        // "Until end of turn, you may cast instant and sorcery spells from the top of
        // your graveyard" (Bösium Strip): whatever card is on top as it is cast. A
        // permission is for one object (`allow_play`), and the one lasting rule for a
        // graveyard (`EffectKind::PlayFromGraveyard`) takes every card in it.
        if let ObjectSet::Take { objects, .. } = cards {
            if matches!(
                &**objects,
                ObjectSet::Zone { scope: ZoneScope::Players { zone: EventZone::Graveyard, players: PlayerSet::One(PlayerRef::Affected) }, .. }
            ) {
                return Err(gap("a lasting permission to cast from the top of a graveyard"));
            }
        }
        // "Until end of turn, you may play cards from the top of your library"
        // (Gwenom, The Belligerent): the same for a library, where the lasting
        // rule a static ability has (`PlayRule::LibraryTop`) has no effect to
        // give it for a while.
        if top_of_their(cards, EventZone::Library) {
            return Err(gap("a lasting permission to play from the top of a library"));
        }
        let chosen = match filter {
            ObjectFilter::Any => cards.clone(),
            filter => ObjectSet::Matching {
                objects: Box::new(cards.clone()),
                filter: filter.clone(),
            },
        };
        let code = format!(
            "let cards_{id}_{i} = {}; let players_{id}_{i} = {players}; cantrip_api::instruction(g, |g| {{ for &card in &cards_{id}_{i} {{ if g.obj(card).zone.is({}) {{ for &player in &players_{id}_{i} {{ g.allow_play(card, player, {grant}); }} }} }} }});",
            self.fetch(self.objects(&chosen)?),
            zone_kind(from)?,
        );
        match planned_feature {
            Some(feature) => self.plan_code(feature, &code, out),
            None => writeln!(out, "{code}").unwrap(),
        }
        Ok(true)
    }
    /// A rule on players the arms in `players.rs` have no word for.
    pub(super) fn more_player_rule(
        &mut self,
        label: &str,
        players: &PlayerSet,
        rule: &PlayerRule,
        parts: &mut Parts,
    ) -> Result<()> {
        match rule {
            PlayerRule::ActivationCost {
                abilities,
                change,
                floor,
            } => self.activation_cost(label, players, abilities, change, *floor, parts),
            PlayerRule::PlayPermission { .. } | PlayerRule::PlayPermissionWith { .. } => {
                self.static_play(label, players, rule, parts)
            }
            PlayerRule::ActivationGrant { abilities, grant } => {
                self.activation_grant(label, players, abilities, grant, parts)
            }
            PlayerRule::SpendManaAs { spend_as, on } => {
                self.spend_mana_as(label, players, spend_as, on, parts)
            }
            PlayerRule::LimitPerTurn {
                action: PlayerAction::CastSpell { from: None, filter },
                amount: Expr::Constant(n),
            } => self.cast_limit(label, players, filter, *n, parts),
            PlayerRule::Skip(section) => {
                let step = match section {
                    TurnSection::Untap => "Step::Untap",
                    TurnSection::Upkeep => "Step::Upkeep",
                    TurnSection::Draw => "Step::Draw",
                    other => return Err(gap(format!("player rule Skip {}", form(other)))),
                };
                const FEATURE: &str = "skipping a step of the turn";
                let applies = self.player_test(players, FEATURE, &format!("{label}_skips"))?;
                self.plan_entry(
                    FEATURE,
                    "skip_steps",
                    &format!("SkipStep {{ applies: {applies}, step: {step} }}"),
                    parts,
                );
                Ok(())
            }
            PlayerRule::UntapLimit {
                permanents,
                amount: Expr::Constant(n),
            } => {
                const FEATURE: &str = "a limit on what untaps in an untap step";
                let test = self.filter(permanents, &Candidate::of("id"))?;
                let applies = self.player_test(players, FEATURE, &format!("{label}_untaps"))?;
                let params = "q: &dyn Query, asker: &Asker, id: ObjectId";
                let permanents = self.rule_fn(FEATURE, &format!("{label}_untapping"), params, "bool", &test);
                self.plan_entry(
                    FEATURE,
                    "untap_limits",
                    &format!("UntapLimit {{ applies: {applies}, permanents: {permanents}, amount: {n} }}"),
                    parts,
                );
                Ok(())
            }
            PlayerRule::HandSize(size) => {
                let size = match size {
                    HandSizeRule::Unlimited => "HandSize::Unlimited".to_string(),
                    HandSizeRule::Set(Expr::Constant(n)) => format!("HandSize::Set({n})"),
                    HandSizeRule::Adjust(Expr::Constant(n)) => format!("HandSize::Adjust({n})"),
                    other => return Err(gap(format!("player rule HandSize {}", form(other)))),
                };
                const FEATURE: &str = "a maximum hand size of other players";
                let applies = self.player_test(players, FEATURE, &format!("{label}_hand_size"))?;
                self.plan_entry(
                    FEATURE,
                    "hand_size_rules",
                    &format!("HandSizeRule {{ applies: {applies}, size: {size} }}"),
                    parts,
                );
                Ok(())
            }
            PlayerRule::Visibility { objects, audience } => {
                self.visibility(players, objects, audience, parts)
            }
            PlayerRule::SpellsHaveAbilities {
                spells,
                from,
                nth_each_turn: None,
                abilities,
            } => self.spells_have(label, players, spells, from.as_ref(), abilities, parts),
            PlayerRule::KeywordCost {
                keyword,
                cards,
                change,
                per,
                first_each_turn,
            } => self.keyword_cost(
                label,
                players,
                keyword,
                cards,
                change,
                per.as_ref(),
                *first_each_turn,
                parts,
            ),
            other => Err(gap(format!("player rule {}", form(other)))),
        }
    }
    /// "You may pay {0} rather than pay the equip cost of the first equip ability
    /// you activate each turn" (CR 118.9), and what other keywords' costs want.
    #[allow(clippy::too_many_arguments)]
    fn keyword_cost(
        &mut self,
        label: &str,
        players: &PlayerSet,
        keyword: &CostedKeyword,
        cards: &ObjectFilter,
        change: &KeywordCostChange,
        per: Option<&Expr>,
        first_each_turn: bool,
        parts: &mut Parts,
    ) -> Result<()> {
        let failure = || gap("player rule KeywordCost");
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        if let (
            CostedKeyword::Equip,
            KeywordCostChange::Alternate(amount),
            ObjectFilter::Any,
            None,
            true,
        ) = (keyword, change, cards, per, first_each_turn)
        {
            if generic_amount(amount) == Some(0) {
                self.permission_entry(
                    None,
                    "ability_alternatives",
                    &format!("{label}_rule"),
                    ("q: &dyn Query, asker: &Asker, player: PlayerId, permanent: ObjectId, def: &ActivatedDef", "", "Option<ManaCost>"),
                    &format!("({guard}{member} && def.equip && !cantrip_mechanics::history::activated_an_equip_ability_this_turn(q, player)).then_some(ManaCost::ZERO)"),
                    |function| function.to_string(),
                    parts,
                );
                return Ok(());
            }
        }
        let test = self.filter(cards, &Candidate::of("card"))?;
        let amount = |c: &mut Self, amount: &Operand| match amount {
            Operand::Cost(cost) => c.cost(cost, &format!("{label}_cost")),
            other => generic_amount(other)
                .map(|n| format!("Cost::generic({n})"))
                .ok_or_else(failure),
        };
        let change = match change {
            KeywordCostChange::Reduce(n) => {
                format!("KeywordCostChange::Reduce({})", amount(self, n)?)
            }
            KeywordCostChange::Increase(n) => {
                format!("KeywordCostChange::Increase({})", amount(self, n)?)
            }
            KeywordCostChange::Alternate(n) => {
                format!("KeywordCostChange::Alternate({})", amount(self, n)?)
            }
        };
        let per = match per {
            None => "1".to_string(),
            Some(per) => self.number(per)?.code,
        };
        const FEATURE: &str = "a keyword's cost changed";
        let params = "q: &dyn Query, asker: &Asker, player: PlayerId, card: ObjectId";
        let body = format!("{guard}{member} && {test}");
        let applies = self.rule_fn(FEATURE, &format!("{label}_keyword_cards"), params, "bool", &body);
        let per = self.rule_fn(FEATURE, &format!("{label}_keyword_per"), "q: &dyn Query, asker: &Asker", "i64", &per);
        self.plan_entry(
            FEATURE,
            "keyword_costs",
            &format!("KeywordCostRule {{ keyword: CostedKeyword::{}, applies: {applies}, change: {change}, per: {per}, first_each_turn: {first_each_turn} }}", name(keyword)),
            parts,
        );
        Ok(())
    }
    /// "Spells you cast have delve", "... from your hand have cascade": a keyword a
    /// spell has as it is cast (CR 702.66a, 702.85a).
    fn spells_have(
        &mut self,
        label: &str,
        players: &PlayerSet,
        spells: &ObjectFilter,
        from: Option<&EventZone>,
        abilities: &Operand,
        parts: &mut Parts,
    ) -> Result<()> {
        let failure = || gap("player rule SpellsHaveAbilities");
        let items = match abilities {
            Operand::List(items) => items.as_slice(),
            other => std::slice::from_ref(other),
        };
        let mut forms = Vec::new();
        for item in items {
            let Operand::Term { form, operands, .. } = item else {
                return Err(failure());
            };
            if !operands.is_empty() {
                return Err(failure());
            }
            forms.push(form.as_str());
        }
        let paying = |word: &str| matches!(word, "Convoke" | "Delve" | "Improvise" | "Assist");
        if forms.iter().copied().all(paying) {
            if from.is_some() {
                return Err(failure());
            }
            let (guard, member) = (self.guard()?, self.player_member(players)?);
            let test = self.filter(
                spells,
                &Candidate {
                    announced: Some(String::new()),
                    ..super::players::cast_card()
                },
            )?;
            let fields: Vec<String> = forms
                .iter()
                .map(|f| format!("{}: true", f.to_lowercase()))
                .collect();
            self.permission_entry(
                None,
                "grants_pay_with",
                &format!("{label}_rule"),
                ("q: &dyn Query, source: ObjectId, player: PlayerId, spell: ObjectId", "", "PayWith"),
                &format!("{PRELUDE} if {guard}{member} && {test} {{ PayWith {{ {}, ..PayWith::NONE }} }} else {{ PayWith::NONE }}", fields.join(", ")),
                |function| function.to_string(),
                parts,
            );
            return Ok(());
        }
        let mut changes = Vec::new();
        for word in &forms {
            changes.push(match *word {
                "Cascade" => Change::GrantAbility(Box::new(Ability::Mechanic {
                    mechanic: Mechanic::Cascade,
                    cost: None,
                    amount: None,
                    filter: None,
                })),
                "CantBeCountered" => Change::Rule(RuleChange::Prohibit(ObjectAction::Counter)),
                _ => return Err(failure()),
            });
        }
        let filter = ObjectFilter::All(vec![
            ObjectFilter::Cast {
                by: Some(players.clone()),
                from: from.map(|zone| ZoneScope::Shared(zone.clone())),
            },
            spells.clone(),
        ]);
        let affected = EffectSubjects::Objects(ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Stack),
            kind: ObjectKind::Spell,
            filter,
        });
        self.static_ability(label, &StaticKind::Ordinary, &affected, &changes, parts)
    }
    /// "You have hexproof until end of turn" (CR 702.11c, 702.16j): an effect each
    /// player controls, about the player. False if this is no such rule.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn keyword_for_a_while(
        &mut self,
        id: usize,
        i: usize,
        source: &ObjectRef,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<bool> {
        let (kind, feature) = match rule {
            PlayerRule::GrantKeyword(PlayerKeyword::Hexproof) => {
                ("EffectKind::PlayerHexproof", None)
            }
            PlayerRule::GrantKeyword(PlayerKeyword::Shroud) => (
                "EffectKind::PlayerShroud",
                Some("player rule for a while shroud"),
            ),
            PlayerRule::GrantKeyword(PlayerKeyword::Protection(quality))
                if matches!(
                    &**quality,
                    Protection::Objects {
                        filter: ObjectFilter::Any,
                        ..
                    }
                ) =>
            {
                ("EffectKind::PlayerProtectionFromEverything", None)
            }
            _ => return Ok(false),
        };
        let players = self.fetch(self.players(players)?);
        let source = self.fetch(self.object(source)?);
        let duration = self.duration(duration)?;
        let code = format!(
            "let players_{id}_{i} = {players}; if let Some(source) = {source} {{ for &player in &players_{id}_{i} {{ g.add_player_effect(player, source, {kind}, {duration}); }} }}"
        );
        match feature {
            Some(feature) => self.plan_code(feature, &code, out),
            None => writeln!(out, "{code}").unwrap(),
        }
        Ok(true)
    }
    /// "Spells your opponents cast cost {2} more until your next turn" (CR 611.2c,
    /// 601.2f): the nth of the card's `lasting_cost_changes`, whatever becomes of
    /// its source. False if this is no such rule.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cost_for_a_while(
        &mut self,
        id: usize,
        i: usize,
        source: &ObjectRef,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<bool> {
        let PlayerRule::SpellCost { spells, modifier } = rule else {
            return Ok(false);
        };
        let refused = || gap("player rule for a while SpellCost");
        // The callback lasts past the body: it can name nothing the body made. The
        // filter reader takes any state it does not know for "enchanted", so a state
        // on a spell ("face-down spells") stays a gap.
        let words = format!("{spells:?}");
        let bound = [
            "Chosen",
            "Target",
            "Result",
            "Selected",
            "Earlier",
            "Transition",
            "AbilityController",
            "State(",
        ];
        if !self.stack_item || bound.iter().any(|w| words.contains(w)) {
            return Err(refused());
        }
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let delta = self.cost_delta(modifier);
        let test = self.filter(
            spells,
            &Candidate {
                announced: Some("targets".into()),
                ..super::players::cast_card()
            },
        );
        (self.reader, self.event_at) = saved;
        let (delta, test) = (delta?, test?);
        // Its asker is the player it changes the spells of, not the controller.
        if delta.contains("asker") || delta.contains("q.") || test.contains("asker") {
            return Err(refused());
        }
        let function = format!("lasting_cost_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ let asker = &Asker::new(source, player); let Costed::Spell(spell) = costed else {{ return CostDelta::NONE; }}; if {test} {{ {delta} }} else {{ CostDelta::NONE }} }}").unwrap();
        let text = if matches!(modifier, SpellCostModifier::Increase(_)) {
            "These spells cost more to cast."
        } else {
            "These spells cost less to cast."
        };
        let index = self.made_cost_changes.len();
        self.made_cost_changes.push(format!(
            "LastingCostChange {{ text: {}, change: {function}, next: None }}",
            quoted(text)
        ));
        let players = self.fetch(self.players(players)?);
        let source = self.fetch(self.object(source)?);
        let duration = self.duration(duration)?;
        writeln!(out, "let players_{id}_{i} = {players}; if let Some(source) = {source} {{ for &player in &players_{id}_{i} {{ g.add_player_effect(asker.controller, source, EffectKind::SpellCostChange {{ card: item.ability.card, index: {index}, player }}, {duration}); }} }}").unwrap();
        Ok(true)
    }
    /// "You may look at that card for as long as it remains exiled": information a
    /// resolved effect opens (CR 400.2). False if this is no such rule.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn look_for_a_while(
        &mut self,
        id: usize,
        i: usize,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<bool> {
        let PlayerRule::Visibility {
            objects,
            audience: VisibilityAudience::AffectedPlayer,
        } = rule
        else {
            return Ok(false);
        };
        let expiry = play_expiry(duration)?;
        // "Until end of turn, you may look at the top card of your library any
        // time" (Gwenom, The Belligerent): whatever card is on top as it changes,
        // a rule for the player for a while. `allow_look` is for one object, and
        // `PlayRule::LookAtLibraryTop` is a static ability's only.
        if top_of_their(objects, EventZone::Library) {
            return Err(gap("a lasting rule to look at the top card of a library"));
        }
        let cards = self.fetch(self.objects(objects)?);
        let players = self.fetch(self.players(players)?);
        // What a player knows of a card face down in exile lasts as long as that
        // object does (CR 406.3), which is "for as long as it remains exiled".
        use crate::ir::Duration as D;
        if matches!(duration, D::WhileExiled(_) | D::WhileAnyExiled(_)) {
            writeln!(out, "let cards_{id}_{i} = {cards}; let players_{id}_{i} = {players}; for &player in &players_{id}_{i} {{ g.look_at(player, &cards_{id}_{i}); }}").unwrap();
            return Ok(true);
        }
        self.plan_code(
            "a card a player may look at for a while",
            &format!("let cards_{id}_{i} = {cards}; let players_{id}_{i} = {players}; for &card in &cards_{id}_{i} {{ for &player in &players_{id}_{i} {{ g.allow_look(card, player, {expiry}); }} }}"),
            out,
        );
        Ok(true)
    }
    /// "Your choice of flying or vigilance": one of the card's own words, kept for
    /// the `ChosenAbility` that follows in the same body.
    pub(super) fn choose_ability(
        &mut self,
        id: usize,
        player: &str,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let failure = || gap("a choice of Ability");
        if !self.stack_item || (self.entering_labels.is_some() && self.choice_labels.is_empty()) {
            return Err(failure());
        }
        let mut abilities = Vec::new();
        let mut labels = Vec::new();
        for option in options {
            let ability = match option {
                Operand::Ability(ability) => (**ability).clone(),
                Operand::Term { form, operands, .. } => {
                    hasable_keyword(form, operands).ok_or_else(failure)?
                }
                _ => return Err(failure()),
            };
            match &ability {
                Ability::Keyword(keyword) => labels.push(match keyword {
                    Keyword::Protection(quality) => match &**quality {
                        Protection::Objects {
                            filter: ObjectFilter::Color(color),
                            ..
                        } => format!("protection from {}", name(color).to_lowercase()),
                        _ => return Err(failure()),
                    },
                    Keyword::Ward(_) => return Err(failure()),
                    other => camel_words(&form(other)),
                }),
                other => match self.grants_keyword(other) {
                    Some(keyword) => {
                        labels.push(keyword.replacen("Keyword::", "", 1));
                    }
                    None => return Err(failure()),
                },
            }
            abilities.push(ability);
        }
        let base = self.choice_labels.len();
        let end = base + labels.len();
        self.choice_labels.extend(labels);
        let quoted_labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
        self.entering_labels = Some(format!("&[{}]", quoted_labels.join(", ")));
        self.ability_choice = abilities;
        writeln!(
            out,
            "let player_{id} = {player}; let chosen_{id} = cantrip_mechanics::choose::option(g, player_{id}, asker.source, item.ability.card, {base}..{end})? - {base};"
        )
        .unwrap();
        Ok(())
    }
    /// `Change::ChosenAbility`: the one effect the choice asks for.
    pub(super) fn chosen_ability(&mut self) -> Result<Vec<String>> {
        let local = self.chosen_local(ValueKind::Ability)?;
        let options = self.ability_choice.clone();
        let last = options.len().saturating_sub(1);
        let mut arms = Vec::new();
        for (i, ability) in options.iter().enumerate() {
            let kinds = match ability {
                Ability::Keyword(keyword) => {
                    self.effect_kinds(&Change::GrantKeyword(keyword.clone()))?
                }
                other => match self.grants_keyword(other) {
                    Some(keyword) => {
                        vec![format!("EffectKind::AddKeywords(Keywords::of(&[{keyword}]))")]
                    }
                    None => return Err(gap("continuous change ChosenAbility")),
                },
            };
            let [kind] = kinds.as_slice() else {
                return Err(gap("continuous change ChosenAbility"));
            };
            let pattern = if i == last {
                "_".to_string()
            } else {
                i.to_string()
            };
            arms.push(format!("{pattern} => {kind},"));
        }
        Ok(vec![format!("match {local} {{ {} }}", arms.join(" "))])
    }
    /// A closure over `player`: whether the rule is about them now.
    /// Which players a rule of the card's tables is about: a `fn` of the card's
    /// own the entry holds, asked of a player as the rule is read.
    fn player_test(&mut self, players: &PlayerSet, feature: &str, stem: &str) -> Result<String> {
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let params = "q: &dyn Query, asker: &Asker, player: PlayerId";
        Ok(self.rule_fn(feature, stem, params, "bool", &format!("{guard}{member}")))
    }
    /// "Play with the top card of your library revealed", "you may look at cards
    /// exiled with this": information a rule opens (CR 400.2, 401.5, 402.3).
    fn visibility(
        &mut self,
        players: &PlayerSet,
        objects: &ObjectSet,
        audience: &VisibilityAudience,
        parts: &mut Parts,
    ) -> Result<()> {
        let affected_zone = |set: &ObjectSet, wanted: &EventZone| {
            matches!(
                set,
                ObjectSet::Zone {
                    scope: ZoneScope::Players { zone, players: PlayerSet::One(PlayerRef::Affected) },
                    filter: ObjectFilter::Any,
                    ..
                } if zone == wanted
            )
        };
        let top = matches!(
            objects,
            ObjectSet::Take { objects, count: Expr::Constant(1), from: SequenceEnd::Top }
                if affected_zone(objects, &EventZone::Library)
        );
        let everyone = matches!(audience, VisibilityAudience::Everyone);
        if top {
            const FEATURE: &str = "a library's top card open to view";
            let applies = self.player_test(players, FEATURE, "library_top_open")?;
            let rule = if everyone {
                "PlayRule::LibraryTopRevealed"
            } else {
                "PlayRule::LooksAtLibraryTop"
            };
            self.plan_entry(
                FEATURE,
                "play_rules",
                &format!("{rule}({applies})"),
                parts,
            );
            return Ok(());
        }
        if everyone && affected_zone(objects, &EventZone::Hand) {
            const FEATURE: &str = "a hand played revealed";
            let applies = self.player_test(players, FEATURE, "hand_revealed")?;
            self.plan_entry(
                FEATURE,
                "play_rules",
                &format!("PlayRule::HandRevealed({applies})"),
                parts,
            );
            return Ok(());
        }
        if everyone {
            return Err(gap(format!("player rule Visibility {}", form(objects))));
        }
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let cards = self.objects(objects)?;
        const FEATURE: &str = "cards a player may look at";
        let params = "q: &dyn Query, asker: &Asker, player: PlayerId, card: ObjectId";
        let body = format!("{guard}{member} && ({cards}).contains(&card)");
        let applies = self.rule_fn(FEATURE, "may_look_at", params, "bool", &body);
        self.plan_entry(FEATURE, "play_rules", &format!("PlayRule::MayLookAt({applies})"), parts);
        Ok(())
    }
    /// "You can't cast more than one spell each turn": the spells this player
    /// has cast this turn, read from the history, whenever the board was.
    fn cast_limit(
        &mut self,
        label: &str,
        players: &PlayerSet,
        filter: &ObjectFilter,
        n: i64,
        parts: &mut Parts,
    ) -> Result<()> {
        fn past(filter: &ObjectFilter) -> Option<String> {
            match filter {
                ObjectFilter::Any => Some("true".to_string()),
                ObjectFilter::CardType(t) => {
                    Some(format!("c.is(CardType::{})", card_type(t).ok()?))
                }
                ObjectFilter::Not(inner) => Some(format!("!({})", past(inner)?)),
                ObjectFilter::Subtype(kind) => Some(format!(
                    "c.chars(q).has_subtype(q.words(asker.source).subtype(Subtype::{}))",
                    name(kind)
                )),
                _ => None,
            }
        }
        let Some(counted) = past(filter) else {
            return Err(gap(format!("player rule LimitPerTurn {}", form(filter))));
        };
        let test = self.filter(filter, &super::players::cast_card())?;
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        self.permission_entry(
            None,
            "spell_rules",
            &format!("{label}_rule"),
            ("q: &dyn Query, asker: &Asker, player: PlayerId, spell: ObjectId", "", "bool"),
            &format!("!({guard}{member} && {test} && q.history().casts_by(player).filter(|c| c.spell != spell && {counted}).count() >= {n})"),
            |function| function.to_string(),
            parts,
        );
        Ok(())
    }
    /// "You may spend mana as though it were mana of any color to ..." (CR 609.4b).
    fn spend_mana_as(
        &mut self,
        label: &str,
        players: &PlayerSet,
        spend_as: &SpendAs,
        on: &ManaPayment,
        parts: &mut Parts,
    ) -> Result<()> {
        // Cantrip's spending rules recolor mana only (`spend_any_color`); "as though
        // it were mana of any type" lets colorless or snow-less mana pay what its
        // type wouldn't, which no rule reads (Sharkey, Tyrant of the Shire).
        if matches!(spend_as, SpendAs::AnyType) {
            return Err(gap("mana spent as though of any type"));
        }
        let test = match on {
            ManaPayment::Anything => "true".to_string(),
            ManaPayment::Spells(filter) => format!(
                "s.purpose == SpendingFor::Casting && s.source.is_some_and(|spell| {})",
                self.filter(filter, &super::players::cast_card())?
            ),
            ManaPayment::Abilities(ObjectFilter::AbilityOf(objects)) => format!(
                "s.purpose == SpendingFor::Activating && s.source.is_some_and(|id| {})",
                self.filter(objects, &Candidate::of("id"))?
            ),
            other => return Err(gap(format!("player rule SpendManaAs {}", form(other)))),
        };
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let body = format!("let player = s.player; {guard}{member} && {test}");
        let (table, feature): (&'static str, _) = match spend_as {
            SpendAs::AnyColor => ("spend_any_color", None),
            other => return Err(gap(format!("player rule SpendManaAs {}", form(other)))),
        };
        self.permission_entry(
            feature,
            table,
            &format!("{label}_rule"),
            (
                "q: &dyn Query, asker: &Asker, s: &Spending",
                "q, asker, s",
                "bool",
            ),
            &body,
            |function| function.to_string(),
            parts,
        );
        Ok(())
    }
    /// "You may activate ... as though it had haste", "any time you could cast an
    /// instant", "twice each turn rather than only once" (CR 602.5).
    fn activation_grant(
        &mut self,
        label: &str,
        players: &PlayerSet,
        abilities: &ObjectFilter,
        grant: &ActivationGrant,
        parts: &mut Parts,
    ) -> Result<()> {
        let function = format!("{label}_rule");
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let plain = |parts: &mut Parts, c: &mut Self, table, params, body: &str| {
            c.permission_entry(
                None,
                table,
                &function,
                (params, "", "bool"),
                body,
                |f| f.to_string(),
                parts,
            );
        };
        const ASKED_ABILITY: &str = "let def = q.activated_ability(ability);";
        const ABILITY_PARAMS: &str =
            "q: &dyn Query, asker: &Asker, player: PlayerId, permanent: ObjectId, ability: AbilityRef";
        match grant {
            ActivationGrant::AsThoughHaste => {
                let ObjectFilter::AbilityOf(objects) = abilities else {
                    return Err(gap(format!(
                        "player rule ActivationGrant {}",
                        form(abilities)
                    )));
                };
                if !matches!(players, PlayerSet::One(PlayerRef::AbilityController)) {
                    return Err(gap("as though haste for other players"));
                }
                let test = self.filter(objects, &Candidate::of("creature"))?;
                plain(
                    parts,
                    self,
                    "activate_as_though_haste",
                    "q: &dyn Query, asker: &Asker, creature: ObjectId",
                    &format!("{guard}{test}"),
                );
            }
            ActivationGrant::InstantSpeed | ActivationGrant::InstantSpeedDuring(PlayerSet::All) => {
                let test = self.ability_test(abilities)?;
                plain(
                    parts,
                    self,
                    "instant_activation",
                    "q: &dyn Query, asker: &Asker, source: ObjectId, index: u8, player: PlayerId",
                    &format!("let Some(ability) = q.chars(source).abilities().nth(index as usize) else {{ return false; }}; {ASKED_ABILITY} let permanent = source; {guard}{member} && {test}"),
                );
            }
            ActivationGrant::AdditionalTimes(Expr::Constant(n))
                if guard.is_empty()
                    && matches!(players, PlayerSet::One(PlayerRef::AbilityController))
                    && format!("{abilities:?}").contains("PowerUp")
                    && u8::try_from(*n).is_ok() =>
            {
                parts.field("extra_power_up_activations", n.to_string());
            }
            ActivationGrant::AdditionalTimes(Expr::Constant(n)) => {
                let test = self.ability_test(abilities)?;
                const FEATURE: &str = "an ability activated more times each turn";
                let body = format!("{ASKED_ABILITY} {guard}{member} && {test}");
                let applies = self.rule_fn(FEATURE, &format!("{label}_abilities"), ABILITY_PARAMS, "bool", &body);
                self.plan_entry(
                    FEATURE,
                    "extra_activations",
                    &format!("ExtraActivations {{ abilities: {applies}, times: {n} }}"),
                    parts,
                );
            }
            ActivationGrant::AsThoughNotActivated => {
                let test = self.ability_test(abilities)?;
                const FEATURE: &str = "an ability activated as though it had not been";
                let body = format!("{ASKED_ABILITY} {guard}{member} && {test}");
                let applies = self.rule_fn(FEATURE, &format!("{label}_abilities"), ABILITY_PARAMS, "bool", &body);
                self.plan_entry(FEATURE, "activate_again", &applies, parts);
            }
            other => return Err(gap(format!("player rule ActivationGrant {}", form(other)))),
        }
        Ok(())
    }
    /// The objects a layer callback reads the abilities of. Those exiled with the
    /// source are its own to name, read as `Agatha's Soul Cauldron` reads them.
    pub(super) fn ids_in_layer(&self, objects: &ObjectSet) -> Result<String> {
        if let ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Exile),
            filter,
            ..
        } = objects
        {
            let items = match filter {
                ObjectFilter::All(items) => items.as_slice(),
                other => std::slice::from_ref(other),
            };
            let exiled_with =
                |f: &ObjectFilter| matches!(f, ObjectFilter::ExiledWith(ObjectRef::Source));
            if items.iter().any(exiled_with) {
                let tests = items
                    .iter()
                    .filter(|f| !exiled_with(f))
                    .map(|f| self.filter(f, &Candidate::of("id")))
                    .collect::<Result<Vec<_>>>()?;
                let keep = if tests.is_empty() {
                    "true".to_string()
                } else {
                    tests.join(" && ")
                };
                // "The exiled cards used to craft it" are linked to the craft card
                // as it waited in exile, which became this (CR 702.167c), not to an
                // ability of this object.
                if self.line_text().is_some_and(|line| line.contains("used to craft")) {
                    return Ok(format!("cantrip_mechanics::craft::used_to_craft(q.state(), source).into_iter().filter(|&id| {keep}).collect::<Vec<ObjectId>>()"));
                }
                return Ok(format!("q.exiled_with(q.linked_to(source)).into_iter().filter(|&id| {keep}).collect::<Vec<ObjectId>>()"));
            }
        }
        self.objects(objects)
    }
    /// "Has all activated abilities of ...", "as long as a card exiled with this has
    /// flying, this has flying. The same is true for ..." (CR 613.1f, layer 6).
    pub(super) fn gain_abilities_of(
        &mut self,
        change: &Change,
        spend_handled: bool,
    ) -> Result<statics::LayerChange> {
        let failure = || gap("static change GainAbilitiesOf");
        // The activated abilities a permanent has are its characteristics as the
        // layers leave them (CR 613.1f), not its card's printed list, which is all
        // `grant_activated_abilities_of` reads (Drana and Linvala, Scheming Fence,
        // Sharkey, Experiment Kraj, Marvin, Robaran Mercenaries, Manascape
        // Refractor): read each through the view, which in layer 6 is the object
        // as the layer's effects applied so far leave it (CR 613.8, `LayerQuery::chars`).
        // "You may spend mana as though it were mana of any color to activate those
        // abilities" is a spending rule the caller writes (`spend_handled`).
        if let Change::GainAbilitiesOf { objects, kinds, once_each_turn, spend_as } = change {
            let on_battlefield = matches!(objects, crate::battlefield_query!(_));
            let asked = format!("{kinds:?}");
            let permanents = on_battlefield || asked.contains("AbilityOfPermanent");
            let activated = asked.contains("ActivatedAbility") || asked.contains("AbilityOfPermanent");
            if permanents && activated {
                if *once_each_turn || (spend_as.is_some() && !spend_handled) {
                    return Err(gap("the activated abilities of other permanents"));
                }
                fn kept(kinds: &Operand) -> Option<String> {
                    match kinds {
                        Operand::List(items) => items
                            .iter()
                            .map(kept)
                            .collect::<Option<Vec<_>>>()
                            .map(|tests| format!("({})", tests.join(" || "))),
                        Operand::Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
                            ("ActivatedAbility", []) | ("AbilityOfPermanent", [_]) => Some("true".into()),
                            ("NonManaAbility", []) => Some("!d.is_mana_ability()".into()),
                            ("NonLoyaltyAbility", []) => Some("!d.is_loyalty_ability()".into()),
                            ("And", parts) => parts
                                .iter()
                                .map(kept)
                                .collect::<Option<Vec<_>>>()
                                .map(|tests| format!("({})", tests.join(" && "))),
                            _ => None,
                        },
                        _ => None,
                    }
                }
                let Some(keep) = kept(kinds) else {
                    return Err(gap("the activated abilities of other permanents"));
                };
                let ids = self.ids_in_layer(objects)?;
                // Each ability is the object's own as written, so it is the
                // ability's source and "this" means it (CR 113.7, 602.1); a special
                // action is no ability (CR 116.2). A permanent doesn't gain its own,
                // and what has left the battlefield has none to give.
                return Ok(statics::LayerChange::of(
                    "Ability",
                    format!(
                        "let ids: Vec<ObjectId> = {ids}; for id in ids {{ if id == source || !q.state().obj(id).zone.is(ZoneKind::Battlefield) {{ continue; }} let other = q.chars(id); for r in other.abilities() {{ if matches!(q.cards().ability(r), AbilityDef::Activated(d) if !d.special_action && {keep}) {{ if let Some(slot) = chars.abilities.iter().position(|a| a.is_none()) {{ chars.abilities[slot] = Some(r); }} }} }} }}"
                    ),
                ));
            }
        }
        let Change::GainAbilitiesOf {
            objects,
            kinds,
            once_each_turn,
            spend_as: None,
        } = change
        else {
            return Err(failure());
        };
        fn words(kinds: &Operand, out: &mut Vec<String>) -> bool {
            match kinds {
                Operand::List(items) => items.iter().all(|item| words(item, out)),
                Operand::Term { form, operands, .. } if operands.is_empty() => {
                    out.push(form.clone());
                    true
                }
                _ => false,
            }
        }
        let mut kinds_named = Vec::new();
        if !words(kinds, &mut kinds_named) {
            return Err(failure());
        }
        let ids = self.ids_in_layer(objects)?;
        let body = match kinds_named.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["ActivatedAbility"] if *once_each_turn => {
                // "You may activate each of those abilities only once each turn"
                // (CR 602.5b): one limited instance for each of the cards that
                // prints the ability, as they are when it would be activated.
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                let asked = self.ids_in_layer(objects);
                (self.reader, self.event_at) = saved;
                let asked = asked?;
                if self.granted_once.borrow().is_some() {
                    return Err(failure());
                }
                *self.granted_once.borrow_mut() = Some(format!(
                    "let asker = &Asker::new(source, q.controller(source)); let ids: Vec<ObjectId> = {asked}; ids.into_iter().filter(|&id| q.state().obj(id).card == ability.card).count()"
                ));
                format!(
                    "let cards: Vec<CardId> = ({ids}).into_iter().map(|id: ObjectId| q.state().obj(id).card).collect(); cantrip_mechanics::statics::grant_activated_abilities_of(q, &cards, chars);"
                )
            }
            _ if *once_each_turn => return Err(failure()),
            ["ActivatedAbility"] => format!(
                "let cards: Vec<CardId> = ({ids}).into_iter().map(|id: ObjectId| q.state().obj(id).card).collect(); cantrip_mechanics::statics::grant_activated_abilities_of(q, &cards, chars);"
            ),
            ["LoyaltyAbility"] => format!(
                "let ids = {ids}; {};",
                planned::planned_value(
                    "the loyalty abilities of other permanents",
                    "()",
                    "cantrip_mechanics::statics::grant_loyalty_abilities_of(q, &ids, chars)",
                )
            ),
            // "Has all activated and triggered abilities of the exiled card"
            // (Idris, CR 613.1f): each such ability its card prints, the
            // object's own as written (CR 113.7); the core asks triggered
            // abilities of what an object has (`Chars::abilities`).
            ["ActivatedAbility", "TriggeredAbility"] | ["TriggeredAbility", "ActivatedAbility"] => format!(
                "let cards: Vec<CardId> = ({ids}).into_iter().map(|id: ObjectId| q.state().obj(id).card).collect(); for card in cards {{ for (index, ability) in q.cards().get(card).abilities.iter().enumerate() {{ if matches!(ability, AbilityDef::Activated(a) if !a.special_action) || matches!(ability, AbilityDef::Triggered(_)) {{ if let Some(slot) = chars.abilities.iter().position(|a| a.is_none()) {{ chars.abilities[slot] = Some(AbilityRef {{ card, index: index as u8 }}); }} }} }} }}"
            ),
            // Ability stickers (Unfinity, CR 123.4): Cantrip has no stickers.
            names if names.iter().any(|n| n.contains("Sticker")) => {
                return Err(gap("the abilities of ability stickers"))
            }
            names if names.iter().any(|n| n.ends_with("Ability")) => return Err(failure()),
            names => {
                let list = names
                    .iter()
                    .map(|n| format!("Keyword::{n}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "let ids = {ids}; {};",
                    planned::planned_value(
                        "the keywords of other objects",
                        "()",
                        &format!("cantrip_mechanics::statics::grant_keywords_of(q, &ids, &[{list}], chars)"),
                    )
                )
            }
        };
        Ok(statics::LayerChange::of("Ability", body))
    }
    /// "Creature spells can't be cast this turn" (Moonhold, Cease Fire): which
    /// spells a resolving effect forbids, asked of each spell as it would be cast
    /// (CR 611.2c), is a rule of the card's `lasting_spell_rules`, as
    /// `lasting_rules` hold one of permanents; the effect names its index, and
    /// what the words named as it resolved (a target) is its `about`: what else
    /// the resolution chose (a name) is a gap of its own, as for `lasting_rules`.
    fn lasting_spell_rule(&mut self, feature: &str, filter: &ObjectFilter) -> Result<String> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let asked = self.filter(filter, &super::players::cast_card());
        (self.reader, self.event_at) = saved;
        // What the resolution names is read as it has it, then handed over.
        let test = match asked {
            Ok(test) => test,
            Err(_) => self.filter(filter, &super::players::cast_card())?,
        };
        // "That spell" of a trigger (Living Breakthrough): its subject.
        const SUBJECT: &str = "item.subject";
        let (test, subject) = match test.contains(SUBJECT) {
            true => (test.replace(SUBJECT, "about"), true),
            false => (test, false),
        };
        // A rule reading what the resolution chose but a target can't keep it.
        let (test, about) = self.named_as_resolved(&test)?;
        let about = match (subject, about.as_str()) {
            (false, _) => about,
            (true, "Target::None") => SUBJECT.to_string(),
            (true, _) => return Err(gap("a lasting rule reading two things its ability chose")),
        };
        let index = self.holes.matches("=> lasting_spell_rules:").count();
        let function = format!("lasting_spell_rule_{index}");
        let params = "q: &dyn Query, asker: &Asker, about: Target, player: PlayerId, spell: ObjectId";
        let body = format!("let source = asker.source; {test}");
        let item = planned::planned_fn(feature, &function, params, "bool", &body);
        writeln!(self.functions, "{item}").unwrap();
        let id = planned::requirement(feature);
        writeln!(self.holes, "planned_item!({id:?} => lasting_spell_rules: {function});").unwrap();
        Ok(format!(
            "EffectKind::CantCast {{ player, card: item.ability.card, index: {index}, about: {about} }}"
        ))
    }
    /// "Until end of turn, target player can't cast spells": a rule about the
    /// player, not an ability of anything (CR 611.2c). False if this is no such rule.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prohibit_for_a_while(
        &mut self,
        id: usize,
        i: usize,
        source: &ObjectRef,
        players: &PlayerSet,
        rule: &PlayerRule,
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<bool> {
        let locked = PlayerAction::GainLife;
        let (action, locked) = match rule {
            PlayerRule::Prohibit(action) => (action, false),
            PlayerRule::LifeTotalLocked => (&locked, true),
            _ => return Ok(false),
        };
        let (kind, feature) = match action {
            PlayerAction::GainLife if locked => {
                ("EffectKind::LifeTotalCantChange(player)".to_string(), None)
            }
            PlayerAction::GainLife => ("EffectKind::CantGainLife(player)".to_string(), None),
            PlayerAction::CastSpell {
                from: None,
                filter: ObjectFilter::Any,
            } => (
                "EffectKind::CantCastSpellsFrom(player, ZoneKinds::ALL)".to_string(),
                None,
            ),
            PlayerAction::CastOutside(zone) => (
                format!(
                    "EffectKind::CantCastSpellsFrom(player, ZoneKinds::all_but(&[{}]))",
                    zone_kind(zone)?
                ),
                None,
            ),
            // "Can't cast spells from their graveyards" (Shaman's Trance): from
            // that zone of any player, CR 601.3.
            PlayerAction::CastSpell {
                from:
                    Some(ZoneScope::Players {
                        zone,
                        players: PlayerSet::All,
                    }),
                filter: ObjectFilter::Any,
            } => (
                format!(
                    "EffectKind::CantCastSpellsFrom(player, ZoneKinds::of(&[{}]))",
                    zone_kind(zone)?
                ),
                None,
            ),
            PlayerAction::CastSpell { from: None, filter } => {
                const FEATURE: &str = "player rule for a while can't cast some spells";
                (self.lasting_spell_rule(FEATURE, filter)?, Some(FEATURE))
            }
            PlayerAction::PlayLand {
                from: None,
                filter: ObjectFilter::Any,
            } => (
                "EffectKind::CantPlayLands(player)".to_string(),
                Some("player rule for a while can't play lands"),
            ),
            PlayerAction::LoseGame { reason: None } => (
                "EffectKind::CantLoseGame(player)".to_string(),
                Some("player rule for a while can't lose the game"),
            ),
            PlayerAction::WinGame => (
                "EffectKind::CantWinGame(player)".to_string(),
                Some("player rule for a while can't win the game"),
            ),
            PlayerAction::LoseLife => (
                "EffectKind::CantLoseLife(player)".to_string(),
                Some("player rule for a while can't lose life"),
            ),
            PlayerAction::SearchLibraries => (
                "EffectKind::CantSearchLibraries(player)".to_string(),
                Some("player rule for a while can't search libraries"),
            ),
            PlayerAction::BecomeMonarch => (
                "EffectKind::CantBecomeMonarch(player)".to_string(),
                Some("player rule for a while can't become the monarch"),
            ),
            other => {
                return Err(gap(format!(
                    "player rule for a while Prohibit {}",
                    form(other)
                )))
            }
        };
        let players = self.fetch(self.players(players)?);
        let source = self.fetch(self.object(source)?);
        let duration = self.duration(duration)?;
        let code = format!(
            "let players_{id}_{i} = {players}; if let Some(source) = {source} {{ for &player in &players_{id}_{i} {{ g.add_player_effect(asker.controller, source, {kind}, {duration}); }} }}"
        );
        match feature {
            Some(feature) => self.plan_code(feature, &code, out),
            None => writeln!(out, "{code}").unwrap(),
        }
        Ok(true)
    }
    /// What the rule asks of the ability being activated, `def` its definition
    /// (none for the core's own) and `permanent` its source. "Of a creature" means a
    /// permanent (109.2) unless the words name a zone or say "sources".
    pub(super) fn ability_test(&self, filter: &ObjectFilter) -> Result<String> {
        let def = |test: &str| format!("def.is_some_and(|d| {test})");
        Ok(match filter {
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(filters) | ObjectFilter::Either(filters) => {
                let tests = filters
                    .iter()
                    .map(|f| self.ability_test(f))
                    .collect::<Result<Vec<_>>>()?;
                let joint = if matches!(filter, ObjectFilter::All(_)) {
                    " && "
                } else {
                    " || "
                };
                format!("({})", tests.join(joint))
            }
            ObjectFilter::Not(inner) => format!("!({})", self.ability_test(inner)?),
            ObjectFilter::OnStack(StackTrait::Mana) => def("d.is_mana_ability()"),
            ObjectFilter::OnStack(StackTrait::Loyalty) => def("d.is_loyalty_ability()"),
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Equip)) => def("d.equip"),
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::PowerUp)) => {
                def("d.power_up")
            }
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Exhaust)) => def("d.exhaust"),
            ObjectFilter::AbilityOf(objects) => {
                let test = self.filter(objects, &Candidate::of("permanent"))?;
                let words = format!("{objects:?}");
                if words.contains("InZone") || words.contains("Chosen(") {
                    test
                } else {
                    format!("(def.is_some_and(|d| d.from == ZoneKind::Battlefield) && {test})")
                }
            }
            ObjectFilter::TargetsAny(_) => self.filter(
                filter,
                &Candidate {
                    announced: Some("targets".into()),
                    ..Candidate::of("permanent")
                },
            )?,
            other => return self.ability_stated(other),
        })
    }
    /// "Activated abilities of creatures you control cost {1} less to activate"
    /// (CR 601.2f, 118.7), which the core asks of every ability but a mana ability.
    fn activation_cost(
        &mut self,
        label: &str,
        players: &PlayerSet,
        abilities: &ObjectFilter,
        change: &ActivationCostChange,
        floor: Option<i64>,
        parts: &mut Parts,
    ) -> Result<()> {
        let test = self.ability_test(abilities)?;
        let less = |n: &str| match floor {
            None => Ok(format!("CostDelta::less({n})")),
            Some(1) => Ok(format!("CostDelta::less_to_one({n})")),
            Some(_) => Err(gap("activation cost reduction to another floor")),
        };
        let mut feature = None;
        let delta = match change {
            ActivationCostChange::Reduce(amount) => {
                let n = generic_amount(amount)
                    .ok_or_else(|| gap("activation cost reduction of colored mana"))?;
                less(&n.to_string())?
            }
            ActivationCostChange::ReduceX { amount, times }
                if format!("{amount:?}").contains("CostReduceX") =>
            {
                // A cost's generic mana is 32 bits; `less_to_one` keeps a byte.
                let times = self.number(times)?;
                less(&match floor {
                    None => times.amount(u32::MAX.into(), "u32")?,
                    _ => times.fit(0, 255, "u8")?,
                })?
            }
            ActivationCostChange::ReduceX { .. } => {
                return Err(gap("activation cost reduction of colored mana"))
            }
            ActivationCostChange::Increase(amount) => match &**amount {
                Operand::Cost(Cost { parts: priced }) => match priced.as_slice() {
                    [CostPart::Mana(symbols)] => match symbols.as_slice() {
                        [ManaSymbol::Generic(n)] if u8::try_from(*n).is_ok() => {
                            format!("CostDelta::more({n})")
                        }
                        _ => return Err(gap("activation cost increase of colored mana")),
                    },
                    [CostPart::Loyalty(Expr::Constant(n))] if u8::try_from(*n).is_ok() => {
                        feature = Some("activation cost of an additional loyalty");
                        format!("CostDelta {{ loyalty: {n}, ..CostDelta::NONE }}")
                    }
                    _ => return Err(gap("activation cost increase other than mana")),
                },
                other => {
                    let n = generic_amount(other)
                        .ok_or_else(|| gap("activation cost increase of colored mana"))?;
                    format!("CostDelta::more({n})")
                }
            },
            ActivationCostChange::Additional(cost) => {
                feature = Some("activation cost with another cost");
                let cost = self.cost(cost, &format!("{label}_also"))?;
                format!("CostDelta {{ also: {cost}, ..CostDelta::NONE }}")
            }
        };
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let body = format!(
            "{PRELUDE} let Costed::Ability(permanent, ability, _) = costed else {{ return CostDelta::NONE; }}; let def = q.activated_ability(ability); if {guard}{member} && {test} {{ {delta} }} else {{ CostDelta::NONE }}"
        );
        self.permission_entry(
            feature,
            "cost_changes",
            &format!("{label}_rule"),
            ("q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]", "q, source, player, costed, targets", "CostDelta"),
            &body,
            |function| function.to_string(),
            parts,
        );
        Ok(())
    }
    /// An entry of a table, and the function it asks: a closure of the same words
    /// where the entry waits on `feature`, as uncompiled code must not reach `functions`.
    pub(super) fn permission_entry(
        &mut self,
        feature: Option<&str>,
        table: &'static str,
        function: &str,
        (params, names, returns): (&str, &str, &str),
        body: &str,
        entry: impl Fn(&str) -> String,
        parts: &mut Parts,
    ) {
        let _ = names;
        match feature {
            // The same `fn`, an item of the hole until the table lands: a table
            // holds no closure.
            Some(feature) => {
                let item = planned::planned_fn(feature, function, params, returns, body);
                writeln!(self.functions, "{item}").unwrap();
                self.plan_entry(feature, table, &entry(function), parts);
            }
            None => {
                writeln!(
                    self.functions,
                    "#[allow(unused_variables)] fn {function}({params}) -> {returns} {{ {body} }}"
                )
                .unwrap();
                parts.table(table, entry(function));
            }
        }
    }
    /// What a play permission asks whatever zone it opens: while what, on whose
    /// turn, how often. `card` is the card asked of, `player` who may play it.
    fn play_tests(
        &self,
        action: &CardPlayAction,
        from: &EventZone,
        terms: &PlayTerms,
        card: &str,
        player: &str,
    ) -> Result<(Vec<String>, Option<&'static str>)> {
        let mut tests = Vec::new();
        let mut feature = None;
        if let Some(condition) = &terms.condition {
            tests.push(format!("({})", self.condition(condition)?));
        }
        match &terms.during {
            None => {}
            Some(PlayerSet::One(PlayerRef::AbilityController | PlayerRef::Affected)) => {
                tests.push(format!("q.state().turn.active == {player}"))
            }
            Some(other) => return Err(gap(format!("play permission during {}", form(other)))),
        }
        match terms.limit {
            None => {}
            Some(PlayLimit::EachTurn(1)) => tests.push(
                if matches!((from, action), (EventZone::Exile, CardPlayAction::CastSpell)) {
                    // A land played by a permission beside this one is no spell.
                    "!q.history().this_turn().iter().any(|r| matches!(r.event, Event::Cast(c) if c.granted_by().is_some_and(|g| g.object == asker.source)))".to_string()
                } else {
                    "!cantrip_mechanics::history::permission_used_this_turn(q, asker.source)"
                        .to_string()
                },
            ),
            Some(PlayLimit::OnePerCardType(_)) => {
                feature = Some("play permission of one card of each permanent type");
                tests.push(format!("!cantrip_mechanics::history::permanent_type_used_this_turn(q, asker.source, {card})"));
            }
            Some(_) => return Err(gap("play permission for a number of cards")),
        }
        Ok((tests, feature))
    }
    /// "You may play lands from your graveyard", "you may cast spells from among
    /// cards exiled with this": permissions a permanent gives while it is there
    /// (CR 601.3, 305.1, 604.1), each in the table for the zone it opens.
    fn static_play(
        &mut self,
        label: &str,
        players: &PlayerSet,
        rule: &PlayerRule,
        parts: &mut Parts,
    ) -> Result<()> {
        let plain = plain_terms();
        let (permission, terms) = match rule {
            PlayerRule::PlayPermissionWith { permission, terms } => (&**permission, terms),
            other => (other, &plain),
        };
        let PlayerRule::PlayPermission {
            cards,
            from,
            action,
            filter,
        } = permission
        else {
            return Err(gap(format!("player rule {}", form(rule))));
        };
        if !matches!(
            players,
            PlayerSet::One(PlayerRef::AbilityController) | PlayerSet::All
        ) {
            return Err(gap(format!("play permission for {}", form(players))));
        }
        if !terms.effects.is_empty() {
            return Err(gap(
                "static play permission with an effect on what is played",
            ));
        }
        if terms.may_look || terms.then.is_some() || terms.face_down.is_some() {
            return Err(gap("static play permission on other terms"));
        }
        let own = |set: &PlayerSet| {
            matches!(
                set,
                PlayerSet::One(PlayerRef::AbilityController | PlayerRef::Affected)
            )
        };
        match (from, cards) {
            (
                EventZone::Graveyard,
                ObjectSet::Zone {
                    scope:
                        ZoneScope::Players {
                            zone: EventZone::Graveyard,
                            players: scope,
                        },
                    filter: within,
                    ..
                },
            ) => {
                let own = own(scope) || format!("{within:?}").contains("InZone");
                self.graveyard_play(label, players, action, within, filter, terms, own, parts)
            }
            (
                EventZone::Exile,
                ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Exile),
                    filter: within,
                    ..
                },
            ) => self.exile_play(label, players, action, within, filter, terms, parts),
            (
                EventZone::Hand,
                ObjectSet::Zone {
                    scope:
                        ZoneScope::Players {
                            zone: EventZone::Hand,
                            players: scope,
                        },
                    filter: within,
                    ..
                },
            ) if own(scope) => self.hand_play(label, players, action, within, filter, terms, parts),
            (
                EventZone::Library,
                ObjectSet::Take {
                    objects,
                    count: Expr::Constant(1),
                    from: SequenceEnd::Top,
                },
            ) if matches!(players, PlayerSet::One(_))
                && matches!(
                    &**objects,
                    ObjectSet::Zone {
                        scope: ZoneScope::Players { zone: EventZone::Library, players: scope },
                        filter: ObjectFilter::Any,
                        ..
                    } if own(scope)
                ) =>
            {
                self.library_top_play(label, action, filter, terms, parts)
            }
            _ => Err(gap(format!("play permission {}", form(cards)))),
        }
    }
    /// Who, and which cards: a permission's tests that are the same in each table.
    pub(super) fn permission_tests(
        &self,
        players: &PlayerSet,
        within: &ObjectFilter,
        filter: &ObjectFilter,
        card: &str,
    ) -> Result<Vec<String>> {
        let mut tests = vec![self.player_member(players)?];
        if let Some(condition) = &self.static_guard {
            tests.push(format!("({})", self.condition(condition)?));
        }
        for filter in [within, filter] {
            if !matches!(filter, ObjectFilter::Any) {
                tests.push(self.filter(filter, &Candidate::of(card))?);
            }
        }
        Ok(tests)
    }
    /// `GrantedWay`: only the caster's own graveyard, unless `own` says it is not.
    #[allow(clippy::too_many_arguments)]
    fn graveyard_play(
        &mut self,
        label: &str,
        players: &PlayerSet,
        action: &CardPlayAction,
        within: &ObjectFilter,
        filter: &ObjectFilter,
        terms: &PlayTerms,
        own: bool,
        parts: &mut Parts,
    ) -> Result<()> {
        if terms.spend_as.is_some() {
            return Err(gap("static play permission on other terms"));
        }
        let mut tests = self.permission_tests(players, within, filter, "card")?;
        let (more, mut feature) =
            self.play_tests(action, &EventZone::Graveyard, terms, "card", "player")?;
        tests.extend(more);
        let lands = match action {
            CardPlayAction::Either => true,
            CardPlayAction::CastSpell => false,
            CardPlayAction::PlayLand => {
                tests.push("q.chars(card).is(CardType::Land)".to_string());
                true
            }
        };
        const PRINTED: &str = "Cost::mana(q.card(q.chars(card).card).cost)";
        let (price, permission) = match &terms.cost {
            PlayCost::Normal => (PRINTED.to_string(), true),
            PlayCost::Free => ("Cost::NONE".to_string(), false),
            PlayCost::Alternate(Operand::Cost(price)) => {
                (self.cost(price, &format!("{label}_cost"))?, false)
            }
            PlayCost::Additional(Operand::Cost(price)) => (
                format!(
                    "{PRINTED}.and({})",
                    self.cost(price, &format!("{label}_cost"))?
                ),
                false,
            ),
            _ => return Err(gap("play permission for another cost")),
        };
        let text = quoted(&self.said(lane_k::Said::Sentence, "Cast from your graveyard".into()));
        let mut fields = format!("text: {text}, permission: {permission}, lands: {lands}, ");
        if terms.flash {
            fields.push_str("instant_speed: true, ");
        }
        if terms.into_exile {
            fields.push_str("flashback: true, ");
        }
        if !own {
            feature = Some("play permission from every graveyard");
            fields.push_str("any_graveyard: true, ");
        }
        let body = format!("({}).then(|| {price})", tests.join(" && "));
        self.permission_entry(
            feature,
            "granted_ways",
            &format!("{label}_cost"),
            (
                "q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId",
                "q, asker, card, player",
                "Option<Cost>",
            ),
            &body,
            |cost| format!("GrantedWay {{ {fields}cost: {cost}, ..GrantedWay::CAST }}"),
            parts,
        );
        Ok(())
    }
    /// `ExileCast`: a permission and no way of paying.
    #[allow(clippy::too_many_arguments)]
    fn exile_play(
        &mut self,
        label: &str,
        players: &PlayerSet,
        action: &CardPlayAction,
        within: &ObjectFilter,
        filter: &ObjectFilter,
        terms: &PlayTerms,
        parts: &mut Parts,
    ) -> Result<()> {
        if terms.into_exile {
            return Err(gap("static play permission on other terms"));
        }
        // "The cards they don't own exiled with this": of the player asked.
        let outer = std::mem::replace(&mut self.rule_player, true);
        let tests = self.permission_tests(players, within, filter, "card");
        let played = self.play_tests(action, &EventZone::Exile, terms, "card", "player");
        self.rule_player = outer;
        let mut tests = tests?;
        let (more, mut feature) = played?;
        tests.extend(more);
        let lands = match action {
            CardPlayAction::Either => true,
            CardPlayAction::CastSpell => false,
            CardPlayAction::PlayLand => {
                tests.push("q.chars(card).is(CardType::Land)".to_string());
                true
            }
        };
        let (additional, life) = match &terms.cost {
            PlayCost::Normal => ("Cost::NONE".to_string(), false),
            PlayCost::Additional(Operand::Cost(price)) => {
                (self.cost(price, &format!("{label}_cost"))?, false)
            }
            PlayCost::Alternate(Operand::Cost(price))
                if matches!(price.parts.as_slice(), [CostPart::LifeEqualToOwnManaValue]) =>
            {
                ("Cost::NONE".to_string(), true)
            }
            _ => return Err(gap("play permission for another cost")),
        };
        let mut any_type = false;
        let mut extra = String::new();
        match &terms.spend_as {
            None => {}
            Some(SpendAs::AnyType) => any_type = true,
            Some(SpendAs::AnyColor) => {
                feature = Some("play permission with mana as though any color");
                extra.push_str(", any_color: true");
            }
            Some(other) => {
                feature = Some("play permission with one mana as though any color");
                write!(extra, ", spend_as: SpendAs::{}", form(other)).unwrap();
            }
        }
        let text = if lands {
            "Play from exile"
        } else {
            "Cast from exile"
        };
        let body = tests.join(" && ");
        // "And you may cast them as though they had flash": what may be cast this way
        // may be cast then (`FlashRule`, asked of the card in the zone it's cast from).
        if terms.flash {
            self.permission_entry(
                None,
                "flash_rules",
                &format!("{label}_flash"),
                ("q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId", "q, asker, card, player", "bool"),
                &body,
                |rule| rule.to_string(),
                parts,
            );
        }
        self.permission_entry(
            feature,
            "exile_casts",
            &format!("{label}_allows"),
            ("q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId", "q, asker, card, player", "bool"),
            &body,
            |allows| format!("ExileCast {{ text: \"{text}\", allows: {allows}, additional: {additional}, any_type: {any_type}, lands: {lands}, life_for_mana_value: {life}{extra} }}"),
            parts,
        );
        Ok(())
    }
    /// `SpellAlternative`: a cost other than the mana cost, for cards in a hand.
    #[allow(clippy::too_many_arguments)]
    fn hand_play(
        &mut self,
        label: &str,
        players: &PlayerSet,
        action: &CardPlayAction,
        within: &ObjectFilter,
        filter: &ObjectFilter,
        terms: &PlayTerms,
        parts: &mut Parts,
    ) -> Result<()> {
        if !matches!(action, CardPlayAction::CastSpell)
            || terms.spend_as.is_some()
            || terms.into_exile
        {
            return Err(gap("static play permission on other terms"));
        }
        // The data has the once and not whose turns it's during, which the line says.
        let own_turns = matches!(terms.limit, Some(PlayLimit::EachTurn(_))) && terms.during.is_none();
        if own_turns
            && !self.line_text().is_some_and(|line| line.contains("during each of your turns"))
        {
            return Err(gap("play permission from a hand once each turn"));
        }
        let (cost, text) = match &terms.cost {
            PlayCost::Free => (
                "InsteadOfManaCost::WithoutPaying".to_string(),
                "Without paying its mana cost".to_string(),
            ),
            PlayCost::Alternate(Operand::Cost(price)) => match price.parts.as_slice() {
                [CostPart::Energy(Expr::Constant(n))] if u8::try_from(*n).is_ok() => (
                    format!("InsteadOfManaCost::Energy({n})"),
                    format!(
                        "Pay {} rather than its mana cost",
                        "{E}".repeat(*n as usize)
                    ),
                ),
                [CostPart::Mana(symbols)] => (
                    format!("InsteadOfManaCost::Mana({})", mana(symbols)?),
                    "Pay mana rather than its mana cost".to_string(),
                ),
                _ => return Err(gap("play permission for another cost")),
            },
            _ => return Err(gap("play permission for another cost")),
        };
        // Told apart from another card's by what it says.
        let text = match own_turns {
            true => self.said(lane_k::Said::Sentence, text),
            false => text,
        };
        let mut tests = self.permission_tests(players, within, filter, "spell")?;
        let (more, feature) =
            self.play_tests(action, &EventZone::Hand, terms, "spell", "player")?;
        tests.extend(more);
        if own_turns {
            tests.push("q.state().turn.active == player".to_string());
        }
        // An alternative cost applies wherever the spell may be cast from, unless
        // the card says where.
        if self.says_from_hand() {
            tests.push("q.obj(spell).zone == Zone::hand(player)".to_string());
        }
        let body = tests.join(" && ");
        let flash = terms.flash;
        self.permission_entry(
            feature,
            "spell_alternatives",
            &format!("{label}_applies"),
            ("q: &dyn Query, asker: &Asker, spell: ObjectId, player: PlayerId", "q, asker, spell, player", "bool"),
            &body,
            |applies| format!("SpellAlternative {{ text: \"{text}\", applies: {applies}, cost: {cost}, flash: {flash} }}"),
            parts,
        );
        Ok(())
    }
    /// `PlayRule`: the top card of the library of the permanent's controller.
    fn library_top_play(
        &mut self,
        label: &str,
        action: &CardPlayAction,
        filter: &ObjectFilter,
        terms: &PlayTerms,
        parts: &mut Parts,
    ) -> Result<()> {
        if terms.spend_as.is_some() || terms.into_exile {
            return Err(gap("static play permission on other terms"));
        }
        let mut tests = Vec::new();
        if !matches!(filter, ObjectFilter::Any) {
            tests.push(self.filter(filter, &Candidate::of("card"))?);
        }
        let (more, feature) = self.play_tests(
            action,
            &EventZone::Library,
            terms,
            "card",
            "asker.controller",
        )?;
        if feature.is_some() {
            return Err(gap("play permission for a number of cards"));
        }
        tests.extend(more);
        let test = if tests.is_empty() {
            "true".to_string()
        } else {
            tests.join(" && ")
        };
        let card_params = "q: &dyn Query, asker: &Asker, card: ObjectId";
        match action {
            CardPlayAction::Either => {
                return Err(gap(
                    "play permission from the top of a library on other terms",
                ))
            }
            CardPlayAction::PlayLand => {
                if test == "true" && matches!(terms.cost, PlayCost::Normal) {
                    return self.play_rule(label, "PlayRule::LibraryTopLands".into(), parts);
                }
                const FEATURE: &str = "play permission for lands from the top of a library with a filter";
                let applies = self.rule_fn(FEATURE, &format!("{label}_cards"), card_params, "bool", &test);
                self.plan_entry(
                    FEATURE,
                    "play_rules",
                    &format!("PlayRule::LibraryTopLandsIf({applies})"),
                    parts,
                );
            }
            CardPlayAction::CastSpell => {
                let extra = match &terms.cost {
                    PlayCost::Normal => "Cost::NONE".to_string(),
                    PlayCost::Additional(Operand::Cost(price)) => {
                        self.cost(price, &format!("{label}_cost"))?
                    }
                    _ => return Err(gap("play permission for another cost")),
                };
                if extra == "Cost::NONE" && !terms.flash {
                    let function = format!("{label}_rule");
                    writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, card: ObjectId) -> bool {{ {test} }}").unwrap();
                    return self.play_rule(
                        label,
                        format!("PlayRule::LibraryTopSpells({function}, None)"),
                        parts,
                    );
                }
                const FEATURE: &str = "play permission from the top of a library on other terms";
                let applies = self.rule_fn(FEATURE, &format!("{label}_cards"), card_params, "bool", &test);
                self.plan_entry(
                    FEATURE,
                    "play_rules",
                    &format!("PlayRule::LibraryTopSpellsWith({applies}, LibraryTerms {{ additional: {extra}, flash: {} }})", terms.flash),
                    parts,
                );
            }
        }
        Ok(())
    }
    /// A static ability that functions from a zone other than the battlefield
    /// (CR 113.6): a permission to cast the card itself, a rule about the spell,
    /// or an ability of a card in a graveyard.
    pub(super) fn scoped_static(
        &mut self,
        zones: &[EventZone],
        condition: Option<&Condition>,
        ability: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let failure = || gap(format!("source-zone scope on {}", form(ability)));
        match (ability, zones) {
            // Corrupted flashback: a card's own way from its graveyard, on a condition.
            (
                Ability::Casting(rule @ CastingRule::Permission { .. }),
                [EventZone::Graveyard],
            ) => return self.casting(rule, label, parts, condition),
            // Nothing on the card: the spell it names counts it (`read::counted_as_named`).
            (Ability::CountsAsNamed { name, for_spells }, [EventZone::Graveyard])
                if condition.is_none() && name == for_spells =>
            {
                return Ok(());
            }
            _ => {}
        }
        let Ability::Static {
            affected, changes, ..
        } = ability
        else {
            return Err(failure());
        };
        if let (
            EffectSubjects::Players(PlayerSet::One(PlayerRef::AbilityController)),
            [Change::PlayerRule(rule)],
        ) = (affected, changes.as_slice())
        {
            let (permission, terms) = match rule {
                PlayerRule::PlayPermissionWith { permission, terms } => {
                    (&**permission, Some(terms))
                }
                other => (other, None),
            };
            if let PlayerRule::PlayPermission {
                cards: ObjectSet::One(ObjectRef::Source),
                from,
                action: CardPlayAction::CastSpell,
                filter: ObjectFilter::Any,
            } = permission
            {
                return match self.own_cast(from, condition, terms, label, parts) {
                    Err(failed) if failed.feature == "cast the card itself on these terms" => {
                        self.stated_own_cast(from, condition, terms, label, failed, parts)
                    }
                    other => other,
                };
            }
        }
        match zones {
            [EventZone::Stack] => {
                let [Change::Rule(RuleChange::Prohibit(action))] = changes.as_slice() else {
                    return Err(failure());
                };
                let EffectSubjects::Objects(ObjectSet::Zone { filter, .. }) = affected else {
                    return Err(failure());
                };
                if !matches!(filter, ObjectFilter::SameObject(ObjectRef::Source)) {
                    return Err(failure());
                }
                let (word, text) = match action {
                    ObjectAction::Counter => ("CantBeCountered", "This spell can't be countered."),
                    ObjectAction::Copy => ("CantBeCopied", "This spell can't be copied."),
                    _ => return Err(failure()),
                };
                let Some(condition) = condition else {
                    parts.keywords.push(format!("Keyword::{word}"));
                    return Ok(());
                };
                // What it says of itself holds wherever it is: only the spell is asked.
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Layer, EventAt::Nowhere);
                let test = self.condition(condition);
                (self.reader, self.event_at) = saved;
                let function = format!("{label}_apply");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn LayerQuery, source: ObjectId, chars: &mut Chars) {{ let asker = &Asker::new(source, q.controller(source)); if {} {{ chars.keywords = chars.keywords.with(Keyword::{word}); }} }}", test?).unwrap();
                parts.statics.push(format!(
                    "characteristic_defining(Layer::Ability, {function}, {})",
                    quoted(text)
                ));
                Ok(())
            }
            [EventZone::Graveyard] => {
                if self.static_guard.is_some() {
                    return Err(failure());
                }
                let mut scoped = Parts::default();
                self.static_guard = condition.cloned();
                let compiled = self.ability(ability, label, &mut scoped, None);
                self.static_guard = None;
                compiled?;
                let only_statics = scoped.keywords.is_empty()
                    && scoped.abilities.is_empty()
                    && scoped.replacements.is_empty()
                    && scoped.preventions.is_empty()
                    && scoped.enters.is_empty()
                    && scoped.block_rules.is_empty()
                    && scoped.block_requirements.is_empty()
                    && scoped.attack_rules.is_empty()
                    && scoped.untap_bans.is_empty()
                    && scoped.tables.is_empty()
                    && scoped.fields.is_empty();
                if !only_statics {
                    return Err(failure());
                }
                for made in scoped.statics {
                    parts
                        .statics
                        .push(format!("{made}.from(ZoneKind::Graveyard)"));
                }
                Ok(())
            }
            _ => Err(failure()),
        }
    }
    /// "You may cast this card from your graveyard" or "from exile": its own say-so,
    /// which works where it is (CR 113.6b, 113.6e).
    fn own_cast(
        &mut self,
        from: &EventZone,
        condition: Option<&Condition>,
        terms: Option<&PlayTerms>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let refused = || gap("cast the card itself on these terms");
        let (cost, exile) = match terms {
            Some(terms) => {
                if terms.limit.is_some()
                    || terms.during.is_some()
                    || terms.spend_as.is_some()
                    || terms.condition.is_some()
                    || !terms.effects.is_empty()
                    || terms.flash
                    || terms.may_look
                    || terms.then.is_some()
                    || terms.face_down.is_some()
                {
                    return Err(refused());
                }
                (&terms.cost, terms.into_exile)
            }
            None => (&PlayCost::Normal, false),
        };
        match from {
            EventZone::Exile => {
                if !matches!(cost, PlayCost::Normal) || exile {
                    return Err(refused());
                }
                let test = match condition {
                    Some(condition) => format!(
                        " && {}(q, asker)",
                        self.activation_condition(condition, &format!("{label}_if"))?
                    ),
                    None => String::new(),
                };
                let function = format!("{label}_allows");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId) -> bool {{ card == asker.source && player == asker.controller{test} }}").unwrap();
                parts.table(
                    "exile_casts",
                    format!("ExileCast {{ text: \"Cast from exile\", allows: {function}, additional: Cost::NONE, any_type: false, lands: false, life_for_mana_value: false }}"),
                );
                Ok(())
            }
            EventZone::Graveyard => {
                let printed = |c: &Self| -> Result<CostPart> {
                    let symbols = c.printed_mana.clone().ok_or_else(refused)?;
                    Ok(CostPart::Mana(symbols))
                };
                let price = match cost {
                    PlayCost::Normal if condition.is_none() && !exile => {
                        parts.keywords.push("Keyword::CastFromYourGraveyard".into());
                        return Ok(());
                    }
                    PlayCost::Using(CostedKeyword::Bestow) if condition.is_none() && !exile => {
                        parts.field("bestow_from_graveyard", "true".into());
                        return Ok(());
                    }
                    PlayCost::Normal => Cost {
                        parts: vec![printed(self)?],
                    },
                    PlayCost::Alternate(Operand::Cost(price)) => price.clone(),
                    PlayCost::Additional(Operand::Cost(price)) => {
                        let mut all = vec![printed(self)?];
                        all.extend(price.parts.iter().cloned());
                        Cost { parts: all }
                    }
                    _ => return Err(refused()),
                };
                let price = self.cost(&price, label)?;
                let mut way = format!(
                    "way({price}, ZoneKind::Graveyard, \"Cast from your graveyard\").only_from()"
                );
                if exile {
                    way.push_str(".exile_after()");
                }
                if let Some(condition) = condition {
                    let test = self.activation_condition(condition, &format!("{label}_if"))?;
                    write!(way, ".condition({test})").unwrap();
                }
                // A way is an alternative cost (CR 118.9): one that pays the mana
                // cost shuts out Fist of Suns or Dream Halls, which the permission
                // leaves open. Only the printed keyword is a permission of its own.
                if matches!(cost, PlayCost::Normal | PlayCost::Additional(_)) {
                    let feature = "engine: a card's own terms to cast it from its graveyard";
                    self.plan_entry(feature, "ways", &way, parts);
                } else {
                    parts.table("ways", way);
                }
                Ok(())
            }
            _ => Err(refused()),
        }
    }
    /// "You may have this enter as a copy of ...": the choice of what it copies
    /// is one with the choice to copy (CR 707.2), and what the body says next is
    /// what doing so does.
    pub(super) fn copy_as_enters(
        &mut self,
        decider: &PlayerRef,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        let mut flat = Vec::new();
        lane_m::flat_groups(&body.instructions, &mut flat);
        let [Instruction::Effect {
            result,
            operation:
                Operation::ModifyEvent {
                    change:
                        EventChange::Entry(EntryState::CopyOf {
                            objects,
                            effects,
                            until,
                        }),
                },
        }, rest @ ..] = flat.as_slice()
        else {
            return Err(gap("may enter as a copy"));
        };
        let PlayerRef::ControllerOf(entering) = decider else {
            return Err(gap("may enter as a copy, asked of another player"));
        };
        if !matches!(**entering, ObjectRef::EventSubject) {
            return Err(gap("may enter as a copy, asked of another player"));
        }
        let id = self.fresh();
        writeln!(
            out,
            "const ASK_{id}: Question = Question::new(\"CopyAsEnters\", \"enter as a copy?\", \"You choose what it copies next. If you don't, it enters as itself\", \"Choose a copy\", \"Enter as itself\"); let candidates_{id}: Vec<ObjectId> = ({}).into_iter().filter(|&id| id != entering.object).collect(); if let Some(chosen_{id}) = cantrip_mechanics::copy::choose_copy_of_one_of(g, entering, &candidates_{id}, &ASK_{id})? {{",
            self.fetch(self.objects(objects)?)
        )
        .unwrap();
        let values = self.copy_values(
            &format!("Some(chosen_{id})"),
            std::slice::from_ref(&**effects),
        )?;
        writeln!(
            out,
            "entering.copy = {values}; let picked_{id} = vec![chosen_{id}];"
        )
        .unwrap();
        self.selections.insert(result.0, format!("picked_{id}"));
        // "The copied creature": one object, which "when you do" carries.
        self.single.insert(result.0);
        self.notes.earlier.push(("Selected", result.clone()));
        if let Some(until) = until {
            let until = self.duration(until)?;
            self.plan_code(
                "copy as it enters for a duration",
                &format!("entering.copy_lasts = {until};"),
                out,
            );
        }
        let mut after = String::new();
        self.block(
            &Program {
                instructions: rest.to_vec(),
            },
            &mut after,
            true,
        )?;
        out.push_str(&copy_types_as_it_enters(&after, "entering.copy.as_ref()"));
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// What it is as it enters, its own copiable values changed.
    fn copiable_entry(&mut self, changes: &[Change], out: &mut String) -> Result<()> {
        // "And has changeling", of what it chose to enter as a copy of.
        if let [Change::Copiable(list)] = changes {
            if let [Change::GrantAbility(ability)] = list.as_slice() {
                if lane_n::every_creature_type(ability) {
                    // Changeling is a characteristic-defining ability (CR 702.73a),
                    // which the copy has as an exception (Omni-Changeling).
                    writeln!(
                        out,
                        "entering.copy = entering.copy.map(|values| values.with_keyword(Keyword::Changeling));"
                    )
                    .unwrap();
                    return Ok(());
                }
            }
        }
        let mut values = "q.copiable_values(entering.object)".to_string();
        for change in changes {
            let Change::Copiable(list) = change else {
                return Err(gap("event change as it enters Copiable mixed"));
            };
            for change in list {
                match change {
                    Change::SetBasePowerToughness {
                        power: Expr::Constant(power),
                        toughness: Expr::Constant(toughness),
                    } => write!(values, ".sized({power}, {toughness})").unwrap(),
                    Change::AddCardType(kind) => {
                        write!(values, ".with_type(CardType::{})", name(kind)).unwrap()
                    }
                    Change::GrantKeyword(word) => {
                        write!(values, ".with_keyword({})", keyword(word)?).unwrap()
                    }
                    Change::CreatureTypes {
                        mode: ChangeMode::Add,
                        types: CreatureTypeSet::Listed(types),
                    } => {
                        for kind in types {
                            let word = name(kind);
                            if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                                return Err(gap(format!("subtype {word}")));
                            }
                            write!(values, ".with_subtype(Subtype::{word})").unwrap();
                        }
                    }
                    other => {
                        return Err(gap(format!(
                            "event change as it enters Copiable {}",
                            form(other)
                        )))
                    }
                }
            }
        }
        writeln!(out, "entering.copy = Some({});", self.fetch(values)).unwrap();
        Ok(())
    }
    /// "As this enters, choose ...": one option of the labels of the card.
    pub(super) fn entry_option(
        &mut self,
        labels: Vec<String>,
        arms: Vec<String>,
        out: &mut String,
    ) -> Result<()> {
        if self.entering_labels.is_some() {
            return Err(gap("two choices of labels as it enters"));
        }
        let count = labels.len();
        let quoted_labels: Vec<String> = labels.iter().map(|l| quoted(l)).collect();
        self.entering_labels = Some(format!("&[{}]", quoted_labels.join(", ")));
        writeln!(
            out,
            "cantrip_mechanics::choose::option_as_enters(g, entering, &CARD, 0..{count})?; match entering.chosen_option.map(|chosen| chosen.index) {{"
        )
        .unwrap();
        for (i, arm) in arms.iter().enumerate() {
            writeln!(out, "Some({i}) => {{ {arm} }}").unwrap();
        }
        writeln!(out, "_ => {{}} }}").unwrap();
        Ok(())
    }
    /// How `Entry` says the permanent enters, past tapped and counters.
    pub(super) fn entry_state(&mut self, state: &EntryState, out: &mut String) -> Result<()> {
        match state {
            EntryState::Prepared => writeln!(out, "entering.prepared = true;").unwrap(),
            EntryState::Changes(changes) if all_copiable(changes) => {
                self.copiable_entry(changes, out)?
            }
            EntryState::ChooseChanges(options) => {
                let mut labels = Vec::new();
                let mut arms = Vec::new();
                for option in options {
                    let mut arm = String::new();
                    self.copiable_entry(option, &mut arm)?;
                    arms.push(arm);
                    labels.push(entry_label(option));
                }
                self.entry_option(labels, arms, out)?;
            }
            EntryState::Changes(changes) => self.entering_becomes(changes, None, out)?,
            EntryState::ChangesUntil { changes, duration } => {
                self.entering_becomes(changes, Some(duration), out)?
            }
            EntryState::CountersOfChoice {
                kinds,
                amount: Expr::Constant(amount),
                distinct,
            } => {
                let mut options = Vec::new();
                match (*distinct, *amount) {
                    (false, 1) => {
                        for kind in kinds {
                            options.push(vec![kind]);
                        }
                    }
                    (true, 2) => {
                        for (i, one) in kinds.iter().enumerate() {
                            for other in &kinds[i + 1..] {
                                options.push(vec![one, other]);
                            }
                        }
                    }
                    _ => return Err(gap("event change as it enters a number of chosen counters")),
                }
                let mut labels = Vec::new();
                let mut arms = Vec::new();
                for option in options {
                    let mut arm = String::new();
                    let mut label = Vec::new();
                    for kind in option {
                        let code = counter_kind(kind)?;
                        write!(arm, "entering.counters[{code} as usize] = entering.counters[{code} as usize].saturating_add(1); ").unwrap();
                        label.push(form(kind));
                    }
                    arms.push(arm);
                    labels.push(label.join(" and "));
                }
                self.entry_option(labels, arms, out)?;
            }
            // One choice for each: "for each card exiled this way".
            EntryState::CountersOfChoice {
                kinds,
                amount,
                distinct: false,
            } => {
                let amount = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                let kinds = kinds
                    .iter()
                    .map(counter_kind)
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                self.plan_code(
                    "event change as it enters Entry CountersOfChoice",
                    &format!(
                        "let many = {}; for _ in 0..many {{ cantrip_mechanics::choose::counter_as_enters(g, &mut *entering, &[{kinds}])?; }}",
                        self.fetch(amount)
                    ),
                    out,
                );
            }
            EntryState::CopyOf {
                objects: ObjectSet::One(object),
                effects,
                until: None,
            } => {
                let object = self.object(object)?;
                let values = self.copy_values(&object, std::slice::from_ref(&**effects))?;
                self.plan_code(
                    "event change as it enters Entry CopyOf",
                    &format!("entering.copy = {values};"),
                    out,
                );
            }
            EntryState::CountersOf {
                object: ObjectRef::Named { result, .. },
                one_of_each: false,
            } => {
                let Some(list) = self.selections.get(&result.0).cloned() else {
                    return Err(gap("event change as it enters CountersOf"));
                };
                writeln!(out, "if let Some(&original) = {list}.first() {{ for kind in CounterKind::ALL {{ let count = g.obj(original).counters(kind); entering.counters[kind as usize] = entering.counters[kind as usize].saturating_add(count); }} }}").unwrap();
            }
            EntryState::AttachedToChosen(hosts) => {
                let hosts = self.fetch(self.objects(&ObjectSet::battlefield(hosts.clone()))?);
                let could = planned::planned_value(
                    "what a permanent could be attached to",
                    "true",
                    "q.could_be_attached(object, host)",
                );
                writeln!(
                    out,
                    "let object = entering.object; let hosts: Vec<ObjectId> = {hosts}.into_iter().filter(|&host| {{ let q: &dyn Query = g; {could} }}).collect(); cantrip_mechanics::choose::object_as_enters(g, entering, hosts, cantrip_mechanics::select::Selection::of(cantrip_mechanics::select::Count::UpTo(1), &[]))?; entering.attach = entering.chosen_object;"
                )
                .unwrap();
            }
            other => {
                return Err(gap(format!(
                    "event change as it enters Entry {}",
                    form(other)
                )))
            }
        }
        Ok(())
    }
    /// "It enters with flying": what the effect says it is, on it from the moment
    /// it enters and for as long as it is that object (`Entering::becomes`).
    fn entering_becomes(
        &mut self,
        changes: &[Change],
        duration: Option<&Duration>,
        out: &mut String,
    ) -> Result<()> {
        if let [Change::SetPower(power), Change::SetToughness(toughness)] = changes {
            let power = self
                .number(power)?
                .fit(i16::MIN.into(), i16::MAX.into(), "i16")?;
            let toughness = self
                .number(toughness)?
                .fit(i16::MIN.into(), i16::MAX.into(), "i16")?;
            self.plan_code(
                "event change as it enters a power and toughness read then",
                &format!(
                    "let size = {}; entering.base_power_toughness = Some(size);",
                    self.fetch(format!("({power}, {toughness})"))
                ),
                out,
            );
            return Ok(());
        }
        let mut keywords = Vec::new();
        let mut abilities = Vec::new();
        for change in changes {
            match change {
                Change::GrantKeyword(word) => keywords.push(keyword(word)?),
                Change::GrantAbility(ability) => abilities.push(self.grant(ability)?.to_string()),
                other => {
                    return Err(gap(format!(
                        "event change as it enters Change {}",
                        form(other)
                    )))
                }
            }
        }
        let what = format!(
            "EntersAs {{ abilities: &[{}], keywords: Keywords::of(&[{}]), ..EntersAs::NONE }}",
            abilities.join(", "),
            keywords.join(", ")
        );
        if let Some(duration) = duration {
            let duration = self.duration(duration)?;
            self.plan_code(
                "event change as it enters until a duration",
                &format!("entering.becomes_until = Some(({what}, {duration}));"),
                out,
            );
            return Ok(());
        }
        let id = self.fresh();
        let (earlier_abilities, earlier_keywords) = self.entering_as.clone();
        self.entering_as.0.extend(abilities);
        self.entering_as.1.extend(keywords);
        // An earlier replacement on this card may have applied: both are so.
        let (mut extra, mut chosen) = (String::new(), format!("&WHAT_{id}"));
        if !earlier_abilities.is_empty() || !earlier_keywords.is_empty() {
            extra = format!(
                " static WHAT_{id}_TOO: EntersAs = EntersAs {{ abilities: &[{}], keywords: Keywords::of(&[{}]), ..EntersAs::NONE }};",
                self.entering_as.0.join(", "),
                self.entering_as.1.join(", ")
            );
            chosen = format!(
                "if entering.becomes.is_some() {{ &WHAT_{id}_TOO }} else {{ &WHAT_{id} }}"
            );
        }
        writeln!(
            out,
            "static WHAT_{id}: EntersAs = {what};{extra} entering.becomes = Some(Becomes {{ what: {chosen}, card: {}, words: g.words(entering.object) }});",
            self.fetch(self.source_card())
        )
        .unwrap();
        Ok(())
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
    pub(super) fn object_test(&mut self, function: &str, affected: &ObjectSet) -> Result<()> {
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
    pub(super) fn defended(&self, defenders: &AttackDefenders) -> Result<String> {
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
            AttackDefenders::DefendingPlayers(set) => Ok(format!(
                "cantrip_api::defending_player_of(q.state(), defender).is_some_and(|defending| ({}).contains(&defending))", self.players(set)?
            )),
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
    pub(super) fn targeting_source(&self, filter: &ObjectFilter) -> Result<String> {
        let candidate = Candidate {
            id: "by.source".into(),
            chars: "(*by_source)".into(),
            controller: "by.controller".into(),
            announced: None,
            entry_from: false,
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
            R::DamageStaysMarked => self.object_table("damage_stays", function, affected, parts)?,
            R::LegendRuleExempt => {
                self.object_table("legend_rule_exempt", function, affected, parts)?
            }
            R::Prohibit(ObjectAction::Activate(filter)) => {
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
                let barred = self.barred_activation(filter)?;
                // A creature card in hand is no creature: it may still cycle.
                let here = match affected {
                    crate::battlefield_query!(_) => "q.is_on_battlefield(id) && ",
                    _ => "",
                };
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, object: ObjectId, ability: &ActivatedDef) -> bool {{ let id = object; !({here}{guard}{test} && ({barred})) }}").unwrap();
                parts.table("activation_rules", function.to_string());
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
            R::Prohibit(ObjectAction::Regenerate) if planned::switched_on("regeneration-bans") => {
                self.object_table("regeneration_bans", function, affected, parts)?
            }
            R::Prohibit(ObjectAction::Attack(
                defenders @ (AttackDefenders::Players(_)
                | AttackDefenders::PlayersAndPlaneswalkers(_)
                | AttackDefenders::DefendingPlayers(_)
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
                        // The cost as an ability's is written (Cantrip's
                        // `AttackCost` is the decision's mana); its words if
                        // the generator can't write it yet.
                        let (gaps, functions) = (self.gaps.len(), self.functions.len());
                        let mana_x = self.mana_x;
                        let paid = self.cost(cost, &format!("{function}_cost"));
                        self.mana_x = mana_x;
                        let paid = match paid {
                            Ok(code) => code,
                            Err(_) => {
                                self.gaps.truncate(gaps);
                                self.functions.truncate(functions);
                                quoted(&format!("{cost:?}"))
                            }
                        };
                        self.plan_entry(
                            "static rule CombatCost other than mana",
                            "attack_cost_rules",
                            &format!("AttackPayment {{ applies: {function}, cost: {paid} }}"),
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
                    BlockCapacity::Additional(n) => {
                        self.number(n)?.amount(u32::MAX.into(), "u32")?
                    }
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
            R::BlockWhileTapped => self.object_plan(
                "static rule BlockWhileTapped",
                "blocks_while_tapped",
                function,
                affected,
                parts,
            )?,
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
            // No "has": a rule of this permanent's, not an ability the creature
            // gains (113.1a), so it outlasts the creature's own abilities.
            R::CombatGroup {
                role: CombatRole::BlockedAttacker,
                participants: ObjectFilter::Any,
                others_only: false,
                comparison: Comparison::LessOrEqual,
                amount: Expr::Constant(1),
            } if !matches!(affected, ObjectSet::One(ObjectRef::Source)) => {
                let (guard, test) = (self.guard()?, self.rule_subject(affected, "id")?);
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, id: ObjectId) -> Option<usize> {{ let asker = &Asker::new(source, q.controller(source)); ({guard}{test}).then_some(1) }}").unwrap();
                parts.table("blocker_limits", function.to_string());
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
                // How many it may be with, at least and at most: two numbers a
                // table holds, not a range, which is no `Copy`.
                let count = match comparison {
                    Comparison::Greater => format!("at_least: {}, at_most: u32::MAX", n + 1),
                    Comparison::GreaterOrEqual => format!("at_least: {n}, at_most: u32::MAX"),
                    Comparison::LessOrEqual => format!("at_least: 0, at_most: {n}"),
                    Comparison::Equal => format!("at_least: {n}, at_most: {n}"),
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
                    &format!("CombatGroupRule {{ role: CombatRole::{role}, applies: {function}, counted: {function}_counted, others_only: {others_only}, {count} }}"),
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
            other => return self.stated_rule(function, affected, other, parts),
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
        let text = format!(
            "You may exert this creature as it attacks. When you do, {}",
            ability_text(body)
        );
        let text = quoted(&self.said(lane_k::Said::Sentence, text));
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
    pub(super) fn outside_subject(&self, set: &ObjectSet, c: &Candidate) -> Result<Option<String>> {
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
            // Every spell and ability: a flag, not a rule asked of each.
            self.plan_entry(
                "static rule Prohibit Counter of abilities",
                "abilities_cant_be_countered",
                "true",
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
            Mechanic::SplitSecond => "chars.keywords = chars.keywords.with(Keyword::SplitSecond);",
            Mechanic::Retrace => "chars.keywords = chars.keywords.with(Keyword::Retrace);",
            // A core keyword the rules read as the Saga would be on the
            // battlefield, given ones included (CR 702.155a, 614.12).
            Mechanic::ReadAhead => "chars.keywords = chars.keywords.with(Keyword::ReadAhead);",
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
            // "Can't be attacked" (CR 508.1c): the keyword the attack rules read,
            // as The Aetherspark grants it to itself.
            R::Prohibit(ObjectAction::BeAttacked) => keyword("CantBeAttacked"),
            other => return Err(gap(format!("static rule {}", rule_name(other)))),
        })
    }
}

/// What a permission's effect on what it lets be played waits on. `PlayGrant` has
/// `more` and no less, and `enters_tapped` is of lands; a spell's replacement is a
/// `GrantedWay`'s, which a resolving effect is not.
fn played_effect_hole(effect: &Operand) -> &'static str {
    let term = match effect {
        Operand::List(terms) => terms.first(),
        other => Some(other),
    };
    match term {
        Some(Operand::Term { form, .. }) if form == "DecreaseManaCost" => {
            "engine: a play permission that costs less"
        }
        Some(Operand::Term { form, .. }) if form == "EntersTapped" => {
            "engine: a spell a play permission casts enters tapped"
        }
        Some(Operand::Term { form, .. }) if form.starts_with("EntersWith") => {
            "engine: a spell a play permission casts enters changed"
        }
        _ => "play permission with an effect on what is played",
    }
}

/// The keyword a `CheckHasable` word names, as a choice of abilities lists it.
fn hasable_keyword(word: &str, operands: &[Operand]) -> Option<Ability> {
    let keyword = match (word, operands) {
        ("Flying", []) => Keyword::Flying,
        ("FirstStrike", []) => Keyword::FirstStrike,
        ("Trample", []) => Keyword::Trample,
        ("Landwalk", [Operand::Objects(crate::battlefield_query!(land))]) => {
            Keyword::Landwalk(Box::new(land.clone()))
        }
        _ => return None,
    };
    Some(Ability::Keyword(keyword))
}
/// "The top card of [the affected player's] library": the card on top of the
/// zone of the player a rule is for, whichever card that is as it changes.
fn top_of_their(objects: &ObjectSet, zone: EventZone) -> bool {
    matches!(
        objects,
        ObjectSet::Take { objects, count: Expr::Constant(1), from: SequenceEnd::Top }
            if matches!(&**objects, ObjectSet::Zone { scope: ZoneScope::Players { zone: z, players: PlayerSet::One(PlayerRef::Affected) }, .. } if *z == zone)
    )
}
