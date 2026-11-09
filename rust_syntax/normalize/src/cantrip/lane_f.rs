//! A record's faces joined into one card, the way `CardDef` joins them: the
//! file's top level is the card `CARD` and everything it brings in `EXTRA`, and a
//! face that is not `CARD` is a module beside it in the same file.
//!
//! `back`, `split`, `adventure`, `omen` and `prepare` join faces today. A layout
//! Cantrip has no model of keeps its `layout X` gap, and its file says in a hole
//! what it will have: a flip card, a meld pair, a card of a casual variant.
use super::*;
use crate::mtg_types::OracleCard;
use serde_json::Value;

/// Definitions a mechanic shares: no module of the file holds them.
const M: &str = "cantrip_mechanics::";

/// What a face's place on its card changes in its module.
#[derive(Default)]
pub(super) struct Place {
    /// It isn't the definition an object's `card` names, so code that means the
    /// card printing it has to name it. It lists no `EXTRA`: `CARD` does.
    pub own_face: bool,
    pub door: Option<Door>,
    /// A prepare spell is only ever a copy (CR 722.3c), so no card has its name.
    pub no_card: bool,
    /// Another face reads what was exiled with this one (CR 607.1b).
    pub links_exile: bool,
    /// A face beside it earthbends, which gives the land the delayed ability 0
    /// of an object's `card`: this one, so it lists `earthbend::RETURN` first.
    pub earthbend: bool,
    /// More `CardDef` fields, as struct-literal text.
    pub fields: String,
    /// Holes for fields Cantrip doesn't have yet.
    pub holes: String,
    /// More definitions for `EXTRA`.
    pub extras: Vec<String>,
    /// The face's Oracle text, where it is known.
    pub printed: Option<String>,
    /// Values the object keeps from the ability that made it, as an emblem
    /// keeps the creature type chosen as it was made (CR 607.2d).
    pub keeps: Vec<crate::ir::ValueKind>,
}

/// A door of a Room (CR 709.5).
#[derive(Clone, Copy)]
pub(super) enum Door {
    Left,
    Right,
}
impl Door {
    /// The test of `Event::DoorUnlocked` that this door was the one unlocked.
    pub fn unlocked(self) -> &'static str {
        match self {
            Door::Left => "*door == DOOR_LEFT",
            Door::Right => "*door == DOOR_RIGHT",
        }
    }
}

impl Parts {
    /// What `CardDef::door_unsupported` refuses, and what its comment says is read
    /// off the Room rather than the door: a standing rule. What a door does as a
    /// spell is its own, as is what its resolving abilities make for a duration.
    pub(super) fn door_refused(&self) -> Option<&'static str> {
        const ITS_OWN: [&str; 5] = [
            "play_rules",
            "ways",
            "own_cost_changes",
            "granted_preventions",
            "granted_replacements",
        ];
        let standing = !self.block_rules.is_empty()
            || !self.block_requirements.is_empty()
            || !self.attack_rules.is_empty()
            || !self.untap_bans.is_empty()
            || self.tables.keys().any(|table| !ITS_OWN.contains(table));
        [
            (!self.statics.is_empty(), "static abilities"),
            (!self.replacements.is_empty(), "replacement effects"),
            (!self.preventions.is_empty(), "prevention effects"),
            (!self.enters.is_empty(), "an enters rule"),
            (
                !self.keywords.is_empty() || self.protection != 0,
                "keywords",
            ),
            (
                self.abilities.iter().any(|a| a.contains(".functions_in(")),
                "a triggered ability off the battlefield",
            ),
            (standing, "a standing rule"),
        ]
        .into_iter()
        .find_map(|(has, what)| has.then_some(what))
    }
}

impl Compiler {
    /// "Meld them into": the objects are read now, and the rest waits on Cantrip.
    pub(super) fn plan_meld(
        &mut self,
        objects: &ObjectSet,
        into: &str,
        out: &mut String,
    ) -> Result<()> {
        let id = self.next;
        self.next += 1;
        let objects = self.fetch(self.objects(objects)?);
        writeln!(out, "let melding_{id} = {objects};").unwrap();
        let code = format!("g.meld(&melding_{id}, {into:?});");
        self.plan_code("operation Meld", &code, out);
        Ok(())
    }
}

