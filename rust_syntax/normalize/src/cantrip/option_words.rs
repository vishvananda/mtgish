//! The words of a choice's options. The IR keeps what each option does but not
//! what the card calls it, so a question that names them reads the printed line:
//! "create a Food token or a Treasure token", "second from the top or on the
//! bottom", "tap all ... or untap all ...". A line whose list can't be told from
//! its words has no labels, and the choice stays a hole.

/// Words that open a clause of their own: what is on the right of "or" is a
/// whole option, not a noun sharing the verb on its left.
const VERBS: &[&str] = &[
    "add", "attach", "cast", "counter", "create", "deal", "destroy", "discard", "draw", "exchange",
    "exile", "gain", "get", "have", "look", "lose", "mill", "pay", "play", "put", "regenerate",
    "remove", "return", "reveal", "sacrifice", "scry", "search", "shuffle", "skip", "surveil",
    "tap", "transform", "turn", "unlock", "untap",
];
/// Subjects of a clause on the right of "or".
const SUBJECTS: &[&str] = &["it", "that", "they", "this", "you"];
/// Words after which an option's own words are over.
const STOPS: &[&str] = &[
    " on ",
    " until ",
    " and ",
    " to ",
    " with ",
    " at ",
    " onto the battlefield",
    ", where",
    " instead",
];
/// The words naming what a card is, which "or" joins inside one noun.
const KINDS: &[&str] = &[
    "artifact",
    "battle",
    "creature",
    "enchantment",
    "instant",
    "land",
    "legendary",
    "nonland",
    "permanent",
    "planeswalker",
    "sorcery",
    "spell",
    "white",
    "blue",
    "black",
    "red",
    "green",
];

fn first_word(words: &str) -> &str {
    words.split(' ').next().unwrap_or("")
}
fn last_word(words: &str) -> &str {
    words.rsplit(' ').next().unwrap_or("")
}
fn is_article(word: &str) -> bool {
    matches!(word, "a" | "an" | "the")
}
/// Whether `words` opens with a verb or a subject, as a clause does.
fn opens_clause(words: &str) -> bool {
    let word = first_word(words).to_lowercase();
    VERBS.contains(&word.as_str()) || SUBJECTS.contains(&word.as_str())
}
/// Where what `sentence` says happens starts, past "When ...," or "If ...,": the
/// first comma a clause follows, since a trigger's own words can hold a list.
fn after_intro(sentence: &str) -> usize {
    const INTROS: &[&str] = &["When", "Whenever", "At ", "If ", "As long", "During"];
    if !INTROS.iter().any(|intro| sentence.starts_with(intro)) {
        return 0;
    }
    sentence
        .match_indices(", ")
        .map(|(at, _)| at + ", ".len())
        .find(|&at| {
            let next = &sentence[at..];
            opens_clause(next) || matches!(first_word(next), "each" | "target" | "choose")
        })
        .unwrap_or(0)
}
fn capitalized(words: &str) -> String {
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
/// The start of the clause `left` ends in.
fn clause_of(left: &str) -> &str {
    let mut clause = left.rsplit(", ").next().unwrap_or(left);
    if clause.starts_with("then ") {
        clause = left.rsplit(", ").nth(1).map_or(clause, |first| {
            let start = first.as_ptr() as usize - left.as_ptr() as usize;
            &left[start..]
        });
    }
    for lead in [
        "then ",
        "and ",
        "you may have ",
        "you may ",
        "that player may ",
        "may ",
        "instead ",
        "choose ",
    ] {
        clause = clause.strip_prefix(lead).unwrap_or(clause);
    }
    clause
}
/// How much of `right` is the option: up to the words the options share again,
/// which can't be among the words of the option `like` beside it.
fn own_words<'a>(right: &'a str, like: &str) -> &'a str {
    if right.starts_with("up to ") {
        return right;
    }
    let end = STOPS
        .iter()
        .filter(|stop| !like.contains(*stop))
        .filter_map(|stop| right.find(stop))
        .min()
        .unwrap_or(right.len());
    &right[..end]
}
/// The option at the end of `left` that opens as `opening` does.
fn parallel<'a>(left: &'a str, opening: &str) -> Option<&'a str> {
    let same = |word: &str| word == opening || (is_article(word) && is_article(opening));
    let words: Vec<&str> = left.split(' ').collect();
    let at = words.iter().rposition(|word| same(word))?;
    let skipped: usize = words[..at].iter().map(|w| w.len() + 1).sum();
    Some(&left[skipped..])
}
/// As many words from the end of `left` as `right` has.
fn same_length<'a>(left: &'a str, right: &str) -> Option<&'a str> {
    let words: Vec<&str> = left.split(' ').collect();
    let count = right.split(' ').count();
    (words.len() > count).then(|| {
        let skipped: usize = words[..words.len() - count]
            .iter()
            .map(|w| w.len() + 1)
            .sum();
        &left[skipped..]
    })
}
/// One option as a card labels it: the shared noun and its article gone.
fn tidy(words: &str) -> String {
    let words = words.trim().trim_end_matches([',', '.', '"']);
    let bare = words
        .split_once(' ')
        .filter(|(article, rest)| {
            is_article(article) && (rest.ends_with("token") || rest.ends_with("counter"))
        })
        .map_or(words, |(_, rest)| rest);
    capitalized(bare)
}

