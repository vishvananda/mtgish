//! What a card's file holds where Cantrip or the generator cannot yet do a
//! part of it. A gap stays a gap: `module` still refuses such a card. For a
//! tracked file the rest of the card is still written, and the part that is
//! missing becomes a hole naming its requirement, so the code around it is what
//! the card ends up with and the hole says exactly what it waits on.
use super::*;
use crate::corpus;
use std::path::Path;

/// The tracker's name for a gap feature: a stable slug of its words.
pub fn requirement(feature: &str) -> String {
    let mut id = String::new();
    for c in feature.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && id.ends_with(|p: char| p.is_ascii_lowercase()) {
                id.push('-');
            }
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_string()
}

/// Requirements that have landed in Cantrip: their code is no longer held back.
/// Adding one here and regenerating is all a card waiting on it needs.
const LANDED: &[&str] = &[
    "engine-no-mutate",
    "engine-no-mutate-event",
    "numeric-expression-times-mutated",
];

/// Cantrip features some Cantrip has and another may not: (name, the line of
/// the API or mechanics source that shows a tree has it, the requirements it lands).
/// The code is always the code the feature takes; where the pinned Cantrip
/// (`cantrip_vocab::FEATURES`) lacks it, that code is a hole naming the
/// requirement, so the file compiles there and the two differ only by the
/// holes. `tools/update_cantrip_vocabulary.py` reads this table to write
/// `FEATURES`: keep each entry on one line, its name a literal.
const SWITCHED: &[(&str, &str, &[&str])] = &[
    // `CardDef::lasting_rules` with `Game::add_lasting_rule` (a rule on an open
    // set, CR 611.2c), `EffectKind::DoesntUntap`, and the durations
    // `DuringItsControllersNextUntapStep` and `DuringNextUntapStepOf`: Cantrip
    // PR #897. Its rules on open sets are `plan_switched` holes, not landed ids:
    // a rule it has no `LastingRuleKind` for shares their requirement.
    ("lasting-rules", "fn add_lasting_rule", &["continuous-rule-untap-step", "duration-during-controllers-next-untap-step", "duration-during-next-untap-step"]),
    ("rampage", "pub mod rampage;", &["engine-no-rampage"]),
    ("combat-removal", "pub fn remove_from_combat_and_unblock(", &["engine-no-effect-removes-from-combat"]),
    ("blocked-status", "fn become_blocked(", &["engine-no-effect-makes-a-creature-blocked", "keyword-action-become-blocked"]),
    // A P/T counter also needs its exact kind in COUNTERS; do not globally land unknown pairs.
    ("pt-counters", "pub const POWER_TOUGHNESS:", &[]),
    ("regeneration-bans", "pub const fn granted_regeneration_ban(", &[]),
    ("mana-counter-costs", "pub const fn x_is_mana_amount(", &[]),
    ("next-turn-delayed", "StepOfNextTurn { step: Step, of: Option<PlayerId> }", &["delayed-trigger-starting-next-turn", "delayed-trigger-starting-players-next-turn"]),
    ("blocking-status", "fn is_blocking(&self, id: ObjectId) -> bool", &["blocking-status-after-attacker-leaves-combat"]),
    ("damage-poison", "pub const fn on_damage_to_player<const N: u32>", &[]),
    ("combat-lki", "fn creatures_blocked_by(&self, source: ObjectId)", &["last-known-combat-relationships"]),
    ("entry-actor", "fn put_onto_battlefield_by(&mut self", &[]),
    ("resolving-counter-costs", "pub fn may_pay_on_one(", &[]),
    ("aura-graveyard-identity", "fn found_aura_after_enchanted_left(", &[]),
    ("delayed-subject-members", "fn delayed_trigger_with(", &[]),
    ("counter-placement-ceiling", "fn add_counters_with_limit(", &[]),
    ("divide-reads-context", "pub reads: Option<fn(&dyn crate::Query, &Asker, u32) -> u32>,", &[]),
    ("typed-prevention-amount", "pub enum PreventionAmount {", &[]),
    ("32-bit-effect-quantities", "fn damage(&mut self, source: ObjectId, target: Target, amount: u32) -> u32;", &[]),
    ("wide-effect-quantities", "fn damage(&mut self, source: ObjectId, target: Target, amount: i64) -> i64;", &[]),
    ("snow-mana", "pub snow: u32,", &[]),
    ("non-target-attachment-hosts", "fn can_attach_to(&self, attachment: ObjectId, host: ObjectId) -> bool;", &[]),
    ("simultaneous-replacement-damage", "pub type GrantedDamage = fn(&dyn Query, &Asker, ObjectId, &DamageEvent, &DamageEvent) -> Vec<DamageComponent>;", &[]),
    ("target-original-context", "replacing: Option<(u32, Target)>", &[]),
    ("delayed-watched-source", "ThisTurnWatching(ObjectId)", &[]),
    ("multiple-blocks", "pub const fn block_allowance(", &[]),
    ("cost-object-receipts", "fn mana_ability_cost_objects(&self,", &["cost-object-receipts"]),
    ("game-draw-legality", "fn cant_draw(&self, player: PlayerId, n: u32) -> bool;", &[]),
    ("damage-loyalty-floor", "pub fn leave_loyalty(&mut self, minimum: u32)", &["damage-loyalty-floor"]),
    ("creation-exile-links", "fn creation_linked_exiled(&self, source: ObjectId) -> Vec<ObjectId>;", &["noncopiable-creation-exile-links"]),
    ("state-trigger-player-capture", "fn state_triggered_with_player(", &["state-trigger-player-capture"]),
    ("event-object-capture", "fn when_asked_with_other_object(", &["trigger-time-secondary-object-capture"]),
    ("left-battlefield-receipts", "pub struct LeftBattlefield {", &[]),
    ("delayed-watched-subject", "WatchingThisTurn,", &[]),
    ("core-riot", "pub riot: bool,", &[]),
    ("exile-cast-fields", "ExileCast", &[]),
    ("permanent-cost-exclusions", "pub excluded: Option<ObjectId>,", &[]),
    ("spell-alternative-kinds", "SpellAlternative", &[]),
    ("multi-zone-search", "search_zone_and_or_library", &[]),
    ("job-select-token", "pub static HERO_1_1_COLORLESS:", &[]),
    ("resolved-land-types", "SetLandType(Subtype),", &["engine-no-resolving-effect-sets-land-types"]),
    ("untap-events", "Untapped { permanent: ObjectId },", &["engine-no-untap-event"]),
    ("counter-removal-events", "CountersRemoved { object: ObjectId, kind: CounterKind, amount: i32, total: i32 },", &["engine-no-event-for-a-counter-being-removed"]),
    ("control-change-events", "ControlChanged { permanent: ObjectId, from: PlayerId, controller: PlayerId },", &["trigger-event-player-loses-control"]),
    ("event-delayed-object-lists", "fn delayed_trigger_about(", &["engine-a-delayed-ability-on-an-event-keeps-no-list"]),
    ("exile-on-leave", "fn exile_on_leave(&mut self, permanent: ObjectId);", &[]),
    ("life-exchange", "pub fn life_totals(g: &mut dyn Game, a: PlayerId, b: PlayerId) -> u32", &[]),
    ("compound-cumulative-upkeep", "pub fn resolve(g: &mut dyn Game, item: &StackItem, payment: Payment)", &[]),
    ("no-other-auras", "pub const ENCHANTED_NO_OTHER_AURAS:", &[]),
    ("resolution-x-payment", "fn may_pay_x(&mut self, payer: PlayerId,", &[]),
    ("landwalk-blocking-exceptions", "pub landwalk_exceptions: &'static [LandwalkException],", &[]),
    ("independent-pt-settings", "SetPower(i32),", &["engine-no-effect-sets-power-or-toughness-alone"]),
    ("no-combat-damage-assignment", "NoCombatDamageAssignment,", &[]),
    ("world-rule", "pub world_since: u32,", &[]),
    ("other-activators", "pub activators: Option<ActivationPlayers>,", &["engine-no-activation-by-other-players"]),
    ("step-deadlines", "UntilNextStepOf { step: Step, player: Option<PlayerId> },", &[]),
    ("counted-untap-steps", "DuringControllersNextUntapSteps(u32),", &["duration-during-controllers-next-untap-steps"]),
    ("explicit-mana-loss", "fn empty_mana_pool(&mut self, player: PlayerId) -> ManaPool;", &[]),
    ("random-discard-costs", "pub discard_at_random: u8,", &[]),
    ("attack-as-though-haste", "pub attacks_as_though_haste: &'static [AttackHasteRule],", &["static-rule-attack-exception-summoning-sickness"]),
    ("nonmana-mana-tap-triggers", "pub const TAPPED_FOR_MANA: EventMask", &[]),
    ("keyword-family-metadata", "pub keyword_family: Option<KeywordFamily>,", &[]),
    ("combat-event-delays", "DelayedWhen::ThisCombat", &["event-trigger-for-end-of-combat"]),
    ("nonmana-combat-costs", "pub attack_cost_rules: &'static [AttackPayment],", &["static-rule-combat-cost-other-than-mana"]),
    ("combat-group-constraints", "pub combat_groups: &'static [CombatGroupRule],", &["static-rule-combat-group-attacker", "static-rule-combat-group-blocker"]),
    ("revealed-hands", "HandRevealed(fn(&dyn Query, &Asker, PlayerId) -> bool)", &["a-hand-played-revealed"]),
    ("computed-x", "pub x_rule: Option<XRule>,", &["engine-no-rule-on-an-ability-s-x"]),
    ("bands-with-other", "pub struct BandsWithOthersDef {", &["engine-no-bands-with-others", "engine-no-bands-with-others-granted", "losing-ability-any-bands-with-others"]),
    ("rechosen-values", "fn remember_chosen(&mut self, by: Linked, value: ChosenValue);", &["a-choice-of-number-made-again", "a-choice-of-color-made-again"]),
    ("hand-size-rules", "pub hand_size_rules: &'static [HandSizeRule],", &["a-maximum-hand-size-of-other-players"]),
    ("skip-steps", "pub skip_steps: &'static [SkipStep],", &["skipping-a-step-of-the-turn"]),
    ("untap-limits", "pub untap_limits: &'static [UntapLimit],", &["a-limit-on-what-untaps-in-an-untap-step"]),
];
/// The feature of `SWITCHED` that writes rules on open sets of objects.
pub(super) const LASTING_RULES: &str = "lasting-rules";

