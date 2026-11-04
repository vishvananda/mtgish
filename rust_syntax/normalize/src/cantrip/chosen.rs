//! Values chosen as a spell or ability resolves, kept in a local, or as a
//! permanent enters, kept on it for the abilities linked to that choice
//! (CR 607.2d), and what reads them.
use super::*;

const M: &str = "cantrip_mechanics";
/// How a value kept past its block is read where something asks for the value.
pub(super) const KEPT: &str = ".expect(\"it chose one\")";

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
        "ColorList" => vocabulary(operands, "Color")?
            .iter()
            .try_fold(0u8, |mask, word| {
                let index = lane_p::COLORS
                    .iter()
                    .position(|c| c == word)
                    .ok_or_else(|| gap(format!("color choice of {word}")))?;
                Ok(mask | (1 << index))
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
        other => return lane_p::name_kind_p(other, operands),
    })
}

/// Whether a mana ability's body adds mana of the chosen color.
pub(super) fn makes_chosen_color(body: &Program) -> bool {
    let Some(Instruction::Effect {
        operation: Operation::AddMana { mana, .. },
        ..
    }) = body.instructions.first()
    else {
        return false;
    };
    let one = match mana {
        ManaProduction::Repeat { production, .. } => production.as_ref(),
        other => other,
    };
    matches!(
        one,
        ManaProduction::Amount {
            color: ManaColor::Chosen,
            ..
        }
    )
}

