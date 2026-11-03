//! The printed words of a card, for the text Cantrip keeps on each ability. The
//! AST holds no Oracle wording, and a scenario names an ability or mode by what
//! it prints (`Ability: {T}: Add {C}`), so the text comes from the face's Oracle
//! lines, aligned here with its abilities. A line that can't be aligned leaves the
//! generator's own label: a card never fails over a label.
use super::card_file::without_reminder;
use super::*;

/// What a printed line is, by its grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Said {
    /// One keyword of a line of keywords.
    Word,
    /// "Cost: effect", ability word or no.
    Activated,
    /// "When", "Whenever", "At", ability word or no.
    Triggered,
    /// Anything else: a spell's text, a static ability.
    Sentence,
}

/// One ability's printed words.
#[derive(Clone, Debug)]
pub(super) struct Line {
    kind: Said,
    /// The line, reminder text removed, modes joined on as Cantrip's cards have them.
    text: String,
    /// The line as printed above its modes, reminder text removed.
    head: String,
    /// The bulleted lines under it, without their bullets.
    modes: Vec<String>,
    /// What it quotes: the text of an ability it grants or a token has.
    quotes: Vec<(Said, String)>,
    /// A counter stage's power and toughness, "LEVEL 1-4: 2/2".
    stats: bool,
    /// Where a counter stage's line starts past its stage, "LEVEL 1-4: ".
    staged: usize,
    /// A counter stage's line some ability of the stage has said.
    said: bool,
}

/// A face's printed text, in the order it reads.
#[derive(Debug, Default)]
pub(super) struct Printed {
    lines: Vec<Line>,
    /// A Saga's chapters, by number. "I, II — ..." is the text of both.
    chapters: BTreeMap<i64, String>,
    /// What each counter stage prints under its "12+" or "LEVEL 2-6", by the
    /// counters it starts at.
    stages: BTreeMap<i64, Vec<Line>>,
}

fn first_word_is(line: &str, words: &[&str]) -> bool {
    words.iter().any(|word| {
        line.strip_prefix(word)
            .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// A line past its ability word (CR 207.2c), which has no game function.
/// Where the stage a line opens starts: "12+", "LEVEL 2-6", "LEVEL 7+".
fn stage_marker(line: &str) -> Option<i64> {
    let bare = line.strip_prefix("LEVEL ").unwrap_or(line);
    let digits: String = bare.chars().take_while(char::is_ascii_digit).collect();
    let rest = &bare[digits.len()..];
    let ranged = rest
        .strip_prefix('-')
        .is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()));
    match rest == "+" || ranged {
        true => digits.parse().ok(),
        false => None,
    }
}

fn past_ability_word(line: &str) -> &str {
    match line.split_once(" \u{2014} ") {
        Some((word, rest))
            if !word.contains(['.', ':', '"', '{']) && word.split_whitespace().count() <= 6 =>
        {
            rest
        }
        _ => line,
    }
}

fn kind_of(line: &str) -> Said {
    let body = past_ability_word(line);
    if first_word_is(body, &["When", "Whenever", "At"]) {
        return Said::Triggered;
    }
    match body.split_once(": ") {
        Some((cost, _)) if !cost.contains(['.', '"']) => Said::Activated,
        _ => Said::Sentence,
    }
}

/// A line of keywords: no sentence in it, and short between its commas.
fn keyword_line(line: &str) -> bool {
    !line.ends_with(['.', '\u{2014}', '"', ':'])
        && !line.contains(": ")
        && line
            .split([',', ';'])
            .all(|piece| piece.split_whitespace().count() <= 7)
}

/// What an additional cost's line says to do, or the line itself if it isn't one.
fn additional_cost_words(line: &str) -> &str {
    let words = line
        .strip_prefix("As an additional cost to cast this spell, ")
        .unwrap_or(line);
    words
        .split(". ")
        .next()
        .unwrap_or(words)
        .trim_end_matches('.')
}

/// The costs a line offers, each starting with its verb: "or" alone also joins
/// the objects of one ("sacrifice an artifact or creature").
fn cost_options(words: &str) -> Vec<String> {
    const VERBS: [&str; 11] = [
        "pay",
        "sacrifice",
        "discard",
        "blight",
        "behold",
        "exile",
        "tap",
        "reveal",
        "remove",
        "return",
        "put",
    ];
    let mut options: Vec<Vec<&str>> = vec![Vec::new()];
    let tokens: Vec<&str> = words.split(' ').collect();
    for (at, word) in tokens.iter().enumerate() {
        if *word == "or" && tokens.get(at + 1).is_some_and(|next| VERBS.contains(next)) {
            options.push(Vec::new());
            continue;
        }
        let option = options.last_mut().expect("one to start with");
        match word.strip_suffix(',') {
            Some(word) if tokens.get(at + 1).is_some_and(|next| VERBS.contains(next)) => {
                option.push(word);
                options.push(Vec::new());
            }
            _ => option.push(word),
        }
    }
    let options = options.iter().map(|option| option.join(" "));
    options
        .map(|option| capitalized(option.trim_end_matches(',')))
        .collect()
}

/// The first word of `words` as "blighting".
fn gerund(words: &str) -> String {
    let verb = words.split(' ').next().unwrap_or(words);
    match verb {
        "tap" | "put" => format!("{verb}{}ing", &verb[verb.len() - 1..]),
        _ => format!("{}ing", verb.strip_suffix('e').unwrap_or(verb)),
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What a line quotes, in order, each with what it is.
fn quotes(line: &str) -> Vec<(Said, String)> {
    line.split('"')
        .skip(1)
        .step_by(2)
        .map(|quote| (kind_of(quote), quote.to_string()))
        .collect()
}

/// `[+1]: Draw a card.` is printed `+1: Draw a card.`.
fn loyalty_printed(line: &str) -> String {
    match line
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("]: "))
    {
        Some((cost, rest)) => format!("{cost}: {rest}"),
        None => line.to_string(),
    }
}

/// The chapter numbers a line of a Saga starts with: "I, II — ".
fn chapter_numbers(line: &str) -> Option<(Vec<i64>, &str)> {
    let (numbers, rest) = line.split_once(" \u{2014} ")?;
    let numbers = numbers
        .split(", ")
        .map(|number| match number {
            "I" => Some(1),
            "II" => Some(2),
            "III" => Some(3),
            "IV" => Some(4),
            "V" => Some(5),
            "VI" => Some(6),
            "VII" => Some(7),
            _ => None,
        })
        .collect::<Option<Vec<i64>>>()?;
    Some((numbers, rest))
}

/// A line of `kind`. What it quotes is read from `raw`, reminder and all: a
/// token's ability may be printed nowhere else.
fn line_of(kind: Said, text: String, raw: &str) -> Line {
    Line {
        kind,
        quotes: quotes(raw),
        head: text.clone(),
        text,
        modes: Vec::new(),
        stats: false,
        staged: 0,
        said: false,
    }
}

/// The sentences of a printed line, each with its period.
fn sentences(text: &str) -> Vec<String> {
    let sentences = text.split_inclusive(". ").map(|s| s.trim().to_string());
    let sentences: Vec<String> = sentences.filter(|s| !s.is_empty()).collect();
    match sentences.is_empty() {
        true => vec![text.to_string()],
        false => sentences,
    }
}

/// The sentences of a line of `kind` past its cost: an activated ability's
/// cost isn't what it does.
fn effect_sentences(kind: Said, text: &str) -> Vec<String> {
    sentences(past_cost(kind, text))
}

/// A line past its activation cost, which is no word of the effect: "Sacrifice
/// this creature: ...". A triggered ability's "When ...:" is no cost.
fn past_cost(kind: Said, text: &str) -> &str {
    match text.split_once(": ") {
        Some((cost, words)) if kind != Said::Triggered && !cost.contains(['"', '.']) => words,
        _ => text,
    }
}

/// Oracle words as Cantrip's `waiting_texts_match_oracle` compares them:
/// lowercase, straight quotes, single spaces, no parentheses.
fn oracle_words(text: &str) -> String {
    let text = text
        .to_lowercase()
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(['(', ')'], " ")
        .replace('\u{2212}', "-");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Printed {
    pub(super) fn read(oracle: &str) -> Self {
        let mut printed = Printed::default();
        let mut stage = None;
        // Cantrip quotes a stage's line with its stage, as Hexdrinker's "LEVEL
        // 3-7: 4/4" and Adagia's "12+ | {3}{W}, {T}: ...".
        let mut stage_said = String::new();
        for raw in oracle.lines() {
            let line = loyalty_printed(&without_reminder(raw));
            // "12+ | {1}{R}, {T}: ..." prints the stage and its line as one.
            let (marker, line) = match line.split_once(" | ") {
                Some((marker, rest)) if stage_marker(marker).is_some() => {
                    (marker.to_string(), rest.to_string())
                }
                _ => (line.clone(), line),
            };
            let opened = stage_marker(&marker);
            if marker == line && opened.is_some() {
                stage = opened;
                stage_said = format!("{marker}: ");
            } else if let Some(minimum) = opened.or(stage) {
                if opened.is_some() {
                    stage_said = format!("{marker} | ");
                }
                stage = Some(minimum);
                // Beside what it says, a stage prints a power and toughness.
                let stats = line.chars().all(|c| c.is_ascii_digit() || "/*".contains(c));
                let mut said = line_of(kind_of(&line), format!("{stage_said}{line}"), raw);
                said.head = said.text.clone();
                said.stats = stats;
                said.staged = stage_said.len();
                printed.stages.entry(minimum).or_default().push(said);
            } else if line.is_empty() {
                // A basic land type's ability is printed as a reminder (CR 305.6).
                let inner = raw.trim().trim_matches(['(', ')']);
                if kind_of(inner) == Said::Activated {
                    printed
                        .lines
                        .push(line_of(Said::Activated, inner.into(), ""));
                }
            } else if let Some(mode) = line.strip_prefix("\u{2022} ") {
                printed.add_mode(mode.to_string());
            } else if line.starts_with("+ ") {
                printed.add_mode(line);
            } else if chapter_numbers(&line).is_some() {
                let (numbers, _) = chapter_numbers(&line).expect("just read");
                for number in numbers {
                    printed.chapters.insert(number, line.clone());
                }
            } else if keyword_line(&line) {
                for piece in line.split(", ").flat_map(|p| p.split("; ")) {
                    printed
                        .lines
                        .push(line_of(Said::Word, capitalized(piece), raw));
                }
            } else {
                printed.lines.push(line_of(kind_of(&line), line, raw));
            }
        }
        for line in &mut printed.lines {
            if !line.modes.is_empty() {
                let modes: Vec<&str> = line
                    .modes
                    .iter()
                    .map(|mode| mode.trim_end_matches('.'))
                    .collect();
                line.text = format!("{} {}.", line.text, modes.join("; or "));
                if line.kind == Said::Word {
                    line.kind = Said::Sentence;
                }
            }
        }
        printed
    }
    fn add_mode(&mut self, mode: String) {
        if let Some(line) = self.lines.last_mut() {
            line.modes.push(mode);
        }
    }

    /// The printed line of each of `abilities`, or `None` for what has none.
    pub(super) fn aligned(&self, abilities: &[Ability]) -> Vec<Option<Line>> {
        let (n, m) = (abilities.len(), self.lines.len());
        // `best[a][l][used]`: the score of abilities from `a` on and lines from
        // `l` on, `used` saying the line at `l` already has an ability.
        let mut best = vec![vec![[0i32; 2]; m + 1]; n + 1];
        let mut taken = vec![vec![[false; 2]; m + 1]; n + 1];
        for a in (0..n).rev() {
            for l in (0..m).rev() {
                for used in [0usize, 1] {
                    // Passing a line that nothing read costs a little.
                    let past = best[a][l + 1][0] - i32::from(used == 0);
                    let alone = best[a + 1][l][used];
                    let fit = fit(&abilities[a], &self.lines[l])
                        .map(|fit| fit / (used as i32 + 1) + best[a + 1][l][1]);
                    let most = past.max(alone);
                    match fit {
                        Some(fit) if fit >= most => {
                            best[a][l][used] = fit;
                            taken[a][l][used] = true;
                        }
                        _ => best[a][l][used] = most,
                    }
                }
            }
        }
        let mut aligned = vec![None; n];
        let (mut a, mut l, mut used) = (0, 0, 0);
        while a < n && l < m {
            if taken[a][l][used] {
                let mut line = self.lines[l].clone();
                // A cost and the spell it branches are two lines of one ability.
                if matches!(abilities[a], Ability::CostBranch { .. }) && line.modes.is_empty() {
                    if let Some(next) = self.lines.get(l + 1) {
                        line.modes = next.modes.clone();
                    }
                }
                aligned[a] = Some(line);
                (a, used) = (a + 1, 1);
            } else if best[a][l][used] == best[a + 1][l][used] {
                a += 1;
            } else {
                (l, used) = (l + 1, 0);
            }
        }
        aligned
    }
}

/// Whether "you may" of this cost is its choice, up to one: the question is which
/// object, and answering none is declining (Cantrip's `may_discard_a_card`).
/// Whether a line says what the AST of it doesn't.
pub(super) fn printed_otherwise(line: Option<&str>, words: &str) -> bool {
    line.is_some_and(|line| line.contains(words))
}
/// What pays is chosen even if there's only one thing that can.
pub(super) fn asks_even_of_one(out: &mut String, mark: usize) {
    let picks = out.split_off(mark);
    out.push_str(&picks.replace(
        "if eligible.len() <= n as usize",
        "if eligible.len() < n as usize",
    ));
}
/// `object` is what a moved object became: "that card", in its new zone.
pub(super) fn what_the_event_moved(object: &ObjectRef) -> bool {
    match object {
        ObjectRef::EventDestination => true,
        ObjectRef::Query { objects, index: 0 } => match objects.as_ref() {
            ObjectSet::InZone { objects, .. } => {
                matches!(&**objects, ObjectSet::One(ObjectRef::EventDestination))
            }
            _ => false,
        },
        _ => false,
    }
}
/// Whether what becomes a target is a permanent, for a filter that names no
/// other place for it to be.
pub(super) fn a_permanent_targeted(objects: &ObjectFilter) -> bool {
    let said = format!("{objects:?}");
    !["InZone", "Spell", "Stack", "Source"]
        .iter()
        .any(|word| said.contains(word))
}
/// What a triggered mana ability adds, so what's offered to pay counts it.
pub(super) fn triggered_mana(body: &Program) -> &'static str {
    if lane_g::adds_what_was_produced(body) {
        return ".mana_ability(TriggeredMana::OneOfProduced)";
    }
    let color = match body.instructions.as_slice() {
        [Instruction::Effect {
            operation:
                Operation::AddMana {
                    mana:
                        ManaProduction::Amount {
                            color,
                            amount: Expr::Constant(1),
                        },
                    restriction: None,
                    ..
                },
            ..
        }] => Some(color),
        _ => None,
    };
    match color {
        Some(ManaColor::Chosen) => ".mana_ability(TriggeredMana::ChosenColor)",
        Some(ManaColor::White) => ".mana_ability(TriggeredMana::One(Mana::W))",
        Some(ManaColor::Blue) => ".mana_ability(TriggeredMana::One(Mana::U))",
        Some(ManaColor::Black) => ".mana_ability(TriggeredMana::One(Mana::B))",
        Some(ManaColor::Red) => ".mana_ability(TriggeredMana::One(Mana::R))",
        Some(ManaColor::Green) => ".mana_ability(TriggeredMana::One(Mana::G))",
        _ => ".mana_ability(TriggeredMana::Unstated)",
    }
}
/// What the creatures that convoked it convoked, if that's all this set says.
pub(super) fn convoked(objects: &ObjectSet) -> Option<&ObjectRef> {
    match objects {
        ObjectSet::Battlefield(ObjectFilter::History {
            event: ObjectHistory::Convoked(object),
            ..
        }) => Some(object),
        _ => None,
    }
}
/// Whether a target the card says is from any graveyard is, in the AST, from yours.
pub(super) fn only_your_graveyard(line: Option<&str>, filter: &ObjectFilter) -> bool {
    printed_otherwise(line, "card from a graveyard")
        && format!("{filter:?}")
            .contains("InZone(Players { zone: Graveyard, players: One(AbilityController) })")
}
/// Who controlled what an instruction acted on doesn't depend on its having moved:
/// "Destroy target land. That land's controller ..." is them all the same.
pub(super) fn gone_or_not(read: String, object: &ObjectRef) -> String {
    match object {
        ObjectRef::Result {
            phase: ObjectPhase::Before,
            ..
        } => read.replace(&effects::gone(""), ".clone()"),
        _ => read,
    }
}
/// A mana ability that chooses a color first of all names it as it's activated
/// (a mana ability doesn't wait, and it's the activation that asks): the code
/// reading the color it was given, if the code is such a one.
pub(super) fn color_as_activated(out: &str) -> Option<String> {
    let at = out.find("let Answer::Colors(colors_")?;
    let (before, ask) = out.split_at(at);
    let line = ask.lines().next()?;
    if before.contains('\n')
        || !before.contains("Some(asker.controller)")
        || !line.contains("choices: ColorSet(31), count: 1,")
    {
        return None;
    }
    let (_, number) = line.rsplit_once("let chosen_")?;
    let number = number.split(' ').next()?;
    let rest = &ask[line.len()..];
    Some(format!(
        "{before}let Some(chosen_{number}) = chosen else {{ return Ok(()) }};{rest}"
    ))
}
/// Whether an intervening "if" asks how an object was cast: as the object enters,
/// the event that says which spell it was is not in the history yet.
pub(super) fn asks_how_it_was_cast(condition: &Condition) -> bool {
    matches!(
        condition,
        Condition::ObjectMatches {
            filter: ObjectFilter::Cast { .. },
            ..
        }
    )
}
/// An intervening "if" of how an object was cast, independent of who controls the ability: the recorded cast cannot
/// change. A condition such as "if you cast it" must still be checked at resolution (CR 603.4): an opponent can copy
/// Zacama's trigger, and "you" then names the copy's controller (CR 707.10).
pub(super) fn cast_history_is_fixed(condition: &Condition) -> bool {
    matches!(
        condition,
        Condition::ObjectMatches {
            filter: ObjectFilter::Cast {
                by: None,
                from: None
                    | Some(ZoneScope::Shared(_))
                    | Some(ZoneScope::Players {
                        players: PlayerSet::All,
                        ..
                    }),
            },
            ..
        }
    )
}
/// A trigger keeps the new object's number only if something reads it: the game
/// compares what a trigger carries to find a loop (732.4), and a number that grows
/// round the loop would keep it from ever coming back to the same position.
pub(super) fn without_unread_data(functions: &str) -> String {
    const KEPT: &str = "let data = [new.0, u32::MAX];";
    if !functions.contains(KEPT) || functions.contains("item.data") || functions.contains("data[0]")
    {
        return functions.to_string();
    }
    functions.replace(KEPT, "let data = [0u32, u32::MAX];")
}
/// "Shuffle it into its owner's library" of a card in a zone: the object as named,
/// not as found, since the library is shuffled even if it has gone (701.24c).
pub(super) fn shuffled_in_though_gone<'a>(
    objects: &'a ObjectSet,
    destination: &Destination,
) -> &'a ObjectSet {
    if let (
        ObjectSet::One(ObjectRef::Query {
            objects: found,
            index: 0,
        }),
        Destination::OwnersLibraryShuffled,
    ) = (objects, destination)
    {
        if let ObjectSet::InZone { objects: named, .. } = found.as_ref() {
            if matches!(named.as_ref(), ObjectSet::One(_)) {
                return named;
            }
        }
    }
    objects
}
/// "A permanent", "three permanents": a number as a card prints it.
fn printed_many(n: i64, noun: &str) -> String {
    const WORDS: [&str; 9] = ["two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    match usize::try_from(n - 2).ok().and_then(|at| WORDS.get(at)) {
        Some(word) => format!("{word} {noun}s"),
        None if n == 1 => {
            let article = if noun.starts_with(|c| "aeiou".contains(c)) { "an" } else { "a" };
            format!("{article} {noun}")
        }
        None => format!("{n} {noun}s"),
    }
}