// A stand-in for the snapshot's FEATURES, for tests that write both forms.
// One bit per SWITCHED entry; None means the pinned snapshot. Fixed words scale
// with the catalog and allocate nothing, including past the first 64 features.
std::thread_local! {
    // A test's profile must not change concurrent generation using the snapshot.
    static STAND_IN: std::cell::Cell<Option<FeatureBits>> = const { std::cell::Cell::new(None) };
}
const FEATURE_WORDS: usize = SWITCHED.len().div_ceil(64);
type FeatureBits = [u64; FEATURE_WORDS];

/// Writes as if the pinned Cantrip had exactly the `SWITCHED` features
/// `features` while `f` runs, one caller at a time. For tests only: the
/// generator's own answer is the snapshot's.
#[doc(hidden)]
pub fn with_features<R>(features: &[&str], f: impl FnOnce() -> R) -> R {
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(!(features.contains(&"wide-effect-quantities") && features.contains(&"32-bit-effect-quantities")), "conflicting quantity profiles");
    let mut bits = [0u64; FEATURE_WORDS];
    for feature in features {
        let at = SWITCHED.iter().position(|s| s.0 == *feature)
            .unwrap_or_else(|| panic!("{feature:?} is no feature of SWITCHED"));
        bits[at / 64] |= 1u64 << (at % 64);
    }
    in_features(Some(bits), f)
}

