//! Holes of the last round: the code a card will have, stated, where it waited on
//! a word or a read Cantrip has not got. Each arm reads the card's own operands.
use super::read::compare;
use super::*;

const M: &str = "cantrip_mechanics";

/// One of a card's printed card types that Cantrip has no word for. A dungeon has
/// none in Cantrip: `CardDef::dungeon` is what makes one (CR 309).
pub(super) fn printed_type(feature: &str, word: &str) -> Option<String> {
    (word != "Dungeon")
        .then(|| planned::planned_value(feature, "CardType::Land", &format!("CardType::{word}")))
}

/// One of a card's printed subtypes that Cantrip has no word for.
pub(super) fn printed_subtype(word: &str) -> String {
    planned::planned_value(
        &format!("subtype {word}"),
        "Subtype::Plains",
        &format!("Subtype::{word}"),
    )
}

/// `Supertypes` of a card, with the words Cantrip has no field for.
pub(super) fn printed_supertypes(flags: [bool; 3], others: &[String]) -> String {
    let [basic, legendary, snow] = flags;
    let known = format!("basic: {basic}, legendary: {legendary}, snow: {snow}");
    let mut value = format!("Supertypes {{ {known} }}");
    for other in others {
        let field = other.to_lowercase();
        value = planned::planned_value(
            &format!("supertype {other}"),
            &value,
            &format!("Supertypes {{ {known}, {field}: true }}"),
        );
    }
    value
}

/// A filter or condition Cantrip can't ask yet: `false` holds its place.
fn held(feature: &str, code: &str) -> String {
    planned::planned_value(feature, "false", code)
}
/// A set of objects Cantrip can't read yet.
fn held_objects(feature: &str, code: &str) -> String {
    planned::planned_value(feature, "Vec::<ObjectId>::new()", code)
}
/// One object Cantrip can't read yet.
fn held_object(feature: &str, code: &str) -> String {
    planned::planned_value(feature, "None::<ObjectId>", code)
}

