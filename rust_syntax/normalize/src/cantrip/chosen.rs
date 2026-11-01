//! Values chosen as a spell or ability resolves, kept in a local, or as a
//! permanent enters, kept on it for the abilities linked to that choice
//! (CR 607.2d), and what reads them.
use super::*;

const M: &str = "cantrip_mechanics";

/// The words of `operands`, as `words` is to a copy's exceptions.
fn vocabulary<'a>(operands: &'a [Operand], family: &str) -> Result<Vec<&'a str>> {
    operands
        .iter()
        .map(|operand| match operand {
            Operand::Vocabulary {
                family: f, form, ..
            } if f == family => Ok(form.as_str()),
            other => Err(gap(format!("choice among {}", super::form(other)))),
        })
        .collect()
}

/// What `options` leave of the five colors.
fn color_choices(options: &[Operand]) -> Result<u8> {
    let [Operand::Term { form, operands, .. }] = options else {
        return Err(gap("color choice among stated options"));
    };
    match form.as_str() {
        "AnyColor" => Ok(0b11111),
        "Other" => vocabulary(operands, "Color")?
            .iter()
            .try_fold(0b11111u8, |mask, word| {
                let index = ["White", "Blue", "Black", "Red", "Green"]
                    .iter()
                    .position(|c| c == word)
                    .ok_or_else(|| gap(format!("color choice other than {word}")))?;
                Ok(mask & !(1 << index))
            }),
        other => Err(gap(format!("color choice {other}"))),
    }
}

/// `NameKind` of a name chosen among `options`.
fn name_kind(options: &[Operand]) -> Result<String> {
    let [Operand::Term { form, operands, .. }] = options else {
        return Err(gap("name choice among stated options"));
    };
    let types = |operands: &[Operand]| -> Result<String> {
        let types = vocabulary(operands, "CardType")?
            .iter()
            .map(|t| format!("CardType::{t}"))
            .collect::<Vec<_>>();
        Ok(format!("TypeSet::of(&[{}])", types.join(", ")))
    };
    Ok(match form.as_str() {
        "AnyOracleCard" => "NameKind::ANY".into(),
        "IsCardtype" => format!(
            "NameKind {{ types: {}, ..NameKind::ANY }}",
            types(operands)?
        ),
        "IsNonCardtype" => format!(
            "NameKind {{ non_types: {}, ..NameKind::ANY }}",
            types(operands)?
        ),
        other => return Err(gap(format!("name choice {other}"))),
    })
}

/// Whether a mana ability's body adds mana of the chosen color.
pub(super) fn makes_chosen_color(body: &Program) -> bool {
    matches!(
        body.instructions.first(),
        Some(Instruction::Effect {
            operation: Operation::AddMana {
                mana: ManaProduction::Amount {
                    color: ManaColor::Chosen,
                    ..
                },
                ..
            },
            ..
        })
    )
}

/// The values `card` chooses as it enters.
pub(super) fn enters_choosing(card: &Card) -> Vec<ValueKind> {
    let mut kinds = Vec::new();
    for ability in &card.abilities {
        let Ability::Replacement {
            active: ReplacementActive::SourceEntering,
            body,
            ..
        } = ability
        else {
            continue;
        };
        for instruction in &body.instructions {
            if let Instruction::Effect {
                operation: Operation::ChooseValue { kind, .. },
                ..
            } = instruction
            {
                kinds.push(*kind);
            }
        }
    }
    kinds
}