/// Use the pinned native profile plus additional features, for checking a candidate native foundation.
/// Unlike `with_features`, this retains pinned capabilities except that an explicitly
/// selected quantity profile replaces the mutually exclusive pinned quantity profile.
#[doc(hidden)]
pub fn with_additional_features<R>(features: &[&str], f: impl FnOnce() -> R) -> R {
    assert!(!(features.contains(&"wide-effect-quantities") && features.contains(&"32-bit-effect-quantities")), "conflicting quantity profiles");
    let mut combined = crate::cantrip_vocab::FEATURES.to_vec();
    if features.contains(&"32-bit-effect-quantities") { combined.retain(|f| *f != "wide-effect-quantities"); }
    if features.contains(&"wide-effect-quantities") { combined.retain(|f| *f != "32-bit-effect-quantities"); }
    for feature in features {
        if !combined.contains(feature) { combined.push(feature); }
    }
    with_features(&combined, f)
}

/// Use the pinned native profile without selected features, for legacy-target checks.
#[doc(hidden)]
pub fn without_features<R>(features: &[&str], f: impl FnOnce() -> R) -> R {
    let remaining: Vec<&str> = crate::cantrip_vocab::FEATURES.iter().copied()
        .filter(|feature| !features.contains(feature)).collect();
    with_features(&remaining, f)
}

fn in_features<R>(bits: Option<FeatureBits>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<FeatureBits>);
    impl Drop for Restore {
        fn drop(&mut self) {
            STAND_IN.with(|stand_in| stand_in.set(self.0));
        }
    }
    let _restore = Restore(STAND_IN.with(|stand_in| stand_in.replace(bits)));
    f()
}
/// Corpus generation runs on workers. Carry this caller's profile explicitly;
/// a plain unrelated thread must continue to see the pinned native metadata.
pub(super) fn map_cards<T: Send>(
    path: &Path,
    map: impl Fn(usize, crate::mtg_types::OracleCard) -> corpus::Result<T> + Sync,
    visit: impl FnMut(usize, T) -> corpus::Result<()>,
) -> corpus::Result<()> {
    let bits = STAND_IN.with(std::cell::Cell::get);
    corpus::map_cards(path, |line, source| in_features(bits, || map(line, source)), visit)
}

/// Whether the Cantrip being written for has the `SWITCHED` feature `name`.
pub(super) fn switched_on(name: &str) -> bool {
    match STAND_IN.with(std::cell::Cell::get) {
        None => crate::cantrip_vocab::FEATURES.contains(&name),
        Some(bits) => SWITCHED.iter().position(|s| s.0 == name).is_some_and(|at| bits[at / 64] & (1u64 << (at % 64)) != 0),
    }
}

/// Draw legality moved from the free API helper to Game so it includes every
/// rules text and emblem. Keep explicit legacy-target generation usable.
pub(super) fn cant_draw(player: &str, amount: impl std::fmt::Display) -> String {
    if switched_on("game-draw-legality") {
        format!("g.cant_draw({player}, {amount})")
    } else {
        format!("cant_draw(g, {player}, {amount})")
    }
}
/// Whether the requirement `id` has landed in the Cantrip being written for.
fn landed_id(id: &str) -> bool {
    LANDED.contains(&id)
        || SWITCHED.iter().any(|(name, _, ids)| ids.contains(&id) && switched_on(name))
}

/// An item the card will have once `feature` exists: `planned_item!` of it
/// until then.
pub(super) fn planned_item(feature: &str, item: &str) -> String {
    match landed(feature) {
        true => item.to_string(),
        false => format!("planned_item!({:?} => {item});", requirement(feature)),
    }
}

/// An item of the `SWITCHED` feature `switched`, `planned_item!` of
/// `feature` until the Cantrip being written for has it.
pub(super) fn switched_item(switched: &str, feature: &str, item: &str) -> String {
    match switched_on(switched) {
        true => item.to_string(),
        false => format!("planned_item!({:?} => {item});", requirement(feature)),
    }
}

/// A function the code of a hole of `feature` names: a rule the engine asks
/// later, which a card's tables hold as a `fn`, never a closure. It is an item
/// of the card's own, `planned_item!` until the feature lands, so the file
/// without it compiles and the two forms differ only by the holes.
pub(super) fn planned_fn(feature: &str, name: &str, params: &str, ret: &str, body: &str) -> String {
    let item = format!("#[allow(unused_variables)] fn {name}({params}) -> {ret} {{ {body} }}");
    match landed(feature) {
        true => item,
        false => format!("planned_item!({:?} => {item});", requirement(feature)),
    }
}