/// A list of mana colors, as code: `Color::White, Color::Red`.
pub(super) fn colors_code(colors: &[ManaColor]) -> Result<String> {
    let colors = colors
        .iter()
        .map(|c| match c {
            ManaColor::Colorless => Ok("Mana::C".to_string()),
            other => Ok(format!("Color::{}", color(other)?)),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(colors.join(", "))
}
fn occurrence(mana: &ManaOccurrence) -> String {
    format!("ManaOccurrence::{}", form(mana))
}
fn comparison_code(comparison: &Comparison) -> String {
    format!("Comparison::{}", form(comparison))
}

/// A property equal to any one of several values is any one of those filters.
pub(super) fn any_of_filter(f: &ObjectFilter) -> Option<ObjectFilter> {
    let ObjectFilter::Property {
        property,
        comparison: Comparison::Equal,
        value: Expr::AnyOf(values),
    } = f
    else {
        return None;
    };
    Some(ObjectFilter::Either(
        values
            .iter()
            .map(|value| ObjectFilter::Property {
                property: property.clone(),
                comparison: Comparison::Equal,
                value: value.clone(),
            })
            .collect(),
    ))
}

/// `NameKind`s past one type: "a nonbasic land card name", "a noncreature, nonland
/// card name", "a card name other than a basic land card name" (CR 201.4a).
pub(super) fn name_kind_p(kind: &str, operands: &[Operand]) -> Result<String> {
    let failed = || gap(format!("name choice {kind}"));
    let word = |operands: &[Operand]| match operands {
        [Operand::Vocabulary { form, .. }] => Some(form.clone()),
        _ => None,
    };
    let terms = |operands: &[Operand]| -> Option<Vec<(String, String)>> {
        operands
            .iter()
            .map(|operand| match operand {
                Operand::Term { form, operands, .. } => Some((form.clone(), word(operands)?)),
                _ => None,
            })
            .collect()
    };
    let is_basic_land = |terms: &[(String, String)]| {
        terms.len() == 2
            && terms
                .iter()
                .any(|(f, w)| f == "IsSupertype" && w == "Basic")
            && terms.iter().any(|(f, w)| f == "IsCardtype" && w == "Land")
    };
    match kind {
        "Not" => {
            let [Operand::Term {
                form: inner,
                operands,
                ..
            }] = operands
            else {
                return Err(failed());
            };
            match inner == "And" && terms(operands).is_some_and(|t| is_basic_land(&t)) {
                true => Ok("NameKind::OTHER_THAN_BASIC_LAND".into()),
                false => Err(failed()),
            }
        }
        "And" => {
            let terms = terms(operands).ok_or_else(failed)?;
            let (mut types, mut non_types, mut nonbasic) = (Vec::new(), Vec::new(), false);
            for (form, word) in &terms {
                match form.as_str() {
                    "IsCardtype" => types.push(format!("CardType::{word}")),
                    "IsNonCardtype" => non_types.push(format!("CardType::{word}")),
                    "IsNonSupertype" if word == "Basic" => nonbasic = true,
                    _ => return Err(failed()),
                }
            }
            // "Nonbasic" alone isn't what the field means: it says "basic land".
            if nonbasic && !types.iter().any(|t| t == "CardType::Land") {
                return Err(failed());
            }
            Ok(format!(
                "NameKind {{ types: TypeSet::of(&[{}]), non_types: TypeSet::of(&[{}]), other_than_basic_land: {nonbasic} }}",
                types.join(", "),
                non_types.join(", ")
            ))
        }
        other => Err(gap(format!("name choice {other}"))),
    }
}

/// How a kind of value is kept: the read and its type.
fn kept(kind: ValueKind) -> Option<(&'static str, &'static str)> {
    Some(match kind {
        ValueKind::Color => ("chosen_color", "Color"),
        ValueKind::CreatureType
        | ValueKind::BasicLandType
        | ValueKind::LandType
        | ValueKind::NonbasicLandType
        | ValueKind::PlaneswalkerType => ("chosen_subtype", "Subtype"),
        ValueKind::CardName => ("chosen_card", "CardId"),
        ValueKind::Player => ("chosen_player", "PlayerId"),
        ValueKind::CardType | ValueKind::PermanentType => ("chosen_card_type", "CardType"),
        ValueKind::Number => ("chosen_number", "u32"),
        _ => return None,
    })
}
/// `Option` of a value a resolving ability chose, read by another ability or a
/// later turn. The choice's own body says to remember it: `remember_choice`.
pub(super) fn chosen_elsewhere(linked: &str, kind: ValueKind) -> Result<String> {
    let feature = format!("a choice of {kind:?} outside its body");
    let (read, ty) = kept(kind).ok_or_else(|| gap(feature.as_str()))?;
    Ok(planned::planned_value(
        &feature,
        &format!("None::<{ty}>"),
        &format!("q.state().{read}({linked})"),
    ))
}
/// `Option` of a value chosen as a permanent entered, of a kind it can't keep yet.
pub(super) fn chosen_kept(linked: &str, kind: ValueKind) -> Result<String> {
    let feature = format!("a choice of {kind:?} kept on a permanent");
    let (read, ty) = kept(kind).ok_or_else(|| gap(feature.as_str()))?;
    Ok(planned::planned_value(
        &feature,
        &format!("None::<{ty}>"),
        &format!("q.state().{read}({linked})"),
    ))
}
const REMEMBER: &str = "/*remember ";
/// What a resolving ability says to keep a value it chose for its other abilities.
fn remember_code(kind: &str, local: &str) -> String {
    let value = match kind {
        "Color" => "Color",
        "CreatureType" | "BasicLandType" | "LandType" | "NonbasicLandType" | "PlaneswalkerType" => {
            "Subtype"
        }
        "CardName" => "Card",
        "Player" => "Player",
        "CardType" | "PermanentType" => "CardType",
        _ => "Number",
    };
    format!(
        "let by = {{ let q: &dyn Query = g; q.linked(item) }}; g.remember_chosen(by, ChosenValue::{value}({local}));"
    )
}
/// A resolving body's choice, said to be kept only if something read it elsewhere,
/// which is known once the whole card is written.
pub(super) fn remembered(rust: String) -> String {
    if !rust.contains(REMEMBER) {
        return rust;
    }
    let mut out = String::with_capacity(rust.len());
    for line in rust.split_inclusive('\n') {
        let Some(mark) = line.trim_start().strip_prefix(REMEMBER) else {
            out.push_str(line);
            continue;
        };
        let mut words = mark.split_whitespace();
        let (kind, local) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
        let feature = format!("a choice of {kind} outside its body");
        if rust.contains(&format!("planned_value!({feature:?}")) {
            let code = remember_code(kind, local);
            let id = planned::requirement(&feature);
            out.push_str(&format!("planned!({id:?} => {code});\n"));
        }
    }
    out
}

/// "Choose one. If this spell was kicked, choose any number instead.": the same
/// modes, and how many of them if it is kicked.
pub(super) fn kicked_choice(paid: &Announcement, unpaid: &Announcement) -> Option<(u8, u8)> {
    let (paid, unpaid) = (paid.modes.as_ref()?, unpaid.modes.as_ref()?);
    let (Expr::Constant(min), Expr::Constant(max)) = (&paid.minimum, &paid.maximum) else {
        return None;
    };
    let same = format!("{:?}", paid.modes) == format!("{:?}", unpaid.modes);
    (same && !paid.repeat && !unpaid.repeat).then_some(())?;
    Some((u8::try_from(*min).ok()?, u8::try_from(*max).ok()?))
}
/// The result of a resolution's payment, where `objects` is what went as it was
/// before: how many went needs nothing of what they were.
pub(super) fn paid_by_moving(objects: &ObjectSet) -> Option<&ResultId> {
    match objects {
        ObjectSet::CostObjects {
            payment: PaymentRef::Resolution(result),
            phase: ObjectPhase::Before,
            ..
        } => Some(result),
        _ => None,
    }
}
/// Whether each of a walk's players moves only what they own, from a zone of
/// their own (CR 400.3), or by what they own.
pub(super) fn only_their_own(objects: &ObjectSet) -> bool {
    fn theirs(filter: &ObjectFilter) -> bool {
        let bound = |players: &PlayerSet| matches!(players, PlayerSet::One(PlayerRef::Bound(_)));
        match filter {
            ObjectFilter::All(filters) => filters.iter().any(theirs),
            ObjectFilter::InZone(ZoneScope::Players { zone, players }) => {
                matches!(
                    zone,
                    EventZone::Graveyard | EventZone::Hand | EventZone::Library
                ) && bound(players)
            }
            ObjectFilter::OwnedBy(players) => bound(players),
            _ => false,
        }
    }
    matches!(objects, ObjectSet::Zone { filter, .. } | ObjectSet::Battlefield(filter) if theirs(filter))
}

/// "(Artifact, battle, creature, enchantment, instant, kindred, land, planeswalker,
/// and sorcery are card types.)"
const CARD_TYPES: [&str; 9] = [
    "Artifact",
    "Battle",
    "Creature",
    "Enchantment",
    "Instant",
    "Kindred",
    "Land",
    "Planeswalker",
    "Sorcery",
];

/// The colors, in the order of `Color`.
pub(super) const COLORS: [&str; 5] = ["White", "Blue", "Black", "Red", "Green"];
/// The permanent types (CR 110.4b).
const PERMANENT_TYPES: [&str; 6] =
    ["Artifact", "Battle", "Creature", "Enchantment", "Land", "Planeswalker"];
/// The types a card offers as it enters, each spelled as a `CardType`.
pub(super) fn entering_types(options: &[Operand]) -> Vec<String> {
    let listed: Vec<String> = options
        .iter()
        .filter_map(|option| match option {
            Operand::Vocabulary { form, .. } => Some(format!("CardType::{form}")),
            _ => None,
        })
        .collect();
    match listed.is_empty() {
        true => CARD_TYPES.iter().map(|word| format!("CardType::{word}")).collect(),
        false => listed,
    }
}
/// The options of the card type `card` chooses as it enters.
pub(super) fn entering_card_type(card: &Card) -> Option<Vec<String>> {
    card.abilities.iter().find_map(|ability| {
        let Ability::Replacement {
            active: ReplacementActive::SourceEntering,
            body,
            ..
        } = ability
        else {
            return None;
        };
        body.instructions.iter().find_map(|instruction| match instruction {
            Instruction::Effect {
                operation:
                    Operation::ChooseValue {
                        kind: ValueKind::CardType,
                        options,
                        ..
                    },
                ..
            } => Some(entering_types(options)),
            _ => None,
        })
    })
}

impl Compiler {
    /// "Choose a color of a permanent you control. Add one mana of that color.":
    /// the colors it can make, which the engine offers as a mana ability is.
    pub(super) fn mana_of_a_color_among(
        &mut self,
        body: &Program,
        label: &str,
    ) -> Result<Option<String>> {
        let [Instruction::Effect {
            operation:
                Operation::ChooseValue {
                    actor: PlayerSet::One(PlayerRef::AbilityController),
                    kind: ValueKind::Color,
                    count: 1,
                    options,
                },
            ..
        }, Instruction::Effect {
            operation:
                Operation::AddMana {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    mana:
                        ManaProduction::Amount {
                            color: ManaColor::Chosen,
                            amount: Expr::Constant(1),
                        },
                    restriction: None,
                },
            ..
        }] = body.instructions.as_slice()
        else {
            return Ok(None);
        };
        let [Operand::Term { form, operands, .. }] = options.as_slice() else {
            return Ok(None);
        };
        let colors = match (form.as_str(), operands.as_slice()) {
            ("ColorAmoungPermanents", [Operand::Objects(objects)]) => self.colors_of(objects)?,
            (
                "ColorsOfCardsInPlayersGraveyard",
                [Operand::Filter(ObjectFilter::Any), Operand::Player(player)],
            ) => format!(
                "{}.map_or(ColorSet::NONE, |p: PlayerId| q.state().zone(Zone::graveyard(p)).iter().fold(ColorSet::NONE, |all: ColorSet, &o: &ObjectId| ColorSet(all.0 | q.chars(o).colors.0)))",
                self.player(player)?
            ),
            _ => return Ok(None),
        };
        writeln!(self.functions, "#[allow(unused_variables)] fn {label}_colors(q: &dyn Query, asker: &Asker) -> ColorSet {{ {colors} }}").unwrap();
        self.mana_colors = Some((format!(".colors_from({label}_colors)"), None));
        writeln!(self.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(), Suspend> {{ if let Some(color) = chosen {{ g.add_mana(asker.controller, color.mana(), 1); }} Ok(()) }}").unwrap();
        Ok(Some(format!("add_by({label}).choose_color().yields_any(1)")))
    }
    /// Leaves word for `remembered`, where a resolving ability made this choice.
    pub(super) fn remember_choice(&mut self, kind: ValueKind, local: &str, out: &mut String) {
        if !self.stack_item || self.pending == Pending::Entering || kept(kind).is_none() {
            return;
        }
        // "The last chosen number": a read of what it chose as it entered is a
        // read of this, once it has chosen again (CR 603.7d is about the link).
        if self.enters_choosing.contains(&kind) {
            let code = remember_code(&format!("{kind:?}"), local);
            self.plan_code(&format!("a choice of {kind:?} made again"), &code, out);
            return;
        }
        writeln!(out, "{REMEMBER}{kind:?} {local} */").unwrap();
    }
    /// The value this body chose, or the one its source keeps.
    pub(super) fn chosen_read(&self, kind: ValueKind) -> Result<String> {
        if let Ok(local) = self.chosen_local(kind) {
            return Ok(local);
        }
        let feature = format!("a choice of {kind:?} outside its body");
        let (read, placeholder) = match kind {
            ValueKind::Color => ("chosen_color", "Color::White"),
            ValueKind::LandType | ValueKind::BasicLandType | ValueKind::CreatureType => {
                ("chosen_subtype", "Subtype::Plains")
            }
            ValueKind::Ability => ("chosen_ability", "Keyword::Flying"),
            _ => return Err(gap(feature.as_str())),
        };
        Ok(planned::planned_value(
            &feature,
            placeholder,
            &format!(
                "q.state().{read}({}).expect(\"it chose one\")",
                self.linked()
            ),
        ))
    }
    /// Whether these objects are, or may be, cards from outside the game.
    pub(super) fn from_outside(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::OutsideTheGame(_) => true,
            ObjectSet::Selected(result) => self.notes.outside.contains(&result.0),
            ObjectSet::Union(sets) => sets.iter().any(|set| self.from_outside(set)),
            ObjectSet::Matching { objects, .. }
            | ObjectSet::Difference { objects, .. }
            | ObjectSet::Take { objects, .. }
            | ObjectSet::InZone { objects, .. }
            | ObjectSet::Random { objects, .. }
            | ObjectSet::Chosen { objects, .. }
            | ObjectSet::One(ObjectRef::Query { objects, .. }) => self.from_outside(objects),
            _ => false,
        }
    }
    /// A choice between a card from outside the game and a card elsewhere: Karn
    /// and Coax reveal the former, which the row does not say.
    pub(super) fn outside_shown(&self, objects: &ObjectSet) -> bool {
        matches!(objects, ObjectSet::Selected(result) if self.notes.outside_shown.contains(&result.0))
    }
    /// What `choose_value_rest` leaves.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_value_p(
        &mut self,
        kind: ValueKind,
        player: &str,
        source: &str,
        options: &[Operand],
        local: &str,
        id: usize,
        out: &mut String,
    ) -> Result<()> {
        let entering = self.pending == Pending::Entering;
        let feature = format!("a choice of {kind:?}");
        // The words listed, of the family a kind of value is written in.
        let listed = |family: &str| -> Result<Vec<String>> {
            options
                .iter()
                .map(|option| match option {
                    Operand::Vocabulary { form, .. } => Ok(format!("{family}::{form}")),
                    other => Err(gap(format!("choice among {}", form(other)))),
                })
                .collect()
        };
        match kind {
            // CR 205.3: a land type, or a planeswalker type, is a subtype to ask for.
            ValueKind::LandType | ValueKind::NonbasicLandType | ValueKind::PlaneswalkerType => {
                let every = match kind {
                    ValueKind::LandType => "Subtype::ALL.into_iter().filter(|&kind| SubtypeSet::LAND_TYPES.has(kind)).collect::<Vec<Subtype>>()".to_string(),
                    ValueKind::NonbasicLandType => "Subtype::ALL.into_iter().filter(|&kind| SubtypeSet::LAND_TYPES.has(kind) && kind.basic_mana().is_none()).collect::<Vec<Subtype>>()".to_string(),
                    // CR 205.3j: the subtypes of a planeswalker alone.
                    _ => "Subtype::ALL.into_iter().filter(|&kind| kind.card_types() == TypeSet::of(&[CardType::Planeswalker])).collect::<Vec<Subtype>>()".to_string(),
                };
                let words = listed("Subtype")?;
                let choices = match words.is_empty() {
                    true => every,
                    false => format!("vec![{}]", words.join(", ")),
                };
                match entering {
                    true => writeln!(out, "let Some({local}) = {M}::chosen_type::choose_type_among(g, entering, {choices})? else {{ return Ok(()) }};").unwrap(),
                    false => writeln!(out, "let player_{id} = {player}; let choices_{id} = {choices}; let Answer::Subtype({local}) = g.ask(player_{id}, RequestKind::ChooseSubtype {{ source: {source}, choices: choices_{id}, purpose: WordUse::Chosen }})? else {{ return Ok(()) }};").unwrap(),
                }
            }
            // The word itself, as Arachne's: `labels` of the card, in the order they
            // are printed, each standing for its type.
            ValueKind::CardType if self.stack_item && !entering => {
                let words = listed("CardType")?;
                let words: Vec<String> = match words.is_empty() {
                    true => CARD_TYPES.iter().map(|word| format!("CardType::{word}")).collect(),
                    false => words,
                };
                let base = self.choice_labels.len();
                self.choice_labels
                    .extend(words.iter().map(|word| word.replacen("CardType::", "", 1)));
                let end = self.choice_labels.len();
                let labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                writeln!(
                    out,
                    "let player_{id} = {player}; let {local}: CardType = [{}][usize::from({M}::choose::option(g, player_{id}, asker.source, item.ability.card, {base}..{end})? - {base})];",
                    words.join(", ")
                )
                .unwrap();
            }
            // As it enters: the word, kept as its chosen option, as Arachne's.
            ValueKind::CardType if entering => {
                if !self.choice_labels.is_empty() || self.entering_labels.is_some() {
                    return Err(gap(feature.as_str()));
                }
                let words = entering_types(options);
                self.choice_labels
                    .extend(words.iter().map(|word| word.replacen("CardType::", "", 1)));
                let labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                writeln!(
                    out,
                    "{M}::choose::option_as_enters(g, entering, &CARD, 0..{})?; let {local}: CardType = [{}][usize::from(entering.chosen_option.map_or(0, |chosen| chosen.index))];",
                    words.len(),
                    words.join(", ")
                )
                .unwrap();
            }
            ValueKind::PermanentType if self.stack_item => {
                let words = PERMANENT_TYPES;
                let base = self.choice_labels.len();
                self.choice_labels.extend(words.iter().map(|word| word.to_string()));
                let end = self.choice_labels.len();
                let labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                let words: Vec<String> = words.iter().map(|word| format!("CardType::{word}")).collect();
                writeln!(
                    out,
                    "let player_{id} = {player}; let {local}: CardType = [{}][usize::from({M}::choose::option(g, player_{id}, asker.source, item.ability.card, {base}..{end})? - {base})];",
                    words.join(", ")
                )
                .unwrap();
            }
            ValueKind::CardType | ValueKind::PermanentType => {
                return Err(gap(feature.as_str()));
            }
            // CR 105.2c: colorless is no color, so no request for colors offers it.
            ValueKind::ColorOrColorless => {
                if !self.stack_item
                    || (self.entering_labels.is_some() && self.choice_labels.is_empty())
                {
                    return Err(gap(feature.as_str()));
                }
                writeln!(out, "let player_{id} = {player}; let {local} = if {M}::effects::ask_may(g, player_{id}, {source}, Purpose::Optional)? {{ None }} else {{ Some({M}::choose_color(g, player_{id}, {source})?) }};").unwrap();
            }
            ValueKind::Direction => {
                let code = format!("{M}::choose::direction(g, {player}, {source})?");
                let value = planned::planned_value(&feature, "0u8", &code);
                writeln!(out, "let {local} = {value};").unwrap();
            }
            ValueKind::Filter => {
                return self.choose_filter(player, source, options, local, id, out)
            }
            other => return Err(gap(format!("a choice of {other:?}"))),
        }
        Ok(())
    }
    /// One of the filters listed: an option of the card, and the filters it keeps
    /// for `ObjectFilter::ChosenFilter`.
    fn choose_filter(
        &mut self,
        player: &str,
        source: &str,
        options: &[Operand],
        local: &str,
        id: usize,
        out: &mut String,
    ) -> Result<()> {
        let failure = || gap("a choice of Filter");
        let filters = options
            .iter()
            .map(|option| match option {
                Operand::Filter(filter) => Ok(filter.clone()),
                Operand::Objects(ObjectSet::Battlefield(filter)) => Ok(filter.clone()),
                _ => Err(failure()),
            })
            .collect::<Result<Vec<ObjectFilter>>>()?;
        let labels = filters
            .iter()
            .map(filter_word)
            .collect::<Option<Vec<String>>>()
            .ok_or_else(failure)?;
        if !self.stack_item || (self.entering_labels.is_some() && self.choice_labels.is_empty()) {
            return Err(failure());
        }
        let base = self.choice_labels.len();
        let end = base + labels.len();
        self.choice_labels.extend(labels);
        let words: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
        self.entering_labels = Some(format!("&[{}]", words.join(", ")));
        writeln!(out, "let player_{id} = {player}; let {local} = {M}::choose::option(g, player_{id}, {source}, item.ability.card, {base}..{end})? - {base};").unwrap();
        self.notes.filters = Some((local.to_string(), filters));
        Ok(())
    }
    /// Devotion to the color chosen (CR 700.5).
    pub(super) fn devotion_to_chosen(&self, objects: &ObjectSet) -> Result<Number> {
        let ObjectSet::Battlefield(ObjectFilter::ControlledBy(PlayerSet::One(player))) = objects
        else {
            return Err(gap("mana symbols other than a devotion to one color"));
        };
        Ok(lane_j::counted(format!(
            "{}.zip({}).map_or(0i64, |(p, color): (PlayerId, Color)| i64::from(cantrip_mechanics::devotion::devotion(q, p, color)))",
            self.player(player)?,
            self.chosen_value(ValueKind::Color)?
        )))
    }
    /// The rest of `object`.
    pub(super) fn object_p(&self, object: &ObjectRef) -> Result<String> {
        let feature = format!("object reference {}", form(object));
        Ok(match object {
            ObjectRef::AtEnd { objects, end } => {
                let pick = match end {
                    SequenceEnd::Top => "first",
                    SequenceEnd::Bottom => "last",
                };
                format!("({}).{pick}().copied()", self.objects(objects)?)
            }
            ObjectRef::TheOther { among, excluding } => format!(
                "{{ let among: Vec<ObjectId> = {}; let excluding: Vec<ObjectId> = {}; among.iter().copied().find(|o| !excluding.contains(o)) }}",
                self.objects(among)?,
                self.objects(excluding)?
            ),
            // Every target the spell has, as announced, the one object (601.2c).
            ObjectRef::SoleTargetOf(of) => format!(
                "{}.and_then(|spell: ObjectId| {{ let state = q.state(); let item = state.stack.iter().find(|i| i.kind == StackKind::Spell && i.object == spell).or_else(|| state.left_stack(spell))?; let mut chosen = state.targets(item).iter(); let first = chosen.next()?.object()?; match chosen.any(|t| t.object() != Some(first)) {{ true => None, false => Some(first) }} }})",
                self.object(of)?
            ),
            ObjectRef::SourceOf(of) => held_object(
                &feature,
                &format!("{}.and_then(|o: ObjectId| q.ability_source(o))", self.object(of)?),
            ),
            ObjectRef::MadeHere => {
                held_object(&feature, &format!("q.made_by({})", self.linked()))
            }
            ObjectRef::ChosenBy { result, chooser } => held_object(
                &feature,
                &format!(
                    "{}.and_then(|p: PlayerId| q.picked_by(item, {}, p))",
                    self.player(chooser)?,
                    result.0
                ),
            ),
            ObjectRef::NthChosen { index, by } => {
                let by = match by {
                    Some(player) => self.player(player)?,
                    None => "Some(asker.controller)".into(),
                };
                held_object(&feature, &format!("q.nth_chosen(asker, {index}, {by})"))
            }
            ObjectRef::Earlier { what, by: Some(by) } => held_object(
                &feature,
                &format!(
                    "q.earlier_object(item, EarlierObjects::{}, {})",
                    form(what),
                    self.player(by)?
                ),
            ),
            // What the effect around this one applies to, which no local holds here.
            ObjectRef::Affected => held_object(&feature, "asker.affected()"),
            ObjectRef::Bound(binding) => held_object(&feature, &format!("bound_{}", binding.0)),
            other => return self.object_stated(other),
        })
    }
    /// The rest of `objects`.
    pub(super) fn objects_p(&self, objects: &ObjectSet) -> Result<String> {
        let feature = format!("object collection {}", form(objects));
        let keep = |filter: &ObjectFilter, list: String| -> Result<String> {
            Ok(format!(
                "({list}).into_iter().filter(|&id| {}).collect::<Vec<ObjectId>>()",
                self.filter(filter, &Candidate::of("id"))?
            ))
        };
        Ok(match objects {
            // CR 400.11: only what the sideboard holds is outside a match.
            ObjectSet::OutsideTheGame(filter) if self.reader != Reader::Layer => {
                keep(filter, "q.outside_the_game(asker.controller)".into())?
            }
            // "Spells you've cast this turn": a spell gone from the stack is no
            // object to look for, and the row has lost the turn.
            ObjectSet::AnyZone(ObjectFilter::Cast { .. }) => {
                return Err(gap("spells cast at no stated time"));
            }
            ObjectSet::AnyZone(filter) => keep(
                filter,
                "{ let mut all: Vec<ObjectId> = q.state().battlefield().to_vec(); all.extend_from_slice(q.state().zone(Zone::STACK)); all.extend_from_slice(q.state().zone(Zone::COMMAND)); for p in q.state().seats() { for zone in [Zone::library(p), Zone::hand(p), Zone::graveyard(p), Zone::exile(p)] { all.extend_from_slice(q.state().zone(zone)); } } all }".into(),
            )?,
            ObjectSet::Damage(subject) => held_objects(
                &feature,
                &format!("q.damage_objects(item, DamageSubject::{})", form(subject)),
            ),
            ObjectSet::EventGroup => held_objects(&feature, "q.event_objects(item).to_vec()"),
            ObjectSet::Unpaid(result) if self.unpaid.contains(&result.0) => {
                format!("unpaid_{}.clone()", result.0)
            }
            ObjectSet::Unpaid(result) => held_objects(&feature, &format!("unpaid_{}.clone()", result.0)),
            ObjectSet::OutsideTheGame(filter) => keep(
                filter,
                held_objects(&feature, "q.outside_the_game(asker.controller)"),
            )?,
            ObjectSet::Earlier {
                what,
                by: Some(players),
            } => held_objects(
                &feature,
                &format!(
                    "q.earlier_objects(item, EarlierObjects::{}, {})",
                    form(what),
                    self.players(players)?
                ),
            ),
            ObjectSet::Chosen {
                chooser,
                objects,
                count,
            } => held_objects(
                &feature,
                &format!(
                    "{M}::choose::among(g, {}, asker.source, {}, {})?",
                    self.players(chooser)?,
                    self.objects(objects)?,
                    self.select_count(count)?
                ),
            ),
            ObjectSet::SplicedOnto(of) => held_objects(
                &feature,
                &format!(
                    "{}.map_or_else(Vec::new, |o: ObjectId| q.spliced_onto(o).to_vec())",
                    self.object(of)?
                ),
            ),
            ObjectSet::TargetsOf(of) => held_objects(
                &feature,
                &format!(
                    "{}.map_or_else(Vec::new, |o: ObjectId| q.targets_of(o))",
                    self.object(of)?
                ),
            ),
            // "Each player exiles ..., then puts all cards they exiled this way
            // ...": a player's share of a walk over the players.
            ObjectSet::MovedBy {
                result,
                phase,
                player,
            } => {
                let list = self.transition(result, phase)?;
                let player = self.player(player)?;
                match self.notes.own.contains(&result.0) {
                    true => format!("({list}).into_iter().filter(|&o: &ObjectId| Some(q.state().obj(o).owner) == {player}).collect::<Vec<ObjectId>>()"),
                    false => held_objects(&feature, &format!("q.moved_by({list}, {player})")),
                }
            }
            ObjectSet::Unselected(result) => held_objects(
                &feature,
                &format!("q.not_chosen({})", self.selected(result)?),
            ),
            ObjectSet::FromHistory { event, window } => {
                let scope = match window {
                    HistoryWindow::ThisTurn => "q.history().this_turn().iter()".to_string(),
                    HistoryWindow::ThisGame => "q.history().all().iter()".to_string(),
                    HistoryWindow::LastTurn => "q.history().all().iter().filter(|r| r.turn + 1 == q.state().turn.number)".to_string(),
                    HistoryWindow::ThisCombat => format!("{M}::history::this_combat(q).iter()"),
                    other => self.history_window_to_come(other)?,
                };
                let mut asked = self.clone();
                asked.history = Some(History {
                    outer: self.candidates.borrow().last().map(|c| c.id.clone()),
                    event_at: self.event_at,
                    frame: self.frame,
                });
                let matched = asked.event(event)?;
                held_objects(&feature, &format!("{scope}.filter_map({}).filter_map(|f: Fired| f.subject.object()).collect::<Vec<ObjectId>>()", matched.history_test()))
            }
            other => return self.objects_stated(other),
        })
    }
    /// The rest of `number`.
    pub(super) fn number_p(&self, n: &Expr) -> Result<Number> {
        use lane_j::{counted, held_number};
        let feature = format!("numeric expression {}", form(n));
        Ok(match n {
            // The share a delayed ability's target was given when its spell resolved.
            Expr::DistributedShare(slot)
                if self.reader == Reader::Resolution
                    && self.stack_item
                    && !self.slots.contains_key(&slot.0) =>
            {
                held_number(
                    "target reference".into(),
                    &format!("i64::from(item.remembered_share({}, object))", slot.0),
                )
            }
            Expr::DistributedShare(slot) if self.reader == Reader::Resolution && self.stack_item => {
                counted(format!(
                    "i64::from(q.state().share(item, {}))",
                    self.slot(slot)?
                ))
            }
            Expr::SharedValues {
                object,
                property: DistinctProperty::CardTypes,
                with,
            } => counted(format!(
                "{}.map_or(0i64, |o: ObjectId| {{ let others: Vec<ObjectId> = {}; q.chars(o).types.iter().filter(|&t| others.iter().any(|&other| q.chars(other).types.has(t))).count() as i64 }})",
                self.object(object)?,
                self.objects(with)?
            )),
            // "When you cycle this card": the card, where the discard put it, is the
            // source, and its cycling ability is on the stack below (or was countered).
            Expr::EventActivationX => counted("q.state().stack.iter().chain(q.state().left_stack.iter()).rev().find(|i| i.kind == StackKind::Activated && q.obj(i.object).became == Some(asker.source)).map_or(0i64, |i| i64::from(i.x))".into()),
            // "Hexproof from" is hexproof too; it has no bit of its own.
            Expr::AbilitiesAmong { abilities, objects } => match abilities_among(abilities) {
                Some(tests) => counted(format!(
                    "{{ let among: Vec<Chars> = {}.into_iter().map(|o: ObjectId| q.chars(o)).collect(); [{}].iter().filter(|&&has| has).count() as i64 }}",
                    self.objects(objects)?,
                    tests.join(", ")
                )),
                None => {
                    let abilities = self.clone().operand_code(abilities);
                    held_number(
                        feature,
                        &format!("q.abilities_among({abilities}, &{})", self.objects(objects)?),
                    )
                }
            },
            Expr::ActivationCostSymbols(colors) => held_number(
                feature,
                &format!("q.activation_cost_symbols(asker, &[{}])", colors_code(colors)?),
            ),
            Expr::AdditionalCostPaid(parts) => {
                let mut read = self.clone();
                let parts: Vec<String> = parts.iter().map(|p| read.operand_code(p)).collect();
                held_number(
                    feature,
                    &format!("q.additional_cost_paid(asker, &[{}])", parts.join(", ")),
                )
            }
            // "For each of the chosen colors it is": `Source` says "it" of a spell whose
            // cost a rule of this source changes.
            Expr::ChosenColorsOf(object) if self.enters_two == Some(ValueKind::Color) => {
                let it = match (object, self.costed_spell) {
                    (ObjectRef::Source, true) => "Some(spell)".to_string(),
                    _ => self.object(object)?,
                };
                counted(format!(
                    "{it}.map_or(0i64, |o: ObjectId| {}.iter().flatten().filter(|&&chosen| q.chars(o).colors.has(chosen)).count() as i64)",
                    self.chosen_two()
                ))
            }
            Expr::ChosenColorsOf(object) => held_number(
                feature,
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| q.state().chosen_colors(o).iter().count() as i64)",
                    self.object(object)?
                ),
            ),
            Expr::ChosenPowerToughness { result, toughness } if self.stack_item => {
                lane_j::counted(format!("chosen_pt_{}.{}", result.0, u8::from(*toughness)))
            }
            Expr::ChosenPowerToughness { result, toughness } => {
                let part = u8::from(*toughness);
                held_number(feature, &format!("i64::from(chosen_pt_{}.{part})", result.0))
            }
            Expr::ClampedDamage(result) => {
                held_number(feature, &format!("clamped_damage_{}", result.0))
            }
            Expr::CountersPaid { object, kind } => held_number(
                feature,
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| q.counters_paid(o, {}))",
                    self.object(object)?,
                    counter_kind(kind)?
                ),
            ),
            Expr::DamageBy(object) => held_number(
                feature,
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| q.history().damage_by(o))",
                    self.object(object)?
                ),
            ),
            Expr::ManaOf { mana, filter } => {
                let colors = match &filter.colors {
                    Some(colors) => format!("Some(&[{}])", colors_code(colors)?),
                    None => "None".to_string(),
                };
                held_number(
                    feature,
                    &format!(
                        "q.mana_of(item, {}, {colors}, {})",
                        occurrence(mana),
                        filter.snow
                    ),
                )
            }
            Expr::RepeatedCostsUnpaid { result, player } => held_number(
                feature,
                &format!(
                    "{}.map_or(0i64, |p: PlayerId| unpaid_{}.iter().filter(|who| **who == p).count() as i64)",
                    self.player(player)?,
                    result.0
                ),
            ),
            Expr::Game(value) => held_number(feature, &format!("q.game_value(item, GameValue::{})", form(value))),
            Expr::Earlier {
                what,
                measure,
                result,
                filter,
                by,
            } => {
                let by = match by {
                    Some(player) => self.player(player)?,
                    None => "None".to_string(),
                };
                let list = match result {
                    Some(result) => format!("q.earlier_objects_of(item, {})", result.0),
                    None => format!("q.earlier_objects(item, EarlierObjects::{}, {by})", form(what)),
                };
                let keep = match filter {
                    Some(filter) => format!(
                        ".filter(|&id| {})",
                        self.filter(filter, &Candidate::of("id"))?
                    ),
                    None => String::new(),
                };
                let those = format!("{list}.into_iter(){keep}");
                let code = match measure {
                    ObjectMeasure::Count => format!("{those}.count() as i64"),
                    ObjectMeasure::Of(property) => format!(
                        "{those}.next().map_or(0i64, |id: ObjectId| {})",
                        self.property("id", property)?.code
                    ),
                    ObjectMeasure::Total(property) => format!(
                        "{those}.map(|id: ObjectId| {}).sum::<i64>()",
                        self.property("id", property)?.code
                    ),
                    ObjectMeasure::Greatest(property) => format!(
                        "{those}.map(|id: ObjectId| {}).max().unwrap_or(0)",
                        self.property("id", property)?.code
                    ),
                };
                held_number(feature, &code)
            }
            Expr::Dice(measure) => {
                let last = "g.last_roll(asker.controller)";
                let code = match measure {
                    DiceMeasure::Result | DiceMeasure::Total => format!("{last}.result"),
                    DiceMeasure::Nth(n) => format!("{last}.dice[{n}]"),
                    DiceMeasure::Different => format!("{last}.different()"),
                    DiceMeasure::MostEqual => format!("{last}.most_equal()"),
                    DiceMeasure::Matching(comparison, value) => format!(
                        "{last}.matching({}, {})",
                        comparison_code(comparison),
                        self.number(value)?.code
                    ),
                };
                held_number(feature, &code)
            }
            Expr::TurnsSinceForetold => {
                held_number(feature, "q.turns_since_foretold(asker.source)")
            }
            // A number compared with "1 or 2": equal to one of them.
            Expr::AnyOf(values) => {
                let mut each = Vec::new();
                for value in values {
                    each.push(self.number(value)?.code);
                }
                held_number(feature, &format!("{M}::numbers::AnyOf(&[{}])", each.join(", ")))
            }
            Expr::ModesChosen(object) => held_number(
                feature,
                &format!("{}.map_or(0, |o: ObjectId| q.modes_chosen(o))", self.object(object)?),
            ),
            // A mana ability has no item: its place on the card names it, the same
            // on a copy (CR 707.2), and the history has it once its costs are paid.
            Expr::TimesActivatedThisTurn if self.activated_mana.is_some() => {
                let index = self.activated_mana.expect("asked");
                let card = self.source_card();
                counted(format!(
                    "{{ let this = AbilityRef {{ card: {card}, index: {index} }}; q.history().this_turn().iter().filter(|r| matches!(r.event, Event::AbilityActivated {{ source, ability, .. }} if source == asker.source && ability == this)).count() as i64 }}"
                ))
            }
            Expr::TimesActivatedThisTurn => {
                held_number(feature, "i64::from(q.times_activated_this_turn(asker))")
            }
            other => return Err(gap(format!("numeric expression {}", form(other)))),
        })
    }
    /// The rest of `predicate`.
    pub(super) fn predicate_p(&self, f: &ObjectFilter, c: &Candidate) -> Result<String> {
        let (id, chars) = (&c.id, &c.chars);
        let feature = format!("object filter {}", form(f));
        let test = |filter: &ObjectFilter, on: &str| self.filter(filter, &Candidate::of(on));
        Ok(match f {
            ObjectFilter::ChosenFilter => {
                let Some((local, filters)) = &self.notes.filters else {
                    return Ok(held(
                        &feature,
                        &format!("q.state().chosen_filter(asker.source).is_some_and(|f| f(q, asker, {id}))"),
                    ));
                };
                let mut arms = Vec::new();
                for (i, filter) in filters.iter().enumerate() {
                    let pattern = match i + 1 == filters.len() {
                        true => "_".to_string(),
                        false => i.to_string(),
                    };
                    arms.push(format!("{pattern} => {}", self.filter(filter, c)?));
                }
                format!("match {local} {{ {} }}", arms.join(", "))
            }
            ObjectFilter::ProtectedBy(players) => format!(
                "q.state().obj({id}).marks.protector.is_some_and(|protector: PlayerId| {})",
                self.player_in(players, "protector")?
            ),
            ObjectFilter::SharesNameWithAny(objects) => format!(
                "!{chars}.name.is_empty() && ({}).into_iter().any(|other: ObjectId| q.chars(other).name == {chars}.name)",
                self.objects(objects)?
            ),
            ObjectFilter::PairedWith(filter) => held(
                &feature,
                &format!(
                    "q.state().obj({id}).paired_with.is_some_and(|other: ObjectId| {})",
                    test(filter, "other")?
                ),
            ),
            ObjectFilter::AttachedCount {
                attachments,
                comparison,
                value,
            } => compare(
                &format!(
                    "q.state().battlefield().iter().copied().filter(|&attached| q.state().obj(attached).attached_to == Some({id}) && {}).count() as i64",
                    test(attachments, "attached")?
                ),
                comparison,
                &self.number(value)?.code,
            ),
            ObjectFilter::AttackingSame(object) => format!(
                "{}.is_some_and(|other: ObjectId| {{ let at = |o: ObjectId| q.state().attackers.iter().find(|a| a.id == o).map(|a| a.target); at({id}).is_some() && at({id}) == at(other) }})",
                self.object(object)?
            ),
            ObjectFilter::SharesMostCommonColor(objects) => format!(
                "{{ let those: Vec<ObjectId> = {}; let count = |c: Color| those.iter().filter(|&&o| q.chars(o).colors.has(c)).count(); let best = Color::ALL.into_iter().map(count).max().unwrap_or(0); best > 0 && Color::ALL.into_iter().any(|c| count(c) == best && {chars}.colors.has(c)) }}",
                self.objects(objects)?
            ),
            ObjectFilter::BandedWith(object) => held(
                &feature,
                &format!(
                    "{}.is_some_and(|other: ObjectId| q.banded_with({id}, other))",
                    self.object(object)?
                ),
            ),
            ObjectFilter::CanTargetOnly(filter) => held(
                &feature,
                &format!(
                    "q.can_target_only({id}, |target: ObjectId| {})",
                    test(filter, "target")?
                ),
            ),
            ObjectFilter::CastFromAmong(objects) => held(
                &feature,
                &format!("q.cast_from_among({id}, &{})", self.objects(objects)?),
            ),
            ObjectFilter::ChosenBy { kind, player } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.chosen_by(asker, p, ChoiceKind::{kind:?}).matches({id}))",
                    self.player(player)?
                ),
            ),
            ObjectFilter::ChosenOn { kind, object } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|o: ObjectId| q.chosen_on(o, ChoiceKind::{kind:?}).matches({id}))",
                    self.object(object)?
                ),
            ),
            // "With enchant creature" and "could enchant a creature" come here as
            // the same set of creatures, and only the second looks at what is there.
            ObjectFilter::CouldEnchant(hosts) if matches!(&**hosts, ObjectSet::One(_)) => format!(
                "({}).into_iter().any(|host: ObjectId| could_enchant(q, {id}, asker.controller, host))",
                self.objects(hosts)?
            ),
            ObjectFilter::CouldEnchant(hosts) => held(
                &feature,
                &format!(
                    "({}).into_iter().any(|host: ObjectId| q.could_enchant({id}, host))",
                    self.objects(hosts)?
                ),
            ),
            ObjectFilter::CouldProduce(mana) => {
                let Operand::Vocabulary { form: word, .. } = &**mana else {
                    return Err(gap(feature.as_str()));
                };
                let word = word.trim_start_matches("ManaProduce");
                held(&feature, &format!("q.could_produce({id}, Mana::{word})"))
            }
            ObjectFilter::CouldProduceManaOf(object) => held(
                &feature,
                &format!(
                    "{}.is_some_and(|other: ObjectId| q.could_produce_mana_of({id}, other))",
                    self.object(object)?
                ),
            ),
            ObjectFilter::ExiledBy {
                player,
                with,
                this_turn,
            } => {
                let with = match with {
                    Some(object) => self.object(object)?,
                    None => "None".into(),
                };
                held(
                    &feature,
                    &format!(
                        "q.exiled_by({id}, {}, {with}, {this_turn})",
                        self.player(player)?
                    ),
                )
            }
            ObjectFilter::InPile(object) => {
                let of = match object {
                    Some(object) => self.object(object)?,
                    None => "None".into(),
                };
                held(&feature, &format!("q.in_pile(asker, {id}, {of})"))
            }
            ObjectFilter::InSector(object) => {
                let of = match object {
                    Some(object) => self.object(object)?,
                    None => "None".into(),
                };
                held(&feature, &format!("q.in_sector(asker, {id}, {of})"))
            }
            ObjectFilter::InStartingDeck(players) => {
                let owner = format!("q.state().obj({id}).owner");
                held(
                    &feature,
                    &format!(
                        "q.state().obj({id}).in_starting_deck && {}",
                        self.player_in(players, &owner)?
                    ),
                )
            }
            ObjectFilter::ManaCostIs(symbols) => {
                let mut read = self.clone();
                let symbols: Vec<String> = symbols.iter().map(|s| read.operand_code(s)).collect();
                held(
                    &feature,
                    &format!(
                        "{} == ManaCost::of(&[{}])",
                        self.cost_of(id, chars),
                        symbols.join(", ")
                    ),
                )
            }
            ObjectFilter::ManaCostPayableWith(mana) => held(
                &feature,
                &format!("q.mana_cost_payable_with(item, {id}, {})", occurrence(mana)),
            ),
            ObjectFilter::ManaSpent {
                color,
                sources,
                payers,
                comparison,
                value,
            } => {
                let color = match color {
                    Some(color) => format!("Some({})", colors_code(std::slice::from_ref(color))?),
                    None => "None".to_string(),
                };
                let sources = match sources {
                    Some(sources) => format!(
                        "|source: ObjectId| {}",
                        test(sources, "source")?
                    ),
                    None => "|_: ObjectId| true".to_string(),
                };
                let payers = match payers {
                    Some(payers) => format!("|payer: PlayerId| {}", self.player_in(payers, "payer")?),
                    None => "|_: PlayerId| true".to_string(),
                };
                held(
                    &feature,
                    &compare(
                        &format!("q.history().mana_spent({id}, {color}, {sources}, {payers})"),
                        comparison,
                        &self.number(value)?.code,
                    ),
                )
            }
            ObjectFilter::ManaSpentFrom(sources) => held(
                &feature,
                &format!(
                    "q.history().mana_spent_from({id}).any(|source: ObjectId| {})",
                    test(sources, "source")?
                ),
            ),
            ObjectFilter::ManaSymbolInCost(symbol) => held(
                &feature,
                &format!(
                    "{}.has_symbol({})",
                    self.cost_of(id, chars),
                    colors_code(std::slice::from_ref(symbol))?
                ),
            ),
            ObjectFilter::MostCommonCreatureTypeIn(player) => format!(
                "{}.is_some_and(|p: PlayerId| {{ let kinds: Vec<SubtypeSet> = q.state().zone(Zone::library(p)).iter().map(|&card| q.chars(card).subtypes).collect(); let count = |kind: Subtype| kinds.iter().filter(|set| set.has(kind)).count(); let most = SubtypeSet::CREATURE_TYPES.iter().map(count).max().unwrap_or(0); most > 0 && SubtypeSet::CREATURE_TYPES.iter().any(|kind| {chars}.has_subtype(kind) && count(kind) == most) }})",
                self.player(player)?
            ),
            ObjectFilter::NameFirstPrintedIn(set) => held(
                &feature,
                &format!("{chars}.first_printing == Printing::{}", form(set)),
            ),
            ObjectFilter::Noted(kind) => held(
                &feature,
                &format!("q.noted(asker, ChoiceKind::{kind:?}).matches({id})"),
            ),
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                scope,
            } => {
                let by = match by {
                    Some(players) => self.player_in(players, "caster")?,
                    None => "true".to_string(),
                };
                held(
                    &feature,
                    &format!(
                        "q.history().nth_cast({}, CastScope::{}, |spell: ObjectId, caster: PlayerId| {} && {by}) == Some({id})",
                        self.number(ordinal)?.code,
                        form(scope),
                        test(spells, "spell")?
                    ),
                )
            }
            ObjectFilter::NthChosen(index) => held(
                &feature,
                &format!("q.nth_chosen_value(asker, {index}).matches({id})"),
            ),
            ObjectFilter::SharesColorWithMana(mana) => held(
                &feature,
                &format!(
                    "q.mana_colors(item, {}).intersects({chars}.colors)",
                    occurrence(mana)
                ),
            ),
            ObjectFilter::TargetableBy(object) => held(
                &feature,
                &format!(
                    "{}.is_some_and(|spell: ObjectId| q.targetable_by(spell, {id}))",
                    self.object(object)?
                ),
            ),
            ObjectFilter::TargetedBy(filter) => held(
                &feature,
                &format!(
                    "q.targeted_by({id}).into_iter().any(|by: ObjectId| {})",
                    test(filter, "by")?
                ),
            ),
            ObjectFilter::TargetCount(comparison, value) => held(
                &feature,
                &compare(
                    &format!("q.targets_of({id}).len() as i64"),
                    comparison,
                    &self.number(value)?.code,
                ),
            ),
            ObjectFilter::WasAttachedTo(host) => held(
                &feature,
                &format!("{}.is_some_and(|host: ObjectId| q.was_attached_to({id}, host))", self.object(host)?),
            ),
            ObjectFilter::AbilityOf(source) => held(
                &feature,
                &format!(
                    "q.source_of_ability({id}).is_some_and(|id: ObjectId| {})",
                    self.filter(source, &Candidate::of("id"))?
                ),
            ),
            // As a layer asks it: the spell on the stack, as it was cast.
            ObjectFilter::Cast { by, from } => {
                let mut tests = vec![format!("q.state().obj({id}).cast_from.is_some()")];
                let caster = |who: String| {
                    format!("q.state().stack.iter().any(|i| i.kind == StackKind::Spell && i.object == {id} && {who})")
                };
                if let Some(by) = by {
                    tests.push(caster(self.player_in(by, "i.controller")?));
                }
                match from {
                    None => {}
                    Some(ZoneScope::Shared(zone)) => {
                        tests.push(format!("q.state().obj({id}).cast_from == Some({})", zone_kind(zone)?));
                    }
                    Some(ZoneScope::Players { zone, players }) => {
                        tests.push(format!("q.state().obj({id}).cast_from == Some({})", zone_kind(zone)?));
                        tests.push(match players {
                            PlayerSet::One(PlayerRef::CasterOf(of))
                                if matches!(**of, ObjectRef::Candidate) =>
                            {
                                caster(format!("i.controller == q.state().obj({id}).owner"))
                            }
                            players => self.player_in(players, &format!("q.state().obj({id}).owner"))?,
                        });
                    }
                }
                format!("({})", tests.join(" && "))
            }
            // A permanent the destruction of would be a thing the rules forbid to
            // the filter's objects (Equinox), asked of what the spell would destroy.
            ObjectFilter::WouldDestroy(filter) => held(
                &feature,
                &format!("q.would_destroy({id}, |id: ObjectId| {})", test(filter, "id")?),
            ),
            other => return Err(gap(format!("object filter {}", form(other)))),
        })
    }
    /// The rest of `condition`.
    pub(super) fn condition_p(&self, condition: &Condition) -> Result<String> {
        let feature = format!("condition {}", form(condition));
        Ok(match condition {
            Condition::IsPrime(value) => format!(
                "{{ let n: i64 = {}; n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| n % d != 0) }}",
                self.number(value)?.code
            ),
            Condition::ChoseExtreme { player, highest } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.chose_extreme(asker, p, {highest}))",
                    self.player(player)?
                ),
            ),
            // Each player's own coin, flipped one at a time.
            Condition::CoinCameUp {
                player: Some(player),
                outcome: outcome @ (CoinOutcome::Heads | CoinOutcome::Tails),
            } if self.coin_turns.is_some() => {
                let (players, flips) = self.coin_turns.clone().expect("flipped");
                let player = self.player(player)?;
                let heads = matches!(outcome, CoinOutcome::Heads);
                format!("{players}.iter().zip(&{flips}).any(|(&flipper, &heads)| Some(flipper) == {player} && heads == {heads})")
            }
            // The coin flipped for the object being looked at.
            Condition::CoinCameUp {
                player: None,
                outcome: outcome @ (CoinOutcome::Heads | CoinOutcome::Tails),
            } if self.coin_objects.is_some() => {
                let flips = self.coin_objects.clone().expect("flipped");
                let candidate = self.candidates.borrow().last().map(|c| c.id.clone());
                let candidate = candidate.ok_or_else(|| gap(feature.as_str()))?;
                let heads = matches!(outcome, CoinOutcome::Heads);
                format!("{flips}.iter().any(|&(flipped, heads)| flipped == {candidate} && heads == {heads})")
            }
            Condition::CoinCameUp { player, outcome } => {
                let player = match player {
                    Some(player) => self.player(player)?,
                    None => "None".into(),
                };
                held(
                    &feature,
                    &format!("q.coin_came_up(asker, {player}, CoinOutcome::{outcome:?})"),
                )
            }
            Condition::CompletedDungeon { player, name } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.history().completed_dungeon(p, {}))",
                    self.player(player)?,
                    quoted(name)
                ),
            ),
            // A graveyard's top is its last (CR 404.2, as Corpse Dance walks it). The
            // data says "above" of what the card says "directly above". "Above this
            // card" names no graveyard, so the order isn't declared and is never asked.
            Condition::GraveyardPosition {
                card,
                player,
                above,
                comparison,
                amount,
            } if self
                .line_text()
                .is_some_and(|line| line.contains("graveyard") && line.contains("above it")) =>
            {
                let directly = self.line_text().is_some_and(|line| line.contains("directly above"));
                let test = self.filter(above, &Candidate::of("id"))?;
                let read = match directly {
                    true => format!("cards.get(at + 1).copied().is_some_and(|id| {test})"),
                    false => compare(
                        &format!("cards[at + 1..].iter().copied().filter(|&id| {test}).count() as i64"),
                        comparison,
                        &self.number(amount)?.code,
                    ),
                };
                format!(
                    "{}.zip({}).is_some_and(|(p, card): (PlayerId, ObjectId)| {{ let cards = q.state().zone(Zone::graveyard(p)); cards.iter().position(|&o| o == card).is_some_and(|at| {read}) }})",
                    self.player(player)?,
                    self.object(card)?,
                )
            }
            Condition::GraveyardPosition {
                card,
                player,
                above,
                comparison,
                amount,
            } => held(
                &feature,
                &format!(
                    "{}.zip({}).is_some_and(|(p, card): (PlayerId, ObjectId)| {{ let cards = q.state().zone(Zone::graveyard(p)); cards.iter().position(|&o| o == card).is_some_and(|at| {}) }})",
                    self.player(player)?,
                    self.object(card)?,
                    compare(
                        &format!(
                            "cards[..at].iter().copied().filter(|&id| {}).count() as i64",
                            self.filter(above, &Candidate::of("id"))?
                        ),
                        comparison,
                        &self.number(amount)?.code
                    )
                ),
            ),
            Condition::HadExtreme {
                player,
                what,
                filter,
                highest,
            } => {
                let keep = match filter {
                    Some(filter) => self.filter(filter, &Candidate::of("id"))?,
                    None => "true".to_string(),
                };
                held(
                    &feature,
                    &format!(
                        "{}.is_some_and(|p: PlayerId| q.had_extreme(item, p, EarlierObjects::{}, |id: ObjectId| {keep}, {highest}))",
                        self.player(player)?,
                        form(what)
                    ),
                )
            }
            Condition::NotedForDraft { player, note } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.draft_notes(p, DraftNoteKind::{}, {}))",
                    self.player(player)?,
                    form(&note.kind),
                    quoted(&note.card)
                ),
            ),
            Condition::SubsetMeets {
                objects,
                comparison,
                amount,
                constraints,
            } => {
                let rules: Vec<String> = constraints
                    .iter()
                    .map(|rule| format!("SelectionConstraint::{}", form(rule)))
                    .collect();
                held(
                    &feature,
                    &format!(
                        "{M}::select::subset_meets(q, &{}, {}, {}, &[{}])",
                        self.objects(objects)?,
                        comparison_code(comparison),
                        self.number(amount)?.code,
                        rules.join(", ")
                    ),
                )
            }
            Condition::TargetCount {
                object,
                filter,
                comparison,
                value,
            } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|o: ObjectId| {})",
                    self.object(object)?,
                    compare(
                        &format!(
                            "q.targets_of(o).into_iter().filter(|&id| {}).count() as i64",
                            self.filter(filter, &Candidate::of("id"))?
                        ),
                        comparison,
                        &self.number(value)?.code
                    )
                ),
            ),
            Condition::TargetsMatch {
                object,
                kind,
                filter,
            } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|o: ObjectId| q.targets_of(o).into_iter().all(|id: ObjectId| ObjectKind::{}.is(q, id) && {}))",
                    self.object(object)?,
                    form(kind),
                    self.filter(filter, &Candidate::of("id"))?
                ),
            ),
            Condition::ValueChosenIs(player) if self.enters_two == Some(ValueKind::Player) => format!(
                "{}.is_some_and(|p: PlayerId| q.state().chosen_player({linked}) == Some(p) || q.state().chosen_number({linked}) == Some(u32::from(p.0)))",
                self.player(player)?,
                linked = self.linked()
            ),
            Condition::ValueChosenIs(player) => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.state().chosen_players({}).contains(&p))",
                    self.player(player)?,
                    self.linked()
                ),
            ),
            Condition::VotedAs { player, other, same } => held(
                &feature,
                &format!(
                    "{}.zip({}).is_some_and(|(a, b): (PlayerId, PlayerId)| g.votes().same(a, b) == {same})",
                    self.player(player)?,
                    self.player(other)?
                ),
            ),
            Condition::Searched(result) => held(&feature, &format!("q.searched(asker, {})", result.0)),
            Condition::ValueChosen(kind @ (ValueKind::Color | ValueKind::CreatureType))
                if self.enters_choosing.contains(kind) =>
            {
                format!("{}.is_some()", self.chosen_value(*kind)?)
            }
            Condition::ValueChosen(kind) => held(
                &feature,
                &format!(
                    "q.state().chosen_value({}, ChoiceKind::{kind:?}).is_some()",
                    self.linked()
                ),
            ),
            Condition::ChoseNth { player, nth } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.chose_nth(asker, p, {nth}))",
                    self.player(player)?
                ),
            ),
            Condition::WonSubgame { player, result } => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.won_subgame(p, {}))",
                    self.player(player)?,
                    result.0
                ),
            ),
            other => return Err(gap(format!("condition {}", form(other)))),
        })
    }
}