/// The kind `card` chooses two of as it enters, if the engine can keep them.
pub(super) fn enters_two(card: &Card) -> Option<ValueKind> {
    card.abilities.iter().find_map(|ability| {
        let Ability::Replacement {
            active: ReplacementActive::SourceEntering,
            body,
            ..
        } = ability
        else {
            return None;
        };
        body.instructions
            .iter()
            .find_map(|instruction| match instruction {
                Instruction::Effect {
                    operation:
                        Operation::ChooseValue {
                            kind: kind @ (ValueKind::Color | ValueKind::Player),
                            terms,
                            ..
                        },
                    ..
                } if terms.ordinary_count() == Some(2) => Some(*kind),
                _ => None,
            })
    })
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
                operation: Operation::ChooseValue { kind, terms, .. },
                ..
            } = instruction
            {
                if terms.ordinary_count().is_some()
                    || (*kind == ValueKind::CardType
                        && lane_p::entering_card_types_but(terms).is_some())
                {
                    kinds.push(*kind);
                    if *kind == ValueKind::ColorAndCreatureType {
                        kinds.extend([ValueKind::Color, ValueKind::CreatureType]);
                    }
                }
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
        if self.chosen_words(id, actor, kind, count, out)? {
            return Ok(());
        }
        // "The color or colors of your choice": every color asked for, a repeat
        // being no color more, is any of them or all (106.1b's question).
        if let (ValueKind::Color, 0, PlayerSet::One(player)) = (kind, count, actor) {
            if !self.chosen.iter().any(|(k, _)| *k == kind) {
                let player = self.fetch(self.certain_player(player)?);
                let local = format!("colors_{id}");
                writeln!(out, "let player_{id} = {player}; let Answer::Colors(picked_{id}) = g.ask(player_{id}, RequestKind::ChooseColors {{ source: asker.source, choices: ColorSet(0b11111), count: 5, purpose: ColorUse::Any }})? else {{ unreachable!(\"the core answers a color choice with colors\") }}; let {local} = picked_{id}.into_iter().fold(ColorSet::NONE, |set, color| set.with(color));").unwrap();
                self.chosen.push((kind, local));
                return Ok(());
            }
        }
        if count == 2 && self.pending == Pending::Entering && self.enters_two == Some(kind) {
            return self.choose_two_entering(id, kind, options, out);
        }
        if count != 1 {
            let players = self.fetch(self.players(actor)?);
            let code = format!(
                "{M}::choose::values(g, {players}, asker.source, ChoiceKind::{kind:?}, {count})?"
            );
            self.value_to_come("a choice of several values", id, kind, &code, out);
            return Ok(());
        }
        // "For each player, choose friend or foe": one player's choice of many
        // words, whatever the data says of who makes it.
        if kind == ValueKind::Word
            && matches!(actor, PlayerSet::All)
            && self.stack_item
            && self
                .line_text()
                .is_some_and(|t| t.contains("For each player, choose"))
        {
            return self.word_for_each_player(id, actor, options, out);
        }
        let PlayerSet::One(player) = actor else {
            let players = self.fetch(self.players(actor)?);
            let code = format!(
                "{M}::choose::each_chooses(g, {players}, asker.source, ChoiceKind::{kind:?})?"
            );
            self.value_to_come("a value chosen by several players", id, kind, &code, out);
            return Ok(());
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
                self.notes.named_players.push(format!("Some({local})"));
                writeln!(out, "let player_{id} = {player}; let candidates_{id} = {candidates}; let Some({local}) = {M}::choose::player(g, player_{id}, {source}, candidates_{id}, \"Choose\", 0)? else {{ return Ok(()) }};").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_player = Some({local});").unwrap();
                }
            }
            ValueKind::Ability => self.choose_ability(id, &player, options, out)?,
            ValueKind::Parity => {
                self.choose_value_more(ValueKind::Parity, id, &player, options, out)?
            }
            ValueKind::Recipient => {
                self.choose_recipient(id, &player, source, options, &local, out)?
            }
            other => self.choose_value_rest(other, &player, source, options, &local, id, out)?,
        }
        // Each read is of the latest choice: "choose an opponent" a second time.
        self.remember_choice(kind, &local, out);
        self.chosen.insert(0, (kind, local));
        Ok(())
    }
    /// "As this enters, choose two colors" or "two players": two different values,
    /// the first where one is kept and the second in its chosen number.
    fn choose_two_entering(
        &mut self,
        id: usize,
        kind: ValueKind,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let ask = "entering.controller";
        let source = "entering.object";
        match kind {
            ValueKind::Color => {
                let choices = match options.is_empty() {
                    true => 0b11111,
                    false => color_choices(options)?,
                };
                writeln!(out, "let Answer::Colors(first_{id}) = g.ask({ask}, RequestKind::ChooseColors {{ source: {source}, choices: ColorSet({choices}), count: 1, purpose: ColorUse::Any }})? else {{ return Ok(()) }}; let left_{id} = {choices} & !(1 << (first_{id}[0] as u8)); let Answer::Colors(second_{id}) = g.ask({ask}, RequestKind::ChooseColors {{ source: {source}, choices: ColorSet(left_{id}), count: 1, purpose: ColorUse::Any }})? else {{ return Ok(()) }}; entering.chosen_color = Some(first_{id}[0]); entering.chosen_number = Some(second_{id}[0] as u32);").unwrap();
            }
            ValueKind::Player => {
                let [Operand::Players(candidates)] = options else {
                    return Err(gap("a choice of Player"));
                };
                let candidates = self.fetch(self.players(candidates)?);
                writeln!(out, "let candidates_{id}: Vec<PlayerId> = {candidates}; let Some(first_{id}) = {M}::choose::player(g, {ask}, {source}, candidates_{id}.clone(), \"Choose\", 0)? else {{ return Ok(()) }}; let left_{id}: Vec<PlayerId> = candidates_{id}.into_iter().filter(|&p| p != first_{id}).collect(); let Some(second_{id}) = {M}::choose::player(g, {ask}, {source}, left_{id}, \"Choose\", 0)? else {{ return Ok(()) }}; entering.chosen_player = Some(first_{id}); entering.chosen_number = Some(u32::from(second_{id}.0));").unwrap();
            }
            other => return Err(gap(format!("a choice of two {other:?}"))),
        }
        Ok(())
    }
    /// The two values this source chose as it entered.
    pub(super) fn chosen_two(&self) -> String {
        let linked = self.linked();
        format!("[q.state().chosen_color({linked}), q.state().chosen_number({linked}).map(|n| Color::ALL[n as usize])]")
    }
    /// The `index`th choice this body made, in the order it made them (each local
    /// is numbered as it is written), and its `Option` read.
    pub(super) fn nth_chosen_local(&self, index: u32) -> Option<(ValueKind, String)> {
        let number = |local: &str| -> Option<usize> {
            let name = local.strip_suffix(KEPT).unwrap_or(local);
            name.rsplit('_').next()?.parse().ok()
        };
        let mut made: Vec<(usize, ValueKind, &String)> = self
            .chosen
            .iter()
            .filter_map(|(kind, local)| Some((number(local)?, *kind, local)))
            .collect();
        made.sort_by_key(|(n, ..)| *n);
        made.dedup_by_key(|(n, ..)| *n);
        let (_, kind, local) = made.get(index as usize)?;
        // Only a lone value (a list is no one choice).
        if local.starts_with("colors_") || local.contains(['(', ' ', '[']) && !local.ends_with(KEPT)
        {
            return None;
        }
        let read = match local.strip_suffix(KEPT) {
            Some(kept) => kept.to_string(),
            None => format!("Some({local})"),
        };
        Some((*kind, read))
    }
    /// The local that holds this choice, in the body that made it.
    pub(super) fn chosen_local(&self, kind: ValueKind) -> Result<String> {
        // A land type is what "a basic land type" chose too.
        let answers = |k: ValueKind| {
            k == kind
                || (kind == ValueKind::LandType
                    && matches!(k, ValueKind::BasicLandType | ValueKind::NonbasicLandType))
                || (kind == ValueKind::CardType && k == ValueKind::PermanentType)
        };
        self.chosen
            .iter()
            .find(|(k, _)| answers(*k))
            .map(|(_, local)| local.clone())
            .ok_or_else(|| gap(format!("a choice of {kind:?} outside its body")))
    }
    /// A choice read outside the body that made it, where what makes it is
    /// itself a hole of this card: a commander's color chosen before the game
    /// begins ("a deck rule a commander sets"), a color chosen as protection is
    /// given ("keyword action ChooseProtection"), or a color chosen as an
    /// Equipment becomes attached (`AttachAction::ChooseAColor`, "ability
    /// AsEvent"). That hole already says what the file waits on, so the read
    /// follows it instead of naming a hole of its own.
    pub(super) fn follows_choice(&self, kind: ValueKind, read: String) -> String {
        let made_by_a_hole = kind == ValueKind::Color
            && (self.gaps.iter().any(|g| {
                g.feature == "a deck rule a commander sets"
                    || g.feature == "keyword action ChooseProtection"
            }) || self.functions.contains("AttachAction::ChooseAColor")
                || self.gaps.iter().any(|g| g.feature == "ability AsEvent"));
        if !made_by_a_hole {
            return read;
        }
        let feature = format!(
            "planned_value!({:?}",
            format!("a choice of {kind:?} outside its body")
        );
        read.replace(&feature, &format!("planned_value!({FOLLOWS_GAP:?}"))
    }
    /// `Option` of the choice: this body's, or what was chosen as the source
    /// entered, to the ability reading it.
    pub(super) fn chosen_value(&self, kind: ValueKind) -> Result<String> {
        if let (ValueKind::Color, Some(color)) = (kind, &self.each_color) {
            return Ok(format!("Some({color})"));
        }
        if let Ok(local) = self.chosen_local(kind) {
            return Ok(match local.strip_suffix(KEPT) {
                Some(kept) => kept.to_string(),
                None => format!("Some({local})"),
            });
        }
        // The choice itself is a hole, so this read follows it.
        if self.as_event_chooses && kind == ValueKind::CardType {
            return lane_p::chosen_elsewhere(self.linked(), kind).map(|read| {
                read.replacen("a choice of CardType outside its body", FOLLOWS_GAP, 1)
            });
        }
        // The engine keeps a permanent's choices as it entered, and no other.
        if !self.enters_choosing.contains(&kind) {
            // "Choose a color. Whenever this becomes blocked by a creature of that
            // color this turn" (Zombie Boa): the ability that chose it creates the
            // one that reads it, and Cantrip keeps no resolution's choice for it.
            // "{2}, {T}, Choose flying, first strike, trample, or shadow: target
            // creature with the chosen ability" (Phyrexian Splicer): chosen as
            // its cost, before its targets, and Cantrip announces no such choice.
            if self
                .activation_cost
                .as_ref()
                .is_some_and(|cost| chooses_in_cost(cost, kind))
            {
                return Err(gap(
                    "a value chosen as an activation cost, read by its targets",
                ));
            }
            if self.stack_item
                && self
                    .line_text()
                    .is_some_and(|line| line.contains("hoose a ") && line.contains("henever"))
            {
                return Err(gap("a resolution's choice read by the trigger it creates"));
            }
            let read = lane_p::chosen_elsewhere(self.linked(), kind)
                .or_else(|_| self.chosen_elsewhere(kind))?;
            return Ok(self.follows_choice(kind, read));
        }
        let read = match kind {
            ValueKind::Color => "chosen_color",
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                "chosen_subtype"
            }
            ValueKind::CardName => "chosen_card",
            ValueKind::Player => "chosen_player",
            ValueKind::PlaneswalkerType => "chosen_subtype",
            ValueKind::CardType if self.entering_card_type.is_some() => "chosen_option",
            other => return lane_p::chosen_kept(self.linked(), other),
        };
        let linked = self.linked();
        if let (ValueKind::CardType, Some(words)) = (kind, &self.entering_card_type) {
            let card = match self.reader {
                Reader::Layer => "q.cards().get(c.card)",
                _ => "q.card(c.card)",
            };
            return Ok(format!(
                "q.state().chosen_option({linked}).filter(|c| std::ptr::eq({card}, &CARD)).map(|c| [{}][usize::from(c.index)])",
                words.join(", ")
            ));
        }
        Ok(format!("q.state().{read}({linked})"))
    }
    /// `ObjectFilter::Chosen`, of an object known by `chars` and `id`.
    pub(super) fn chosen_filter(&self, kind: ValueKind, chars: &str, id: &str) -> Result<String> {
        let list = self.chosen_lists.iter().find(|(k, _)| *k == kind);
        if list.is_none() && self.earlier_chosen_lists.contains(&kind) {
            return Err(gap(format!(
                "a {kind:?} each player chose as it entered, read by another ability"
            )));
        }
        let value = match list {
            Some(_) => String::new(),
            None => self.chosen_value(kind)?,
        };
        let test = match kind {
            ValueKind::Color => format!("{chars}.colors.has(chosen)"),
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                format!("{chars}.has_subtype(chosen)")
            }
            ValueKind::CardName if self.reader != Reader::Layer => {
                format!("{M}::library::object_has_name(q, {id}, chosen)")
            }
            ValueKind::CardType | ValueKind::PermanentType => format!("{chars}.is(chosen)"),
            ValueKind::PlaneswalkerType | ValueKind::NonbasicLandType => {
                format!("{chars}.has_subtype(chosen)")
            }
            // In a layer: the name of the chosen card, as it is written.
            ValueKind::CardName => {
                format!("!{chars}.name.is_empty() && q.cards().get(chosen).name == {chars}.name")
            }
            other => return Err(gap(format!("object filter Chosen {other:?}"))),
        };
        if let Some((_, list)) = list {
            if let Some(player) = self.own_choice.borrow().as_ref() {
                return Ok(format!("{list}.iter().zip(&{list}_who).any(|(&chosen, &who)| Some(who) == {player} && {test})"));
            }
            return Ok(format!("{list}.iter().any(|&chosen| {test})"));
        }
        if kind == ValueKind::Color && self.enters_two == Some(kind) {
            let two = self.chosen_two();
            return Ok(format!("{two}.iter().flatten().any(|&chosen| {test})"));
        }
        Ok(format!("{value}.is_some_and(|chosen| {test})"))
    }
    /// The `EffectKind`s of a change by a value this body chose.
    pub(super) fn chosen_effect(&self, change: &Change) -> Result<Vec<String>> {
        Ok(match change {
            Change::ChosenColor {
                mode: ChangeMode::Set,
            } => {
                let chosen = self.chosen_read(ValueKind::Color)?;
                match chosen.starts_with("colors_") {
                    true => vec![format!("EffectKind::SetColors({chosen})")],
                    false => vec![format!("EffectKind::SetColors(ColorSet::of({chosen}))")],
                }
            }
            Change::ChosenColor { .. } => vec![lane_g::planned_kind(
                "engine: no effect adds colors",
                format!(
                    "EffectKind::AddColors(ColorSet::of({}))",
                    self.chosen_read(ValueKind::Color)?
                ),
            )],
            // The source reads every "becomes that type" as adding; only "in addition
            // to its other types" keeps them (205.1a).
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Add,
            } if self.printed_in_addition() => vec![format!(
                "EffectKind::AddSubtype({})",
                self.chosen_local(*kind)?
            )],
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Add,
            } => vec![format!(
                "EffectKind::SetCreatureType({})",
                self.chosen_local(*kind)?
            )],
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Set,
            } => vec![format!(
                "EffectKind::SetCreatureType({})",
                self.chosen_local(*kind)?
            )],
            Change::ChosenType {
                kind: kind @ (ValueKind::BasicLandType | ValueKind::LandType),
                mode: ChangeMode::Add,
            } if !self.printed_in_addition() => vec![lane_g::planned_kind(
                "engine: no resolving effect sets land types",
                format!("EffectKind::SetLandType({})", self.chosen_local(*kind)?),
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
    /// Whether the printed line keeps what the object was. Without a line, as read.
    fn printed_in_addition(&self) -> bool {
        self.line_text()
            .is_none_or(|line| line.contains("in addition"))
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
            // Only a creature or a Kindred has creature types (205.3d).
            Change::ChosenType {
                kind: kind @ ValueKind::CreatureType,
                mode: ChangeMode::Set,
            } => statics::LayerChange::of(
                "Type",
                format!(
                    "if let Some(chosen) = {} {{ chars.subtypes = chars.subtypes.minus(SubtypeSet::CREATURE_TYPES); if chars.is(CardType::Creature) || chars.is(CardType::Kindred) {{ chars.subtypes = chars.subtypes.with(chosen); }} }}",
                    self.chosen_value(*kind)?
                ),
            ),
            other => return self.chosen_more(other),
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
        // "From colorless or from the color of your choice".
        if let (ValueKind::Color, Ok(local)) =
            (*kind, self.chosen_local(ValueKind::ColorOrColorless))
        {
            return Ok(Some((
                format!("{local}.map_or(EffectKind::ProtectionFromColorless, |color| EffectKind::Protection(ColorSet::of(color)))"),
                format!("if let Some(color) = {local} {{ chars.protection = chars.protection.with(color); }} else {{ chars.protection_colorless = true; }}"),
            )));
        }
        // What it chose as it entered, to an ability that may have nothing to read.
        if self.chosen_local(*kind).is_err() && self.enters_choosing.contains(kind) {
            let value = self.chosen_value(*kind)?;
            return match kind {
                ValueKind::Color => Ok(Some((
                    format!("EffectKind::Protection({value}.map_or(ColorSet::NONE, ColorSet::of))"),
                    format!("if let Some(color) = {value} {{ chars.protection = chars.protection.with(color); }}"),
                ))),
                other => Err(gap(format!("protection from a chosen {other:?}"))),
            };
        }
        let chosen = self.chosen_read(*kind)?;
        match kind {
            ValueKind::Color => Ok(Some((
                format!("EffectKind::Protection(ColorSet::of({chosen}))"),
                format!("chars.protection = chars.protection.with({chosen});"),
            ))),
            ValueKind::CardType => Ok(Some((
                format!("EffectKind::ProtectionFromTypes(TypeSet::of(&[{chosen}]))"),
                format!("chars.protection_types = TypeSet(chars.protection_types.0 | TypeSet::of(&[{chosen}]).0);"),
            ))),
            other => Err(gap(format!("protection from a chosen {other:?}"))),
        }
    }
}

/// Choices announced while paying this cost, excluding nested ability declarations.
fn chooses_in_cost(cost: &Cost, kind: ValueKind) -> bool {
    use crate::visit::{Visitor, Walk};
    struct Choices {
        kind: ValueKind,
        found: bool,
    }
    impl Visitor for Choices {
        fn ability(&mut self, _: &Ability) -> bool {
            false
        }
        fn operation(&mut self, operation: &Operation) -> bool {
            if matches!(operation, Operation::ChooseValue {kind, ..} if *kind == self.kind) {
                self.found = true;
            }
            true
        }
    }
    let mut reader = Choices { kind, found: false };
    cost.walk(&mut reader);
    reader.found
}

#[cfg(test)]
mod value_choice_scope_tests {
    use super::*;
    fn choice() -> Program {
        Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(0),
                operation: Operation::ChooseValue {
                    actor: PlayerSet::One(PlayerRef::AbilityController),
                    kind: ValueKind::Color,
                    terms: ChoiceTerms::default(),
                },
            }],
        }
    }
    fn cost(operand: Operand) -> Cost {
        Cost {
            parts: vec![CostPart::Primitive {
                name: "scope-control".into(),
                operands: vec![operand],
            }],
        }
    }
    #[test]
    fn activation_cost_choice_reader_is_typed_and_respects_ability_scopes() {
        assert!(chooses_in_cost(
            &cost(Operand::Program(choice())),
            ValueKind::Color
        ));
        assert!(!chooses_in_cost(
            &cost(Operand::Program(choice())),
            ValueKind::Ability
        ));
        let nested = Ability::Spell {
            announcement: Announcement {
                targets: vec![],
                modes: None,
            },
            body: choice(),
        };
        assert!(!chooses_in_cost(
            &cost(Operand::Ability(Box::new(nested))),
            ValueKind::Color
        ));
        assert!(!chooses_in_cost(
            &cost(Operand::Text("ChooseValue { kind: Color, }".into())),
            ValueKind::Color
        ));
    }
}