/// One face of a record.
pub struct WholeFace {
    pub name: String,
    /// Its own module, with no regard for the rest of its card.
    pub rust: String,
    pub gaps: Vec<Diagnostic>,
}

/// A record's code.
pub struct Whole {
    pub faces: Vec<WholeFace>,
    /// The record's file, where it is more than its one face's module.
    pub rust: Option<String>,
}

/// How a record's faces make a card.
enum Shape {
    /// One face, or a record that is no card.
    Alone,
    /// A card, and one face beside it in `module`.
    Beside {
        main: usize,
        other: usize,
        module: &'static str,
    },
    /// Two halves, and a card that is both of them at once.
    Halves { left: usize, right: usize },
    /// Faces nothing in Cantrip joins.
    Unjoined,
}

fn shape(card: &UnifiedCard, args: &Value) -> Shape {
    let at = |fields: &Value| {
        let name = fields["Name"].as_str();
        card.faces
            .iter()
            .position(|f| Some(f.name.as_str()) == name)
    };
    let beside = |main: &Value, module| match (card.faces.len(), at(main)) {
        (2, Some(main)) => Shape::Beside {
            main,
            other: 1 - main,
            module,
        },
        _ => Shape::Unjoined,
    };
    let halves = |left: &Value| match (card.faces.len(), at(left)) {
        (2, Some(left)) => Shape::Halves {
            left,
            right: 1 - left,
        },
        _ => Shape::Unjoined,
    };
    match &card.layout {
        Layout::Transforming | Layout::ModalDFC => beside(&args["FrontFace"], "back"),
        Layout::Flip => beside(&args["Unflipped"], "flipped"),
        Layout::Adventurer => beside(args, "adventure"),
        Layout::Ominous => beside(args, "omen"),
        Layout::Preparer => beside(args, "prepared"),
        Layout::Room => halves(&args["LeftDoor"]),
        Layout::Split => halves(&args["Cards"][0]),
        _ if card.faces.len() < 2 || card.layout == Layout::TokenDefinition => Shape::Alone,
        _ => Shape::Unjoined,
    }
}

/// Characteristics two values list, with none of them twice.
fn both<T: Clone + std::fmt::Debug>(left: &[T], right: &[T]) -> Vec<T> {
    let mut all = left.to_vec();
    for value in right {
        if !all.iter().any(|v| name(v) == name(value)) {
            all.push(value.clone());
        }
    }
    all
}

/// The card a split card or Room is in every zone but the stack (CR 709.4,
/// 709.5): both names, both costs together, and the types of both.
fn both_halves(left: &Card, right: &Card) -> Card {
    let (l, r) = (&left.characteristics, &right.characteristics);
    let cost = l.mana_cost.iter().chain(&r.mana_cost).flatten().cloned();
    Card {
        schema_version: left.schema_version,
        name: format!("{} // {}", left.name, right.name),
        characteristics: Characteristics {
            supertypes: both(&l.supertypes, &r.supertypes),
            card_types: both(&l.card_types, &r.card_types),
            subtypes: both(&l.subtypes, &r.subtypes),
            mana_cost: (l.mana_cost.is_some() || r.mana_cost.is_some()).then(|| cost.collect()),
            color_indicator: both(&l.color_indicator, &r.color_indicator),
            power_toughness: None,
            loyalty: None,
            defense: None,
        },
        abilities: Vec::new(),
    }
}

/// `face` without `mechanic`, and whether it had it: `CardDef` says that with a
/// field, where it joins its card.
fn without(face: &Card, mechanic: Mechanic) -> (Card, bool) {
    let mut face = face.clone();
    let printed = face.abilities.len();
    face.abilities
        .retain(|a| !matches!(a, Ability::Mechanic { mechanic: m, .. } if *m == mechanic));
    let had = face.abilities.len() != printed;
    (face, had)
}