/// "Nonbasic landwalk", and landwalk of what no word of Cantrip's names (CR 702.14).
pub(super) fn landwalk(land: &ObjectFilter) -> Result<String> {
    let feature = format!("landwalk of {}", form(land));
    if let ObjectFilter::Not(inner) = land {
        if matches!(
            **inner,
            ObjectFilter::Supertype(crate::mtg_types::SuperType::Basic)
        ) {
            return Ok("Keyword::NonbasicLandwalk".to_string());
        }
    }
    if matches!(
        land,
        ObjectFilter::Supertype(crate::mtg_types::SuperType::Legendary)
    ) {
        return Ok("Keyword::LegendaryLandwalk".to_string());
    }
    let filter = land_filter(land).ok_or_else(|| gap(feature.as_str()))?;
    Ok(planned::planned_value(
        &feature,
        "Keyword::Swampwalk",
        &format!("Keyword::Landwalk({filter})"),
    ))
}
fn land_filter(land: &ObjectFilter) -> Option<String> {
    Some(match land {
        ObjectFilter::Subtype(kind) => format!("LandFilter::Subtype(Subtype::{})", name(kind)),
        ObjectFilter::Supertype(kind) => {
            format!("LandFilter::Supertype(Supertype::{})", name(kind))
        }
        ObjectFilter::CardType(kind) => format!("LandFilter::CardType(CardType::{})", name(kind)),
        ObjectFilter::Chosen(kind) => format!("LandFilter::Chosen(ChoiceKind::{kind:?})"),
        ObjectFilter::Not(inner) => format!("LandFilter::Not(&{})", land_filter(inner)?),
        ObjectFilter::All(parts) => {
            let parts = parts.iter().map(land_filter).collect::<Option<Vec<_>>>()?;
            format!("LandFilter::All(&[{}])", parts.join(", "))
        }
        _ => return None,
    })
}

