//! A card's file the way Cantrip's own are written, for a pull request:
//! behaviour by struct update over the generated `oracle::` constant, the shared
//! recipe where `cantrip-mechanics` has one, the printed text as each ability's
//! text, and the Oracle sentence as a doc comment on what implements it.
//!
//! `module` emits whatever compiles for any card, with characteristics inline.
//! That is the capability scan; a reviewer would block it for overlap with
//! `cantrip-mechanics`. This table is closed instead: an ability no recipe covers
//! is a gap, and the card has no file, because inline code here would be the very
//! thing the review refuses.
use super::{form, gap, Result};
use crate::ir::*;
use crate::mtg_types::CardType;
use std::collections::BTreeSet;
use std::fmt::Write;

/// How a recipe reaches the `Effect` an ability takes.
enum Code {
    /// A shared function that already is an `Effect`.
    Path(String),
    /// A shared effect with an argument, called in place.
    Closure(String),
    /// A named function of the card's own around a shared call.
    Statements(Vec<String>),
}

struct Recipe {
    code: Code,
    imports: Vec<&'static str>,
    /// The rules the doc comment cites beside the Oracle sentence.
    cites: &'static str,
}

/// The ability's controller, the only player these recipes name.
fn controller(players: &PlayerSet) -> bool {
    matches!(players, PlayerSet::One(PlayerRef::AbilityController))
}

fn constant(value: &Expr) -> Result<i64> {
    match value {
        Expr::Constant(n) => Ok(*n),
        other => Err(gap(format!("card file: amount {}", form(other)))),
    }
}

fn first_target(objects: &ObjectRef) -> bool {
    matches!(
        objects,
        ObjectRef::Target {
            slot: Slot(0),
            index: 0
        }
    )
}

/// Which `targets::` constant says this target. Only forms it spells exactly.
fn target_constant(spec: &TargetSpec) -> Result<&'static str> {
    use ObjectFilter as F;
    let simple = spec.chooser.is_none()
        && spec.distribute.is_none()
        && spec.constraints.is_empty()
        && matches!(&spec.count, Count::Exactly(Expr::Constant(1)));
    if !simple {
        return Err(gap("card file: target other than one target chosen by you"));
    }
    let permanent = |domain: &ObjectDomain| {
        domain.kind == ObjectKind::Permanent && domain.zones == [EventZone::Battlefield]
    };
    let all_of = |filters: &[F], wanted: &[CardType]| {
        filters.len() == wanted.len()
            && wanted.iter().all(|w| {
                filters
                    .iter()
                    .any(|f| matches!(f, F::CardType(t) if format!("{t:?}") == format!("{w:?}")))
            })
    };
    match (&spec.kind, &spec.filter, &spec.players) {
        (TargetKind::DamageRecipient, F::Any, None) => Ok("ANY"),
        (TargetKind::Player, _, None) => Ok("PLAYER"),
        (TargetKind::Object(domain), filter, None) if permanent(domain) => match filter {
            F::CardType(CardType::Creature) => Ok("CREATURE"),
            F::CardType(CardType::Land) => Ok("LAND"),
            F::CardType(CardType::Artifact) => Ok("ARTIFACT"),
            F::CardType(CardType::Enchantment) => Ok("ENCHANTMENT"),
            F::Either(filters) if all_of(filters, &[CardType::Artifact, CardType::Enchantment]) => {
                Ok("ARTIFACT_OR_ENCHANTMENT")
            }
            F::Either(filters)
                if all_of(filters, &[CardType::Creature, CardType::Planeswalker]) =>
            {
                Ok("CREATURE_OR_PLANESWALKER")
            }
            other => Err(gap(format!("card file: target {}", form(other)))),
        },
        (kind, filter, _) => Err(gap(format!(
            "card file: target {} of {}",
            form(kind),
            form(filter)
        ))),
    }
}

fn pump(power: i64, toughness: i64, function: &str, cites: &'static str) -> Recipe {
    Recipe {
        code: Code::Path(format!("effects::{function}::<{power}, {toughness}>")),
        imports: vec!["effects"],
        cites,
    }
}