/// What a card of a casual variant says about itself: what it is played with,
/// which is no player's library (CR 901.3, 902.3, 904.3, 905.1).
fn variant(card: &UnifiedCard, args: &Value) -> Option<String> {
    let number = |key: &str| args[key].as_i64().unwrap_or_default();
    Some(match &card.layout {
        Layout::Planar => "variant: Some(Variant::Planechase)".to_string(),
        Layout::Scheme => "variant: Some(Variant::Archenemy)".to_string(),
        Layout::Conspiracy => "variant: Some(Variant::ConspiracyDraft)".to_string(),
        Layout::Vanguard => format!(
            "variant: Some(Variant::Vanguard {{ hand: {}, life: {} }})",
            number("HandModifier"),
            number("LifeModifier")
        ),
        _ => return None,
    })
}

/// A dungeon's rooms as `CardDef::dungeon` lists them: where each one's arrows
/// lead (CR 309.5a), by room.
fn dungeon(face: &Card) -> Vec<String> {
    let rooms: Vec<(&String, &Vec<String>)> = face
        .abilities
        .iter()
        .filter_map(|a| match a {
            Ability::DungeonRoom { name, leads_to, .. } => Some((name, leads_to)),
            _ => None,
        })
        .collect();
    let arrows = rooms.iter().map(|(_, leads_to)| {
        let to = leads_to
            .iter()
            .filter_map(|room| rooms.iter().position(|(name, _)| *name == room))
            .map(|at| at.to_string());
        format!("&[{}]", to.collect::<Vec<_>>().join(", "))
    });
    arrows.collect()
}