#[cfg(test)]
mod value_choice_result_tests {
    use super::*;
    fn choice(kind: ValueKind, count: ChoiceCount, secret: bool) -> Operation {
        Operation::ChooseValue {
            actor: PlayerSet::One(PlayerRef::AbilityController),
            kind,
            terms: ChoiceTerms {
                count,
                secret,
                ..Default::default()
            },
        }
    }
    #[test]
    fn planned_counts_keep_one_zero_up_to_and_unbounded_distinct() {
        for (count, expected, up_to) in [
            (ChoiceCount::default(), "1i64", false),
            (ChoiceCount::Exactly(Expr::Constant(0)), "0i64", false),
            (ChoiceCount::UpTo(Expr::Constant(2)), "2i64", true),
            (ChoiceCount::AnyNumber, "None", false),
        ] {
            let mut compiler = Compiler::new();
            let mut code = String::new();
            compiler
                .operation(
                    &choice(ValueKind::Number, count, true),
                    &ResultId(8),
                    &mut code,
                )
                .unwrap();
            let rendered = code
                .split("count: ")
                .nth(1)
                .unwrap()
                .split(", secret:")
                .next()
                .unwrap();
            if expected == "None" {
                assert_eq!(rendered, "None");
            } else {
                assert!(rendered.starts_with("Some("), "{code}");
                assert!(rendered.contains(expected), "{code}");
            }
            assert!(code.contains(&format!("up_to: {up_to}")), "{code}");
            assert!(code.contains("a value chosen in secret"), "{code}");
        }
    }
    #[test]
    fn consecutive_numeric_choices_have_separate_results_and_lexical_lifetimes() {
        let mut compiler = Compiler::new();
        let mut code = String::new();
        let operation = choice(ValueKind::Number, ChoiceCount::default(), false);
        compiler
            .operation(&operation, &ResultId(10), &mut code)
            .unwrap();
        let first = compiler.performed[&10].clone();
        let snapshot = compiler.results();
        compiler
            .operation(&operation, &ResultId(11), &mut code)
            .unwrap();
        let second = compiler.performed[&11].clone();
        assert_ne!(first, second);
        assert_eq!(compiler.performed[&10], first);
        assert!(code.contains("chosen_0"));
        assert!(code.contains("chosen_1"));
        compiler.restore_results(snapshot);
        assert_eq!(compiler.performed[&10], first);
        assert!(!compiler.performed.contains_key(&11));
    }
    #[test]
    fn a_secret_object_choice_keeps_its_selection_result() {
        let mut compiler = Compiler::new();
        let mut code = String::new();
        compiler
            .operation(
                &choice(ValueKind::Object, ChoiceCount::default(), true),
                &ResultId(21),
                &mut code,
            )
            .unwrap();
        assert_eq!(compiler.selections[&21], "chosen_0");
        assert!(code.contains("Vec::<ObjectId>::new()"));
        assert!(!compiler.selections.contains_key(&0));
    }
}