/// What a history question asks, past how many and how much.
pub(super) fn history_measure(measure: &HistoryMeasure, hits: &str) -> String {
    let feature = format!("history measure {}", form(measure));
    let code = match measure {
        HistoryMeasure::Property(property, aggregation) => format!(
            "{M}::history::aggregate({hits}, Property::{}, Aggregation::{})",
            form(property),
            form(aggregation)
        ),
        HistoryMeasure::Distinct(property) => format!(
            "{M}::history::distinct({hits}, DistinctProperty::{})",
            form(property)
        ),
        other => format!(
            "{M}::history::measure({hits}, HistoryMeasure::{})",
            form(other)
        ),
    };
    planned::planned_value(&feature, "0i64", &code)
}

impl Compiler {
    /// A maximum hand size that isn't a stated number (CR 402.2).
    pub(super) fn hand_size_to_come(&self, size: &HandSizeRule) -> Result<String> {
        let feature = format!("hand size {}", form(size));
        let (read, n) = match size {
            HandSizeRule::Adjust(n) => (
                "q.maximum_hand_size(asker.controller).saturating_add_signed",
                n,
            ),
            HandSizeRule::Set(n) => ("Some", n),
            HandSizeRule::Unlimited => return Err(gap(feature.as_str())),
        };
        Ok(planned::planned_value(
            &feature,
            "Some(7)",
            &format!("{read}(({}) as _)", self.number(n)?.code),
        ))
    }
    /// A window of the history no read names.
    pub(super) fn history_window_to_come(&self, window: &HistoryWindow) -> Result<String> {
        let feature = format!("history window {}", form(window));
        let HistoryWindow::SinceLastTurnOf(player) = window else {
            return Err(gap(feature.as_str()));
        };
        Ok(planned::planned_value(
            &feature,
            "q.history().this_turn().iter()",
            &format!(
                "{M}::history::since_last_turn_of(q, {}).iter()",
                self.player(player)?
            ),
        ))
    }
}

/// `objects_{id}` into hands, those outside the game by the way that brings them in.
pub(super) fn bring_into_hand(id: usize, revealed: bool) -> String {
    let way = if revealed { "_revealed" } else { "" };
    format!(
        "{{ let (outside, inside): (Vec<ObjectId>, Vec<ObjectId>) = objects_{id}.iter().partition(|&&o| g.state().obj(o).zone.is(ZoneKind::Sideboard)); let mut moved = g.bring_into_hand{way}(&outside); moved.extend(g.return_to_hand{way}(&inside)); moved }}"
    )
}

/// "When you do, choose one": the modes aside, what the reflexive ability announces.
pub(super) fn without_modes(announcement: &Announcement) -> Announcement {
    let mut announcement = announcement.clone();
    announcement.modes = None;
    announcement
}