impl Compiler {
    /// `Operation::ChooseValue`.
    pub(super) fn choose_value(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        kind: ValueKind,
        count: u32,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        if count != 1 {
            return Err(gap("a choice of several values"));
        }
        if self.chosen.iter().any(|(k, _)| *k == kind) {
            return Err(gap("two choices of one kind of value"));
        }
        let PlayerSet::One(player) = actor else {
            return Err(gap("a value chosen by several players"));
        };
        let entering = self.pending == Pending::Entering;
        let player = if entering {
            "entering.controller".to_string()
        } else {
            self.fetch(self.certain_player(player)?)
        };
        let source = if entering {
            "entering.object"
        } else {
            "asker.source"
        };
        let local = format!("chosen_{id}");
        match kind {
            ValueKind::Color => {
                let choices = color_choices(options)?;
                writeln!(out, "let player_{id} = {player}; let Answer::Colors(colors_{id}) = g.ask(player_{id}, RequestKind::ChooseColors {{ source: {source}, choices: ColorSet({choices}), count: 1, purpose: ColorUse::Any }})? else {{ return Ok(()) }}; let {local} = colors_{id}[0];").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_color = Some({local});").unwrap();
                }
            }
            ValueKind::CreatureType if entering => writeln!(
                out,
                "let Some({local}) = {M}::chosen_type::choose_creature_type(g, entering)? else {{ return Ok(()) }};"
            )
            .unwrap(),
            ValueKind::CreatureType => writeln!(
                out,
                "let player_{id} = {player}; let {local} = {M}::chosen_type::creature_type_now(g, player_{id}, {source})?;"
            )
            .unwrap(),
            ValueKind::BasicLandType => {
                let basics = "vec![Subtype::Plains, Subtype::Island, Subtype::Swamp, Subtype::Mountain, Subtype::Forest]";
                if entering {
                    writeln!(out, "let Some({local}) = {M}::chosen_type::choose_type_among(g, entering, {basics})? else {{ return Ok(()) }};").unwrap();
                } else {
                    writeln!(out, "let player_{id} = {player}; let Answer::Subtype({local}) = g.ask(player_{id}, RequestKind::ChooseSubtype {{ source: {source}, choices: {basics}, purpose: WordUse::Chosen }})? else {{ return Ok(()) }};").unwrap();
                }
            }
            ValueKind::CardName => {
                let kind = name_kind(options)?;
                writeln!(out, "let player_{id} = {player}; let Answer::Name(Some({local})) = g.ask(player_{id}, RequestKind::ChooseName {{ source: {source}, looked_at: Vec::new(), suggestion: None, kind: {kind} }})? else {{ return Ok(()) }};").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_card = Some({local});").unwrap();
                }
            }
            ValueKind::Player => {
                let [Operand::Players(candidates)] = options else {
                    return Err(gap("a choice of Player"));
                };
                let candidates = self.fetch(self.players(candidates)?);
                writeln!(out, "let player_{id} = {player}; let candidates_{id} = {candidates}; let Some({local}) = {M}::choose::player(g, player_{id}, {source}, candidates_{id}, \"Choose\", 0)? else {{ return Ok(()) }};").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_player = Some({local});").unwrap();
                }
            }
            ValueKind::Ability => self.choose_ability(id, &player, options, out)?,
            other => self.choose_value_more(other, options, out)?,
        }
        self.chosen.push((kind, local));
        Ok(())
    }
    /// The local that holds this choice, in the body that made it.
    pub(super) fn chosen_local(&self, kind: ValueKind) -> Result<String> {
        self.chosen
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, local)| local.clone())
            .ok_or_else(|| gap(format!("a choice of {kind:?} outside its body")))
    }
    /// `Option` of the choice: this body's, or what was chosen as the source
    /// entered, to the ability reading it.
    pub(super) fn chosen_value(&self, kind: ValueKind) -> Result<String> {
        if let Ok(local) = self.chosen_local(kind) {
            return Ok(format!("Some({local})"));
        }
        // The engine keeps a permanent's choices as it entered, and no other.
        if !self.enters_choosing.contains(&kind) {
            return Err(gap(format!("a choice of {kind:?} outside its body")));
        }
        let read = match kind {
            ValueKind::Color => "chosen_color",
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                "chosen_subtype"
            }
            ValueKind::CardName => "chosen_card",
            ValueKind::Player => "chosen_player",
            other => return Err(gap(format!("a choice of {other:?} kept on a permanent"))),
        };
        let linked = match self.reader {
            Reader::Layer => "q.linked_to(asker.source)",
            Reader::Resolution if self.stack_item => "q.linked(item)",
            _ => "q.linked_to(asker)",
        };
        Ok(format!("q.state().{read}({linked})"))
    }
    /// `ObjectFilter::Chosen`, of an object known by `chars` and `id`.
    pub(super) fn chosen_filter(&self, kind: ValueKind, chars: &str, id: &str) -> Result<String> {
        let value = self.chosen_value(kind)?;
        let test = match kind {
            ValueKind::Color => format!("{chars}.colors.has(chosen)"),
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                format!("{chars}.has_subtype(chosen)")
            }
            ValueKind::CardName if self.reader != Reader::Layer => {
                format!("{M}::library::object_has_name(q, {id}, chosen)")
            }
            other => return Err(gap(format!("object filter Chosen {other:?}"))),
        };
        Ok(format!("{value}.is_some_and(|chosen| {test})"))
    }
    /// The `EffectKind`s of a change by a value this body chose.
    pub(super) fn chosen_effect(&self, change: &Change) -> Result<Vec<String>> {
        Ok(match change {
            Change::ChosenColor {
                mode: ChangeMode::Set,
            } => {
                let chosen = self.chosen_local(ValueKind::Color)?;
                vec![format!("EffectKind::SetColors(ColorSet::of({chosen}))")]
            }
            Change::ChosenColor { .. } => vec![lane_g::planned_kind(
                "engine: no effect adds colors",
                format!(
                    "EffectKind::AddColors(ColorSet::of({}))",
                    self.chosen_local(ValueKind::Color)?
                ),
            )],
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Add,
            } => vec![format!(
                "EffectKind::AddSubtype({})",
                self.chosen_local(*kind)?
            )],
            Change::ChosenType {
                kind: kind @ (ValueKind::BasicLandType | ValueKind::LandType),
                mode: ChangeMode::Set,
            } => vec![lane_g::planned_kind(
                "engine: no resolving effect sets land types",
                format!("EffectKind::SetLandType({})", self.chosen_local(*kind)?),
            )],
            Change::ChosenType {
                kind: kind @ (ValueKind::BasicLandType | ValueKind::LandType),
                mode: ChangeMode::Add,
            } => vec![format!(
                "EffectKind::AddSubtype({})",
                self.chosen_local(*kind)?
            )],
            other => return Err(gap(format!("continuous change {}", super::form(other)))),
        })
    }
    /// The body of a layer's change by a value chosen as the source entered.
    pub(super) fn chosen_layer(&self, change: &Change) -> Result<statics::LayerChange> {
        Ok(match change {
            Change::ChosenColor {
                mode: ChangeMode::Set,
            } => statics::LayerChange::of(
                "Color",
                format!(
                    "if let Some(chosen) = {} {{ chars.colors = ColorSet::of(chosen); }}",
                    self.chosen_value(ValueKind::Color)?
                ),
            ),
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Add,
            } => statics::LayerChange::of(
                "Type",
                format!(
                    "if let Some(chosen) = {} {{ chars.subtypes = chars.subtypes.with(chosen); }}",
                    self.chosen_value(*kind)?
                ),
            ),
            other => return Err(gap(format!("static change {}", super::form(other)))),
        })
    }
    /// A protection from what this body chose, or `None` for any other.
    pub(super) fn chosen_protection(
        &self,
        quality: &Protection,
    ) -> Result<Option<(String, String)>> {
        let Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Chosen(kind),
        } = quality
        else {
            return Ok(None);
        };
        let chosen = self.chosen_local(*kind)?;
        match kind {
            ValueKind::Color => Ok(Some((
                format!("EffectKind::Protection(ColorSet::of({chosen}))"),
                format!("chars.protection = chars.protection.with({chosen});"),
            ))),
            other => Err(gap(format!("protection from a chosen {other:?}"))),
        }
    }
}
