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
fn colors_code(colors: &[ManaColor]) -> Result<String> {
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

impl Compiler {
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
        let chosen = matches!(objects, ObjectSet::Selected(result) if self.notes.outside.contains(&result.0));
        chosen || format!("{objects:?}").contains("OutsideTheGame")
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
                    _ => planned::planned_value(
                        "a choice of PlaneswalkerType",
                        "Vec::<Subtype>::new()",
                        "Subtype::ALL.into_iter().filter(|kind| kind.is_planeswalker_type()).collect::<Vec<Subtype>>()",
                    ),
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
            ValueKind::CardType | ValueKind::PermanentType => {
                let words = listed("CardType")?;
                let choices = match (words.is_empty(), kind) {
                    (false, _) => format!("vec![{}]", words.join(", ")),
                    (true, ValueKind::CardType) => "CardType::ALL.to_vec()".to_string(),
                    (true, _) => "CardType::PERMANENT.to_vec()".to_string(),
                };
                let code = format!("{M}::choose::card_type(g, {player}, {source}, {choices})?");
                let value = planned::planned_value(&feature, "CardType::Creature", &code);
                writeln!(out, "let {local}: CardType = {value};").unwrap();
                if entering {
                    self.plan_code(
                        &feature,
                        &format!("entering.chosen_card_type = Some({local});"),
                        out,
                    );
                }
            }
            // CR 105.2c: colorless is no color, so no request for colors offers it.
            ValueKind::ColorOrColorless => {
                if !self.stack_item
                    || (self.entering_labels.is_some() && self.choice_labels.is_empty())
                {
                    return Err(gap(feature.as_str()));
                }
                let base = self.choice_labels.len();
                let words = ["colorless", "white", "blue", "black", "red", "green"];
                self.choice_labels.extend(words.map(String::from));
                let end = self.choice_labels.len();
                let labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                writeln!(out, "let player_{id} = {player}; let {local}_at = {M}::choose::option(g, player_{id}, {source}, item.ability.card, {base}..{end})? - {base}; let {local} = [None, Some(Color::White), Some(Color::Blue), Some(Color::Black), Some(Color::Red), Some(Color::Green)][usize::from({local}_at)];").unwrap();
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
            ObjectRef::SoleTargetOf(of) => held_object(
                &feature,
                &format!("{}.and_then(|o: ObjectId| q.sole_target(o))", self.object(of)?),
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
            ObjectSet::AnyZone(filter) => keep(
                filter,
                "{ let mut all: Vec<ObjectId> = q.state().battlefield().to_vec(); all.extend_from_slice(q.state().zone(Zone::STACK)); all.extend_from_slice(q.state().zone(Zone::COMMAND)); for p in q.state().seats() { for zone in [Zone::library(p), Zone::hand(p), Zone::graveyard(p), Zone::exile(p)] { all.extend_from_slice(q.state().zone(zone)); } } all }".into(),
            )?,
            ObjectSet::Damage(subject) => held_objects(
                &feature,
                &format!("q.damage_objects(item, DamageSubject::{})", form(subject)),
            ),
            ObjectSet::EventGroup => held_objects(&feature, "q.event_objects(item).to_vec()"),
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
            other => return self.objects_stated(other),
        })
    }
    /// The rest of `number`.
    pub(super) fn number_p(&self, n: &Expr) -> Result<Number> {
        use lane_j::{counted, held_number};
        let feature = format!("numeric expression {}", form(n));
        Ok(match n {
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
            Expr::EventActivationX => held_number(feature, "q.event_activation_x(item)"),
            Expr::AbilitiesAmong { abilities, objects } => {
                let abilities = self.clone().operand_code(abilities);
                held_number(
                    feature,
                    &format!("q.abilities_among({abilities}, &{})", self.objects(objects)?),
                )
            }
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
            Expr::ChosenColorsOf(object) => held_number(
                feature,
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| q.state().chosen_colors(o).iter().count() as i64)",
                    self.object(object)?
                ),
            ),
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
                    return Err(gap(feature.as_str()));
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
            ObjectFilter::MostCommonCreatureTypeIn(player) => held(
                &feature,
                &format!(
                    "{}.is_some_and(|p: PlayerId| q.most_common_creature_type(Zone::library(p)).iter().any(|&kind| {chars}.has_subtype(kind)))",
                    self.player(player)?
                ),
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
        let name = self.modal(announcement, label, None)?;
        Ok(Some(name))
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
        let (least, most) = (self.number(least)?.code, self.number(most)?.code);
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
                TokenRecipe::Defined(definition) => {
                    format!(
                        "TokenRecipe::Defined(g.card_id(&{}))",
                        self.token_definition(definition)?
                    )
                }
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
        if failed.feature != "operation ChangeZone" {
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
        Property::ExiledCount => held(format!("q.exiled_with(q.linked_to(&Asker::of({id}))).len() as i64")),
        other => return Err(gap(format!("numeric object property {}", form(other)))),
    })
}