/// The recipe of a whole program, by what it does and to what.
fn recipe(body: &Program, targets: &[TargetSpec]) -> Result<Recipe> {
    let operations: Vec<&Operation> = body
        .instructions
        .iter()
        .map(|instruction| match instruction {
            Instruction::Effect { operation, .. } => Ok(operation),
            other => Err(gap(format!("card file: instruction {}", form(other)))),
        })
        .collect::<Result<_>>()?;
    let in_slot = |recipients: &Recipients| matches!(recipients, Recipients::Targets(Slot(0)));
    match operations.as_slice() {
        [Operation::Draw { players, amount }] if controller(players) => Ok(Recipe {
            code: Code::Closure(format!("effects::draw(g, item, {})", constant(amount)?)),
            imports: vec!["effects"],
            cites: "CR 121.1, 121.2",
        }),
        [Operation::GainLife { players, amount }] if controller(players) => Ok(Recipe {
            code: Code::Closure(format!(
                "effects::gain_life(g, item, {})",
                constant(amount)?
            )),
            imports: vec!["effects"],
            cites: "CR 119.3",
        }),
        [Operation::DealDamage {
            source: ObjectRef::Source,
            recipients,
            amount,
            preventable: true,
        }] if in_slot(recipients) => Ok(Recipe {
            code: Code::Statements(vec![format!(
                "damage_target(g, item, 0, {});",
                constant(amount)?
            )]),
            imports: vec!["damage_target"],
            cites: "CR 120.3, 608.2b",
        }),
        [Operation::DealDamage {
            source: ObjectRef::Source,
            recipients,
            amount,
            preventable: true,
        }, Operation::GainLife {
            players,
            amount: life,
        }] if in_slot(recipients) && controller(players) => Ok(Recipe {
            code: Code::Closure(format!(
                "effects::damage_target_and_gain_life(g, item, {}, {})",
                constant(amount)?,
                constant(life)?
            )),
            imports: vec!["effects"],
            cites: "CR 120.3, 119.3, 608.2b",
        }),
        [Operation::Destroy {
            objects,
            regeneration: true,
        }] if matches!(objects, ObjectSet::One(o) if first_target(o)) => Ok(Recipe {
            code: Code::Statements(vec!["destroy_target(g, item, 0);".into()]),
            imports: vec!["destroy_target"],
            cites: "CR 701.8a, 608.2b",
        }),
        [Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected,
            changes,
            duration: Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        }] => {
            let (power, toughness, also) = match changes.as_slice() {
                [Change::Pump { power, toughness }] => {
                    (constant(power)?, constant(toughness)?, None)
                }
                [Change::Pump { power, toughness }, Change::GrantKeyword(keyword)] => {
                    (constant(power)?, constant(toughness)?, Some(keyword))
                }
                _ => return Err(gap("card file: continuous effect other than a pump")),
            };
            let one =
                matches!(affected, EffectSubjects::Objects(ObjectSet::One(o)) if first_target(o));
            let yours = matches!(affected, EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters)))
                if filters.len() == 2
                    && filters.iter().any(|f| matches!(f, ObjectFilter::CardType(CardType::Creature)))
                    && filters.iter().any(|f| matches!(f, ObjectFilter::ControlledBy(p) if controller(p))));
            let single_creature = matches!(
                targets,
                [spec] if matches!(spec.filter, ObjectFilter::CardType(CardType::Creature))
            );
            match (one && single_creature, yours, also) {
                (true, _, None) => Ok(pump(
                    power,
                    toughness,
                    "target_creature_gets",
                    "CR 611.2a, 514.2, 608.2b",
                )),
                (true, _, Some(Keyword::FirstStrike)) => Ok(pump(
                    power,
                    toughness,
                    "target_creature_gets_and_gains_first_strike",
                    "CR 611.2a, 514.2, 608.2b",
                )),
                (_, true, None) if targets.is_empty() => Ok(pump(
                    power,
                    toughness,
                    "creatures_you_control_get",
                    "CR 611.2a, 611.2c, 514.2",
                )),
                _ => Err(gap("card file: pump of another subject")),
            }
        }
        [operation] => Err(gap(format!("card file: operation {}", form(*operation)))),
        operations => Err(gap(format!("card file: {} instructions", operations.len()))),
    }
}