impl Compiler {
    /// The modes of a reflexive ability, a `ModalDef` of the card.
    pub(super) fn reflexive_modes(
        &mut self,
        announcement: &Announcement,
        label: &str,
    ) -> Result<Option<String>> {
        if announcement.modes.is_none() {
            return Ok(None);
        }
        // Each mode is a body of its own, compiled amid this one, and its target
        // filters read what the ability is about, as any of its reads.
        let scope = self.results();
        let name = self.modal(announcement, label, Some(Frame::default()));
        self.restore_results(scope);
        Ok(Some(name?))
    }
    /// A spell's second body, as "... Then ..." after its modes or its branches.
    pub(super) fn later_spell(
        &mut self,
        announcement: &Announcement,
        body: &Program,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let label = format!("{label}_then");
        let targets = self.announcement(&without_modes(announcement), &label)?;
        let effect = self.effect(body, &label)?;
        // With nothing announced of its own, it is the rest of the one spell.
        if targets == "&[]" && announcement.modes.is_none() {
            if let Some((targets, first)) = parts
                .spell
                .strip_prefix("spell(")
                .and_then(|spell| spell.strip_suffix(')'))
                .and_then(|spell| spell.rsplit_once(", "))
            {
                writeln!(self.functions, "fn {label}_all(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {first}(g, item)?; g.end_sentence(); {effect}(g, item) }}").unwrap();
                parts.spell = format!("spell({targets}, {label}_all)");
                return Ok(());
            }
            let modal = parts
                .modal
                .strip_prefix("Some(&")
                .and_then(|modal| modal.strip_suffix(')'))
                .map(|modal| format!("static {modal}: ModalDef = "));
            if let Some(line) = modal.filter(|_| parts.spell == "Some(SpellDef::MODAL)") {
                if let Some(at) = self.functions.find(&line) {
                    let end = at + self.functions[at..].find(";\n").unwrap_or(0);
                    self.functions.insert_str(end, &format!(".shared({effect})"));
                    return Ok(());
                }
            }
        }
        let mut code = format!("spell({effect}).targets({targets})");
        if announcement.modes.is_some() {
            let modal = self.modal(announcement, &label, None)?;
            code = format!("{code}.modes(&{modal})");
        }
        self.plan_entry("multiple spell bodies", "spell_after", &code, parts);
        Ok(())
    }
    /// "Choose one or more, up to the number of ...": a count the game works out.
    pub(super) fn mode_count_to_come(
        &self,
        listed: &str,
        least: &Expr,
        most: &Expr,
    ) -> Result<String> {
        let told = |code: String| code.replace("i64::from(item.x)", "q.told_amount(asker)");
        let (least, most) = (
            told(self.number(least)?.code),
            told(self.number(most)?.code),
        );
        Ok(planned::planned_value(
            "mode count that is not a constant",
            &format!("{listed}.choose(1, 1)"),
            &format!("{listed}.choose_counted(|q: &dyn Query, asker: &Asker| {least}, |q: &dyn Query, asker: &Asker| {most})"),
        ))
    }
    /// Where a destination is, as code.
    fn destination_code(&self, destination: &Destination) -> Result<String> {
        Ok(match destination {
            Destination::Battlefield(player) => {
                format!(
                    "Destination::Battlefield({})",
                    self.fetch(self.player(player)?)
                )
            }
            Destination::OwnersLibraryFromTop(n) => format!(
                "Destination::OwnersLibraryFromTop({})",
                self.fetch(self.number(n)?.code)
            ),
            other => format!("Destination::{}", form(other)),
        })
    }
    /// How something enters, as code: an `Arrival` the engine doesn't have yet.
    fn arrival_code(&mut self, entry: &EntrySpec, failed: &str) -> Result<String> {
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            counters.push(format!(
                "({}, {})",
                counter_kind(kind)?,
                self.fetch(self.number(n)?.code)
            ));
        }
        let mut states = Vec::new();
        for state in &entry.states {
            states.push(self.arrival_state(state, failed)?);
        }
        Ok(format!(
            "Arrival {{ tapped: {}, counters: &[{}], states: &[{}] }}",
            entry.tapped,
            counters.join(", "),
            states.join(", ")
        ))
    }
    fn arrival_state(&mut self, state: &EntryState, failed: &str) -> Result<String> {
        let changes = |c: &mut Self, changes: &[Change]| -> Result<String> {
            let mut kinds = Vec::new();
            for change in changes {
                kinds.extend(c.effect_kinds(change)?);
            }
            Ok(kinds.join(", "))
        };
        Ok(match state {
            EntryState::Transformed => "ArrivalState::Transformed".into(),
            EntryState::Prepared => "ArrivalState::Prepared".into(),
            EntryState::FaceDown => "ArrivalState::FaceDown".into(),
            EntryState::Flipped => "ArrivalState::Flipped".into(),
            EntryState::ExilePile { shuffled } => {
                format!("ArrivalState::ExilePile {{ shuffled: {shuffled} }}")
            }
            EntryState::Attacking {
                player,
                or_planeswalker,
            } => {
                let player = match player {
                    Some(player) => self.fetch(self.player(player)?),
                    None => "None".to_string(),
                };
                format!("ArrivalState::Attacking {{ player: {player}, or_planeswalker: {or_planeswalker} }}")
            }
            EntryState::Blocking(object) => {
                format!(
                    "ArrivalState::Blocking({})",
                    self.fetch(self.object(object)?)
                )
            }
            EntryState::AttachedTo(object) => {
                format!(
                    "ArrivalState::AttachedTo({})",
                    self.fetch(self.object(object)?)
                )
            }
            EntryState::AttachedToPlayer(player) => {
                format!(
                    "ArrivalState::AttachedToPlayer({})",
                    self.fetch(self.player(player)?)
                )
            }
            EntryState::AttachedToChosen(filter) => format!(
                "ArrivalState::AttachedToChosen(|q: &dyn Query, asker: &Asker, id: ObjectId| {})",
                self.filter(filter, &Candidate::of("id"))?
            ),
            EntryState::Changes(list) => {
                format!("ArrivalState::Changes(&[{}])", changes(self, list)?)
            }
            EntryState::ChangesUntil {
                changes: list,
                duration,
            } => {
                let duration = self.duration(duration)?;
                format!(
                    "ArrivalState::ChangesUntil(&[{}], {duration})",
                    changes(self, list)?
                )
            }
            EntryState::CopyOf {
                objects,
                effects,
                until,
            } => {
                let until = match until {
                    Some(duration) => format!("Some({})", self.duration(duration)?),
                    None => "None".to_string(),
                };
                format!(
                    "ArrivalState::CopyOf({}, {}, {until})",
                    self.fetch(self.objects(objects)?),
                    self.operand_code(effects)
                )
            }
            EntryState::CountersOfChoice {
                kinds,
                amount,
                distinct,
            } => {
                let kinds = kinds
                    .iter()
                    .map(counter_kind)
                    .collect::<Result<Vec<String>>>()?;
                format!(
                    "ArrivalState::CountersOfChoice(&[{}], {}, {distinct})",
                    kinds.join(", "),
                    self.fetch(self.number(amount)?.code)
                )
            }
            EntryState::RulesUntil { rules, until } => {
                let rules = self.stated_operands(rules);
                format!(
                    "ArrivalState::RulesUntil({rules}, {})",
                    self.duration(until)?
                )
            }
            EntryState::ExiledFor(until) => {
                format!("ArrivalState::ExiledFor({})", self.duration(until)?)
            }
            EntryState::ExiledForWith {
                until,
                objects,
                body,
            } => format!(
                "ArrivalState::ExiledForWith({}, {}, {})",
                self.duration(until)?,
                self.fetch(self.objects(objects)?),
                self.operand_code(body)
            ),
            _ => return Err(gap(failed)),
        })
    }
    /// `create_tokens`, or the call it will be for what Cantrip can't make yet.
    pub(super) fn create_tokens_p(
        &mut self,
        id: usize,
        players: &PlayerSet,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        let failed = match self.create_tokens(id, players, tokens, entry, result, &mut code) {
            Ok(()) => {
                out.push_str(&code);
                return Ok(());
            }
            Err(failed) => failed,
        };
        let planned = failed.feature == "tokens entering in a special state"
            || failed.feature == "adjusted token copy"
            || failed.feature == "token of the chosen color"
            || failed.feature == "token of the chosen creature type"
            || failed.feature == "token with the abilities of other objects"
            || failed.feature == "token copy entering with counters"
            || failed.feature == "dynamic defining token characteristics"
            || failed.feature == "several kinds of token entering with counters"
            || failed.feature.starts_with("token recipe ");
        if !planned {
            return Err(failed);
        }
        let mut orders = Vec::new();
        for token in tokens {
            let amount = self.fetch(self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?);
            let adjusted = |c: &mut Self, list: &[Operand]| -> String {
                let list: Vec<String> = list.iter().map(|o| c.operand_code(o)).collect();
                list.join(", ")
            };
            let recipe = match &token.recipe {
                TokenRecipe::Defined(definition) => self.stated_definition(definition)?,
                TokenRecipe::Named(name) => format!("TokenRecipe::Named({})", quoted(name)),
                TokenRecipe::Copy {
                    object,
                    adjustments,
                } => format!(
                    "TokenRecipe::Copy({}, &[{}])",
                    self.fetch(self.object(object)?),
                    adjusted(self, adjustments)
                ),
                TokenRecipe::CopyEach {
                    objects,
                    adjustments,
                } => format!(
                    "TokenRecipe::CopyEach({}, &[{}])",
                    self.fetch(self.objects(objects)?),
                    adjusted(self, adjustments)
                ),
                TokenRecipe::CopyOne {
                    objects,
                    adjustments,
                } => format!(
                    "TokenRecipe::CopyOne({}, &[{}])",
                    self.fetch(self.objects(objects)?),
                    adjusted(self, adjustments)
                ),
                TokenRecipe::CopyNamed { name, adjustments } => format!(
                    "TokenRecipe::CopyNamed({}, &[{}])",
                    quoted(name),
                    adjusted(self, adjustments)
                ),
                TokenRecipe::CopyAtRandom { cards } => {
                    format!("TokenRecipe::CopyAtRandom({})", self.operand_code(cards))
                }
            };
            orders.push(format!("({amount}, {recipe})"));
        }
        let players = self.fetch(self.players(players)?);
        let arrival = self.arrival_code(entry, &failed.feature)?;
        let call = format!(
            "{M}::tokens::create(g, asker, {players}, &[{}], {arrival})?",
            orders.join(", ")
        );
        let created = planned::planned_value(&failed.feature, "Vec::<ObjectId>::new()", &call);
        writeln!(out, "let created_{id}: Vec<ObjectId> = {created};").unwrap();
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    /// `move_objects`, or the call it will be for an arrival Cantrip can't make yet.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn move_objects_p(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        destination: &Destination,
        entry: &EntrySpec,
        face_down: bool,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        let failed = match face_down {
            true => gap("operation Move"),
            false => match self.move_objects(id, objects, destination, entry, result, &mut code) {
                Ok(()) => {
                    out.push_str(&code);
                    return Ok(());
                }
                Err(failed) => failed,
            },
        };
        const STATES: [&str; 5] = [
            "operation Move",
            "move arrival state",
            "move arrival with a color",
            "move arrival flags",
            "several objects arriving in a state",
        ];
        if !STATES.contains(&failed.feature.as_str()) {
            return Err(failed);
        }
        let list = self.fetch(self.objects(objects)?);
        let destination = self.destination_code(destination)?;
        let arrival = self.arrival_code(entry, &failed.feature)?;
        let call = format!(
            "{M}::enter::in_state(g, asker, &objects_{id}, {destination}, {arrival}, {face_down})?"
        );
        let moved = planned::planned_value(&failed.feature, "Vec::<ObjectId>::new()", &call);
        writeln!(
            out,
            "let objects_{id}: Vec<ObjectId> = {list}; let moved_{id}: Vec<ObjectId> = {moved};"
        )
        .unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        Ok(())
    }
    /// `change_zone`, or the call it will be for a rule the chosen cards keep.
    pub(super) fn change_zone_p(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        let failed = match self.change_zone(id, op, result, &mut code) {
            Ok(()) => {
                out.push_str(&code);
                return Ok(());
            }
            Err(failed) => failed,
        };
        let Operation::ChangeZone {
            actor,
            cards,
            count,
            random,
            group,
            destination,
            entry,
            face_down,
        } = op
        else {
            return Err(failed);
        };
        if failed.feature != "operation ChangeZone" && failed.feature != "move to Hand" {
            return Err(failed);
        }
        let actor = self.fetch(self.players(actor)?);
        let cards = self.fetch(self.objects(cards)?);
        let count = match count {
            Some(count) => format!("Some({})", self.fetch(self.select_count(count)?)),
            None => "None".to_string(),
        };
        let group = match group {
            Some(group) => format!("Some({})", self.operand_code(group)),
            None => "None".to_string(),
        };
        let entry: Vec<String> = entry.iter().map(|o| self.operand_code(o)).collect();
        let face_down = match face_down {
            Some(as_) => {
                let shape = self.operand_code(&as_.power_toughness);
                let kind = match &as_.creature_type {
                    Some(kind) => format!("Some({})", self.operand_code(kind)),
                    None => "None".to_string(),
                };
                format!(
                    "Some(FaceDownAs {{ shape: {shape}, creature_type: {kind}, artifact: {} }})",
                    as_.artifact
                )
            }
            None => "None".to_string(),
        };
        let destination = self.destination_code(destination)?;
        let call = format!(
            "{M}::enter::change_zone(g, asker, ZoneChange {{ actor: {actor}, cards: {cards}, count: {count}, random: {random}, group: {group}, destination: {destination}, entry: &[{}], face_down: {face_down} }})?",
            entry.join(", ")
        );
        let moved = planned::planned_value(&failed.feature, "Vec::<ObjectId>::new()", &call);
        writeln!(out, "let moved_{id}: Vec<ObjectId> = {moved};").unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        self.selections.insert(result.0, format!("moved_{id}"));
        Ok(())
    }
    /// `search_zones`, or the call it will be for what `library::search` can't do.
    pub(super) fn search_zones_p(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        let failed = match self.search_zones(id, op, result, &mut code) {
            Ok(()) => {
                out.push_str(&code);
                return Ok(());
            }
            Err(failed) => failed,
        };
        let Operation::Search {
            player,
            library,
            top,
            choices,
            also,
            either,
            outside,
        } = op
        else {
            return Err(failed);
        };
        let planned = failed.feature == "operation Search"
            || failed.feature == "search with several find clauses"
            || failed.feature.starts_with("search constraint ");
        if !planned {
            return Err(failed);
        }
        let mut finds = Vec::new();
        for choice in choices {
            let test = self.filter(&choice.filter, &Candidate::of("id"))?;
            let count = self.fetch(self.select_count(&choice.count)?);
            let mut rules = Vec::new();
            for rule in &choice.constraints {
                rules.push(match self.set_rule(rule) {
                    Some(rule) => rule,
                    None => format!("Part::{}", form(rule)),
                });
            }
            finds.push(format!(
                "Find {{ filter: |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, count: {count}, rules: &[{}] }}",
                rules.join(", ")
            ));
        }
        let top = match top {
            Some(n) => format!("Some({})", self.fetch(self.number(n)?.code)),
            None => "None".to_string(),
        };
        let zones: Vec<String> = also
            .iter()
            .map(|zone| format!("Zone::{}", form(zone)))
            .collect();
        let searcher = self.fetch(self.certain_player(player)?);
        let owner = self.fetch(self.certain_player(library)?);
        let call = format!(
            "{M}::library::search_zones(g, {searcher}, {owner}, asker.source, Searched {{ top: {top}, also: &[{}], either: {either}, outside: {outside}, finds: &[{}] }})?",
            zones.join(", "),
            finds.join(", ")
        );
        let found = planned::planned_value(&failed.feature, "Vec::<ObjectId>::new()", &call);
        writeln!(out, "let found_{id}: Vec<ObjectId> = {found};").unwrap();
        self.selections.insert(result.0, format!("found_{id}"));
        Ok(())
    }
    /// "It perpetually gets +1/+1" (Alchemy, a digital-only rule): to each object,
    /// wherever it goes, for the rest of the game.
    pub(super) fn perpetual(
        &mut self,
        id: usize,
        subject: &EffectSubject,
        effects: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        const FEATURE: &str = "engine: no perpetual effects";
        let EffectSubject::Objects(objects) = subject else {
            return Err(gap(FEATURE));
        };
        let objects = self.fetch(self.objects(objects)?);
        writeln!(out, "let objects_{id}: Vec<ObjectId> = {objects};").unwrap();
        let effects: Vec<String> = effects.iter().map(|e| self.operand_code(e)).collect();
        let applied: Vec<String> = effects
            .iter()
            .map(|effect| format!("{M}::perpetual::apply(g, asker.source, object, {effect});"))
            .collect();
        let code = format!("for &object in &objects_{id} {{ {} }}", applied.join(" "));
        self.plan_code(FEATURE, &code, out);
        Ok(())
    }
}

/// The words of a filter a player is asked to choose: "land", "nonland".
fn filter_word(filter: &ObjectFilter) -> Option<String> {
    Some(match filter {
        ObjectFilter::CardType(t) => name(t).to_lowercase(),
        ObjectFilter::Subtype(t) => name(t),
        ObjectFilter::Color(c) => name(c).to_lowercase(),
        ObjectFilter::Not(inner) => format!("non{}", filter_word(inner)?),
        ObjectFilter::All(parts) => parts
            .iter()
            .map(filter_word)
            .collect::<Option<Vec<_>>>()?
            .join(" "),
        _ => return None,
    })
}

/// The rest of `sharing`: `a` and `b` are the characteristics of two objects.
pub(super) fn sharing_p(property: &DistinctProperty) -> Result<&'static str> {
    Ok(match property {
        DistinctProperty::LandTypes => {
            "a.subtypes.iter().any(|s| SubtypeSet::LAND_TYPES.has(s) && b.subtypes.has(s))"
        }
        DistinctProperty::PermanentTypes => {
            "a.types.iter().any(|t| matches!(t, CardType::Artifact | CardType::Battle | CardType::Creature | CardType::Enchantment | CardType::Land | CardType::Planeswalker) && b.types.has(t))"
        }
        DistinctProperty::ManaValues => {
            "planned_value!(\"sharing ManaValues\", false => a.mana_value() == b.mana_value())"
        }
        DistinctProperty::ManaCosts => {
            "planned_value!(\"sharing ManaCosts\", false => a.mana_cost() == b.mana_cost())"
        }
        other => return Err(gap(format!("sharing {other:?}"))),
    })
}