/// "Discard a card or pay {N}", either way round: N.
pub(super) fn discard_or_pay(costs: &[Cost]) -> Option<u8> {
    let [a, b] = costs else { return None };
    let discard = |c: &Cost| {
        matches!(
            c.parts.as_slice(),
            [CostPart::Discard { count: Expr::Constant(1), filter: ObjectFilter::Any, random: false }]
        )
    };
    let generic = |c: &Cost| match c.parts.as_slice() {
        [CostPart::Mana(symbols)] => match symbols.as_slice() {
            [ManaSymbol::Generic(n)] => u8::try_from(*n).ok(),
            _ => None,
        },
        _ => None,
    };
    match (discard(a), discard(b)) {
        (true, false) => generic(b),
        (false, true) => generic(a),
        _ => None,
    }
}

/// "Ward--Sacrifice a Food" and its like, where Cantrip has the ability.
pub(super) fn ward_by_sacrifice(cost: &Cost) -> Option<String> {
    let [CostPart::Sacrifice {
        objects: ObjectSet::Battlefield(filter),
        choose: Some(Count::Exactly(Expr::Constant(n))),
    }] = cost.parts.as_slice()
    else {
        return None;
    };
    let call = |function: String, what: &str| {
        Some(format!(
            "cantrip_mechanics::triggers::{function}(\"Ward\u{2014}Sacrifice {what}.\")"
        ))
    };
    match (filter, *n) {
        (ObjectFilter::Subtype(subtype), 1) if name(subtype) == "Food" => {
            call("ward_sacrifice_a_food".into(), "a Food")
        }
        (ObjectFilter::Any, n) => {
            call(format!("ward_sacrifice::<{n}>"), &printed_many(n, "permanent"))
        }
        (ObjectFilter::Not(inner), n)
            if matches!(
                **inner,
                ObjectFilter::CardType(crate::mtg_types::CardType::Land)
            ) =>
        {
            call(
                format!("ward_sacrifice_nonland::<{n}>"),
                &printed_many(n, "nonland permanent"),
            )
        }
        _ => None,
    }
}
pub(super) fn paid_by_a_choice(cost: &Cost) -> bool {
    if lane_l::chooses_what_pays(cost) {
        return true;
    }
    match cost.parts.as_slice() {
        [CostPart::Sacrifice {
            choose: Some(Count::Exactly(Expr::Constant(1))),
            ..
        }] => true,
        [CostPart::Discard {
            count: Expr::Constant(1),
            filter: ObjectFilter::Any,
            random: false,
        }] => true,
        [CostPart::Move {
            selections,
            face_down: false,
            ..
        }] => matches!(
            selections.as_slice(),
            [CostObjectSelection {
                choose: Some(Count::Exactly(Expr::Constant(1))),
                ..
            }]
        ),
        _ => false,
    }
}

/// Whether what `decider` may do starts with their choosing one object.
pub(super) fn chooses_first(decider: &PlayerRef, body: &Program) -> bool {
    chooses_first_after(decider, body, false)
}

