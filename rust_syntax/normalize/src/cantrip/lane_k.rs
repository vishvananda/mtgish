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
    /// The bulleted lines under it, without their bullets.
    modes: Vec<String>,
    /// What it quotes: the text of an ability it grants or a token has.
    quotes: Vec<(Said, String)>,
}

/// A face's printed text, in the order it reads.
#[derive(Debug, Default)]
pub(super) struct Printed {
    lines: Vec<Line>,
    /// A Saga's chapters, by number. "I, II — ..." is the text of both.
    chapters: BTreeMap<i64, String>,
}

fn first_word_is(line: &str, words: &[&str]) -> bool {
    words.iter().any(|word| {
        line.strip_prefix(word)
            .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// A line past its ability word (CR 207.2c), which has no game function.
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
    words.split(". ").next().unwrap_or(words).trim_end_matches('.')
}

/// The costs a line offers, each starting with its verb: "or" alone also joins
/// the objects of one ("sacrifice an artifact or creature").
fn cost_options(words: &str) -> Vec<String> {
    const VERBS: [&str; 11] = [
        "pay", "sacrifice", "discard", "blight", "behold", "exile", "tap", "reveal", "remove",
        "return", "put",
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
    options.map(|option| capitalized(option.trim_end_matches(','))).collect()
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
    match line.strip_prefix('[').and_then(|rest| rest.split_once("]: ")) {
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
        text,
        modes: Vec::new(),
    }
}

impl Printed {
    pub(super) fn read(oracle: &str) -> Self {
        let mut printed = Printed::default();
        for raw in oracle.lines() {
            let line = loyalty_printed(&without_reminder(raw));
            if line.is_empty() {
                // A basic land type's ability is printed as a reminder (CR 305.6).
                let inner = raw.trim().trim_matches(['(', ')']);
                if kind_of(inner) == Said::Activated {
                    printed.lines.push(line_of(Said::Activated, inner.into(), ""));
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
                    printed.lines.push(line_of(Said::Word, capitalized(piece), raw));
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
pub(super) fn paid_by_a_choice(cost: &Cost) -> bool {
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
        _ => false,
    }
}

/// Whether what `decider` may do starts with their choosing one object.
pub(super) fn chooses_first(decider: &PlayerRef, body: &Program) -> bool {
    for instruction in &body.instructions {
        match instruction {
            Instruction::Group(group) => return chooses_first(decider, group),
            Instruction::Effect {
                operation: Operation::LookAt { .. },
                ..
            } => continue,
            Instruction::Effect {
                operation:
                    Operation::ChooseObjects {
                        player,
                        count: Count::Exactly(Expr::Constant(1)) | Count::AsManyAsCan(Expr::Constant(1)),
                        constraints,
                        ..
                    },
                ..
            } => {
                return format!("{player:?}") == format!("{decider:?}")
                    && !constraints
                        .iter()
                        .any(|c| matches!(c, SelectionConstraint::AtRandom))
            }
            _ => return false,
        }
    }
    false
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
            Some(format!("Adds {{ pool: ManaPool({pool:?}), ..add_each(&[]) }}"))
        }
        _ => None,
    }
}

/// A pump's "X" that reads a power or toughness: a negative X is 0 (CR 107.1b).
pub(super) fn not_negative(amount: &Expr, code: String) -> String {
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
        Ability::Keyword(keyword) => by_word(&printed_word(keyword)),
        Ability::Mechanic { mechanic, .. } => by_word(&printed_word(mechanic)),
        Ability::Enchant { .. } => by_word("enchant"),
        Ability::Activated { origin, .. } => match (origin, kind) {
            (ActivationOrigin::Ordinary, Said::Activated) => Some(12),
            (ActivationOrigin::Ordinary, Said::Word) => Some(3),
            (ActivationOrigin::Ordinary, _) => None,
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
            Some(line) if letters(&line.text).starts_with(&letters(&fallback)) => {
                line.text.clone()
            }
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
    /// The two answers of the question that offers an optional cost: without it
    /// first, as Cantrip's cards have them, and a scenario names them.
    pub(super) fn cost_labels(&self, cost: &Cost, parts: &mut Parts) {
        let (without, with) = match (cost.parts.as_slice(), &self.line) {
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
        let steps: Vec<(&ResultId, &Operation)> = group
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                Instruction::Effect { result, operation } => Some((result, operation)),
                _ => None,
            })
            .collect();
        if steps.len() != group.instructions.len() {
            return Ok(false);
        }
        let (reveal, steps) = match steps.as_slice() {
            [look, choose, (_, Operation::Reveal { objects: ObjectSet::Selected(_) }), put, rest] => {
                (true, [*look, *choose, *put, *rest])
            }
            [look, choose, put, rest] => (false, [*look, *choose, *put, *rest]),
            _ => return Ok(false),
        };
        let [(_, Operation::LookAt { player, objects: ObjectSet::Take { objects: looked, count: depth, from } }), (chosen, Operation::ChooseObjects { player: chooser, objects, count, constraints }), (put, Operation::Move { objects: ObjectSet::Selected(selected), destination: Destination::OwnersHand, entry, face_down: false }), (_, Operation::ReorderLibrary { end: SequenceEnd::Bottom, order, .. })] = steps
        else {
            return Ok(false);
        };
        let library = matches!(&**looked, ObjectSet::Zone { scope: ZoneScope::Players { zone: EventZone::Library, .. }, filter: ObjectFilter::Any, .. });
        let bounds = match count {
            Count::Exactly(Expr::Constant(n)) | Count::AsManyAsCan(Expr::Constant(n)) => {
                u8::try_from(*n).ok().map(|n| (n, n))
            }
            Count::UpTo(Expr::Constant(n)) => u8::try_from(*n).ok().map(|n| (0, n)),
            _ => None,
        };
        let filter = match objects {
            ObjectSet::Matching { objects, filter } if matches!(&**objects, ObjectSet::InZone { .. }) => Some(filter),
            ObjectSet::InZone { .. } => Some(&ObjectFilter::Any),
            _ => None,
        };
        let rest = match order {
            CardOrder::Chosen => "BottomChosen",
            CardOrder::Random => "BottomRandom",
            CardOrder::Preserve => return Ok(false),
        };
        let plain = format!("{from:?}") == "Top"
            && library
            && format!("{player:?}") == format!("{chooser:?}")
            && selected.0 == chosen.0
            && entry.counters.is_empty()
            && !entry.tapped
            && !constraints
                .iter()
                .any(|c| !matches!(c, SelectionConstraint::CanMoveTo { .. }));
        let (Some((least, most)), Some(filter), true) = (bounds, filter, plain) else {
            return Ok(false);
        };
        let id = self.fresh();
        let mut lets = Vec::new();
        let Some(test) = chars_test(filter, id, &mut lets) else {
            return Ok(false);
        };
        let wanted = match test.as_str() {
            "true" => "|_| true".to_string(),
            _ => format!("|c| {test}"),
        };
        let looker = self.fetch(self.certain_player(player)?);
        let depth = self.fetch(self.number(depth)?.amount(u32::MAX.into(), "usize")?);
        writeln!(out, "let player_{id} = {looker}; let depth_{id} = {depth}; {} let taken_{id} = cantrip_mechanics::library::look_at_top(g, player_{id}, asker.source, depth_{id}, ({least}, {most}), {reveal}, {wanted}, cantrip_mechanics::library::Rest::{rest})?;", lets.join(" ")).unwrap();
        if most == 1 {
            self.single.insert(chosen.0);
        }
        self.selections.insert(chosen.0, format!("taken_{id}"));
        self.moved.insert(put.0, format!("taken_{id}"));
        self.left_behind(put, format!("taken_{id}.clone()"));
        Ok(true)
    }
    /// `count`, or up to it if this is the choice of a "you may".
    pub(super) fn offered_count(&mut self, count: &Count) -> Count {
        match (std::mem::take(&mut self.may_choose), count) {
            (true, Count::Exactly(n) | Count::AsManyAsCan(n)) => Count::UpTo(n.clone()),
            _ => count.clone(),
        }
    }
    /// Past a choice of a "you may", only what was chosen goes on.
    pub(super) fn open_guard(&mut self, chosen: &str, out: &mut String) {
        if let Some(code) = self.chosen_guard.take() {
            writeln!(out, "if !{chosen}.is_empty() {{ {code}").unwrap();
            self.guard_opened = true;
        }
    }
    /// A block whose first choice is a "you may": `chosen` runs if it found anything.
    pub(super) fn block_choosing_first(
        &mut self,
        body: &Program,
        chosen: String,
        out: &mut String,
    ) -> Result<()> {
        self.may_choose = true;
        self.chosen_guard = Some(chosen);
        self.block(body, out, true)?;
        self.may_choose = false;
        self.chosen_guard = None;
        if std::mem::take(&mut self.guard_opened) {
            writeln!(out, "}}").unwrap();
        }
        Ok(())
    }
    /// Read a face's Oracle text, once, before its abilities are compiled.
    pub(super) fn read_printed(&mut self, abilities: &[Ability], oracle: Option<&str>) {
        let Some(oracle) = oracle else {
            return;
        };
        let printed = Printed::read(oracle);
        self.printed = printed.aligned(abilities);
        self.chapters = printed.chapters;
    }
    /// The ability at `at` is the next one compiled.
    pub(super) fn on_ability(&mut self, at: usize) {
        self.line = self.printed.get(at).cloned().flatten();
        self.quoted_used.clear();
    }
    /// The printed line of what's being compiled, as `kind`; a granted ability
    /// reads what the line quotes.
    pub(super) fn said(&mut self, kind: Said, fallback: String) -> String {
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
    /// The printed words of mode `at` of `count`.
    pub(super) fn said_mode(&self, at: usize, count: usize, fallback: String) -> String {
        match &self.line {
            Some(line) if line.modes.len() == count => line.modes[at].clone(),
            _ => fallback,
        }
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