/// The rest of `property_rest`.
pub(super) fn property_p(property: &Property, id: &str) -> Result<Number> {
    let held = |code: String| {
        lane_j::held_number(format!("numeric object property {}", form(property)), &code)
    };
    Ok(match property {
        Property::CardTypeCount => {
            lane_j::counted(format!("q.chars({id}).types.iter().count() as i64"))
        }
        Property::SubtypeCount => {
            lane_j::counted(format!("q.chars({id}).subtypes.iter().count() as i64"))
        }
        Property::SupertypeCount => lane_j::counted(format!(
            "{{ let kinds = q.chars({id}).supertypes; i64::from(kinds.basic) + i64::from(kinds.legendary) + i64::from(kinds.snow) }}"
        )),
        Property::BushidoValue => held(format!("i64::from(q.chars({id}).bushido)")),
        Property::ColorSymbolCount => held(format!("i64::from(q.mana_cost({id}).color_symbols())")),
        Property::CrewCount => held(format!("q.crewed_by({id}).len() as i64")),
        // The ability's own link where it asks of its source (CR 607.5), and the
        // text of any other object.
        Property::ExiledCount => lane_j::counted(format!(
            "q.exiled_with(if {id} == asker.source {{ q.linked_to(asker) }} else {{ q.text_linked({id}) }}).len() as i64"
        )),
        Property::RegenerationsThisTurn => held(format!("i64::from(q.regenerations_this_turn({id}))")),
        other => return Err(gap(format!("numeric object property {}", form(other)))),
    })
}

/// `AuraSwamp` as a module's name: `aura_swamp`.
/// One test over `among` for each kind of ability a list names, where the engine
/// holds each as a keyword.
fn abilities_among(abilities: &Operand) -> Option<Vec<String>> {
    let Operand::List(list) = abilities else {
        return None;
    };
    list.iter()
        .map(|kind| match kind {
            Operand::Term { form, .. } if form == "AnyHexproof" => Some(
                "among.iter().any(|c| c.has(Keyword::Hexproof) || c.hexproof_from != HexproofFrom::NONE)"
                    .to_string(),
            ),
            kind => lane_n::plain_keywords(kind)
                .map(|words| format!("among.iter().any(|c| c.has({}))", words.join(""))),
        })
        .collect()
}