/// Keyword abilities the content file's `keywords` carry, so no code says them.
fn core_keyword(keyword: &Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Flying
            | Keyword::Haste
            | Keyword::Vigilance
            | Keyword::Trample
            | Keyword::FirstStrike
            | Keyword::DoubleStrike
            | Keyword::Deathtouch
            | Keyword::Lifelink
            | Keyword::Reach
            | Keyword::Defender
            | Keyword::Flash
            | Keyword::Hexproof
            | Keyword::Indestructible
            | Keyword::Menace
            | Keyword::Shroud
    )
}

/// One printed line per non-keyword ability, in order. A reminder is no rules
/// text; a line of keywords alone is an `Ability::Keyword` each.
fn printed_lines(oracle_text: &str) -> Vec<String> {
    const KEYWORD_WORDS: &[&str] = &[
        "flying",
        "haste",
        "vigilance",
        "trample",
        "first strike",
        "double strike",
        "deathtouch",
        "lifelink",
        "reach",
        "defender",
        "flash",
        "hexproof",
        "indestructible",
        "menace",
        "shroud",
    ];
    oracle_text
        .lines()
        .map(without_reminder)
        .filter(|line| !line.is_empty())
        .filter(|line| {
            !line
                .split(',')
                .all(|word| KEYWORD_WORDS.contains(&word.trim().to_lowercase().as_str()))
        })
        .collect()
}