/// "A, B, or C": `count` items ending the list that `left` and `right` make.
fn comma_list(left: &str, right: &str, count: usize) -> Option<Vec<String>> {
    let last = own_words(right, "");
    let mut items: Vec<&str> = left.split(", ").collect();
    if items.len() < count - 1 {
        return None;
    }
    let items = items.split_off(items.len() - (count - 1));
    let mut words: Vec<String> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let item = match i {
            0 => {
                let after = [" among ", " chooses ", " of "]
                    .iter()
                    .find_map(|word| item.rfind(word).map(|at| &item[at + word.len()..]))
                    .unwrap_or(item);
                match after.split(' ').position(is_article) {
                    Some(at) if after == *item => {
                        let skipped: usize = after.split(' ').take(at).map(|w| w.len() + 1).sum();
                        &after[skipped..]
                    }
                    _ => after,
                }
            }
            _ => item,
        };
        words.push(item.to_string());
    }
    // "a flying, lifelink, or +1/+1 counter": the noun is theirs together.
    let shared = last_word(last);
    let together =
        matches!(shared, "counter" | "token") && !words.iter().any(|item| item.ends_with(shared));
    let last = match together {
        true => last
            .strip_suffix(shared)
            .map_or(last, |item| item.trim_end()),
        false => last,
    };
    words.push(last.to_string());
    Some(words.iter().map(|item| strip_article(item)).collect())
}
fn strip_article(words: &str) -> String {
    let bare = words
        .split_once(' ')
        .filter(|(article, _)| is_article(article))
        .map_or(words, |(_, rest)| rest);
    tidy(bare)
}

/// The sentence's options, for the " or " at `at`.
fn options_at(sentence: &str, at: usize, count: usize) -> Option<Vec<String>> {
    let (left, right) = (&sentence[..at], &sentence[at + " or ".len()..]);
    let left = left.strip_suffix(',').unwrap_or(left);
    if count > 2 && sentence[..at].ends_with(',') {
        return comma_list(left, right, count);
    }
    if opens_clause(right) {
        let clause = clause_of(left);
        if count == 2 {
            return Some(vec![tidy(clause), tidy(right)]);
        }
        // "Put a counter on it or it gains flying, first strike, or trample."
        let more = list_in(right, count - 1)?;
        let mut words = vec![tidy(clause)];
        words.extend(more);
        return Some(words);
    }
    if count != 2 {
        return None;
    }
    let left = clause_of(left);
    if let Some(item) = parallel(left, first_word(right)) {
        return Some(vec![tidy(item), tidy(own_words(right, item))]);
    }
    let own = own_words(right, "");
    if let Some(item) = same_length(left, own) {
        return Some(vec![tidy(item), tidy(own)]);
    }
    let (verb, object) = left.split_once(' ')?;
    VERBS
        .contains(&verb.to_lowercase().as_str())
        .then(|| vec![tidy(object), tidy(own)])
}
/// The list of `count` options in what is left of a sentence.
fn list_in(words: &str, count: usize) -> Option<Vec<String>> {
    let at = words.find(", or ")? + 1;
    let left = &words[..at - 1];
    let left = match left.rfind(" gains ") {
        Some(verb) => &left[verb + " gains ".len()..],
        None => left,
    };
    comma_list(left, &words[at + " or ".len()..], count)
}