/// `chooses_first`, `looked` if the cards were looked at first.
fn chooses_first_after(decider: &PlayerRef, body: &Program, mut looked: bool) -> bool {
    for instruction in &body.instructions {
        match instruction {
            Instruction::Group(group) => return chooses_first_after(decider, group, looked),
            Instruction::Effect {
                operation: Operation::LookAt { .. },
                ..
            } => {
                looked = true;
                continue;
            }
            Instruction::Effect {
                operation:
                    Operation::Discard {
                        players: PlayerSet::One(player),
                        amount: Expr::Constant(1),
                        random: false,
                    },
                ..
            } => return format!("{player:?}") == format!("{decider:?}"),
            Instruction::Effect {
                operation:
                    Operation::Sacrifice {
                        players: PlayerSet::One(player),
                        choose:
                            Some(
                                Count::Exactly(Expr::Constant(1))
                                | Count::AsManyAsCan(Expr::Constant(1))
                                | Count::AnyNumber,
                            ),
                        ..
                    },
                ..
            } => return format!("{player:?}") == format!("{decider:?}"),
            // "You may attach an Equipment you control to it": which, or none.
            Instruction::Effect {
                operation:
                    Operation::AttachChosen {
                        actor: PlayerSet::One(player),
                        count: Count::Exactly(Expr::Constant(1)),
                        ..
                    },
                ..
            } => return format!("{player:?}") == format!("{decider:?}"),
            Instruction::Effect {
                operation:
                    Operation::ChangeZone {
                        actor: PlayerSet::One(player),
                        count:
                            Some(
                                Count::UpTo(_)
                                | Count::Exactly(Expr::Constant(1))
                                | Count::AsManyAsCan(Expr::Constant(1)),
                            ),
                        random: false,
                        ..
                    },
                ..
            } => return format!("{player:?}") == format!("{decider:?}"),
            Instruction::Effect {
                operation:
                    Operation::ChooseObjects {
                        player,
                        objects,
                        count,
                        constraints,
                    },
                ..
            } if matches!(
                count,
                Count::Exactly(Expr::Constant(1)) | Count::AsManyAsCan(Expr::Constant(1)) | Count::AnyNumber
            ) || (!looked && matches!(count, Count::UpTo(_))) =>
            {
                // "You may discard up to two cards": the choice is the may, none is
                // declining it (Fable of the Mirror-Breaker). Not after a look: "you may
                // look at ..., put up to one ... and the rest on the bottom" still puts the
                // rest there when none is chosen (Call to the Kindred).
                return format!("{player:?}") == format!("{decider:?}")
                    && !matches!(objects, ObjectSet::Created(_))
                    && !constraints
                        .iter()
                        .any(|c| matches!(c, SelectionConstraint::AtRandom))
            }
            _ => return false,
        }
    }
    false
}

/// What of the cards looked at goes to the hand, the rest to the bottom.
struct Taking<'a> {
    chosen: &'a ResultId,
    put: &'a ResultId,
    reveal: bool,
    least: u8,
    most: u8,
    lets: Vec<String>,
    wanted: String,
    rest: &'static str,
}

/// `body` as a `Taking`, what `looker` looked at.
fn taking_from<'a>(looker: &PlayerRef, body: &'a [Instruction], id: usize) -> Option<Taking<'a>> {
    let steps: Vec<(&ResultId, &Operation)> = body
        .iter()
        .map(|instruction| match instruction {
            Instruction::Effect { result, operation } => Some((result, operation)),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let (reveal, steps) = match steps.as_slice() {
        [choose, (
            _,
            Operation::Reveal {
                objects: ObjectSet::Selected(_),
            },
        ), put, rest] => (true, [*choose, *put, *rest]),
        [choose, put, rest] => (false, [*choose, *put, *rest]),
        _ => return None,
    };
    let [(
        chosen,
        Operation::ChooseObjects {
            player: chooser,
            objects,
            count,
            constraints,
        },
    ), (
        put,
        Operation::Move {
            objects: ObjectSet::Selected(selected),
            destination: Destination::OwnersHand,
            entry,
            face_down: false,
        },
    ), (
        _,
        Operation::ReorderLibrary {
            end: SequenceEnd::Bottom,
            order,
            ..
        },
    )] = steps
    else {
        return None;
    };
    let (least, most) = match count {
        Count::Exactly(Expr::Constant(n)) | Count::AsManyAsCan(Expr::Constant(n)) => {
            u8::try_from(*n).ok().map(|n| (n, n))
        }
        Count::UpTo(Expr::Constant(n)) => u8::try_from(*n).ok().map(|n| (0, n)),
        _ => None,
    }?;
    let filter = match objects {
        ObjectSet::Matching { objects, filter }
            if matches!(&**objects, ObjectSet::InZone { .. }) =>
        {
            filter
        }
        ObjectSet::InZone { .. } => &ObjectFilter::Any,
        _ => return None,
    };
    let rest = match order {
        CardOrder::Chosen => "BottomChosen",
        CardOrder::Random => "BottomRandom",
        CardOrder::Preserve => return None,
    };
    let plain = format!("{looker:?}") == format!("{chooser:?}")
        && selected.0 == chosen.0
        && entry.counters.is_empty()
        && !entry.tapped
        && !constraints
            .iter()
            .any(|c| !matches!(c, SelectionConstraint::CanMoveTo { .. }));
    if !plain {
        return None;
    }
    let mut lets = Vec::new();
    let test = chars_test(filter, id, &mut lets)?;
    let wanted = match test.as_str() {
        "true" => "|_| true".to_string(),
        _ => format!("|c| {test}"),
    };
    Some(Taking {
        chosen,
        put,
        reveal,
        least,
        most,
        lets,
        wanted,
        rest,
    })
}

/// The test of `filter` on a `Chars` named `c`, for what `look_at_top` takes; the
/// words it reads are `lets`, since a text change can have replaced them.
fn chars_test(filter: &ObjectFilter, id: usize, lets: &mut Vec<String>) -> Option<String> {
    let mut all = |filters: &[ObjectFilter], joined: &str| -> Option<String> {
        let tests = filters
            .iter()
            .map(|f| chars_test(f, id, lets))
            .collect::<Option<Vec<_>>>()?;
        Some(format!("({})", tests.join(joined)))
    };
    Some(match filter {
        ObjectFilter::Any => "true".into(),
        ObjectFilter::Either(filters) => all(filters, " || ")?,
        ObjectFilter::All(filters) => all(filters, " && ")?,
        ObjectFilter::Not(inner) => format!("!{}", chars_test(inner, id, lets)?),
        ObjectFilter::CardType(card) => format!("c.is(CardType::{})", card_type(card).ok()?),
        ObjectFilter::HasKeyword(word) => format!("c.has({})", keyword(word).ok()?),
        // A printed {X}, which `Chars` doesn't have.
        ObjectFilter::State(ObjectState::HasX) => {
            let cards = format!("with_x_{id}");
            lets.push(format!(
                "let {cards}: Vec<CardId> = {{ let q: &dyn Query = g; q.state().zone(Zone::library(asker.controller)).iter().filter(|&&o| q.mana_cost(o).x > 0).map(|&o| q.chars(o).card).collect() }};"
            ));
            format!("{cards}.contains(&c.card)")
        }
        ObjectFilter::Subtype(subtype) => {
            let word = format!("word_{id}_{}", lets.len());
            lets.push(format!(
                "let {word} = {{ let q: &dyn Query = g; q.words(asker.source).subtype(Subtype::{}) }};",
                name(subtype)
            ));
            format!("c.has_subtype({word})")
        }
        _ => return None,
    })
}

/// Mana as `Adds` that lists it, with nothing else to do.
pub(super) fn listed_mana(
    plan: &ManaPlan,
    rest: &[Instruction],
    restriction: Option<&ManaUse>,
) -> Option<String> {
    if !rest.is_empty() || !matches!(lane_g::mana_lasting(restriction), (None, _)) {
        return None;
    }
    match plan {
        ManaPlan::SameColor {
            colors,
            amount: Expr::Constant(n),
        } => {
            let n = u8::try_from(*n).ok()?;
            Some(match colors.len() {
                5 => format!("add_any_color({n})"),
                _ => format!(
                    "add_any_color({n}).colors(ColorSet({}))",
                    color_mask(colors).ok()?
                ),
            })
        }
        ManaPlan::Fixed(units) => {
            let mut pool = [0u32; 6];
            for (color, n) in units {
                let Expr::Constant(n) = n else {
                    return None;
                };
                pool[color_index(color).ok()?] = u32::try_from(*n).ok()?;
            }
            Some(format!(
                "Adds {{ pool: ManaPool({pool:?}), ..add_each(&[]) }}"
            ))
        }
        _ => None,
    }
}

/// The objects a move puts onto the battlefield: a token that left it is gone for
/// good, whatever still holds it (CR 111.7).
pub(super) fn returning(objects: String, destination: &Destination) -> String {
    match destination {
        Destination::Battlefield(_) | Destination::OwnersBattlefield => format!(
            "({objects}).into_iter().filter(|&o: &ObjectId| !g.state().obj(o).token).collect::<Vec<ObjectId>>()"
        ),
        _ => objects,
    }
}

/// The keyword a permanent has for a rule it states of itself (Cantrip's
/// `Keyword`), so that losing its abilities takes the rule with them.
pub(super) fn rule_keyword(affected: &ObjectSet, rule: &RuleChange) -> Option<&'static str> {
    if !matches!(affected, ObjectSet::One(ObjectRef::Source)) {
        return None;
    }
    match rule {
        RuleChange::Prohibit(ObjectAction::Block(ObjectFilter::Any)) => Some("CantBlock"),
        RuleChange::Prohibit(ObjectAction::BeBlocked(ObjectFilter::Any)) => Some("CantBeBlocked"),
        RuleChange::UntapStep {
            behavior: UntapBehavior::Skip,
            ..
        } => Some("DoesNotUntap"),
        _ => None,
    }
}

/// Several cards a player owns put on one end of a library together: that
/// player orders them (401.4), and nobody is asked about one.
pub(super) fn in_an_order_asked(id: usize, destination: &Destination) -> String {
    let (end, place) = match destination {
        Destination::OwnersLibraryTop => ("LibraryTop", "Top"),
        _ => ("LibraryBottom", "Bottom"),
    };
    format!("{{ let order_{id} = match objects_{id}.iter().all(|&o| g.state().obj(o).owner == asker.controller) {{ true => cantrip_mechanics::library::ask_library_order(g, asker.controller, asker.source, &objects_{id}, DestinationKind::{end})?, false => objects_{id}.clone() }}; g.put_into_library(&order_{id}, LibraryPlace::{place}) }}")
}
/// What a choice puts on top of a library: one at a time as chosen, so the last
/// chosen ends on top (Cantrip's `draw_then_put_two_back`).
pub(super) fn on_top_in_turn(objects: String, chosen: bool, destination: &Destination) -> String {
    match (chosen, destination) {
        (true, Destination::OwnersLibraryTop) => {
            format!("({objects}).into_iter().rev().collect::<Vec<ObjectId>>()")
        }
        _ => objects,
    }
}

/// Whether all a "may" offers is to tap or untap something: one question, of
/// three answers (Cantrip's `may_tap_or_untap_target`).
pub(super) fn tap_or_untap_only(body: &Program) -> bool {
    matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Keyword {
                action: KeywordAction::TapOrUntap,
                ..
            },
            ..
        }]
    )
}