fn snake(word: &str) -> String {
    let mut out = String::new();
    for (i, c) in word.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// "Target permanent or suspended card" in a "choose one": a choice of target
/// inside a mode is that many modes among those of the card (CR 700.2).
pub(super) fn flat_modes(selection: &ModeSelection) -> Option<Vec<Mode>> {
    let one = |n: &Expr| matches!(n, Expr::Constant(1));
    let inner = |mode: &Mode| match &mode.announcement.modes {
        Some(inner) if one(&inner.minimum) && one(&inner.maximum) && !inner.random => {
            Some(inner.modes.clone())
        }
        _ => None,
    };
    if !one(&selection.minimum) || !one(&selection.maximum) || selection.random {
        return None;
    }
    if !selection.modes.iter().any(|mode| inner(mode).is_some()) {
        return None;
    }
    let mut flat = Vec::new();
    for mode in &selection.modes {
        match inner(mode) {
            Some(modes)
                if mode.announcement.targets.is_empty() && mode.body.instructions.is_empty() =>
            {
                flat.extend(modes)
            }
            Some(_) => return None,
            None => flat.push(mode.clone()),
        }
    }
    Some(flat)
}

impl Compiler {
    /// `read`, as a question asked of the game rather than of a resolution.
    fn reading_asked<T>(&mut self, read: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let read = read(self);
        (self.reader, self.event_at) = saved;
        read
    }
    /// A target of the ability this one came from: a delayed ability's item has
    /// no such slot, and what it remembers is still to come.
    pub(super) fn target_gone(&self, object: &ObjectRef) -> Option<String> {
        let ObjectRef::Target { slot, index } = object else {
            return None;
        };
        let gone = self.stack_item && self.choosing.is_none() && !self.slots.contains_key(&slot.0);
        gone.then(|| {
            planned::planned_value(
                "target reference",
                "None::<ObjectId>",
                &format!("item.remembered_target({}, {index})", slot.0),
            )
        })
    }
    pub(super) fn targets_gone(&self, objects: &ObjectSet) -> Option<String> {
        match objects {
            ObjectSet::One(object) => self
                .target_gone(object)
                .map(|one| format!("{one}.into_iter().collect::<Vec<ObjectId>>()")),
            ObjectSet::Targets(slot) => {
                let gone =
                    self.stack_item && self.choosing.is_none() && !self.slots.contains_key(&slot.0);
                gone.then(|| {
                    planned::planned_value(
                        "target reference",
                        "Vec::<ObjectId>::new()",
                        &format!("item.remembered_targets({})", slot.0),
                    )
                })
            }
            _ => None,
        }
    }
    /// What happened to an object that the history keeps no record of yet.
    pub(super) fn object_history_p(
        &self,
        event: &ObjectHistory,
        id: &str,
        window: &ObjectWindow,
    ) -> Result<String> {
        use ObjectHistory as H;
        let feature = format!("object filter History {}", form(event));
        let word = snake(&form(event));
        match event {
            H::AttackedDefender(filter) => {
                return Ok(format!(
                    "{}.any(|r| matches!(r.event, Event::Attacks {{ attacker, defender: Target::Object(defended), .. }} if attacker == {id} && {}))",
                    self.object_window(window)?,
                    self.filter(filter, &Candidate::of("defended"))?
                ));
            }
            // Enlist taps a creature (CR 702.154a), whatever it has become since.
            H::Enlisted(filter)
                if matches!(**filter, ObjectFilter::CardType(crate::mtg_types::CardType::Creature)) =>
            {
                return Ok(format!(
                    "{}.any(|r| matches!(r.event, Event::PaidToAttack {{ attacker, option: AttackOption::Enlist, tapped, .. }} if attacker == {id} && tapped != ObjectId::NONE))",
                    self.object_window(window)?
                ));
            }
            // A card cycled was discarded to pay for it (CR 702.29a).
            H::CycledOrDiscarded => {
                return Ok(format!(
                    "{}.any(|r| matches!(r.event, Event::Discarded {{ card, player, .. }} if card == {id} && player == asker.controller))",
                    self.object_window(window)?
                ));
            }
            // An activation's cost keeps what it tapped on the item, which stays
            // readable after it has left the stack, however it left.
            H::TappedToPayFor(object) if matches!(window, ObjectWindow::ThisTurn) => {
                return Ok(format!(
                    "{}.is_some_and(|of: ObjectId| {{ let s = q.state(); let began = q.history().this_turn().first().map_or(u32::MAX, |r| r.serial); s.stack.iter().chain(s.left_stack.iter()).any(|i| i.kind == StackKind::Activated && i.since >= began && (i.object == of || q.obj(i.object).became == Some(of)) && q.cost_objects(i).contains(&{id})) }})",
                    self.object(object)?
                ));
            }
            _ => {}
        }
        let about = match event {
            H::AttackedDefender(filter) | H::Enlisted(filter) => format!(
                ", |q: &dyn Query, id: ObjectId| {}",
                self.filter(filter, &Candidate::of("id"))?
            ),
            H::ExiledFromGraveyard(players) => format!(", {}", self.players(players)?),
            H::TappedToPayFor(object) => format!(", {}", self.object(object)?),
            H::CountersPutBy { .. } => String::new(),
            H::CountersPutWith { kind, with } | H::CountersRemovedWith { kind, with } => {
                format!(", {}, {}", counter_kind(kind)?, self.object(with)?)
            }
            H::PhasedOutWith(with) => format!(", {}", self.object(with)?),
            _ => String::new(),
        };
        let during = self.object_window(window)?;
        Ok(planned::planned_value(
            &feature,
            "false",
            &format!("cantrip_mechanics::history::{word}({during}, {id}{about})"),
        ))
    }
    /// A stretch of history with no word yet.
    pub(super) fn object_window_p(&self, window: &ObjectWindow) -> Result<String> {
        let feature = format!("history window {}", form(window));
        let code = match window {
            ObjectWindow::SinceLastTurnOf(player) => {
                return Ok(format!(
                    "{{ let of = {}; let all = q.history().all(); all[all.iter().rposition(|r| matches!(r.event, Event::TurnEnded {{ player }} if Some(player) == of)).map_or(0, |at| at + 1)..].iter() }}",
                    self.player(player)?
                ));
            }
            ObjectWindow::LastCombatOf(player) => {
                format!(
                    "cantrip_mechanics::history::last_combat_of(q, {}).iter()",
                    self.player(player)?
                )
            }
            ObjectWindow::SinceLastUpkeepOf(player) => {
                format!(
                    "cantrip_mechanics::history::since_last_upkeep_of(q, {}).iter()",
                    self.player(player)?
                )
            }
            other => return Err(gap(format!("history window {}", form(other)))),
        };
        Ok(planned::planned_value(
            &feature,
            "q.history().all().iter()",
            &code,
        ))
    }
    /// A number the game works out when it is needed.
    fn number_fn(&mut self, number: &Expr) -> Result<String> {
        let code = self.reading_asked(|c| Ok(c.number(number)?.code))?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker| -> i64 {{ {code} }}"
        ))
    }
    /// A filter the game asks of objects when it is needed.
    fn filter_fn(&mut self, filter: &ObjectFilter) -> Result<String> {
        let code = self.reading_asked(|c| c.filter(filter, &Candidate::of("id")))?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, id: ObjectId| -> bool {{ {code} }}"
        ))
    }
    /// A condition the game asks when it is needed.
    fn condition_fn(&mut self, condition: &Condition) -> Result<String> {
        let code = self.reading_asked(|c| c.condition(condition))?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker| -> bool {{ {code} }}"
        ))
    }
    /// What a keyword with a number the game works out, or a mana cost with an
    /// {X} of its own, will be in the card's lists.
    fn mechanic_entry(
        &mut self,
        mechanic: Mechanic,
        cost: Option<&Cost>,
        amount: Option<&Expr>,
        filter: Option<&ObjectFilter>,
        label: &str,
    ) -> Result<(&'static str, String)> {
        use Mechanic as K;
        let name = format!("{mechanic:?}");
        let module = snake(name.trim_end_matches('X'));
        let word = quoted(&self.said_word(name.clone()));
        let open = || gap(format!("engine: no {mechanic:?}"));
        let amount_fn = |c: &mut Self| -> Result<String> { c.number_fn(amount.ok_or_else(open)?) };
        let price = |c: &mut Self| c.mechanic_cost(cost, mechanic, label);
        Ok(match mechanic {
            K::Firebending | K::Mobilize => (
                "abilities",
                format!("{M}::{module}::trigger_by({}, {word})", amount_fn(self)?),
            ),
            K::AnnihilatorX | K::Bushido | K::Soulshift => (
                "abilities",
                format!(
                    "{M}::triggers::{}_by({}, {word})",
                    snake(name.trim_end_matches('X')),
                    amount_fn(self)?
                ),
            ),
            // Modular—Sunburst: as many counters as colors of mana were spent.
            K::Modular => match amount {
                Some(Expr::ManaSpent { .. }) => {
                    ("abilities", format!("{M}::modular::sunburst({word})"))
                }
                _ => (
                    "abilities",
                    format!("{M}::modular::ability_by({}, {word})", amount_fn(self)?),
                ),
            },
            K::Reinforce => (
                "abilities",
                format!("{M}::reinforce::ability_x({}, {word})", price(self)?),
            ),
            K::BestowX | K::FreerunningX | K::SurgeX | K::WarpX => (
                "ways",
                format!("{M}::{module}::way_x({}, {word})", price(self)?),
            ),
            K::ForetellX => (
                "abilities",
                format!("{M}::foretell::ability_x({})", price(self)?),
            ),
            K::CommanderNinjutsu => (
                "abilities",
                format!("{M}::ninjutsu::commander_ability({}, {word})", price(self)?),
            ),
            K::EmergeFromArtifact => (
                "ways",
                format!("{M}::emerge::from_artifact({}, {word})", price(self)?),
            ),
            K::AuraSwamp => (
                "abilities",
                format!("{M}::aura_swap::ability({}, {word})", price(self)?),
            ),
            K::Equip => {
                let filter = self.filter_fn(filter.ok_or_else(open)?)?;
                let price = price(self)?;
                (
                    "abilities",
                    format!("{M}::equip::ability_onto({price}, {filter}, {word})"),
                )
            }
            K::TrampleOverPlaneswalkers => ("keywords", "Keyword::TrampleOverPlaneswalkers".into()),
            K::SpliceOnto => {
                let filter = self.filter_fn(filter.ok_or_else(open)?)?;
                let price = price(self)?;
                (
                    "splice",
                    format!("Some({M}::splice::onto({filter}, {price}))"),
                )
            }
            K::BasicMayhem => ("ways", format!("{M}::mayhem::land_way({word})")),
            K::SpaceSculptor => ("abilities", format!("{M}::space_sculptor::ABILITY")),
            K::CasualtyX => (
                "optional_cost",
                format!("Some({M}::casualty::optional_x())"),
            ),
            // "Devour X, where X is the number of creatures devoured this way."
            K::Devour => match amount {
                Some(Expr::Earlier {
                    what: EarlierObjects::Devoured,
                    ..
                }) => (
                    "enters",
                    format!("{M}::devour::enters_by(|devoured: u32| devoured)"),
                ),
                _ => return Err(open()),
            },
            _ => return Err(open()),
        })
    }
    /// The entries an ability makes, whatever lists they go in.
    fn entries_of(
        &mut self,
        ability: &Ability,
        label: &str,
    ) -> Result<Vec<(&'static str, String)>> {
        let mut parts = Parts::default();
        self.ability(ability, label, &mut parts, None)?;
        let mut entries = Vec::new();
        for (list, each) in [
            ("keywords", parts.keywords),
            ("abilities", parts.abilities),
            ("statics", parts.statics),
            ("replacements", parts.replacements),
            ("preventions", parts.preventions),
            ("enters", parts.enters),
            ("block_rules", parts.block_rules),
            ("block_requirements", parts.block_requirements),
            ("attack_rules", parts.attack_rules),
            ("untap_bans", parts.untap_bans),
        ] {
            entries.extend(each.into_iter().map(|entry| (list, entry)));
        }
        for (list, each) in parts.tables {
            entries.extend(each.into_iter().map(|entry| (list, entry)));
        }
        Ok(entries)
    }
    /// The entries a failed ability will be, where all of what it says can be
    /// stated: `plan_ability` writes these in the place of its source.
    pub(super) fn state_ability(
        &mut self,
        feature: &str,
        ability: &Ability,
        label: &str,
    ) -> Option<Vec<(&'static str, String)>> {
        match ability {
            Ability::Mechanic {
                mechanic,
                cost,
                amount,
                filter,
            } => {
                let entry = self.mechanic_entry(
                    *mechanic,
                    cost.as_ref(),
                    amount.as_ref(),
                    filter.as_ref(),
                    label,
                );
                self.dividing = None;
                entry.ok().map(|entry| vec![entry])
            }
            // "This has annihilator X": what it says of itself.
            Ability::Static {
                affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                changes,
                ..
            } => match changes.as_slice() {
                [Change::GrantAbility(granted)] => self.state_ability(feature, granted, label),
                _ => self.stated_static(ability, label),
            },
            // "Creature spells you cast have sticker kicker {1}."
            Ability::Static {
                affected: EffectSubjects::Objects(ObjectSet::Zone { filter, .. }),
                changes,
                ..
            } => match changes.as_slice() {
                [Change::GrantAbility(granted)] => {
                    let Ability::Mechanic { mechanic, cost, .. } = &**granted else {
                        return None;
                    };
                    let filter = self.filter_fn(filter).ok()?;
                    let module = snake(&format!("{mechanic:?}"));
                    let call = match cost {
                        Some(_) => {
                            let price = self.mechanic_cost(cost.as_ref(), *mechanic, label).ok()?;
                            format!("{M}::{module}::spells_have({filter}, {price})")
                        }
                        None => format!("{M}::{module}::spells_have({filter})"),
                    };
                    Some(vec![("statics", call)])
                }
                _ => None,
            },
            Ability::Scoped {
                zones,
                condition,
                ability,
            } => {
                let zones = zones
                    .iter()
                    .map(|z| format!("EventZone::{z:?}"))
                    .collect::<Vec<_>>();
                let test = match condition {
                    Some(condition) => format!("Some({})", self.condition_fn(condition).ok()?),
                    None => "None".to_string(),
                };
                let entries = self.state_or_entries(feature, ability, label)?;
                let zones = zones.join(", ");
                Some(
                    entries
                        .into_iter()
                        .map(|(list, entry)| {
                            (
                                list,
                                format!("{M}::zones::scoped(&[{zones}], {test}, {entry})"),
                            )
                        })
                        .collect(),
                )
            }
            Ability::Conditional {
                condition,
                then,
                otherwise,
            } if otherwise.is_empty() => {
                let test = self.condition_fn(condition).ok()?;
                let mut entries = Vec::new();
                for (i, ability) in then.iter().enumerate() {
                    let label = format!("{label}_then_{i}");
                    for (list, entry) in self.state_or_entries(feature, ability, &label)? {
                        entries.push((list, format!("{M}::zones::when({test}, {entry})")));
                    }
                }
                Some(entries)
            }
            Ability::CostBranch {
                keyword,
                cost,
                paid,
                unpaid,
            } => {
                let price = self.cost(cost, &format!("{label}_price")).ok()?;
                let paid = self.spell_branch(paid, &format!("{label}_paid")).ok()?;
                let unpaid = self.spell_branch(unpaid, &format!("{label}_unpaid")).ok()?;
                Some(vec![(
                    "spell",
                    format!(
                        "{M}::cost_branch::spell({}, {price}, {paid}, {unpaid})",
                        quoted(keyword)
                    ),
                )])
            }
            // "As long as you're at max speed, you may cast this card from your graveyard."
            Ability::Static {
                affected: EffectSubjects::Players(PlayerSet::One(PlayerRef::AbilityController)),
                changes,
                ..
            } => {
                let [Change::PlayerRule(PlayerRule::PlayPermission {
                    cards: ObjectSet::One(ObjectRef::Source),
                    from,
                    action,
                    filter,
                })] = changes.as_slice()
                else {
                    return None;
                };
                let filter = self.filter_fn(filter).ok()?;
                Some(vec![(
                    "ways",
                    format!("{M}::zones::may_play_from(EventZone::{from:?}, PlayAction::{action:?}, {filter})"),
                )])
            }
            Ability::Static { .. } => self.stated_static(ability, label),
            Ability::CountsAsNamed { name, for_spells } => Some(vec![(
                "statics",
                format!(
                    "{M}::zones::counts_as_named({}, {})",
                    quoted(name),
                    quoted(for_spells)
                ),
            )]),
            Ability::Gift {
                gift,
                promised,
                unpromised,
            } => {
                let mut gifts = Vec::new();
                for (i, program) in gift.iter().enumerate() {
                    gifts.push(self.apart(program, &format!("{label}_gift_{i}")).ok()?);
                }
                let mut sides = Vec::new();
                for (side, ability) in [("promised", promised), ("unpromised", unpromised)] {
                    sides.push(match ability {
                        Some(ability) => {
                            let branch = self
                                .spell_branch(ability, &format!("{label}_{side}"))
                                .ok()?;
                            format!("Some({branch})")
                        }
                        None => "None".to_string(),
                    });
                }
                Some(vec![
                    (
                        "gift",
                        format!(
                            "Some(GiftDef {{ kind: Gift::Custom, give: &[{}] }})",
                            gifts.join(", ")
                        ),
                    ),
                    (
                        "spell",
                        format!("{M}::gift::spell({}, {})", sides[0], sides[1]),
                    ),
                ])
            }
            Ability::OpeningHand { announcement, body } => {
                self.announcement(announcement, label).ok()?;
                let effect = self.effect(body, label).ok()?;
                Some(vec![(
                    "opening_hand",
                    format!("Some({M}::statics::opening_hand({effect}))"),
                )])
            }
            Ability::Enchant {
                hosts: EnchantHosts::GraveyardCards { filter, owners },
            } => {
                let filter = self.filter_fn(filter).ok()?;
                let owners = self.reading_asked(|c| c.player_in(owners, "owner")).ok()?;
                Some(vec![(
                    "enchant",
                    format!("Some(cantrip_mechanics::targets::graveyard_card(|q: &dyn Query, asker: &Asker, owner: PlayerId| {owners}, {filter}))"),
                )])
            }
            _ => None,
        }
    }
    fn state_or_entries(
        &mut self,
        feature: &str,
        ability: &Ability,
        label: &str,
    ) -> Option<Vec<(&'static str, String)>> {
        match self.entries_of(ability, label) {
            Ok(entries) => Some(entries),
            Err(_) => self.state_ability(feature, ability, label),
        }
    }
    /// What the rest of a counter removal needs: the arms Cantrip has a call for
    /// come first, then what is stated.
    pub(super) fn removal_rest(
        &mut self,
        id: usize,
        i: usize,
        removal: &CounterRemoval,
        out: &mut String,
    ) -> Result<()> {
        let objects = format!("objects_{id}_{i}");
        let player = self.fetch(self.certain_player(&removal.player)?);
        let kind = match &removal.kind {
            Some(kind) => format!("Some({})", counter_kind(kind)?),
            None => "None".to_string(),
        };
        let (feature, amount) = match &removal.amount {
            CounterAmount::AmongThem(count) => (
                format!("counter removal {}", form(&removal.amount)),
                format!("RemovedCounters::AmongThem({})", self.select_count(count)?),
            ),
            CounterAmount::Count(count) => {
                // "Remove X counters": which ones are the player's, of any kinds, and
                // they all come off if they can; "any number" may leave them.
                if removal.kind.is_none() {
                    const MANY: &str = "cantrip_mechanics::select::Count::AsManyAsCan(";
                    let code = self.select_count(count)?;
                    let sized = code.strip_prefix(MANY).and_then(|c| c.strip_suffix(')'));
                    if code.ends_with("::AnyNumber") {
                        writeln!(out, "let player_{id}_{i} = {player}; for object in {objects}.iter().copied() {{ removed_{id} += i64::from(g.remove_counters_up_to(player_{id}_{i}, asker.source, object, u8::MAX)?); }}").unwrap();
                        return Ok(());
                    }
                    if let Some(sized) = sized {
                        let sized = self.fetch(sized.to_string());
                        writeln!(out, "let player_{id}_{i} = {player}; let most_{id}_{i}: u32 = {sized}; for object in {objects}.iter().copied() {{ for _ in 0..most_{id}_{i} {{ let Some(kind) = g.choose_counter_kind(player_{id}_{i}, asker.source, object)? else {{ break }}; removed_{id} += i64::from(g.remove_counters(object, kind, 1)); }} }}").unwrap();
                        return Ok(());
                    }
                }
                let feature = match removal.kind {
                    Some(_) => format!("counter removal {}", form(&removal.amount)),
                    None => "counters of a kind chosen as the effect resolves".to_string(),
                };
                (
                    feature,
                    format!("RemovedCounters::Each({})", self.select_count(count)?),
                )
            }
            CounterAmount::All => return Err(gap("counter removal All")),
        };
        let code = format!("removed_{id} += i64::from({M}::effects::remove_chosen_counters(g, {player}, asker.source, &{objects}, {kind}, {amount})?);");
        self.plan_code(&feature, &code, out);
        Ok(())
    }
    /// The code of a failed instruction, every operand of it in its place, for
    /// `plan_instruction` to write where the operation has no generator arm.
    pub(super) fn state_instruction(
        &mut self,
        feature: &str,
        instruction: &Instruction,
    ) -> Option<String> {
        if let Instruction::ChooseOne {
            decider,
            options,
            offered: Some(offered),
        } = instruction
        {
            let decider = self.fetch(self.certain_player(decider).ok()?);
            let offered = self.fetch(self.number(offered).ok()?.code);
            let mut each = Vec::new();
            for option in options {
                let label = format!("offer_{}", self.fresh());
                each.push(self.apart(option, &label).ok()?);
            }
            return Some(format!(
                "{M}::choose::among_random(g, item, {decider}, {offered}, &[{}])?",
                each.join(", ")
            ));
        }
        let Instruction::Effect { operation, .. } = instruction else {
            return None;
        };
        self.state_operation(feature, operation).ok()
    }
    fn stated_players(&self, players: &PlayerSet) -> Result<String> {
        Ok(self.fetch(self.players(players)?))
    }
    pub(super) fn stated_operands(&mut self, operands: &[Operand]) -> String {
        let mut each = Vec::new();
        for operand in operands {
            each.push(self.operand_code(operand));
        }
        format!("&[{}]", each.join(", "))
    }
    fn state_operation(&mut self, feature: &str, operation: &Operation) -> Result<String> {
        let source = "asker.source";
        let refused = || gap(feature.to_string());
        Ok(match operation {
            Operation::Keyword {
                actor,
                action: KeywordAction::Earthbend,
                object: Some(object),
                amount: Some(amount),
                ..
            } => {
                let actors = self.stated_players(actor)?;
                let object = self.fetch(self.object(object)?);
                let amount = self.fetch(self.number(amount)?.amount(i16::MAX.into(), "i16")?);
                format!("for player in {actors} {{ if let Some(land) = {object} {{ {M}::earthbend::earthbend(g, player, land, {amount}, item)?; }} }}")
            }
            Operation::Keyword {
                actor,
                action: KeywordAction::TakeExtraTurn,
                amount: Some(Expr::HeadsFlipped(result)),
                ..
            } if self.flips.contains_key(&result.0) => {
                let actors = self.stated_players(actor)?;
                let heads = self.flips[&result.0].clone();
                format!("for player in {actors} {{ for _ in 0..{heads}.iter().filter(|won| **won).count() {{ g.take_extra_turn(player); }} }}")
            }
            Operation::Conjure {
                actor,
                card,
                count,
                zone,
                owner,
                spot,
                entry,
            } => {
                let actors = self.stated_players(actor)?;
                let what = match card {
                    ConjuredCard::Named(name) => format!("Conjured::Named({name:?})"),
                    ConjuredCard::OneOf(names) => format!("Conjured::OneOf(&{names:?})"),
                    ConjuredCard::Each(names) => format!("Conjured::Each(&{names:?})"),
                    ConjuredCard::Spellbook { book, pick } => {
                        format!("Conjured::FromSpellbook({book:?}, SpellbookPick::{pick:?})")
                    }
                    ConjuredCard::Duplicate(objects) => {
                        format!(
                            "Conjured::DuplicateOf(&{})",
                            self.fetch(self.objects(objects)?)
                        )
                    }
                    ConjuredCard::Random(cards) => {
                        format!("Conjured::AtRandom({})", self.operand_code(cards))
                    }
                    ConjuredCard::SpellbookMatching { book, cards } => format!(
                        "Conjured::FromSpellbookMatching({book:?}, {})",
                        self.operand_code(cards)
                    ),
                };
                let owner = match owner {
                    Some(owner) => self.fetch(self.certain_player(owner)?),
                    None => "player".to_string(),
                };
                let spot = match spot {
                    None => "ConjureSpot::Random".to_string(),
                    Some(ConjureSpot::Shuffled) => "ConjureSpot::Shuffled".to_string(),
                    Some(ConjureSpot::Top) => "ConjureSpot::Top".to_string(),
                    Some(ConjureSpot::AmongTop(n)) => {
                        format!(
                            "ConjureSpot::AmongTop({})",
                            self.fetch(self.number(n)?.code)
                        )
                    }
                    Some(ConjureSpot::FromTop(n)) => {
                        format!("ConjureSpot::FromTop({})", self.fetch(self.number(n)?.code))
                    }
                };
                let count = self.fetch(self.number(count)?.code);
                let entry = self.stated_operands(entry);
                format!("for player in {actors} {{ {M}::conjure::conjure(g, player, {owner}, {source}, {what}, {count}, ConjureTo::Entering(EventZone::{zone:?}, {spot}, {entry}))?; }}")
            }
            Operation::CreateContinuousEffect {
                affected: EffectSubjects::Objects(objects),
                changes,
                duration,
                ..
            } => {
                let objects = self.fetch(self.objects(objects)?);
                let mut grants = Vec::new();
                for change in changes {
                    grants.push(match change {
                        Change::ChosenAbility => "Grant::ChosenAbility".to_string(),
                        Change::Pump { power, toughness } => format!(
                            "Grant::Pump({}, {})",
                            self.fetch(self.number(power)?.code),
                            self.fetch(self.number(toughness)?.code)
                        ),
                        Change::GrantKeyword(keyword) => format!(
                            "Grant::Keyword({})",
                            self.stated_keyword(keyword).ok_or_else(refused)?
                        ),
                        _ => return Err(refused()),
                    });
                }
                let duration = self.duration(duration)?;
                format!(
                    "{M}::effects::grant(g, item, {objects}, &[{}], {duration})?",
                    grants.join(", ")
                )
            }
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => {
                let players = self.stated_players(players)?;
                let objects = self.fetch(self.objects(objects)?);
                let choose = match choose {
                    Some(count) => format!("Some({})", self.fetch(self.select_count(count)?)),
                    None => "None".to_string(),
                };
                format!(
                    "{M}::effects::sacrifice_among(g, {source}, {players}, {objects}, {choose})?"
                )
            }
            Operation::BeginOnBattlefield { actor, flags } => {
                let actors = self.stated_players(actor)?;
                format!(
                    "{M}::statics::begin_on_battlefield(g, {source}, {actors}, {})?",
                    self.operand_code(flags)
                )
            }
            Operation::PayCost {
                player,
                cost,
                optional,
            } => {
                let [CostPart::Move {
                    selections,
                    destination,
                    entry,
                    face_down,
                }] = cost.parts.as_slice()
                else {
                    return Err(refused());
                };
                let player = self.fetch(self.certain_player(player)?);
                let mut each = Vec::new();
                for selection in selections {
                    let objects = self.fetch(self.objects(&selection.objects)?);
                    let count = match &selection.choose {
                        Some(count) => format!("Some({})", self.fetch(self.select_count(count)?)),
                        None => "None".to_string(),
                    };
                    each.push(format!("({objects}, {count})"));
                }
                let destination = self.destination_code(destination)?;
                let arrival = self.arrival_code(entry, feature)?;
                format!("{M}::enter::pay_by_moving(g, asker, {player}, &[{}], {destination}, {arrival}, {face_down}, {optional})?", each.join(", "))
            }
            Operation::Intensify { cards, owner } => {
                let test = match cards {
                    ObjectFilter::Subtype(word) => format!("|q: &dyn Query, asker: &Asker, id: ObjectId| -> bool {{ q.chars(id).has_subtype(Subtype::{word:?}) }}"),
                    other => self.filter_fn(other)?,
                };
                let owner = self.fetch(self.certain_player(owner)?);
                format!("{M}::intensity::intensify_all(g, {owner}, {test})")
            }
            Operation::ChooseAction {
                actor,
                options,
                villainous,
            } => {
                let actors = self.stated_players(actor)?;
                let options = self.stated_operands(options);
                format!("{M}::choose::action(g, item, {actors}, {options}, {villainous})?")
            }
            Operation::ChoosePile {
                actor,
                from,
                random,
                names,
                object,
                then,
            } => {
                let actors = self.stated_players(actor)?;
                let from = match from {
                    Some(players) => format!("Some({})", self.stated_players(players)?),
                    None => "None".to_string(),
                };
                let object = match object {
                    Some(object) => self.fetch(self.object(object)?),
                    None => "None".to_string(),
                };
                let then = match then {
                    Some(destination) => format!("Some({})", self.destination_code(destination)?),
                    None => "None".to_string(),
                };
                format!("{M}::piles::choose_pile(g, {source}, {actors}, {from}, {object}, &{names:?}, {random}, {then})?")
            }
            Operation::ChooseDefender { actor, attacker } => {
                let actors = self.stated_players(actor)?;
                let attacker = self.fetch(self.certain_player(attacker)?);
                format!("{M}::combat::choose_defender(g, {source}, {actors}, {attacker})?")
            }
            Operation::EachNumber { from, to, action } => {
                let from = self.fetch(self.number(from)?.code);
                let to = self.fetch(self.number(to)?.code);
                let action = self.state_operation(feature, action)?;
                format!("for number in {from}..={to} {{ {action}; }}")
            }
            Operation::EachObject { objects, action } => {
                let objects = self.fetch(self.objects(objects)?);
                let saved = self.affected_object.replace("bound");
                let action = self.state_operation(feature, action);
                self.affected_object = saved;
                format!("for bound in {objects} {{ {}; }}", action?)
            }
            Operation::MoveCounters {
                actor,
                from,
                to,
                kind,
                amount,
            } => {
                let actors = self.stated_players(actor)?;
                let from = self.fetch(self.objects(from)?);
                let to = self.fetch(self.objects(to)?);
                let kind = match kind {
                    Some(kind) => format!("Some({})", counter_kind(kind)?),
                    None => "None".to_string(),
                };
                let amount = match amount {
                    MovedCounters::One => "MovedCounters::One".to_string(),
                    MovedCounters::All => "MovedCounters::All".to_string(),
                    MovedCounters::AnyNumber => "MovedCounters::AnyNumber".to_string(),
                    MovedCounters::OneOfEachKindMissing => {
                        "MovedCounters::OneOfEachKindMissing".to_string()
                    }
                    MovedCounters::Exactly(n) => format!(
                        "MovedCounters::Exactly({})",
                        self.fetch(self.number(n)?.code)
                    ),
                    MovedCounters::AtLeast(n) => format!(
                        "MovedCounters::AtLeast({})",
                        self.fetch(self.number(n)?.code)
                    ),
                };
                format!("{M}::counters::move_counters(g, {source}, {actors}, {from}, {to}, {kind}, {amount})?")
            }
            Operation::PutCountersEach {
                amount,
                kind,
                objects,
                up_to,
            } => {
                let objects = self.fetch(self.objects(objects)?);
                let amount = self.fetch(self.number(amount)?.amount(i16::MAX.into(), "i16")?);
                format!(
                    "{M}::counters::put_on_each(g, {source}, {objects}, {}, {amount}, {up_to})?",
                    counter_kind(kind)?
                )
            }
            Operation::RevealUntilGroup {
                library,
                constraints,
            } => {
                let library = self.fetch(self.certain_player(library)?);
                let rule = self.stated_constraints(constraints)?;
                format!("let revealed = {M}::library::reveal_until_group(g, {library}, {source}, {rule})?")
            }
            Operation::RevealUntil {
                library,
                filter,
                count,
                at_most,
            } => {
                let library = self.fetch(self.certain_player(library)?);
                let filter = self.filter_fn(filter)?;
                let count = self.fetch(self.number(count)?.code);
                let most = match at_most {
                    Some(n) => format!("Some({})", self.fetch(self.number(n)?.code)),
                    None => "None".to_string(),
                };
                format!("let revealed = {M}::library::reveal_until_or(g, {library}, {source}, {filter}, {count}, {most})?")
            }
            // What triggered it, spell or ability, whichever the event was.
            Operation::SetX {
                object: ObjectRef::EventStackObject,
                value,
            } => {
                let value = self.fetch(self.number(value)?.code);
                format!("g.set_x(item.triggered_by(), {value})")
            }
            Operation::SetX { object, value } => {
                let object = self.fetch(self.object(object)?);
                let value = self.fetch(self.number(value)?.code);
                format!("if let Some(spell) = {object} {{ g.set_x(spell, {value}); }}")
            }
            Operation::ChooseObjects {
                player,
                objects,
                count,
                constraints,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let objects = self.fetch(self.objects(objects)?);
                let count = self.fetch(self.select_count(count)?);
                let rules = self.stated_constraints(constraints)?;
                format!(
                    "{M}::choose::select_with(g, {player}, {source}, {objects}, {count}, {rules})?"
                )
            }
            Operation::Search {
                player,
                library,
                top,
                choices,
                also,
                either,
                outside,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let library = self.fetch(self.certain_player(library)?);
                let top = match top {
                    Some(n) => format!("Some({})", self.fetch(self.number(n)?.code)),
                    None => "None".to_string(),
                };
                let mut finds = Vec::new();
                for choice in choices {
                    let filter = self.filter_now(&choice.filter)?;
                    let count = self.fetch(self.select_count(&choice.count)?);
                    let rules = self.stated_constraints(&choice.constraints)?;
                    finds.push(format!(
                        "Find {{ filter: {filter}, count: {count}, rules: {rules} }}"
                    ));
                }
                let also: Vec<String> = also.iter().map(|z| format!("EventZone::{z:?}")).collect();
                format!("{M}::library::search_for(g, {source}, {player}, {library}, {top}, &[{}], &[{}], {either}, {outside})?", finds.join(", "), also.join(", "))
            }
            Operation::Shuffle { players, excluding } => {
                let players = self.stated_players(players)?;
                let excluding = match excluding {
                    Some(objects) => self.fetch(self.objects(objects)?),
                    None => "Vec::new()".to_string(),
                };
                format!("for player in {players} {{ {M}::library::shuffle_around(g, player, &{excluding}); }}")
            }
            Operation::Counter {
                objects,
                destination,
            } => {
                let spells = self.fetch(self.objects(objects)?);
                format!("{M}::effects::counter_onto(g, {source}, {spells}, CounterDestination::{destination:?})?")
            }
            Operation::ChooseValue {
                actor,
                kind,
                count,
                options,
            } => {
                let actors = self.stated_players(actor)?;
                let options = self.stated_operands(options);
                format!("{M}::choose::value(g, {source}, {actors}, ChoiceKind::{kind:?}, {count}, {options})?")
            }
            Operation::ChooseAtRandom { kind, options } => {
                let options = self.stated_operands(options);
                format!("{M}::choose::at_random(g, {source}, ChoiceKind::{kind:?}, {options})?")
            }
            _ => return Err(refused()),
        })
    }
    /// What selection constraints say, for code that is stated.
    pub(super) fn stated_constraints(
        &mut self,
        constraints: &[SelectionConstraint],
    ) -> Result<String> {
        let mut rules = Vec::new();
        for constraint in constraints {
            rules.push(match constraint {
                SelectionConstraint::TotalProperty {
                    property,
                    comparison,
                    amount,
                } => format!(
                    "SelectionRule::Total(Property::{}, Comparison::{}, {})",
                    form(property),
                    form(comparison),
                    self.fetch(self.number(amount)?.code)
                ),
                SelectionConstraint::CommonProperty(property)
                | SelectionConstraint::NoCommonProperty(property) => {
                    format!(
                        "SelectionRule::{}(Property::{property:?})",
                        form(constraint)
                    )
                }
                SelectionConstraint::NamedAfterOneEach(objects) => format!(
                    "SelectionRule::NamedAfterOneEach({})",
                    self.fetch(self.objects(objects)?)
                ),
                SelectionConstraint::OneForEach(filters) => {
                    let mut each = Vec::new();
                    for filter in filters {
                        each.push(self.filter_fn(filter)?);
                    }
                    format!("SelectionRule::OneForEach(&[{}])", each.join(", "))
                }
                SelectionConstraint::CanMoveTo { destination, .. } => format!(
                    "SelectionRule::CanMoveTo({})",
                    self.destination_code(destination)?
                ),
                other => format!("SelectionRule::{}", form(other)),
            });
        }
        Ok(format!("&[{}]", rules.join(", ")))
    }
    /// The durations with no word yet.
    pub(super) fn duration_p(&mut self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        let feature = format!("duration {}", form(duration));
        let later = "Duration::Indefinite";
        let code = match duration {
            D::ExtraTurn(result) => {
                let turn = self
                    .performed
                    .get(&result.0)
                    .cloned()
                    .unwrap_or_else(|| format!("result_{}", result.0));
                format!("Duration::ExtraTurn({turn})")
            }
            D::UntilNext { point, players } => format!(
                "Duration::UntilNext(TurnPoint::{}, {})",
                form(point),
                self.fetch(self.players(players)?)
            ),
            D::ThroughNextTurn => "Duration::ThroughNextTurn".to_string(),
            D::WhileControlled { player, object } => format!(
                "Duration::WhileControlledBy({}, {})",
                self.fetch(self.certain_player(player)?),
                self.fetch(self.object(object)?)
            ),
            D::WhileMatches { object, filter } => format!(
                "Duration::WhileMatches({}, {})",
                self.fetch(self.object(object)?),
                self.filter_now(filter)?
            ),
            D::DuringControllersNextUntapSteps(steps) => format!(
                "Duration::ControllersNextUntapSteps({})",
                self.fetch(self.number(steps)?.code)
            ),
            D::While(condition) => format!("Duration::While({})", self.condition_fn(condition)?),
            D::WhileItMatches(filter) => {
                format!("Duration::WhileItMatches({})", self.filter_fn(filter)?)
            }
            other => return lane_m::duration_more(other),
        };
        Ok(planned::planned_value(&feature, later, &code))
    }
    /// A filter of what this resolution knows, for a game that asks it later: the
    /// values it reads go along with it.
    fn filter_now(&mut self, filter: &ObjectFilter) -> Result<String> {
        match self.filter_fn(filter) {
            Ok(code) => Ok(code),
            Err(_) => {
                let code = self.filter(filter, &Candidate::of("id"))?;
                Ok(format!(
                    "move |q: &dyn Query, id: ObjectId| -> bool {{ {code} }}"
                ))
            }
        }
    }
    /// A filter as `players` states one.
    pub(super) fn stated_filter(&self, filter: &ObjectFilter) -> Result<String> {
        let mut c = self.clone();
        c.filter_fn(filter)
    }
    /// A body with no part in what the rest of its ability reads.
    fn apart(&mut self, body: &Program, label: &str) -> Result<String> {
        let scope = self.results();
        let effect = self.effect(body, label);
        self.restore_results(scope);
        effect
    }
    /// One side of a spell that a cost picks between, its targets and modes its own.
    fn spell_branch(&mut self, ability: &Ability, label: &str) -> Result<String> {
        let Ability::Spell { announcement, body } = ability else {
            return Err(gap("cost branch of abilities that are not spells"));
        };
        if announcement.modes.is_some() {
            let scope = self.results();
            let modal = self.modal(announcement, label, None);
            self.restore_results(scope);
            let modal = modal?;
            return Ok(format!("SpellBranch::Modal(&{modal})"));
        }
        let targets = self.announcement(announcement, &format!("{label}_targets"))?;
        self.announced_filters.clear();
        let divide = self.dividing.take();
        let effect = self.effect(body, label)?;
        Ok(match divide {
            Some(divide) => format!("SpellBranch::Divided({targets}, {effect}, {divide})"),
            None => format!("SpellBranch::Plain({targets}, {effect})"),
        })
    }
    /// A keyword of a quality no word of Cantrip's names.
    fn stated_keyword(&mut self, keyword: &Keyword) -> Option<String> {
        match keyword {
            Keyword::Landwalk(land) => match land.as_ref() {
                ObjectFilter::SharesWithAny { property, with } => {
                    let with = self.reading_asked(|c| c.objects(with)).ok()?;
                    Some(format!("Keyword::Landwalk(LandFilter::Sharing(Property::{property:?}, |q: &dyn Query, asker: &Asker| -> Vec<ObjectId> {{ {with} }}))"))
                }
                land => Some(format!(
                    "Keyword::Landwalk(LandFilter::Matching({}))",
                    self.filter_now(land).ok()?
                )),
            },
            Keyword::Protection(quality) => match &**quality {
                Protection::Objects {
                    filter: ObjectFilter::ColorOutsideCommanderIdentity(player),
                    ..
                } => {
                    let player = self
                        .reading_asked(|c| Ok(c.fetch(c.certain_player(player)?)))
                        .ok()?;
                    Some(format!(
                        "Keyword::ProtectionFromColorsOutsideIdentityOf({player})"
                    ))
                }
                Protection::Objects { filter, .. } => Some(format!(
                    "Keyword::ProtectionFrom({})",
                    self.filter_now(filter).ok()?
                )),
                _ => None,
            },
            _ => None,
        }
    }
    /// A static ability with a grant that is only stated: the rest of it keeps its
    /// own arms.
    fn stated_static(
        &mut self,
        ability: &Ability,
        label: &str,
    ) -> Option<Vec<(&'static str, String)>> {
        let Ability::Static {
            kind,
            affected: EffectSubjects::Objects(objects),
            changes,
        } = ability
        else {
            return None;
        };
        let mut grants = Vec::new();
        let mut rest = Vec::new();
        for change in changes {
            let grant = match change {
                Change::GrantKeyword(keyword) => self.stated_keyword(keyword),
                Change::GrantAbility(granted) => match &**granted {
                    Ability::Mechanic {
                        mechanic,
                        cost,
                        amount,
                        filter,
                    } => self
                        .mechanic_entry(
                            *mechanic,
                            cost.as_ref(),
                            amount.as_ref(),
                            filter.as_ref(),
                            label,
                        )
                        .ok()
                        .map(|(_, entry)| entry),
                    _ => None,
                },
                _ => None,
            };
            match grant {
                Some(grant) => grants.push(grant),
                None => rest.push(change.clone()),
            }
        }
        if grants.is_empty() {
            return None;
        }
        let list = self.reading_asked(|c| c.objects(objects)).ok()?;
        let mut entries = match rest.is_empty() {
            true => Vec::new(),
            false => {
                let rest = Ability::Static {
                    kind: kind.clone(),
                    affected: EffectSubjects::Objects(objects.clone()),
                    changes: rest,
                };
                self.entries_of(&rest, label).ok()?
            }
        };
        for grant in grants {
            entries.push((
                "statics",
                format!("{M}::statics::grant_to(|q: &dyn Query, asker: &Asker| -> Vec<ObjectId> {{ {list} }}, {grant})"),
            ));
        }
        Some(entries)
    }
    /// A token of a definition with what only the game can fill in as it is made:
    /// the rest of it is the definition's own.
    fn stated_definition(&mut self, token: &TokenDefinition) -> Result<String> {
        let dynamic = |n: &Expr| !matches!(n, Expr::Constant(_));
        let sized = token
            .power_toughness
            .as_ref()
            .is_some_and(|(p, t)| dynamic(p) || dynamic(t));
        if !token.chosen_color
            && !token.chosen_creature_type
            && token.abilities_of.is_none()
            && !sized
        {
            return Ok(format!(
                "TokenRecipe::Defined(g.card_id(&{}))",
                self.token_definition(token)?
            ));
        }
        let mut plain = token.clone();
        plain.chosen_color = false;
        plain.chosen_creature_type = false;
        plain.abilities_of = None;
        let color = match token.chosen_color {
            true => format!("Some({})", self.fetch(self.chosen_read(ValueKind::Color)?)),
            false => "None".to_string(),
        };
        let kind = match token.chosen_creature_type {
            true => format!(
                "Some({})",
                self.fetch(self.chosen_read(ValueKind::CreatureType)?)
            ),
            false => "None".to_string(),
        };
        let size = match &token.power_toughness {
            Some((p, t)) if sized => {
                let size = format!(
                    "Some(({}, {}))",
                    self.fetch(self.number(p)?.code),
                    self.fetch(self.number(t)?.code)
                );
                plain.power_toughness = None;
                size
            }
            _ => "None".to_string(),
        };
        let abilities = match &token.abilities_of {
            Some((objects, which)) => format!(
                "Some(({}, {}))",
                self.fetch(self.objects(objects)?),
                self.operand_code(which)
            ),
            None => "None".to_string(),
        };
        let card = self.token_definition(&plain)?;
        Ok(format!(
            "TokenRecipe::Filled(g.card_id(&{card}), TokenFill {{ color: {color}, creature_type: {kind}, power_toughness: {size}, abilities_of: {abilities} }})"
        ))
    }
    /// "As long as this is tapped, no more than one creature can attack you each
    /// combat": a rule of the combat, asked of each thing that can be attacked.
    pub(super) fn conditional_cap(
        &mut self,
        condition: &Condition,
        defender: &Recipients,
        amount: &Expr,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let (
            Recipients::Players(PlayerSet::One(PlayerRef::AbilityController)),
            Expr::Constant(most),
        ) = (defender, amount)
        else {
            return Err(gap("conditional AttackCap"));
        };
        let test = self.reading_asked(|c| c.condition(condition))?;
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn {label}_limit(q: &dyn Query, asker: &Asker, attacked: Target) -> Option<u32> {{ (attacked == Target::Player(asker.controller) && {test}).then_some({most}) }}"
        )
        .unwrap();
        parts.table("attack_limits", format!("{label}_limit"));
        Ok(())
    }
    /// The roll a reflexive ability reads, told to it as a count (CR 603.12).
    pub(super) fn told_roll(
        &self,
        performed: &ResultId,
        announcement: &Announcement,
        body: &Program,
    ) -> Option<String> {
        let read = format!("DieResult(ResultId({}))", performed.0);
        let reads =
            format!("{body:?}").contains(&read) || format!("{announcement:?}").contains(&read);
        let local = self.values.get(&performed.0)?;
        reads.then(|| format!("({local} as i64)"))
    }
    /// A planeswalker's printed loyalty, for what is worked out as it enters.
    pub(super) fn lane_p_loyalty(&mut self, loyalty: &Expr) -> Result<String> {
        self.number_fn(loyalty)
    }
}