/// A record being joined.
struct Joining<'a> {
    card: &'a UnifiedCard,
    faces: Vec<Option<WholeFace>>,
    links_exile: bool,
    /// The modules of the faces that aren't `CARD`.
    modules: String,
    /// `CARD`'s place on the card.
    place: Place,
    gaps: Vec<Diagnostic>,
    /// Each face's Oracle text, by name.
    oracle: Option<&'a BTreeMap<String, String>>,
}
impl Joining<'_> {
    /// A face that is its own module beside `CARD`, compiled from `compiled`.
    fn beside(
        &mut self,
        at: usize,
        compiled: &Card,
        module: &str,
        place: Place,
    ) -> Result<Option<&'static str>> {
        let place = Place {
            own_face: true,
            links_exile: self.links_exile,
            printed: self.printed(at),
            ..place
        };
        let emitted = emit_face(compiled, None, place)?;
        let extras = emitted.extras.iter().map(|p| match p.starts_with(M) {
            true => p.clone(),
            false => format!("{module}::{p}"),
        });
        self.place.extras.extend(extras);
        self.place.earthbend |= emitted.rust.contains("earthbend::effect");
        write!(self.modules, "pub mod {module} {{\n{}}}\n", emitted.rust).unwrap();
        self.record(at, emitted.rust, emitted.gaps);
        Ok(emitted.door_refused)
    }
    /// The Oracle text of the face `at`.
    fn printed(&self, at: usize) -> Option<String> {
        let names: Vec<&str> = self.card.faces.iter().map(|face| face.name.as_str()).collect();
        crate::cantrip::printed_face(self.oracle?, &names, names[at]).cloned()
    }
    fn record(&mut self, at: usize, rust: String, gaps: Vec<Diagnostic>) {
        self.faces[at] = Some(WholeFace {
            name: self.card.faces[at].name.clone(),
            rust,
            gaps,
        });
    }
    /// A field `CardDef` will have once `feature` exists in Cantrip.
    fn plan_field(&mut self, feature: &str, field: &str) {
        if planned::landed(feature) {
            write!(self.place.fields, "{field}, ").unwrap();
        } else {
            let id = planned::requirement(feature);
            writeln!(self.place.holes, "planned_item!({id:?} => {field});").unwrap();
            self.gaps.push(gap(feature).at("layout"));
        }
    }
    /// The entries of the `CardDef` list `list`: a hole for each until `feature`
    /// lands, as `planned_item!` holds one entry of a list.
    fn plan_list(&mut self, feature: &str, list: &str, entries: &[String]) {
        if planned::landed(feature) {
            write!(self.place.fields, "{list}: &[{}], ", entries.join(", ")).unwrap();
        } else {
            let id = planned::requirement(feature);
            for entry in entries {
                writeln!(self.place.holes, "planned_item!({id:?} => {list}: {entry});").unwrap();
            }
            self.gaps.push(gap(feature).at("layout"));
        }
    }
    fn fields(&mut self, fields: &str) {
        self.place.fields.push_str(fields);
    }
    /// `CARD` the face `at`, once all that it joins is known.
    fn main(&mut self, at: usize) -> Result<String> {
        let mut place = std::mem::take(&mut self.place);
        place.printed = self.printed(at);
        let emitted = emit_face(&self.card.faces[at], None, place)?;
        let file = format!("{}{}", emitted.rust, self.modules);
        self.record(at, emitted.rust, emitted.gaps);
        Ok(file)
    }
    /// The file of a card that is its faces at once, none of them `CARD`.
    fn halves(&mut self, left: usize, right: usize) -> Result<String> {
        let room = self.card.layout == Layout::Room;
        let (left_half, left_fuse) = without(&self.card.faces[left], Mechanic::Fuse);
        let (right_half, right_fuse) = without(&self.card.faces[right], Mechanic::Fuse);
        let whole = both_halves(&left_half, &right_half);
        for (at, half, module, door) in [
            (left, &left_half, "left", Door::Left),
            (right, &right_half, "right", Door::Right),
        ] {
            let (half, aftermath) = without(half, Mechanic::Aftermath);
            let place = Place {
                door: room.then_some(door),
                fields: aftermath
                    .then(|| "aftermath: true, ")
                    .unwrap_or_default()
                    .into(),
                ..Place::default()
            };
            if let Some(what) = self.beside(at, &half, module, place)?.filter(|_| room) {
                // The door is written whole, but `CardDef::door_unsupported` refuses it: the file says where.
                let feature = format!("Room door with {what}");
                let id = planned::requirement(&feature);
                writeln!(
                    self.place.holes,
                    "planned_item!({id:?}, \"{module}::CARD: CardDef::door_unsupported refuses a door with {what}\");"
                )
                .unwrap();
                self.gaps.push(gap(feature).at("layout"));
            }
            self.place.extras.push(format!("{module}::CARD"));
        }
        self.fields("split: Some((&left::CARD, &right::CARD)), ");
        if left_fuse || right_fuse {
            self.fields("fuse: true, ");
        }
        let place = std::mem::take(&mut self.place);
        let emitted = emit_face(&whole, None, place)?;
        self.gaps.extend(emitted.gaps);
        Ok(format!("{}{}", emitted.rust, self.modules))
    }
    /// The file of a card with one face beside it.
    fn with_face(&mut self, main: usize, other: usize, module: &'static str) -> Result<String> {
        let face = &self.card.faces[other];
        let no_card = self.card.layout == Layout::Preparer;
        let place = Place {
            no_card,
            ..Place::default()
        };
        self.beside(other, face, module, place)?;
        match &self.card.layout {
            Layout::Flip => self.plan_field("layout Flip", "flip: Some(&flipped::CARD)"),
            Layout::Transforming => self.fields("back: Some(&back::CARD), "),
            Layout::ModalDFC => {
                self.fields("back: Some(&back::CARD), ");
                let types = &face.characteristics.card_types;
                if types.iter().any(|t| name(t) == "Land") {
                    self.fields("back_land: true, ");
                }
            }
            // An inset face or a prepare spell: `EXTRA` puts it in the card table.
            _ => {
                let field = if no_card { "prepare" } else { module };
                write!(self.place.fields, "{field}: Some(&{module}::CARD), ").unwrap();
                self.place.extras.push(format!("{module}::CARD"));
            }
        }
        self.main(main)
    }
}

/// Whether a record is a card to play at all, not a token or a sticker sheet.
pub fn is_card(card: &UnifiedCard) -> bool {
    !card.faces.is_empty() && card.layout != Layout::TokenDefinition
}