/// A line with its parentheses gone. Cantrip's accepted cards print a rules
/// line bare (CR 207.2: reminder text has no game function).
fn without_reminder(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in line.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The generated constant: `crates/cantrip-oracle/build.rs`'s rule.
pub fn oracle_constant(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars().filter(|c| *c != '\'') {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c.to_ascii_uppercase());
        } else {
            gap = true;
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// The whole file of a card. `None` is a card that needs no file, only its
/// content JSON: nothing in it but core keywords.
pub fn card_file(card: &Card, oracle_text: &str) -> Result<Option<String>> {
    let lines = printed_lines(oracle_text);
    let abilities: Vec<&Ability> = card
        .abilities
        .iter()
        .filter(|ability| !matches!(ability, Ability::Keyword(_)))
        .collect();
    for ability in &card.abilities {
        if let Ability::Keyword(keyword) = ability {
            if !core_keyword(keyword) {
                return Err(gap(format!("card file: keyword {}", form(keyword))));
            }
        }
    }
    if abilities.len() != lines.len() {
        return Err(gap("card file: abilities and printed lines do not align"));
    }
    if abilities.is_empty() {
        return Ok(None);
    }
    let mut imports: BTreeSet<&'static str> = BTreeSet::new();
    let mut triggers: BTreeSet<&'static str> = BTreeSet::new();
    let mut functions = String::new();
    let mut spell = None;
    let mut notes = Vec::new();
    let mut triggered = Vec::new();
    let mut unnamed = 0;
    for (ability, line) in abilities.iter().zip(&lines) {
        match ability {
            Ability::Spell { announcement, body } => {
                if announcement.modes.is_some() {
                    return Err(gap("card file: modes"));
                }
                let recipe = recipe(body, &announcement.targets)?;
                imports.extend(recipe.imports.iter());
                let list = target_list(announcement, &mut imports)?;
                let effect = effect_name(&recipe, "effect", line, &mut functions)?;
                notes.extend(shared_note(&recipe, line));
                spell = Some((list, effect));
            }
            Ability::Triggered {
                event,
                intervening_if: None,
                limits:
                    TriggerLimits {
                        per_turn: None,
                        total: None,
                    },
                announcement,
                body,
            } => {
                if announcement.modes.is_some() {
                    return Err(gap("card file: modes"));
                }
                let trigger = match event {
                    EventSpec::ZoneChange {
                        objects: ObjectFilter::SameObject(ObjectRef::Source),
                        from: None,
                        to: EventZone::Battlefield,
                    } => "THIS_ENTERS",
                    EventSpec::ZoneChange {
                        objects: ObjectFilter::SameObject(ObjectRef::Source),
                        from: Some(EventZone::Battlefield),
                        to: EventZone::Graveyard,
                    } => "THIS_DIES",
                    other => return Err(gap(format!("card file: trigger {}", form(other)))),
                };
                triggers.insert(trigger);
                let recipe = recipe(body, &announcement.targets)?;
                imports.extend(recipe.imports.iter());
                let name = if abilities.len() == 1 {
                    "effect".to_string()
                } else {
                    unnamed += 1;
                    format!("effect_{unnamed}")
                };
                let effect = effect_name(&recipe, &name, line, &mut functions)?;
                notes.extend(shared_note(&recipe, line));
                let mut entry = format!("triggered({trigger}, {effect}, {})", text(line));
                if !announcement.targets.is_empty() {
                    write!(
                        entry,
                        ".targets({})",
                        target_list(announcement, &mut imports)?
                    )
                    .unwrap();
                }
                triggered.push(entry);
            }
            other => return Err(gap(format!("card file: ability {}", form(*other)))),
        }
    }
    let mut mechanics: Vec<String> = imports.iter().map(|s| s.to_string()).collect();
    let mut out = String::from("use cantrip_api::*;\n");
    if !mechanics.is_empty() {
        mechanics.sort();
        match mechanics.as_slice() {
            [one] => writeln!(out, "use cantrip_mechanics::{one};").unwrap(),
            many => writeln!(out, "use cantrip_mechanics::{{{}}};", many.join(", ")).unwrap(),
        }
    }
    if !triggers.is_empty() {
        let names: Vec<&str> = triggers.iter().copied().collect();
        match names.as_slice() {
            [one] => writeln!(out, "use cantrip_mechanics::triggers::{one};").unwrap(),
            many => writeln!(
                out,
                "use cantrip_mechanics::triggers::{{{}}};",
                many.join(", ")
            )
            .unwrap(),
        }
    }
    out.push_str("use cantrip_oracle as oracle;\n\n");
    out.push_str(&functions);
    // A shared effect or closure has no function of its own to document.
    for note in &notes {
        writeln!(out, "{note}").unwrap();
    }
    if let Some((list, effect)) = &spell {
        write!(
            out,
            "pub static CARD: CardDef = CardDef {{\n    spell: spell({list}, {effect}),\n"
        )
        .unwrap();
    } else {
        out.push_str("pub static CARD: CardDef = CardDef {\n");
    }
    match triggered.as_slice() {
        [] => {}
        [one] => writeln!(out, "    abilities: &[{one}],").unwrap(),
        many => {
            out.push_str("    abilities: &[\n");
            for entry in many {
                writeln!(out, "        {entry},").unwrap();
            }
            out.push_str("    ],\n");
        }
    }
    writeln!(out, "    ..oracle::{}\n}};", oracle_constant(&card.name)).unwrap();
    Ok(Some(out))
}

fn text(line: &str) -> String {
    format!("{line:?}")
}

fn quoted_line(line: &str) -> String {
    format!("\"{line}\"")
}

fn target_list(
    announcement: &Announcement,
    imports: &mut BTreeSet<&'static str>,
) -> Result<String> {
    if announcement.targets.is_empty() {
        return Ok("&[]".into());
    }
    imports.insert("targets");
    let targets = announcement
        .targets
        .iter()
        .map(|spec| target_constant(spec).map(|c| format!("targets::{c}")))
        .collect::<Result<Vec<_>>>()?;
    Ok(format!("&[{}]", targets.join(", ")))
}

/// The doc comment of an effect: its Oracle sentence and the rules it follows.
fn doc_comment(line: &str, cites: &str) -> String {
    format!("/// {} ({cites}).", quoted_line(line))
}

/// The doc comment an effect with no function of its own has on the card.
fn shared_note(recipe: &Recipe, line: &str) -> Option<String> {
    (!matches!(recipe.code, Code::Statements(_))).then(|| doc_comment(line, recipe.cites))
}

/// What an ability names as its effect, writing a function where one is needed.
fn effect_name(recipe: &Recipe, name: &str, line: &str, functions: &mut String) -> Result<String> {
    Ok(match &recipe.code {
        Code::Path(path) => path.clone(),
        Code::Closure(call) => format!("|g, item| {call}"),
        Code::Statements(statements) => {
            write!(
                functions,
                "{}\nfn {name}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{\n",
                doc_comment(line, recipe.cites)
            )
            .unwrap();
            for statement in statements {
                writeln!(functions, "    {statement}").unwrap();
            }
            functions.push_str("    Ok(())\n}\n\n");
            name.to_string()
        }
    })
}