/// "Look at the top card. You may cast that card. Do this only once each turn": the
/// source has the whole ability offered, but the look isn't, and "this" is the cast
/// (the ruling on Planetarium of Wan Shi Tong). What's asked, looked at unasked.
pub(super) fn looks_then_may(body: &Program) -> Option<Program> {
    let [Instruction::Optional {
        decider,
        body: offered,
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    let [looks @ .., Instruction::Optional { decider: inner, .. }] = offered.instructions.as_slice()
    else {
        return None;
    };
    let only_looks = !looks.is_empty()
        && looks.iter().all(|step| {
            matches!(
                step,
                Instruction::Effect {
                    operation: Operation::LookAt { .. },
                    ..
                }
            )
        });
    (only_looks && format!("{decider:?}") == format!("{inner:?}")).then(|| offered.clone())
}

/// The key of a thing done once each turn, among a card's own: past its abilities,
/// so `Query::history` holds it apart from them.
fn done_key(key: u8, reader: &str) -> String {
    format!("AbilityRef {{ card: {reader}.chars(asker.source).card, index: {key} }}")
}

/// The test on an event that what's done once each turn has yet to be: the action
/// is its controller's (603.2h), so one taken with this ability before its source
/// left and came back counts as well.
pub(super) fn not_yet_done(key: u8) -> String {
    format!(
        "!q.history().this_turn().iter().any(|r| matches!(r.event, Event::Done {{ ability, player, .. }} if ability == {} && player == asker.controller))",
        done_key(key, "q")
    )
}

/// What opens a "may" done once each turn, and what spends the turn's once.
pub(super) fn once_a_turn(key: Option<u8>, id: usize) -> (String, String) {
    let Some(key) = key else {
        return (String::new(), String::new());
    };
    let done = done_key(key, "g");
    (
        format!("!g.history().done(asker.source, {done}, decider_{id}) && "),
        format!(" g.note_done(asker.source, {done}, decider_{id});"),
    )
}

/// What spends the turn's once when it is done, not asked (603.2h).
pub(super) fn spends_once(key: u8) -> String {
    format!(
        "g.note_done(asker.source, {}, asker.controller);",
        done_key(key, "g")
    )
}

/// What must hold for a "may" to be asked at all, as the start of a test: no one
/// is asked to pay energy they haven't got, or to draw when they can't (121.3).
pub(super) fn asked_if(decider: &PlayerRef, id: usize, body: &Program) -> String {
    let mut first = body;
    while let [Instruction::Group(group), ..] = first.instructions.as_slice() {
        first = group;
    }
    let same = |player: &PlayerRef| format!("{player:?}") == format!("{decider:?}");
    match first.instructions.first() {
        Some(Instruction::Effect {
            operation: Operation::PayCost { player, cost, .. },
            ..
        }) if same(player) => match cost.parts.as_slice() {
            [CostPart::Energy(Expr::Constant(n))] => {
                format!("g.state().player(decider_{id}).energy() >= {n} && ")
            }
            _ => String::new(),
        },
        Some(Instruction::Effect {
            operation:
                Operation::Draw {
                    players: PlayerSet::One(player),
                    amount,
                },
            ..
        }) if same(player) => {
            let n = match amount {
                Expr::Constant(n) => *n,
                _ => 1,
            };
            format!("!cant_draw(g, decider_{id}, {n}) && ")
        }
        // A search that can't be caused isn't offered (Ashiok, Dream Render's ruling).
        // A search of other zones too still searches them (Agency Outfitter).
        Some(Instruction::Effect {
            operation: Operation::Search { player, library, also, .. },
            ..
        }) if same(player) && same(library) && also.is_empty() => {
            format!("g.may_search(decider_{id}, decider_{id}) && ")
        }
        // Moving the source: it's a new object once it has moved (400.7).
        Some(Instruction::Effect {
            operation:
                Operation::Move {
                    objects: ObjectSet::One(ObjectRef::Source),
                    ..
                },
            ..
        }) => "g.state().obj(asker.source).became.is_none() && ".to_string(),
        Some(Instruction::Effect {
            operation: Operation::RemoveCounters { removals },
            ..
        }) => match removals.as_slice() {
            [CounterRemoval {
                recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
                kind: None,
                amount: CounterAmount::Count(Count::Exactly(Expr::Constant(1))),
                ..
            }] => "{ let o = g.state().obj(asker.source); o.became.is_none() && o.zone.is(ZoneKind::Battlefield) && CounterKind::ALL.into_iter().any(|k| o.counters(k) > 0) } && ".to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// How much `body` blights, if that's all `player` may do: Cantrip's cost "blight
/// N", which it asks as a payment (118.12).
pub(super) fn blights(player: &PlayerRef, body: &Program) -> Option<u8> {
    let [Instruction::Effect {
        operation:
            Operation::Keyword {
                actor: PlayerSet::One(actor),
                action: KeywordAction::Blight,
                object: None,
                other: None,
                amount: Some(Expr::Constant(n)),
                subtype: None,
            },
        ..
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    (format!("{actor:?}") == format!("{player:?}"))
        .then(|| u8::try_from(*n).ok())
        .flatten()
}

/// Giving a keyword, in a static ability's layer function. A landwalk's land type is
/// a word of the text, which an effect may have changed (612.2).
pub(super) fn keyword_granted(keyword: &str) -> String {
    let land = ["Plains", "Island", "Swamp", "Mountain", "Forest"]
        .into_iter()
        .find(|land| {
            keyword.strip_prefix("Keyword::")
                == Some(&format!("{}walk", land.trim_end_matches('s')))
        });
    match land {
        Some(land) => format!("if let Some(walk) = Keyword::landwalk(q.words(source).subtype(Subtype::{land})) {{ chars.keywords = chars.keywords.with(walk); }}"),
        None => format!("chars.keywords = chars.keywords.with({keyword});"),
    }
}

/// Whether `objects` are read out of a graveyard, whose first card is its top.
pub(super) fn from_a_graveyard(objects: &ObjectSet) -> bool {
    matches!(
        objects,
        ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::Graveyard,
                ..
            },
            ..
        }
    )
}

/// Whether this Oracle text reads the order of a graveyard (404.2): a sentence
/// about a graveyard that places something in it ("the top creature card of your
/// graveyard", "with a creature card directly above it"), not a "top" whose place
/// is a library. The core asks an owner to order cards only for such a card.
pub(super) fn reads_graveyard_order(oracle: &str) -> bool {
    let text = without_reminder(oracle)
        .to_lowercase()
        .replace('\u{2019}', "'");
    text.split(['.', '\n']).any(|sentence| {
        sentence.contains("graveyard")
            && (["above it", "below it", "directly above", "directly below"]
                .iter()
                .any(|words| sentence.contains(words))
                || ["top", "bottom"].iter().any(|end| {
                    sentence.match_indices(end).any(|(at, _)| {
                        let after = &sentence[at..];
                        match (after.find("graveyard"), after.find("library")) {
                            (Some(graveyard), Some(library)) => graveyard < library,
                            (Some(_), None) => true,
                            _ => false,
                        }
                    })
                }))
    })
}

/// What may be discarded, if `cost` is discarding one card of some kind.
fn discards_one(cost: &Cost) -> Option<Vec<&ObjectFilter>> {
    fn of(parts: &[CostPart]) -> Option<&ObjectFilter> {
        match parts {
            [CostPart::Discard {
                count: Expr::Constant(1),
                random: false,
                filter,
            }] => Some(filter),
            _ => None,
        }
    }
    match cost.parts.as_slice() {
        [CostPart::Choice(options)] => options.iter().map(|option| of(&option.parts)).collect(),
        parts => of(parts).map(|filter| vec![filter]),
    }
}

/// Whether a choice of costs is sacrificing an artifact or discarding a card.
fn sacrifice_or_discard(options: &[Cost]) -> bool {
    let [sacrifice, discard] = options else {
        return false;
    };
    let artifact = matches!(
        sacrifice.parts.as_slice(),
        [CostPart::Sacrifice {
            objects: ObjectSet::Battlefield(ObjectFilter::CardType(card)),
            choose: Some(Count::Exactly(Expr::Constant(1))),
        }] if format!("{card:?}") == "Artifact"
    );
    let card = matches!(
        discard.parts.as_slice(),
        [CostPart::Discard {
            count: Expr::Constant(1),
            filter: ObjectFilter::Any,
            random: false,
        }]
    );
    artifact && card
}

/// The text of Reconfigure's two abilities, from "Reconfigure {2}".
pub(super) fn reconfigure_texts(word: &str) -> (String, String) {
    let cost = word.strip_prefix("Reconfigure ").unwrap_or("");
    (
        format!("{word} ({cost}: Attach this permanent to another target creature you control. Reconfigure only as a sorcery.)"),
        format!("{word} ({cost}: Unattach this permanent. Reconfigure only if it is attached to a creature and only as a sorcery.)"),
    )
}

/// A type this effect's text names, as the text has it now (CR 612).
pub(super) fn word_subtype(word: &str) -> String {
    format!("q.words(asker.source).subtype(Subtype::{word})")
}

/// The colors this effect's text names, as the text has them now (CR 612).
pub(super) fn word_colors(mask: u8) -> String {
    format!("q.words(asker.source).colors(ColorSet({mask}))")
}

/// A pump's "X" that reads a power or toughness: a negative X is 0 (CR 107.1b).
/// Doubling, which reads the same, lowers a negative power instead (CR 701.10e).
fn not_negative(amount: &Expr, code: String) -> String {
    let read = |n: &Expr| matches!(n, Expr::ObjectProperty { .. } | Expr::Aggregate { .. });
    match amount {
        n if read(n) => format!("({code}).max(0)"),
        Expr::Multiply(left, right) => match (&**left, &**right) {
            (Expr::Constant(n), x) | (x, Expr::Constant(n)) if *n < 0 && read(x) => {
                format!("({code}).min(0)")
            }
            _ => code,
        },
        _ => code,
    }
}

/// The word a keyword prints, from its name.
fn printed_word(value: &impl std::fmt::Debug) -> String {
    let mut word = String::new();
    for c in name(value).chars().take_while(char::is_ascii_alphanumeric) {
        if c.is_ascii_uppercase() && !word.is_empty() {
            word.push(' ');
        }
        word.push(c.to_ascii_lowercase());
    }
    word
}

/// How well `ability` reads as `line`, or `None` if it can't be that line.
fn fit(ability: &Ability, line: &Line) -> Option<i32> {
    let by_word = |word: &str| {
        let lower = line.text.to_lowercase();
        match line.kind {
            Said::Word | Said::Sentence if lower.starts_with(word) => Some(12),
            Said::Word => Some(2),
            Said::Sentence => Some(1),
            _ => None,
        }
    };
    let kind = line.kind;
    match ability {
        Ability::Scoped { ability, .. } => fit(ability, line),
        // "Max speed — {3}, Exile this card ...: ..." is the ability's own line.
        Ability::Conditional { then, otherwise, .. }
            if otherwise.is_empty()
                && matches!(then.as_slice(), [Ability::Activated { .. } | Ability::Triggered { .. }]) =>
        {
            fit(&then[0], line)
        }
        Ability::Keyword(keyword) => by_word(&printed_word(keyword)),
        Ability::Mechanic { mechanic, .. } => by_word(&printed_word(mechanic)),
        Ability::Enchant { .. } => by_word("enchant"),
        Ability::Activated { origin, .. } => match (origin, kind) {
            (ActivationOrigin::Ordinary, Said::Activated) => Some(12),
            (ActivationOrigin::Ordinary, Said::Word) => Some(3),
            (ActivationOrigin::Ordinary, _) => None,
            (ActivationOrigin::Cycling { .. }, Said::Word | Said::Sentence)
                if line.text.to_lowercase().contains("cycling") =>
            {
                Some(12)
            }
            (_, Said::Activated) => Some(4),
            (origin, _) => by_word(&printed_word(origin)),
        },
        Ability::Triggered { .. } => match kind {
            Said::Triggered => Some(12),
            Said::Word => Some(3),
            Said::Sentence => Some(1),
            Said::Activated => None,
        },
        Ability::Spell { .. } | Ability::OpeningHand { .. } => match kind {
            Said::Sentence => Some(12),
            _ => Some(1),
        },
        Ability::Static { .. }
        | Ability::Replacement { .. }
        | Ability::Conditional { .. }
        | Ability::Casting(_) => match kind {
            Said::Sentence => Some(8),
            Said::Word => Some(3),
            _ => None,
        },
        // Their lines aren't one ability's.
        Ability::Saga { .. } | Ability::CounterStages { .. } => None,
        _ => match kind {
            Said::Sentence | Said::Word => Some(4),
            _ => None,
        },
    }
}

impl Compiler {
    /// Whether the targets of `label` depend on X, so they are chosen after it is
    /// announced (CR 601.2b-c), never with the action.
    pub(super) fn targets_read_x(&self, label: &str) -> bool {
        let function = format!("fn {label}_target_");
        self.functions
            .lines()
            .any(|line| line.contains(&function) && line.contains("q.announced_x()"))
    }
    /// The printed words of a keyword, if its line is that keyword.
    pub(super) fn said_word(&self, fallback: String) -> String {
        let letters = |text: &str| {
            let letters = text.chars().filter(char::is_ascii_alphanumeric);
            letters.map(|c| c.to_ascii_lowercase()).collect::<String>()
        };
        match &self.line {
            Some(line) if letters(&line.text).starts_with(&letters(&fallback)) => line.text.clone(),
            _ => fallback,
        }
    }
    /// The `count` costs of a printed "discard a card or pay 3 life", or none.
    pub(super) fn said_costs(&self, count: usize) -> Vec<String> {
        let Some(line) = &self.line else {
            return Vec::new();
        };
        let costs = cost_options(additional_cost_words(&line.text));
        match costs.len() == count {
            true => costs,
            false => Vec::new(),
        }
    }
    /// How many permanents entered this turn as `event` says, from what the history
    /// keeps of each as it entered: who controlled it then and its types then (603.6a),
    /// whatever it is now and wherever it has gone.
    pub(super) fn entered_this_turn(&self, event: &EventSpec) -> Result<Option<String>> {
        // "That crewed it this turn": the history keeps who crewed what.
        if let EventSpec::Pair {
            event: PairEvent::Crews,
            objects: ObjectFilter::SameObject(ObjectRef::Candidate),
            others: ObjectFilter::SameObject(ObjectRef::Source),
        } = event
        {
            return Ok(self.candidates.borrow().last().map(|candidate| {
                format!(
                    "q.history().crewed(asker.source).filter(|&c| c == {}).count() as i64",
                    candidate.id
                )
            }));
        }
        let EventSpec::ZoneChange {
            objects,
            from: None,
            to: EventZone::Battlefield,
        } = event
        else {
            return Ok(None);
        };
        let mut tests = Vec::new();
        if self.as_it_entered(objects, &mut tests)? {
            return Ok(Some(format!(
                "q.history().entered().filter(|e| {}).count() as i64",
                tests.join(" && ")
            )));
        }
        if self.reader == Reader::Layer {
            return Ok(None);
        }
        // Any other filter reads the permanent as it entered, whatever it is now.
        let entered = Candidate {
            chars: "e.chars(q)".into(),
            controller: "e.controller".into(),
            ..Candidate::of("e.permanent")
        };
        let mut asked = self.clone();
        asked.history = Some(History {
            outer: self.candidates.borrow().last().map(|c| c.id.clone()),
            event_at: self.event_at,
            frame: self.frame,
        });
        let test = asked.matching(Frame::default(), |c| c.filter(objects, &entered));
        // A filter on the event being matched has to come through the record.
        Ok(test.ok().filter(|test| !test.contains("subject") && !test.contains("data")).map(|test| {
            format!("q.history().entered().filter(|e| {test}).count() as i64")
        }))
    }
    fn as_it_entered(&self, filter: &ObjectFilter, tests: &mut Vec<String>) -> Result<bool> {
        match filter {
            ObjectFilter::Any => {}
            ObjectFilter::All(filters) => {
                for filter in filters {
                    if !self.as_it_entered(filter, tests)? {
                        return Ok(false);
                    }
                }
            }
            ObjectFilter::CardType(card) => {
                tests.push(format!("e.is(CardType::{})", card_type(card)?))
            }
            ObjectFilter::ControlledBy(players) => {
                tests.push(self.player_in(players, "e.controller")?)
            }
            ObjectFilter::Not(inner)
                if matches!(&**inner, ObjectFilter::SameObject(ObjectRef::Source)) =>
            {
                tests.push("e.permanent != asker.source".to_string());
            }
            ObjectFilter::SameObject(ObjectRef::Candidate) => {
                match self.candidates.borrow().last() {
                    Some(candidate) => tests.push(format!("e.permanent == {}", candidate.id)),
                    None => return Ok(false),
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    /// What must hold for `payer_{id}` to be asked to pay `cost` as it resolves, as
    /// the start of a test: a cost that can't be paid isn't offered (118.3).
    pub(super) fn can_pay(&self, id: usize, cost: &Cost) -> String {
        match cost.parts.as_slice() {
            [CostPart::Move { selections, .. }]
                if matches!(
                    selections.as_slice(),
                    [CostObjectSelection {
                        objects: ObjectSet::One(ObjectRef::Query { .. }),
                        choose: None,
                    }]
                ) =>
            {
                let ObjectSet::One(object) = &selections[0].objects else {
                    unreachable!("just matched");
                };
                match self.object(object) {
                    Ok(read) => format!("({}).is_some() && ", self.fetch(read)),
                    Err(_) => String::new(),
                }
            }
            [CostPart::Sacrifice {
                objects: ObjectSet::One(object),
                choose: None,
            }] => match self.object(object) {
                Ok(read) => format!(
                    "({}).is_some_and(|o| g.can_sacrifice(payer_{id}, o)) && ",
                    self.fetch(read)
                ),
                Err(_) => String::new(),
            },
            [CostPart::Sacrifice {
                objects,
                choose: Some(Count::Exactly(Expr::Constant(n))),
            }] => match self.objects(objects) {
                Ok(read) => format!(
                    "({}).into_iter().filter(|&o| g.controller(o) == payer_{id} && g.can_sacrifice(payer_{id}, o)).count() >= {n} && ",
                    self.fetch(read)
                ),
                Err(_) => String::new(),
            },
            [CostPart::Draw {
                count: Expr::Constant(n),
            }] => format!("!cant_draw(g, payer_{id}, {n}) && "),
            [CostPart::Energy(Expr::Constant(n))] => {
                format!("g.state().player(payer_{id}).energy() >= {n} && ")
            }
            [CostPart::Discard {
                count: Expr::Constant(n),
                filter: ObjectFilter::Any,
                ..
            }] => format!("g.state().zone(Zone::hand(payer_{id})).len() >= {n} && "),
            [CostPart::ChangeTapState {
                selections,
                tapped: true,
                ..
            }] => match selections.as_slice() {
                [CostObjectSelection {
                    objects,
                    choose: Some(Count::Exactly(Expr::Constant(n))),
                }] => match self.objects(objects) {
                    Ok(read) => format!("({}).len() >= {n} && ", self.fetch(read)),
                    Err(_) => String::new(),
                },
                _ => String::new(),
            },
            [CostPart::PutCounters(placements)] => match placements.as_slice() {
                [CounterPlacement {
                    recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
                    ..
                }] => "g.is_on_battlefield(asker.source) && ".to_string(),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }
    /// The target that makes "two target creatures" of the one before it: the same
    /// target but for being another object. The printed words tell it from "target
    /// creature fights another target creature", whose targets differ.
    pub(super) fn two_targets(&self, announcement: &Announcement) -> Option<usize> {
        let [first, second] = announcement.targets.as_slice() else {
            return None;
        };
        let ObjectFilter::All(filters) = &second.filter else {
            return None;
        };
        let [ObjectFilter::Not(other), filter] = filters.as_slice() else {
            return None;
        };
        let another = matches!(&**other, ObjectFilter::SameObject(ObjectRef::Target { slot, index: 0 }) if slot.0 == first.slot.0);
        let single = |count: &Count| matches!(count, Count::Exactly(Expr::Constant(1)));
        let same = another
            && single(&first.count)
            && single(&second.count)
            && first.kind == second.kind
            && format!("{:?}", first.filter) == format!("{filter:?}")
            && first.constraints.is_empty()
            && second.constraints.is_empty();
        let printed = self
            .line
            .as_ref()
            .is_some_and(|line| line.text.contains("two target"));
        (same && printed).then_some(1)
    }
    /// "Up to three other target creatures from the battlefield and/or creature
    /// cards from graveyards": one count over two kinds of target is one slot, and
    /// the second is what's read of it.
    pub(super) fn limit_shared(announcement: &Announcement) -> Option<usize> {
        let [first, second] = announcement.targets.as_slice() else {
            return None;
        };
        let shared = matches!(second.constraints.as_slice(),
            [SelectionConstraint::SharesLimitWith(slot)] if slot.0 == first.slot.0);
        (shared
            && first.constraints.is_empty()
            && format!("{:?}", first.count) == format!("{:?}", second.count)
            && first.distribute.is_none()
            && second.distribute.is_none()
            && first.chooser.is_none()
            && second.chooser.is_none())
        .then_some(1)
    }
    /// What a block rule that lasts says, from the line that makes it: "can't be
    /// blocked except by creatures with haste", as the object has it for the turn.
    pub(super) fn block_rule_words(&self) -> Option<String> {
        let text = &self.line.as_ref()?.text;
        let at = text
            .find("can't be blocked")
            .or_else(|| text.find("can't block"))?;
        let words = text[at..].split('.').next()?.replace(" this turn", "");
        Some(capitalized(&words))
    }
    /// Whether the line being compiled names the hand, or there is no line to say.
    pub(super) fn says_from_hand(&self) -> bool {
        self.line
            .as_ref()
            .is_none_or(|line| line.text.contains("from your hand"))
    }
    /// The two answers of the question that offers an optional cost: without it
    /// first, as Cantrip's cards have them, and a scenario names them.
    pub(super) fn cost_labels(&self, cost: &Cost, parts: &mut Parts) {
        let (without, with) = match (cost.parts.as_slice(), &self.line) {
            _ if self.kicker_line.is_some() => (
                "kicking".to_string(),
                self.kicker_line.clone().unwrap_or_default(),
            ),
            ([CostPart::Teamwork(n)], _) => (
                "teamwork".to_string(),
                format!("Teamwork {n}: Tap any number of creatures you control with total power {n} or more"),
            ),
            (_, Some(line)) => match line.text.strip_prefix("Kicker\u{2014}") {
                Some(words) => (
                    "kicking".to_string(),
                    format!("Kicker\u{2014}{}", words.trim_end_matches('.')),
                ),
                None => match additional_cost_words(&line.text).strip_prefix("you may ") {
                    Some(words) => (gerund(words), capitalized(words)),
                    None => return,
                },
            },
            _ => return,
        };
        if !parts.fields.contains_key("labels") {
            let without = quoted(&format!("Cast without {without}"));
            parts.field("labels", format!("&[{without}, {}]", quoted(&with)));
        }
    }
    /// "Look at the top N cards of your library. Put M of them into your hand and
    /// the rest on the bottom": one question (Cantrip's `look_at_top`), the cards
    /// going to each place, not a choice and then an order. Whether `group` was it.
    pub(super) fn dig(&mut self, group: &Program, out: &mut String) -> Result<bool> {
        if self.rest_to_graveyard(group, out)? {
            return Ok(true);
        }
        let [Instruction::Effect {
            operation:
                Operation::LookAt {
                    player,
                    objects:
                        ObjectSet::Take {
                            objects: looked,
                            count: depth,
                            from,
                        },
                },
            ..
        }, rest @ ..] = group.instructions.as_slice()
        else {
            return Ok(false);
        };
        let library = matches!(
            &**looked,
            ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    ..
                },
                filter: ObjectFilter::Any,
                ..
            }
        );
        if format!("{from:?}") != "Top" || !library {
            return Ok(false);
        }
        let looker = self.fetch(self.certain_player(player)?);
        let depth = self.fetch(self.number(depth)?.amount(u32::MAX.into(), "usize")?);
        let at = (looker.as_str(), depth.as_str());
        match rest {
            // "Put up to one of them on top of your library and the rest on the
            // bottom": one question, as what's taken to the hand is.
            [Instruction::Effect {
                result: chosen,
                operation:
                    Operation::ChooseObjects {
                        player: chooser,
                        objects: ObjectSet::InZone { .. },
                        count: Count::UpTo(Expr::Constant(most)),
                        constraints,
                    },
            }, Instruction::Effect {
                operation:
                    Operation::ReorderLibrary {
                        objects: ObjectSet::Selected(selected),
                        end: SequenceEnd::Top,
                        order: CardOrder::Chosen,
                        ..
                    },
                ..
            }, Instruction::Effect {
                operation:
                    Operation::ReorderLibrary {
                        end: SequenceEnd::Bottom,
                        order: rest_order @ (CardOrder::Random | CardOrder::Chosen),
                        ..
                    },
                ..
            }] if format!("{player:?}") == format!("{chooser:?}")
                && selected.0 == chosen.0
                && constraints.is_empty()
                && u8::try_from(*most).is_ok() =>
            {
                let id = self.fresh();
                let bottom = match rest_order {
                    CardOrder::Random => {
                        format!("g.bottom_in_random_order(player_{id}, &placements[1]);")
                    }
                    _ => format!("g.arrange_library(player_{id}, &[], &placements[1]);"),
                };
                writeln!(out, "let player_{id} = {looker}; let depth_{id} = {depth}; let looked_{id} = cantrip_mechanics::top_of_library(g, player_{id}, depth_{id}); if !looked_{id}.is_empty() {{ g.look_at(player_{id}, &looked_{id}); let ask = RequestKind::DistributeCards {{ source: asker.source, cards: looked_{id}.clone(), destinations: vec![Destination {{ kind: DestinationKind::LibraryTop, min: 0, max: {most}, eligible: looked_{id}.clone() }}, Destination {{ kind: DestinationKind::LibraryBottom, min: 0, max: u8::MAX, eligible: looked_{id}.clone() }}], may_shuffle: false }}; if let Answer::Placements {{ placements, .. }} = g.ask(player_{id}, ask)? {{ g.arrange_library(player_{id}, &placements[0], &[]); {bottom} }} }}").unwrap();
                Ok(true)
            }
            // "Put one of them into your hand, put one of them on the bottom of your
            // library, and exile one of them": one question with three places, each
            // filled in the order written as far as the cards go (608.2c).
            [Instruction::Effect {
                result: to_hand,
                operation:
                    Operation::ChooseObjects {
                        objects: ObjectSet::InZone { .. },
                        count: Count::AsManyAsCan(Expr::Constant(1)),
                        ..
                    },
            }, Instruction::Effect {
                operation:
                    Operation::Move {
                        objects: ObjectSet::Selected(hand),
                        destination: Destination::OwnersHand,
                        ..
                    },
                ..
            }, Instruction::Effect {
                result: to_bottom,
                operation:
                    Operation::ChooseObjects {
                        objects: ObjectSet::InZone { .. },
                        count: Count::AsManyAsCan(Expr::Constant(1)),
                        ..
                    },
            }, Instruction::Effect {
                operation:
                    Operation::ReorderLibrary {
                        objects: ObjectSet::Selected(bottom),
                        end: SequenceEnd::Bottom,
                        ..
                    },
                ..
            }, Instruction::Effect {
                result: exiled,
                operation:
                    Operation::Move {
                        objects: ObjectSet::InZone { .. },
                        destination: Destination::Exile,
                        ..
                    },
            }] if hand.0 == to_hand.0 && bottom.0 == to_bottom.0 => {
                let id = self.fresh();
                writeln!(out, "let player_{id} = {looker}; let depth_{id} = {depth}; let looked_{id} = cantrip_mechanics::top_of_library(g, player_{id}, depth_{id}); let mut moved_{id}: Vec<ObjectId> = Vec::new(); if !looked_{id}.is_empty() {{ let hand = looked_{id}.len().min(1) as u8; let bottom = (looked_{id}.len() - hand as usize).min(1) as u8; let exile = (looked_{id}.len() - hand as usize - bottom as usize).min(1) as u8; let ask = RequestKind::DistributeCards {{ source: asker.source, cards: looked_{id}.clone(), destinations: vec![Destination {{ kind: DestinationKind::Hand, min: hand, max: hand, eligible: looked_{id}.clone() }}, Destination {{ kind: DestinationKind::LibraryBottom, min: bottom, max: bottom, eligible: looked_{id}.clone() }}, Destination {{ kind: DestinationKind::Exile, min: exile, max: exile, eligible: looked_{id}.clone() }}], may_shuffle: false }}; if let Answer::Placements {{ placements, .. }} = g.ask(player_{id}, ask)? {{ g.return_to_hand(&placements[0]); g.arrange_library(player_{id}, &[], &placements[1]); moved_{id} = g.exile_all(&placements[2]); }} }}").unwrap();
                self.moved.insert(exiled.0, format!("moved_{id}"));
                Ok(true)
            }
            // "If you gained life this turn, you may instead reveal two."
            [Instruction::If {
                condition,
                then,
                otherwise,
            }] => {
                let id = (self.fresh(), self.fresh());
                let taking = (
                    taking_from(player, &then.instructions, id.0),
                    taking_from(player, &otherwise.instructions, id.1),
                );
                let (Some(then), Some(otherwise), Ok(test)) =
                    (taking.0, taking.1, self.condition(condition))
                else {
                    return Ok(false);
                };
                writeln!(out, "if {} {{", self.fetch(test)).unwrap();
                self.take_looked(then, at, id.0, out);
                writeln!(out, "}} else {{").unwrap();
                self.take_looked(otherwise, at, id.1, out);
                writeln!(out, "}}").unwrap();
                Ok(true)
            }
            rest => {
                let id = self.fresh();
                let Some(taking) = taking_from(player, rest, id) else {
                    return Ok(false);
                };
                self.take_looked(taking, at, id, out);
                Ok(true)
            }
        }
    }
    /// "Reveal the top four cards. You may put a land card from among them into
    /// your hand. Put the rest into your graveyard": Cantrip's one question.
    fn rest_to_graveyard(&mut self, group: &Program, out: &mut String) -> Result<bool> {
        if group.instructions.last().is_some_and(
            |last| matches!(last, Instruction::Effect { result, .. } if self.read_later(result)),
        ) {
            return Ok(false);
        }
        let [Instruction::Effect {
            operation: first, ..
        }, Instruction::Effect {
            result: chosen,
            operation:
                Operation::ChooseObjects {
                    player,
                    objects,
                    count,
                    constraints,
                },
        }, Instruction::Effect {
            operation:
                Operation::Move {
                    objects: ObjectSet::Selected(selected),
                    destination: Destination::OwnersHand,
                    entry,
                    face_down: false,
                },
            ..
        }, Instruction::Effect {
            operation:
                Operation::Move {
                    destination: Destination::OwnersGraveyard,
                    ..
                },
            ..
        }] = group.instructions.as_slice()
        else {
            return Ok(false);
        };
        let (taken, reveal) = match first {
            Operation::Reveal { objects } => (objects, true),
            Operation::LookAt { objects, .. } => (objects, false),
            _ => return Ok(false),
        };
        let ObjectSet::Take {
            objects: top,
            count: depth,
            from,
        } = taken
        else {
            return Ok(false);
        };
        let library = matches!(
            &**top,
            ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    ..
                },
                filter: ObjectFilter::Any,
                ..
            }
        );
        let filter = match objects {
            ObjectSet::Matching { objects, filter }
                if matches!(&**objects, ObjectSet::InZone { .. }) =>
            {
                filter
            }
            ObjectSet::InZone { .. } => &ObjectFilter::Any,
            _ => return Ok(false),
        };
        let plain = format!("{from:?}") == "Top"
            && library
            && selected.0 == chosen.0
            && entry.counters.is_empty()
            && !entry.tapped
            && !constraints
                .iter()
                .any(|c| !matches!(c, SelectionConstraint::CanMoveTo { .. }));
        if !plain {
            return Ok(false);
        }
        let mut lets = Vec::new();
        let id = self.fresh();
        // What the card passes to is a plain function: no words to read.
        let call = match (reveal, count, chars_test(filter, id, &mut lets)) {
            (true, Count::UpTo(Expr::Constant(1)), Some(test)) if lets.is_empty() => {
                format!("reveal_top_take_one_rest_to_graveyard(g, player_{id}, asker.source, depth_{id}, |c| {test})")
            }
            (
                false,
                Count::Exactly(Expr::Constant(1)) | Count::AsManyAsCan(Expr::Constant(1)),
                Some(test),
            ) if test == "true" => {
                format!("look_at_top_take_one_rest_to_graveyard(g, player_{id}, asker.source, depth_{id})")
            }
            _ => return Ok(false),
        };
        let looker = self.fetch(self.certain_player(player)?);
        let depth = self.fetch(self.number(depth)?.amount(u32::MAX.into(), "usize")?);
        writeln!(out, "let player_{id} = {looker}; let depth_{id} = {depth}; cantrip_mechanics::library::{call}?;").unwrap();
        Ok(true)
    }
    fn take_looked(
        &mut self,
        taking: Taking,
        (looker, depth): (&str, &str),
        id: usize,
        out: &mut String,
    ) {
        let Taking {
            chosen,
            put,
            reveal,
            least,
            most,
            lets,
            wanted,
            rest,
        } = taking;
        writeln!(out, "let player_{id} = {looker}; let depth_{id} = {depth}; {} let taken_{id} = cantrip_mechanics::library::look_at_top(g, player_{id}, asker.source, depth_{id}, ({least}, {most}), {reveal}, {wanted}, cantrip_mechanics::library::Rest::{rest})?;", lets.join(" ")).unwrap();
        if most == 1 {
            self.single.insert(chosen.0);
        }
        self.selections.insert(chosen.0, format!("taken_{id}"));
        self.moved.insert(put.0, format!("taken_{id}"));
        self.left_behind(put, format!("taken_{id}.clone()"));
    }
    /// What deals damage or is copied: a target only if it is still legal, since one
    /// that isn't does nothing and nothing is done to it (CR 608.2b).
    pub(super) fn damage_source(&self, source: &ObjectRef) -> Result<String> {
        let read = self.object(source)?;
        match source {
            ObjectRef::Target { slot, index }
                if self.choosing.is_none() && self.slots.contains_key(&slot.0) =>
            {
                let i = self.slot(slot)?;
                Ok(format!(
                    "t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).and_then(|t: Target| t.object())"
                ))
            }
            _ => Ok(read),
        }
    }
    /// Which of `body`'s payments are read as "unless", where what follows is for
    /// not paying.
    pub(super) fn note_unless(&mut self, body: &Program) {
        if !self
            .line
            .as_ref()
            .is_some_and(|line| line.text.contains("unless "))
        {
            return;
        }
        for instruction in &body.instructions {
            if let Instruction::If { condition, .. } = instruction {
                if let Condition::Not(inner) = condition {
                    if let Condition::PaymentChosen(result) = &**inner {
                        self.unless_paid.insert(result.0);
                    }
                }
            }
        }
    }
    /// A pump's amount, unless the card says it doubles.
    pub(super) fn pump_amount(&self, amount: &Expr, code: String) -> String {
        let doubles = self.line.as_ref().is_some_and(|l| l.text.contains("ouble"));
        match doubles {
            true => code,
            false => not_negative(amount, code),
        }
    }
    /// The key of "you may ... Do this only once each turn": declining spends
    /// nothing, where "triggers only once each turn" spends the trigger.
    pub(super) fn done_once(&mut self, limits: &TriggerLimits, body: &Program) -> Option<u8> {
        let said = self
            .line
            .as_ref()
            .is_some_and(|line| line.text.contains("Do this only once each turn"));
        if said && limits.per_turn == Some(1) && looks_then_may(body).is_some() {
            let key = self.once_key();
            self.do_once = Some(key);
            return Some(key);
        }
        let [Instruction::Optional {
            decider,
            body: offered,
        }] = body.instructions.as_slice()
        else {
            return None;
        };
        if !said
            || limits.per_turn != Some(1)
            || tap_or_untap_only(offered)
            || !matches!(decider, PlayerRef::AbilityController)
        {
            return None;
        }
        let key = self.once_key();
        self.do_once = Some(key);
        Some(key)
    }
    /// A new key for a thing done once each turn, or the one its sentence's other
    /// abilities spend: "whenever you cast ... or activate ..., you may ... Do
    /// this only once each turn" is done once for both (Glava, Five-Advents Mage).
    fn once_key(&mut self) -> u8 {
        if let Some(Some(key)) = self.once_shared {
            return key;
        }
        let key = 200 + (self.fresh() % 50) as u8;
        if self.once_shared.is_some() {
            self.once_shared = Some(Some(key));
        }
        key
    }
    /// Reads the printed lines of the counter stage starting at `minimum`.
    pub(super) fn on_stage(&mut self, minimum: i64) {
        self.stage_lines = Some(self.stages.get(&minimum).cloned().unwrap_or_default());
    }
    /// The words of an effect that waits to apply, as `GrantedReplacementDef::text`
    /// and `GrantedPreventionDef::text` want them: from the sentence of its line that
    /// makes it to the end of the line, since what it does besides is its words too
    /// (CR 609.7a, 615.5). Cantrip holds them to Oracle text, so they are never reworded.
    pub(super) fn waiting_words(&self) -> String {
        let Some(line) = &self.line else {
            return String::new();
        };
        let makes = |sentence: &str| {
            let sentence = sentence.to_lowercase();
            ["prevent", "would", "instead", "the next time", "enters with", "enter with"]
                .iter()
                .any(|word| sentence.contains(word))
        };
        // A mode prints on a line of its own, which is what Oracle text holds.
        if let Some(mode) = line.modes.iter().find(|mode| makes(mode)) {
            let sentences = sentences(mode);
            return self.printed_run(&sentences, 0, sentences.len());
        }
        let sentences = effect_sentences(line.kind, &line.head);
        match sentences.iter().position(|sentence| makes(sentence)) {
            Some(at) => self.printed_run(&sentences, at, sentences.len()),
            None => past_cost(line.kind, &line.text).to_string(),
        }
    }
    /// Whether the face prints `passage`, as Cantrip holds a waiting text to its
    /// card's Oracle text: by its words, its own reminder text aside, whatever its
    /// case or final period.
    fn prints_passage(&self, passage: &str) -> bool {
        let words = oracle_words(&without_reminder(passage));
        let words = words.trim_end_matches('.').trim_end();
        !words.is_empty() && oracle_words(&self.face_text).contains(words)
    }
    /// `sentences[from..to]`, or as many of them from `from` as the face prints
    /// together: a passage can't reach past reminder text the line has lost.
    fn printed_run(&self, sentences: &[String], from: usize, to: usize) -> String {
        let run = |to: usize| sentences[from..to].join(" ");
        (from + 1..=to)
            .rev()
            .map(run)
            .find(|passage| self.prints_passage(passage))
            .unwrap_or_else(|| run(from + 1))
    }
    /// The words of a delayed triggered ability (CR 603.7), or of a reflexive one
    /// if `reflexive` (CR 603.12), as `TriggeredDef::text` wants them: the sentence
    /// of its line that makes it, as Oracle text prints it. A delayed ability is
    /// its sentence; a reflexive one, "When you do, ...", runs to the end of its
    /// line. A granted ability's line is what the granting line quotes.
    pub(super) fn delayed_words(&mut self, reflexive: bool, fallback: String) -> String {
        let Some(line) = self.line.clone() else {
            return self.said(Said::Triggered, fallback);
        };
        let blocks: Vec<(Said, String)> = match self.granting {
            true => line.quotes.clone(),
            false => {
                let modes = line.modes.iter().map(|mode| (Said::Sentence, mode.clone()));
                modes.chain([(line.kind, line.head.clone())]).collect()
            }
        };
        let makes = |sentence: &str| {
            let sentence = sentence.to_lowercase();
            match reflexive {
                true => sentence.starts_with("when "),
                false => {
                    sentence.starts_with("when")
                        || [
                            "at the beginning of",
                            "at end of combat",
                            "at the end of",
                            ", when",
                            "the next time",
                        ]
                        .iter()
                        .any(|words| sentence.contains(words))
                }
            }
        };
        let mut first = None;
        for (kind, block) in &blocks {
            let sentences = effect_sentences(*kind, block);
            for at in 0..sentences.len() {
                if !makes(&sentences[at]) {
                    continue;
                }
                let to = if reflexive { sentences.len() } else { at + 1 };
                let words = self.printed_run(&sentences, at, to);
                // A triggered ability's first sentence is its own trigger.
                if at == 0 && *kind == Said::Triggered {
                    first.get_or_insert(words);
                    continue;
                }
                return words;
            }
        }
        first.unwrap_or_else(|| self.said(Said::Triggered, fallback))
    }
    /// A flip whose line speaks of heads or tails has no winner (CR 705.2), and
    /// the rest are won or lost. The AST says the same words either way.
    pub(super) fn flip_call(&self) -> Result<&'static str> {
        let line = self
            .line_text()
            .ok_or_else(|| gap("a coin flip with no printed line"))?;
        let called = line
            .split(|c: char| !c.is_alphabetic())
            .any(|word| word.eq_ignore_ascii_case("heads") || word.eq_ignore_ascii_case("tails"));
        Ok(if called { "flip_coin_for_heads" } else { "flip_coin" })
    }
    /// The printed line being compiled.
    pub(super) fn line_text(&self) -> Option<&str> {
        self.line.as_ref().map(|line| line.text.as_str())
    }
    /// "Look at the top three cards of your library, then put them back in any
    /// order. You may shuffle": one question, an order or a shuffle, so nobody
    /// orders cards they're about to shuffle away.
    fn order_or_shuffle(&mut self, rest: &[Instruction], out: &mut String) -> bool {
        let [Instruction::Group(group), Instruction::Optional {
            decider: PlayerRef::AbilityController,
            body,
        }, ..] = rest
        else {
            return false;
        };
        let [Instruction::Effect {
            operation:
                Operation::LookAt {
                    player: PlayerRef::AbilityController,
                    objects: ObjectSet::Take { objects: looked, count, from },
                },
            ..
        }, Instruction::Effect {
            operation:
                Operation::ReorderLibrary {
                    objects: ObjectSet::InZone { .. },
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                    ..
                },
            ..
        }] = group.instructions.as_slice()
        else {
            return false;
        };
        let [Instruction::Effect {
            operation:
                Operation::Shuffle {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    excluding: None,
                },
            ..
        }] = body.instructions.as_slice()
        else {
            return false;
        };
        let library = matches!(
            &**looked,
            ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    players: PlayerSet::One(PlayerRef::AbilityController),
                },
                filter: ObjectFilter::Any,
                ..
            }
        );
        let Expr::Constant(depth) = count else {
            return false;
        };
        if format!("{from:?}") != "Top" || !library || !self.stack_item {
            return false;
        }
        let id = self.fresh();
        writeln!(out, "let looked_{id} = cantrip_mechanics::top_of_library(g, item.controller, {depth}); if !looked_{id}.is_empty() {{ let ask = RequestKind::DistributeCards {{ source: item.object, cards: looked_{id}.clone(), destinations: vec![Destination {{ kind: DestinationKind::LibraryTop, min: 0, max: u8::MAX, eligible: looked_{id} }}], may_shuffle: true }}; if let Answer::Placements {{ placements, shuffle }} = g.ask(item.controller, ask)? {{ if shuffle {{ g.shuffle(item.controller); }} else {{ g.arrange_library(item.controller, &placements[0], &[]); }} }} }}").unwrap();
        true
    }
    /// Some line of the face prints these words.
    pub(super) fn card_prints(&self, words: &str) -> bool {
        self.face_text.contains(words)
    }
    /// "You may exile that card from your graveyard" isn't asked of a card that's
    /// left it.
    pub(super) fn asked_if_there(&self, body: &Program) -> String {
        let mut first = body;
        while let [Instruction::Group(group), ..] = first.instructions.as_slice() {
            first = group;
        }
        match first.instructions.first() {
            Some(Instruction::Effect {
                operation:
                    Operation::Move {
                        objects: ObjectSet::One(object @ ObjectRef::Query { objects, .. }),
                        ..
                    },
                ..
            }) if matches!(&**objects, ObjectSet::InZone { .. }) => match self.object(object) {
                Ok(read) => format!("({}).is_some() && ", self.fetch(read)),
                Err(_) => String::new(),
            },
            // "You may put the top card into your graveyard": not asked of no card.
            Some(Instruction::Effect {
                operation:
                    Operation::Move {
                        objects: objects @ ObjectSet::Take { .. },
                        ..
                    },
                ..
            }) => match self.objects(objects) {
                Ok(read) => format!("!({}).is_empty() && ", self.fetch(read)),
                Err(_) => String::new(),
            },
            // "You may attach this Equipment to it": not asked if it can't be (701.3b).
            Some(Instruction::Effect {
                operation:
                    Operation::Attach {
                        attachments: ObjectSet::One(ObjectRef::Source),
                        host,
                    },
                ..
            }) => match self.object(host) {
                Ok(read) => format!(
                    "cantrip_mechanics::targets::permanent(g, asker.source) && ({}).is_some_and(|o| cantrip_mechanics::targets::permanent(g, o) && g.chars(o).is(CardType::Creature)) && ",
                    self.fetch(read)
                ),
                Err(_) => String::new(),
            },
            // "You may remove a counter from it": not asked of what has none to remove.
            Some(Instruction::Effect {
                operation: Operation::RemoveCounters { removals },
                ..
            }) => match removals.as_slice() {
                [CounterRemoval {
                    recipients: Recipients::Objects(ObjectSet::One(object)),
                    kind: Some(kind),
                    amount: CounterAmount::Count(Count::Exactly(Expr::Constant(1))),
                    ..
                }] => match (self.object(object), read::counter_kind(kind)) {
                    (Ok(read), Ok(kind)) => format!(
                        "({}).is_some_and(|o| cantrip_mechanics::targets::permanent(g, o) && g.obj(o).counters({kind}) > 0) && ",
                        self.fetch(read)
                    ),
                    _ => String::new(),
                },
                _ => String::new(),
            },
            _ => String::new(),
        }
    }
    /// Whether the card can be spliced onto a spell.
    pub(super) fn spliced(&self) -> bool {
        self.printed
            .iter()
            .flatten()
            .any(|line| line.text.starts_with("Splice onto"))
    }
    /// The target numbers of a spell and of a permanent whose control is exchanged.
    pub(super) fn exchanges_a_spell(&self, a: &ObjectRef, b: &ObjectRef) -> Option<(usize, usize)> {
        let (ObjectRef::Target { slot: first, .. }, ObjectRef::Target { slot: second, .. }) =
            (a, b)
        else {
            return None;
        };
        let (spell, spec) = self.slots.get(&first.0)?;
        let (permanent, _) = self.slots.get(&second.0)?;
        match &spec.kind {
            TargetKind::Object(domain) if domain.zones == [EventZone::Stack] => {
                Some((*spell, *permanent))
            }
            _ => None,
        }
    }
    /// The player just picked is asked for even with one to pick from.
    pub(super) fn name_even_the_one(&self) {
        for statement in &mut self.picked.borrow_mut().statements {
            *statement = statement.replace("if among.len() > 1", "if !among.is_empty()");
        }
    }
    /// "Each opponent discards two cards unless they discard a nonland card": each
    /// player's one choice, in turn order, then the cards go together.
    pub(super) fn each_discards_two(
        &mut self,
        players: &PlayerSet,
        binding: &Binding,
        steps: &Program,
        out: &mut String,
    ) -> bool {
        let [Instruction::Effect {
            result,
            operation:
                Operation::PayCost {
                    player: PlayerRef::Bound(payer),
                    cost,
                    optional: true,
                },
        }, Instruction::If {
            condition: Condition::Not(unpaid),
            then,
            otherwise,
        }] = steps.instructions.as_slice()
        else {
            return false;
        };
        let [Instruction::Effect {
            operation:
                Operation::Discard {
                    players: PlayerSet::One(PlayerRef::Bound(discarder)),
                    amount: Expr::Constant(2),
                    random: false,
                },
            ..
        }] = then.instructions.as_slice()
        else {
            return false;
        };
        let answered = matches!(&**unpaid, Condition::PaymentChosen(paid) if paid.0 == result.0);
        if !answered
            || !otherwise.instructions.is_empty()
            || !self.stack_item
            || payer.0 != binding.0
            || discarder.0 != binding.0
        {
            return false;
        }
        let id = self.fresh();
        let mut lets = Vec::new();
        let Some(filters) = discards_one(cost) else {
            return false;
        };
        let tests: Option<Vec<String>> = filters
            .iter()
            .map(|filter| chars_test(filter, id, &mut lets).map(|test| format!("({test})")))
            .collect();
        let (Some(tests), Some(line), Ok(among)) = (tests, &self.line, self.players(players))
        else {
            return false;
        };
        let Some((_, words)) = line.text.split_once("unless they ") else {
            return false;
        };
        if !lets.is_empty() {
            return false;
        }
        let words = capitalized(words.split('.').next().unwrap_or(words));
        let among = self.fetch(among);
        let test = tests.join(" || ");
        writeln!(out, "let among_{id}: Vec<PlayerId> = {among}; let order_{id}: Vec<PlayerId> = g.state().apnap().filter(|p| among_{id}.contains(p)).collect(); cantrip_mechanics::discard_two_unless_discarding(g, &order_{id}, asker.source, |c| {test})?;").unwrap();
        self.wanted_labels = Some(format!("&[{}, \"Discard two cards\"]", quoted(&words)));
        true
    }
    /// A payment and what hangs on it, where they're one question of the card's
    /// `labels`: whether these instructions were that.
    pub(super) fn one_question(&mut self, rest: &[Instruction], out: &mut String) -> bool {
        if self.order_or_shuffle(rest, out) {
            return true;
        }
        let [Instruction::Effect {
            result,
            operation:
                Operation::PayCost {
                    player: PlayerRef::AbilityController,
                    cost,
                    optional: true,
                },
        }, Instruction::If {
            condition,
            then,
            otherwise,
        }, ..] = rest
        else {
            return false;
        };
        if !otherwise.instructions.is_empty() || !self.stack_item {
            return false;
        }
        let you =
            |players: &PlayerSet| matches!(players, PlayerSet::One(PlayerRef::AbilityController));
        let then = then.instructions.as_slice();
        let (call, labels) = match (cost.parts.as_slice(), condition, then) {
            // "You may sacrifice an artifact or discard a card. If you do, draw a card."
            ([CostPart::Choice(options)], Condition::PaymentChosen(paid), [Instruction::Effect { operation: Operation::Draw { players, amount: Expr::Constant(n) }, .. }])
                if paid.0 == result.0 && you(players) && sacrifice_or_discard(options) =>
            {
                (
                    format!("cantrip_mechanics::effects::may_sacrifice_an_artifact_or_discard_to_draw(g, item, {n})?;"),
                    Some("&[\"Sacrifice an artifact\", \"Discard a card\", \"Neither\"]".to_string()),
                )
            }
            // "You may draw a card. If you do, discard a card": "if you do" is that
            // a card was drawn, and an empty library draws none.
            ([CostPart::Draw { count: Expr::Constant(1) }], Condition::PaymentChosen(paid), [Instruction::Effect { operation: Operation::Discard { players, amount: Expr::Constant(1), random: false }, .. }])
                if paid.0 == result.0 && you(players) =>
            {
                ("cantrip_mechanics::effects::may_draw_then_discard(g, item)?;".to_string(), None)
            }
            // "Discard two cards unless you discard an artifact card."
            (_, Condition::Not(unpaid), [Instruction::Effect { operation: Operation::Discard { players, amount: Expr::Constant(2), random: false }, .. }])
                if matches!(&**unpaid, Condition::PaymentChosen(paid) if paid.0 == result.0) && you(players) =>
            {
                let mut lets = Vec::new();
                let id = self.fresh();
                let (Some(filters), Some(line)) = (discards_one(cost), &self.line) else {
                    return false;
                };
                let tests: Option<Vec<String>> = filters
                    .iter()
                    .map(|filter| chars_test(filter, id, &mut lets).map(|test| format!("({test})")))
                    .collect();
                let (Some(tests), Some((_, words))) = (tests, line.text.split_once("unless you ")) else {
                    return false;
                };
                if !lets.is_empty() {
                    return false;
                }
                let test = tests.join(" || ");
                let words = capitalized(words.split('.').next().unwrap_or(words));
                (
                    format!("cantrip_mechanics::discard_two_unless_discarding(g, &[asker.controller], asker.source, |c| {test})?;"),
                    Some(format!("&[{}, \"Discard two cards\"]", quoted(&words))),
                )
            }
            _ => return false,
        };
        writeln!(out, "{call}").unwrap();
        if labels.is_some() {
            self.wanted_labels = labels;
        }
        true
    }
    /// `count`, or up to it if this is the choice of a "you may".
    pub(super) fn offered_count(&mut self, count: &Count) -> Count {
        match (std::mem::take(&mut self.may_choose), count) {
            (true, Count::Exactly(n) | Count::AsManyAsCan(n)) => Count::UpTo(n.clone()),
            _ => count.clone(),
        }
    }
    /// Whether `code` reads something only the running ability holds, so that it
    /// can't stand in a function of its own.
    pub(super) fn reads_a_local(&self, code: &str) -> bool {
        code.contains("willing_")
            || code.contains("bound_")
            || self
                .chosen
                .iter()
                .any(|(_, local)| code.contains(local.as_str()))
    }
    /// Past a choice of a "you may", only what was chosen goes on.
    pub(super) fn open_guard(&mut self, chosen: &str, out: &mut String) {
        if let Some(code) = self.chosen_guard.take() {
            writeln!(out, "if !{chosen}.is_empty() {{ {code}").unwrap();
            self.guard_opened += 1;
        }
    }
    /// A block whose first choice is a "you may": `chosen` runs if it found anything.
    pub(super) fn block_choosing_first(
        &mut self,
        body: &Program,
        chosen: String,
        out: &mut String,
    ) -> Result<()> {
        // One may sit inside another: each closes what it opened.
        let opened = std::mem::take(&mut self.guard_opened);
        let guard = self.chosen_guard.replace(chosen);
        let may = std::mem::replace(&mut self.may_choose, true);
        let done = self.block(body, out, true);
        self.may_choose = may;
        self.chosen_guard = guard;
        let shut = std::mem::replace(&mut self.guard_opened, opened);
        writeln!(out, "{}", "}".repeat(shut)).unwrap();
        done
    }
    /// Read a face's Oracle text, once, before its abilities are compiled.
    pub(super) fn read_printed(&mut self, abilities: &[Ability], oracle: Option<&str>) {
        self.kickers = mechanics::kickers(abilities);
        self.face_text = oracle.unwrap_or_default().to_string();
        let Some(oracle) = oracle else {
            return;
        };
        let printed = Printed::read(oracle);
        self.printed = printed.aligned(abilities);
        self.chapters = printed.chapters;
        self.stages = printed.stages;
        self.spell_targets = abilities.iter().any(|ability| {
            matches!(ability, Ability::Spell { announcement, .. } if !announcement.targets.is_empty())
        });
        self.kicker_line = printed
            .lines
            .iter()
            .find(|line| line.text.starts_with("Kicker\u{2014}"))
            .map(|line| line.text.trim_end_matches('.').to_string());
    }
    /// The ability at `at` is the next one compiled.
    pub(super) fn on_ability(&mut self, at: usize) {
        self.line = self.printed.get(at).cloned().flatten();
        self.quoted_used.clear();
        self.unless_paid.clear();
    }
    /// The printed line of what's being compiled, as `kind`; a granted ability
    /// reads what the line quotes.
    pub(super) fn said(&mut self, kind: Said, fallback: String) -> String {
        self.said_of(kind, false, fallback)
    }
    /// As `said`, a counter stage's power and toughness when `stats`: on a stage,
    /// its own lines, never the line that holds the stages (Caravan Escort's
    /// "Level up {2}").
    pub(super) fn said_of(&mut self, kind: Said, stats: bool, fallback: String) -> String {
        if let Some(lines) = &mut self.stage_lines {
            // The next line of the kind; once each is said, the last again, as
            // "LEVEL 4+: Flying, vigilance" is the words of both its keywords; with
            // none of the kind, the stage's first, as Class F Shuttlecraft's "6+ |
            // Flying, vigilance" is also the words of its becoming a creature.
            let fits = |line: &Line| line.stats == stats && (stats || line.kind == kind);
            let next = lines.iter().position(|line| fits(line) && !line.said);
            // What a stage's line quotes, for what it grants: Joraga Treespeaker's
            // "LEVEL 5+: Elves you control have "{T}: Add {G}{G}."".
            if next.is_none() && self.granting {
                let quoted = lines.iter().flat_map(|line| &line.quotes).find(|(said, _)| *said == kind);
                if let Some((_, quote)) = quoted {
                    return quote.clone();
                }
            }
            let Some(at) = next
                .or_else(|| lines.iter().rposition(fits))
                .or_else(|| lines.iter().position(|line| !line.stats))
                .or((!lines.is_empty()).then_some(0))
            else {
                return fallback;
            };
            let line = &mut lines[at];
            // A stage has its activated and triggered abilities by a static that
            // grants them (CR 711.2a, 721.2a). The ability is said without its
            // stage, and the static that grants it with it (Adagia's "12+ | {3}{W},
            // {T}: ..."), whichever is compiled first.
            if matches!(line.kind, Said::Activated | Said::Triggered) && !line.said {
                line.kind = Said::Sentence;
                return line.text[line.staged..].to_string();
            }
            line.said = true;
            return line.text.clone();
        }
        let Some(line) = &self.line else {
            return fallback;
        };
        if !self.granting {
            return line.text.clone();
        }
        let of_kind: Vec<usize> = (0..line.quotes.len())
            .filter(|&at| line.quotes[at].0 == kind)
            .collect();
        let Some(&first) = of_kind.first() else {
            return fallback;
        };
        let at = of_kind
            .iter()
            .copied()
            .find(|at| !self.quoted_used.contains(at))
            .unwrap_or(first);
        self.quoted_used.insert(at);
        line.quotes[at].1.clone()
    }
    /// The printed lines the ability being compiled may be among.
    pub(super) fn lines_said(&self) -> Vec<String> {
        if let Some(line) = &self.line {
            if self.granting {
                return line.quotes.iter().map(|(_, q)| q.clone()).collect();
            }
            return vec![line.text.clone()];
        }
        let staged = self.stage_lines.iter().flatten().filter(|line| !line.said).map(|line| line.text.clone());
        let chapters = self.chapters.values().cloned();
        staged.chain(chapters).collect()
    }
    /// The printed words of mode `at` of `count`.
    pub(super) fn said_mode(&self, at: usize, count: usize, fallback: String) -> String {
        match &self.line {
            Some(line) if line.modes.len() == count => line.modes[at].clone(),
            _ => fallback,
        }
    }
    /// The chapter `number` is the next thing compiled: its words are what it does,
    /// past its numerals.
    pub(super) fn on_chapter(&mut self, number: i64) {
        self.line = self.chapters.get(&number).map(|line| {
            let words = chapter_numbers(line).map_or(line.as_str(), |(_, words)| words);
            line_of(Said::Sentence, words.to_string(), words)
        });
        self.quoted_used.clear();
    }
    /// The printed line of a Saga's chapter.
    pub(super) fn said_chapter(&self, number: i64, fallback: String) -> String {
        self.chapters.get(&number).cloned().unwrap_or(fallback)
    }
    /// A token or emblem's place: what the line that makes it quotes is its text.
    pub(super) fn quoted_text(&self) -> lane_f::Place {
        let quotes = self.line.iter().flat_map(|line| &line.quotes);
        let text: Vec<&str> = quotes.map(|(_, quote)| quote.as_str()).collect();
        lane_f::Place {
            printed: (!text.is_empty()).then(|| text.join("\n")),
            ..lane_f::Place::default()
        }
    }
}