/// A sticker sheet (CR 123): no card, but the sheet a game of Unfinity has beside it. Cantrip leaves stickers out of the
/// game it plays (its docs/rules-exceptions.md), so each sticker is a hole that keeps what it says, and the sheet is named
/// for its name stickers, the only name it has.
fn sticker_sheet(card: &UnifiedCard) -> Whole {
    let stickers: &[Sticker] = card
        .data
        .iter()
        .find_map(|field| match &field.value {
            Operand::Stickers(stickers) if field.name == "Stickers" => Some(stickers.as_slice()),
            _ => None,
        })
        .unwrap_or_default();
    let names: Vec<&str> = stickers
        .iter()
        .filter_map(|sticker| match sticker {
            Sticker::Name(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let feature = "engine: no stickers";
    let id = planned::requirement(feature);
    let mut rust = String::from("#[allow(unused_imports)] use cantrip_api::*;\n");
    for sticker in stickers {
        let source = ron::to_string(sticker).unwrap_or_default();
        writeln!(rust, "planned_item!({id:?}, {source:?});").unwrap();
    }
    Whole {
        faces: vec![WholeFace {
            name: format!("Sticker sheet: {}", names.join(", ")),
            rust,
            gaps: vec![gap(feature).at("layout")],
        }],
        rust: None,
    }
}

/// A record whole, its faces joined where Cantrip joins them. Everything the
/// card waits on is a gap of one of its faces: `tracked` and `survey` read this.
pub fn whole(source: &OracleCard, card: &UnifiedCard) -> Result<Whole> {
    whole_printed(source, card, None)
}

/// `whole`, with each ability's text from `oracle`, the printed text by face name.
pub fn whole_printed(
    source: &OracleCard,
    card: &UnifiedCard,
    oracle: Option<&BTreeMap<String, String>>,
) -> Result<Whole> {
    if let Layout::Extension { .. } = &card.layout {
        return Err(gap(format!("unsupported layout extension {}", card.layout)));
    }
    if card.layout == Layout::StickerSheet {
        return Ok(sticker_sheet(card));
    }
    let inspected = match &card.layout {
        Layout::Card => Value::Null,
        _ => crate::inspect::value(source).expect("source AST serialization"),
    };
    let args = &inspected["args"];
    let mut joining = Joining {
        card,
        faces: card.faces.iter().map(|_| None).collect(),
        links_exile: card.faces.len() > 1
            && card
                .faces
                .iter()
                .any(|f| format!("{:?}", f.abilities).contains("ExiledWith(Source)")),
        modules: String::new(),
        place: Place::default(),
        gaps: Vec::new(),
        oracle,
    };
    joining.place.links_exile = joining.links_exile;
    let rust = match shape(card, args) {
        Shape::Beside {
            main,
            other,
            module,
        } => Some(joining.with_face(main, other, module)?),
        Shape::Halves { left, right } => Some(joining.halves(left, right)?),
        Shape::Alone => {
            let layout = format!("layout {}", card.layout);
            if let Some(field) = variant(card, args) {
                joining.plan_field(&layout, &field);
            }
            match (&card.layout, card.faces.first()) {
                // A half is an ordinary card: the meld is the partner's instruction
                // to do, or the hole of the one that prints it.
                (Layout::MeldPiece, _) => {}
                (Layout::Melded, _) => joining.plan_field(&layout, "melded: true"),
                (Layout::Dungeon, Some(face)) => joining.plan_list(&layout, "dungeon", &dungeon(face)),
                _ => {}
            }
            for at in 0..card.faces.len() {
                joining.main(at)?;
            }
            None
        }
        Shape::Unjoined => {
            joining
                .gaps
                .push(gap(format!("layout {}", card.layout)).at("layout"));
            for at in 0..card.faces.len() {
                joining.main(at)?;
            }
            None
        }
    };
    let mut faces: Vec<WholeFace> = joining.faces.into_iter().flatten().collect();
    if let Some(first) = faces.first_mut() {
        first.gaps.append(&mut joining.gaps);
    }
    Ok(Whole { faces, rust })
}

/// Everything a record lacks, faces and card.
pub fn whole_gaps(source: &OracleCard, card: &UnifiedCard) -> Vec<Diagnostic> {
    match whole(source, card) {
        Ok(whole) => whole.faces.into_iter().flat_map(|f| f.gaps).collect(),
        Err(gap) => vec![gap],
    }
}

/// A record's file, as `module` is a face's: never partially executable code.
pub fn whole_module(source: &OracleCard, card: &UnifiedCard) -> Result<String> {
    let whole = whole(source, card)?;
    if let Some(gap) = whole.faces.iter().flat_map(|f| &f.gaps).next() {
        return Err(gap.clone());
    }
    let faces = whole.faces.iter().map(|f| f.rust.as_str());
    Ok(whole.rust.unwrap_or_else(|| faces.collect()))
}