/// Every " or " of `sentence` that could join options: not "two or more", and not
/// the types of one noun.
fn joins(sentence: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let start = after_intro(sentence);
    for (at, _) in sentence.match_indices(" or ") {
        if at < start {
            continue;
        }
        let (left, right) = (&sentence[..at], &sentence[at + " or ".len()..]);
        let after = first_word(right).trim_end_matches([',', '.']);
        if matches!(after, "more" | "less" | "greater" | "fewer") {
            continue;
        }
        let before = last_word(left).trim_end_matches(',').to_lowercase();
        let chosen = clause_of(left).split(' ').count() == 1 && left.contains("choose ");
        if KINDS.contains(&before.as_str()) && KINDS.contains(&after) && !chosen {
            continue;
        }
        // "Target permanent or suspended card": what one target may be.
        let named = clause_of(left).contains("target ") || clause_of(left).contains("that ");
        if named && (KINDS.contains(&after) || matches!(after, "card" | "suspended")) {
            continue;
        }
        found.push(at);
    }
    found
}
/// A line's sentences, the cost or chapter number before them gone.
fn sentences(line: &str) -> Vec<&str> {
    let line = match line.split_once(": ") {
        Some((cost, rest)) if !cost.contains('"') => rest,
        _ => line,
    };
    let line = match line.split_once("villainous choice \u{2014} ") {
        Some((_, rest)) => rest,
        // An ability word is short; a dash after "choose one" opens its modes.
        None => match line.split_once(" \u{2014} ") {
            Some((word, rest)) if !word.contains([',', '.']) && !word.ends_with(" one") => rest,
            _ => line,
        },
    };
    line.split(". ")
        .map(|s| s.trim_end_matches('.'))
        .filter(|s| !s.is_empty())
        .collect()
}

/// The words of the first list of `count` options in `lines`, in printed order. A
/// sentence's first list is the choice; a later "or" is inside an option.
pub(super) fn printed_options(lines: &[String], count: usize) -> Option<Vec<String>> {
    lines.iter().find_map(|line| {
        // The source drops "until this leaves the battlefield" from a choice.
        if line.contains(" until ") && line.contains(" leave") {
            return None;
        }
        if count == 2 && line.contains("second from the top or on the bottom") {
            return Some(vec!["Second from the top".into(), "On the bottom".into()]);
        }
        // `text_change::LABELS`. Printed with a duration, the source gives it to the
        // land type alone, and the choice stays a hole.
        if count == 2 && line.contains("one color word with another or one basic land type") {
            return (!line.contains(" until ")).then(|| {
                vec![
                    "Replace a color word".into(),
                    "Replace a basic land type".into(),
                ]
            });
        }
        // "Put your choice of a counter from among first strike, vigilance,
        // deathtouch, and lifelink on it" (Aragorn, Company Leader): a kind each.
        if let Some(at) = line.find("choice of a counter from among ") {
            let list = &line[at + "choice of a counter from among ".len()..];
            let list = list.split(" on ").next().unwrap_or(list);
            let kinds: Vec<String> = list
                .split(", ")
                .map(|kind| kind.trim_start_matches("and ").trim_start_matches("or "))
                .map(capitalized)
                .collect();
            return (kinds.len() == count && kinds.iter().all(|k| !k.is_empty())).then_some(kinds);
        }
        sentences(line).into_iter().find_map(|sentence| {
            joins(sentence)
                .into_iter()
                .find_map(|at| options_at(sentence, at, count))
        })
    })
}