/// Counters that are more than a name, so no line in `counter_kinds!` finishes
/// them: rad, poison, acorn, point and contract counters sit on players, and an
/// exalted counter gives its keyword (CR 122.1b).
const MORE_THAN_A_NAME: &[&str] = &["Acorn", "Contract", "Exalted", "Point", "Poison", "Rad"];

/// Whether `feature` asks only for a counter's name (CR 122.1). Its line goes in
/// `counter_kinds!` as a commit of its own just before the card, so the card is
/// written as it will stand: `cantrip_tree.py` finds the kind in the card's code.
fn only_a_counter_name(feature: &str) -> bool {
    feature
        .strip_prefix("counter kind ")
        .is_some_and(|word| !MORE_THAN_A_NAME.contains(&word))
}

/// Whether the requirement `feature` has landed in Cantrip.
pub(super) fn landed(feature: &str) -> bool {
    landed_id(&requirement(feature)) || only_a_counter_name(feature)
}

/// A value inside compiled code that Cantrip has no word for yet: `placeholder`
/// keeps the file compiling, `code` is what it will say, and `feature` is found
/// again by `value_gaps`, since a read has no way to record a gap of its own.
pub(super) fn planned_value(feature: &str, placeholder: &str, code: &str) -> String {
    if landed(feature) {
        return code.to_string();
    }
    format!("planned_value!({feature:?}, {placeholder} => {code})")
}
/// Every `planned_value` in a file's code.
pub(super) fn value_gaps(rust: &str) -> Vec<Diagnostic> {
    const MARK: &str = "planned_value!(\"";
    let mut found = BTreeSet::new();
    for (at, _) in rust.match_indices(MARK) {
        let mut feature = String::new();
        let mut chars = rust[at + MARK.len()..].chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => feature.extend(chars.next()),
                c => feature.push(c),
            }
        }
        found.insert(feature);
    }
    found.remove(FOLLOWS_GAP);
    found.into_iter().map(gap).collect()
}
/// The index past the bracket that closes the one opening at `open`, string
/// and character literals skipped, and the top-level `;` inside it.
fn close_of(code: &str, open: usize) -> Option<(usize, bool)> {
    let bytes = code.as_bytes();
    let (mut depth, mut i, mut semicolon) = (0usize, open, false);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            // A character literal, not a lifetime.
            b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 2,
            b'\'' if bytes.get(i + 1) == Some(&b'\\') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((i + 1, semicolon));
                }
            }
            b';' if depth == 1 => semicolon = true,
            _ => {}
        }
        i += 1;
    }
    None
}
/// The top-level parts of `code` between `sep` bytes, brackets and string
/// literals skipped.
fn top_level(code: &str, sep: u8) -> Vec<&str> {
    let bytes = code.as_bytes();
    let (mut parts, mut start, mut i) = (Vec::new(), 0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => match close_of(code, i) {
                Some((end, _)) => i = end - 1,
                None => return vec![code],
            },
            c if c == sep => {
                parts.push(&code[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&code[start..]);
    parts
}
/// Whether `code` is an expression of one call, not a statement.
fn one_call(code: &str) -> bool {
    let statement = ["for ", "if ", "let ", "{", "match ", "while ", "cantrip_api::instruction("]
        .iter()
        .any(|s| code.starts_with(s));
    !statement && code.ends_with([')', '?']) && top_level(code, b';').len() == 1
}
/// `code` as an expression whose value is what its call made, or None: one call;
/// a block whose last statement is one; `let name = call` (the name's value);
/// `if cond { call; }` (with nothing made otherwise).
fn made_by(code: &str) -> Option<String> {
    let code = code.trim().trim_end_matches(';').trim_end();
    if one_call(code) {
        return Some(code.to_string());
    }
    // A block gathering what its calls made in a list of its own, its value.
    if code.starts_with('{') && close_of(code, 0).map(|(end, _)| end) == Some(code.len()) {
        let inner = code[1..code.len() - 1].trim();
        let parts = top_level(inner, b';');
        let last = parts.last().map_or("", |last| last.trim());
        let simple = !last.is_empty() && last.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
        if simple && inner.starts_with(&format!("let mut {last}")) {
            return Some(code.to_string());
        }
    }
    if let Some(rest) = code.strip_prefix("let ") {
        let (name, value) = rest.split_once(" = ")?;
        let name = name.split(':').next()?.trim();
        let simple = name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
        return (simple && one_call(value.trim())).then(|| format!("{{ {code}; {name} }}"));
    }
    if code.starts_with('{') && close_of(code, 0).map(|(end, _)| end) == Some(code.len()) {
        let inner = code[1..code.len() - 1].trim().trim_end_matches(';');
        let mut parts = top_level(inner, b';');
        let last = parts.pop()?.trim();
        return one_call(last).then(|| format!("{{ {}; {last} }}", parts.join(";")));
    }
    if code.starts_with("if ") && code.ends_with('}') {
        let bytes = code.as_bytes();
        let (mut i, mut body) = (0, None);
        while i < bytes.len() {
            if matches!(bytes[i], b'(' | b'[' | b'{') {
                let end = close_of(code, i)?.0;
                if bytes[i] == b'{' {
                    body = Some((i, end));
                }
                i = end;
                continue;
            }
            i += 1;
        }
        let (open, end) = body?;
        if code[..open].trim_end().ends_with("else") {
            return None;
        }
        let call = code[open + 1..end - 1].trim().trim_end_matches(';').trim();
        return (end == code.len() && one_call(call)).then(|| {
            format!("{} {{ {call} }} else {{ Default::default() }}", code[..open].trim_end())
        });
    }
    None
}
/// Binds what the instruction whose code begins at `from` did, as `result_N`
/// that the instructions after it read (`unreported`), where its hole's code
/// comes to one call (`made_by`): the proposal returns what it did, as
/// Cantrip's own calls do (`exile_all`, `put_onto_battlefield`). A hole of
/// other statements, or of no code, leaves the readers' own holes waiting.
pub(super) fn bind_hole_result(out: &mut String, from: usize, result: &ResultId) {
    const MARK: &str = "planned!(\"";
    let Some(at) = out[from..].rfind(MARK).map(|at| from + at) else {
        return;
    };
    let open = at + MARK.len() - 2;
    let Some((close, _)) = close_of(out, open) else {
        return;
    };
    let inner = &out[open + 1..close - 1];
    let Some(arrow) = inner.find("\" => ").map(|a| a + 5) else {
        return;
    };
    let Some(made) = made_by(&inner[arrow..]) else {
        return;
    };
    // A bracket another instruction's gap left (`begin_simultaneous`) is not it.
    let bracket = made.starts_with("g.begin_") || made.starts_with("g.end_");
    if inner[1..arrow - 5] == requirement(FOLLOWS_GAP) && bracket {
        return;
    }
    let bound = format!("let result_{} = {made};", result.0);
    let start = open + 1 + arrow;
    out.replace_range(start..close - 1, &bound);
}
/// `placeholder`, read where the result of an instruction with a gap would be:
/// that instruction's own hole already says what the file waits on.
pub(super) fn unreported(placeholder: &str, result: &ResultId) -> String {
    planned_value(FOLLOWS_GAP, placeholder, &format!("result_{}", result.0))
}
/// What a structured instruction reads to know what to do: the code, or if it
/// has none a stand-in, so the body it holds is still compiled.
pub(super) fn planned_read(
    read: Result<String>,
    placeholder: &str,
    value: &impl serde::Serialize,
) -> String {
    match read {
        Ok(code) => code,
        Err(gap) => planned_value(&gap.feature, placeholder, &source(value)),
    }
}

/// The source of a value, for a hole to carry when no code can read it yet.
fn source(value: &impl serde::Serialize) -> String {
    quoted(&ron::to_string(value).unwrap_or_default())
}

impl Compiler {
    /// A rule the code of a hole of `feature` holds, asked later with `params`:
    /// a `fn` of the card's own named from `stem` (`planned_fn`), its name.
    pub(super) fn rule_fn(
        &mut self,
        feature: &str,
        stem: &str,
        params: &str,
        ret: &str,
        body: &str,
    ) -> String {
        let at = self.functions.matches(&format!("fn {stem}_")).count();
        let name = format!("{stem}_{at}");
        let item = planned_fn(feature, &name, params, ret, body);
        writeln!(self.functions, "{item}").unwrap();
        name
    }
    /// "Loyalty X": what a planeswalker enters with, for a number worked out then.
    pub(super) fn plan_loyalty(&mut self, loyalty: &Expr) {
        let feature = "printed loyalty expression";
        let id = requirement(feature);
        self.gaps.push(gap(feature).at("characteristics.loyalty"));
        match self.lane_p_loyalty(loyalty) {
            Ok(code) => {
                writeln!(self.holes, "planned_item!({id:?} => loyalty_by: {code});").unwrap()
            }
            Err(_) => writeln!(self.holes, "planned_item!({id:?}, {});", source(loyalty)).unwrap(),
        }
    }
    /// A part that stopped midway can leave anything scoped to it still set, and
    /// `?` passes over the line that would have put it back: the next ability
    /// starts as the first one did.
    pub(super) fn reset_after_gap(&mut self) {
        self.pending = Pending::Nothing;
        self.may_choose = false;
        self.do_once = None;
        self.chosen_guard = None;
        self.guard_opened = 0;
        self.replacing = Default::default();
        self.dividing = None;
        self.asking = None;
        self.repeating = None;
        self.after.clear();
        self.stack_item = true;
        self.reader = Reader::Resolution;
        self.event_at = EventAt::Nowhere;
        self.frame = Frame::default();
        self.grouped = false;
        self.recipient_subject = false;
        self.affected_in_scope = false;
        self.affected_object = None;
        self.static_guard = None;
        self.ability_guard = None;
        self.history = None;
        self.choosing = None;
        self.costing_targets = false;
        self.choosing_source = None;
        self.granting = false;
        self.outer = None;
        self.hoisted = None;
        self.activation_cost = None;
        self.slots.clear();
        self.bound_objects.clear();
        self.bound_players.clear();
        self.candidates.borrow_mut().clear();
        self.announced_filters.clear();
        self.reset_results();
    }
    /// A structure the generator keeps, its body real, that has a part it can't
    /// do yet: the hole says so beside the code.
    pub(super) fn plan_structure(&mut self, feature: &str, form: &str, out: &mut String) {
        let id = requirement(feature);
        self.gaps.push(gap(feature));
        writeln!(out, "planned!({id:?}, {form});").unwrap();
    }
    /// A printed characteristic Cantrip has no word for.
    pub(super) fn plan_trait(&mut self, path: &str, gap: Diagnostic, value: String) {
        let id = requirement(&gap.feature);
        self.gaps.push(gap.at(path));
        writeln!(self.holes, "planned_item!({id:?}, {value:?});").unwrap();
    }
    /// An ability with no code: the file keeps its source.
    pub(super) fn plan_ability(&mut self, feature: &str, ability: &Ability, label: &str) {
        let id = requirement(feature);
        let gaps = self.gaps.len();
        let stated = self.state_ability(feature, ability, &format!("{label}_stated"));
        self.gaps.truncate(gaps);
        match stated {
            Some(entries) if !entries.is_empty() => {
                for (list, code) in entries {
                    writeln!(self.holes, "planned_item!({id:?} => {list}: {code});").unwrap();
                }
            }
            _ => writeln!(self.holes, "planned_item!({id:?}, {});", source(ability)).unwrap(),
        }
    }
    /// The code a card will have once `feature` exists in Cantrip. Until then it
    /// is written out but not compiled, and the card waits on the requirement:
    /// `planned!("id" => code)`. Everything after it carries on as if it ran.
    pub(super) fn plan_code(&mut self, feature: &str, code: &str, out: &mut String) {
        let id = requirement(feature);
        if landed_id(&id) {
            writeln!(out, "{code}").unwrap();
            return;
        }
        self.gaps.push(gap(feature));
        writeln!(out, "planned!({id:?} => {code});").unwrap();
    }
    /// `code` of the `SWITCHED` feature `switched`, a hole of `feature` until
    /// the Cantrip being written for has it.
    pub(super) fn plan_switched(
        &mut self,
        switched: &str,
        feature: &str,
        code: &str,
        out: &mut String,
    ) {
        if switched_on(switched) {
            writeln!(out, "{code}").unwrap();
            return;
        }
        self.gaps.push(gap(feature));
        writeln!(out, "planned!({:?} => {code});", requirement(feature)).unwrap();
    }
    /// An item (a function a proposed table entry names) the card will have
    /// once `feature` exists: `planned_item!("id" => item)` until then.
    pub(super) fn plan_item(&mut self, feature: &str, item: &str) {
        let id = requirement(feature);
        match landed_id(&id) {
            true => writeln!(self.functions, "{item}").unwrap(),
            false => writeln!(self.functions, "planned_item!({id:?} => {item});").unwrap(),
        }
    }
    /// The entry `list` of the card definition will hold once `feature` exists:
    /// `planned_item!("id" => list: code)` until then.
    pub(super) fn plan_entry(
        &mut self,
        feature: &str,
        list: &'static str,
        code: &str,
        parts: &mut Parts,
    ) {
        let id = requirement(feature);
        if landed_id(&id) {
            parts.entry(list, code.to_string());
            return;
        }
        self.gaps.push(gap(feature));
        if let Some(held) = &mut self.holding {
            held.push((list, code.to_string()));
            return;
        }
        writeln!(self.holes, "planned_item!({id:?} => {list}: {code});").unwrap();
    }
    /// One field of a failed instruction: its value read as the code it would
    /// have been, where the generator can read it, or its source.
    fn plan_field(&self, value: &serde_json::Value) -> String {
        fn read<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Option<T> {
            serde_json::from_value(value.clone()).ok()
        }
        let code = read::<Expr>(value)
            .and_then(|n| self.number(&n).ok().map(|n| n.code))
            .or_else(|| read::<ObjectSet>(value).and_then(|o| self.objects(&o).ok()))
            .or_else(|| read::<ObjectRef>(value).and_then(|o| self.object(&o).ok()))
            .or_else(|| read::<PlayerRef>(value).and_then(|p| self.player(&p).ok()))
            .or_else(|| read::<PlayerSet>(value).and_then(|p| self.players(&p).ok()))
            .or_else(|| read::<Condition>(value).and_then(|c| self.condition(&c).ok()));
        match code {
            Some(code) => self.fetch(code),
            None => source(value),
        }
    }
    /// An instruction with no code: `planned!(requirement, form, field = value)`.
    pub(super) fn plan_instruction(
        &mut self,
        feature: &str,
        instruction: &Instruction,
        out: &mut String,
    ) {
        let id = requirement(feature);
        if let Some(code) = self.state_instruction(feature, instruction) {
            writeln!(out, "planned!({id:?} => {code});").unwrap();
            return;
        }
        let value = serde_json::to_value(instruction).unwrap_or_default();
        let (form, fields) = match instruction {
            Instruction::Effect { .. } => (
                value["args"]["operation"]["kind"].clone(),
                value["args"]["operation"]["args"].clone(),
            ),
            _ => (value["kind"].clone(), value["args"].clone()),
        };
        let form = form.as_str().unwrap_or("Instruction").to_string();
        write!(out, "planned!({id:?}, {form}").unwrap();
        match fields.as_object() {
            Some(fields) => {
                for (field, value) in fields {
                    write!(out, ", {field:?} = {}", self.plan_field(value)).unwrap();
                }
            }
            None if fields.is_null() => {}
            None => write!(out, ", \"value\" = {}", self.plan_field(&fields)).unwrap(),
        }
        writeln!(out, ");").unwrap();
    }
}

/// One face of a tracked card.
#[derive(serde::Serialize)]
pub struct TrackedFace {
    pub name: String,
    pub oracle: Option<String>,
    /// The face's module, left out where its card's `rust` holds it: a reader
    /// reads a card of joined faces from that.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust: Option<String>,
    pub gaps: Vec<Diagnostic>,
}

/// A record's generated code, whole.
#[derive(serde::Serialize)]
pub struct TrackedCard {
    pub line: usize,
    pub name: String,
    pub layout: String,
    pub faces: Vec<TrackedFace>,
    /// The card's file where its faces are joined into one card.
    pub rust: Option<String>,
    /// A gapless single face's file in Cantrip's hand-written style.
    pub file: Option<String>,
}

/// Every card's code, a JSON object a line. A record that can't be emitted
/// stops the run: nothing is left for the tracker to miss.
/// The modules a layout's other faces are written in, each one `CardDef` beside the card's.
fn face_modules(layout: &Layout) -> &'static [&'static str] {
    match layout {
        Layout::Transforming | Layout::ModalDFC => &["back"],
        Layout::Adventurer => &["adventure"],
        Layout::Ominous => &["omen"],
        Layout::Preparer => &["prepared"],
        Layout::Split | Layout::Room => &["left", "right"],
        _ => &[],
    }
}

pub fn tracked(input: &Path, oracle: &Path, output: &Path) -> corpus::Result<usize> {
    tracked_cached(input, oracle, output, None)
}

/// One record's tracked card, as `tracked` and `lane_rows` both write it.
struct Track {
    card: TrackedCard,
    /// A gapless single face whose characteristics are the whole card: `card_file`
    /// wrote no file for it because none is needed.
    content_only: bool,
    kept: Option<super::pr_cache::Kept>,
}

/// The tracked card of one record, its render taken from `renders` where they have it.
fn track(
    line: usize,
    source: &crate::mtg_types::OracleCard,
    oracle: &BTreeMap<String, String>,
    renders: &super::pr_cache::Renders,
) -> corpus::Result<Track> {
    let identity = corpus::Identity::of(source)?;
    let lowered = crate::unified::lower(source);
    let name = identity.names.join(" // ");
    let whole = lane_f::whole_printed(source, &lowered.card, Some(oracle))
        .map_err(|g| format!("{name}: {}: {} ({})", g.path, g.feature, g.message))?;
    let names: Vec<String> = whole.faces.iter().map(|face| face.name.clone()).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut faces: Vec<TrackedFace> = whole
        .faces
        .into_iter()
        .map(|face| TrackedFace {
            oracle: super::printed_face(oracle, &names, &face.name).cloned(),
            name: face.name,
            rust: Some(face.rust),
            gaps: face.gaps,
        })
        .collect();
    let (mut kept, mut content_only) = (None, false);
    let file = match faces.as_slice() {
        [one, two]
            if one.gaps.is_empty()
                && two.gaps.is_empty()
                && !face_modules(&lowered.card.layout).is_empty() =>
        {
            match (&whole.rust, one.oracle.as_deref(), two.oracle.as_deref()) {
                (Some(raw), Some(first), Some(second)) => {
                    let (file, render) = renders.render_faces(
                        raw,
                        face_modules(&lowered.card.layout),
                        &[(&one.name, first), (&two.name, second)],
                    );
                    kept = render;
                    file
                }
                _ => None,
            }
        }
        [one @ TrackedFace { rust: Some(module), .. }]
            if one.gaps.is_empty() && matches!(&lowered.card.layout, Layout::Card | Layout::MeldPiece) =>
        {
            let file = one.oracle.as_deref().and_then(|text| {
                match card_file::card_file(&lowered.card.faces[0], text) {
                    Ok(file) => file,
                    Err(_) => {
                        let (file, render) =
                            renders.render(module, &one.name, text, super::pr_form::Characteristics::Oracle);
                        kept = render;
                        file
                    }
                }
            });
            // Neither writes a file of `..oracle::NAME` alone: the content is the card.
            content_only = file.is_none() && one.oracle.is_some();
            file
        }
        _ => None,
    };
    // A face's code is not written twice: the card's module holds each face it joins.
    if let Some(joined) = &whole.rust {
        for face in &mut faces {
            if face.rust.as_ref().is_some_and(|rust| joined.contains(rust.as_str())) {
                face.rust = None;
            }
        }
    }
    // A sticker sheet has no name of its own: it is its face's.
    let name = match faces.as_slice() {
        [face] if name.is_empty() => face.name.clone(),
        _ => name,
    };
    let card = TrackedCard { line, name, layout: lowered.card.layout.to_string(), faces, rust: whole.rust, file };
    Ok(Track { card, content_only, kept })
}

/// Every record through `track`, each written as `row` makes it, with the
/// render cache kept as `tracked_cached` keeps it.
fn each_tracked(
    input: &Path,
    oracle: &Path,
    output: &Path,
    cache: Option<&Path>,
    row: impl Fn(Track) -> corpus::Result<(String, Option<super::pr_cache::Kept>)> + Sync,
) -> corpus::Result<usize> {
    use super::pr_cache::{Kept, Renders};
    let oracle = oracle_texts(oracle)?;
    let renders = match cache {
        Some(dir) => Renders::open(dir),
        None => Renders::none(),
    };
    let mut keeper = renders.keeper();
    let mut artifact = corpus::Artifact::create(output)?;
    let mut count = 0;
    map_cards(
        input,
        |line, source| -> corpus::Result<(String, Option<(Kept, bool)>)> {
            let (text, kept) = row(track(line, &source, &oracle, &renders)?)?;
            let kept = kept.map(|kept| {
                let found = renders.had(&kept);
                (kept, found)
            });
            // Written out here, on the workers: the tracked rows are 200 MB of JSON.
            Ok((text, kept))
        },
        |_, (row, kept)| {
            artifact.write_all(row.as_bytes())?;
            artifact.write_all(b"\n")?;
            if let Some((kept, found)) = kept {
                keeper.keep(kept, found)?;
            }
            count += 1;
            Ok(())
        },
    )?;
    artifact.commit()?;
    if let Some(dir) = cache {
        eprintln!("pr_form: {} renders from {}, {} rendered", keeper.found, dir.display(), keeper.rendered);
    }
    keeper.commit()?;
    Ok(count)
}

/// `tracked`, keeping `pr_form`'s renders in `cache` from one run to the next
/// (see `pr_cache`): a run after a change to the generator renders again only
/// the modules that changed.
pub fn tracked_cached(input: &Path, oracle: &Path, output: &Path, cache: Option<&Path>) -> corpus::Result<usize> {
    each_tracked(input, oracle, output, cache, |track| Ok((serde_json::to_string(&track.card)?, track.kept)))
}

/// What the card lane reads of a record: `tracked`'s card without its modules,
/// so a row is its file or its gap.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct LaneRow {
    pub line: usize,
    /// Its faces' names joined, the front face first.
    pub name: String,
    pub layout: String,
    pub faces: Vec<String>,
    /// The card's file; `None` with no gap is a card whose content is all of it.
    pub rust: Option<String>,
    pub gap: Option<String>,
}

impl LaneRow {
    fn of(track: Track) -> Self {
        let Track { card, content_only, .. } = track;
        let gap = match (&card.file, content_only) {
            (Some(_), _) | (None, true) => None,
            (None, false) => Some(
                card.faces
                    .iter()
                    .flat_map(|face| face.gaps.first())
                    .map(|gap| gap.feature.clone())
                    .next()
                    .unwrap_or_else(|| format!("no whole file for a {} card", card.layout)),
            ),
        };
        // In the name's order, the front face first: the lowered faces of a transforming card come back first.
        let printed: Vec<&str> = card.name.split(" // ").collect();
        let mut faces: Vec<String> = card.faces.into_iter().map(|face| face.name).collect();
        faces.sort_by_key(|face| printed.iter().position(|name| name == face).unwrap_or(usize::MAX));
        LaneRow {
            line: card.line,
            name: card.name,
            layout: card.layout,
            faces,
            rust: card.file,
            gap,
        }
    }
}

/// Every record as the card lane reads it (`ast cantrip-files`): a card's file
/// is `tracked`'s, byte for byte, whatever its layout.
pub fn lane_rows(input: &Path, oracle: &Path, output: &Path, cache: Option<&Path>) -> corpus::Result<usize> {
    each_tracked(input, oracle, output, cache, |track| {
        let kept = track.kept.clone();
        Ok((serde_json::to_string(&LaneRow::of(track))?, kept))
    })
}

#[cfg(test)]
mod tests {
    use super::{planned_value, switched_on, with_additional_features, with_features, LASTING_RULES, SWITCHED};

    /// The snapshot names only features the generator writes for, and the names
    /// the generator asks for are the table's.
    #[test]
    fn feature_profiles_cross_word_boundaries_without_aliasing_or_losing_the_snapshot() {
        let pinned: Vec<_> = SWITCHED.iter().map(|s| switched_on(s.0)).collect();
        for at in [63, 64] {
            let name=SWITCHED[at].0;
            with_features(&[name], || {
                for (i, feature) in SWITCHED.iter().enumerate() {
                    assert_eq!(switched_on(feature.0), i==at, "{} at {i}", feature.0);
                }
            });
        }
        assert_eq!(SWITCHED.iter().map(|s| switched_on(s.0)).collect::<Vec<_>>(),pinned);
    }

    #[test]
    fn features_are_switched_ones() {
        for feature in crate::cantrip_vocab::FEATURES.iter().chain([&LASTING_RULES]) {
            assert!(SWITCHED.iter().any(|s| s.0 == *feature), "{feature}");
        }
    }

    #[test]
    fn a_counter_that_is_only_a_name_is_written_and_any_other_held() {
        let value = |word: &str| {
            let feature = format!("counter kind {word}");
            planned_value(&feature, "CounterKind::Charge", &format!("CounterKind::{word}"))
        };
        assert_eq!(value("Spore"), "CounterKind::Spore");
        for word in ["Acorn", "Contract", "Exalted", "Point", "Poison", "Rad"] {
            assert!(value(word).starts_with("planned_value!"), "{word}");
        }
    }
    #[test]
    fn an_additional_profile_keeps_the_pin_and_is_thread_local() {
        const EXTRA: &str = "delayed-watched-source";
        let pinned = crate::cantrip_vocab::FEATURES.contains(&EXTRA);
        with_additional_features(&[EXTRA, EXTRA], || {
            assert!(switched_on(EXTRA));
            for name in crate::cantrip_vocab::FEATURES { assert!(switched_on(name), "{name}"); }
            assert_eq!(std::thread::spawn(|| switched_on(EXTRA)).join().unwrap(), pinned);
        });
        assert_eq!(switched_on(EXTRA), pinned);
    }

    #[test]
    fn adding_bounded_32_replaces_only_the_incompatible_pinned_quantity_profile() {
        with_additional_features(&["32-bit-effect-quantities"], || {
            assert!(switched_on("32-bit-effect-quantities"));
            assert!(!switched_on("wide-effect-quantities"));
            for name in crate::cantrip_vocab::FEATURES {
                if *name != "wide-effect-quantities" { assert!(switched_on(name), "{name}"); }
            }
        });
    }

    #[test]
    fn a_profile_override_does_not_leak_to_other_generator_threads() {
        const FEATURE: &str = "wide-effect-quantities";
        let pinned = crate::cantrip_vocab::FEATURES.contains(&FEATURE);
        let features: &[&str] = if pinned { &[] } else { &[FEATURE] };
        with_features(features, || {
            assert_eq!(switched_on(FEATURE), !pinned);
            let ordinary = std::thread::spawn(|| switched_on(FEATURE)).join().unwrap();
            assert_eq!(ordinary, pinned);
        });
        assert_eq!(switched_on(FEATURE), pinned);
    }

}
