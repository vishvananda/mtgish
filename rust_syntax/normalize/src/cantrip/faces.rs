//! Forms the scenarios of cards of several faces turned up, with the cards that
//! print them: each says what Cantrip's own card says, the way it says it.
use super::*;

/// "You may reveal that card" of a library's top card: not asked of an empty
/// library, where there is nothing to reveal (CR 609.3, Delver of Secrets).
pub(super) fn reveal_offered(compiler: &Compiler, body: &Program) -> String {
    let [Instruction::Effect {
        operation: Operation::Reveal { objects },
        ..
    }] = body.instructions.as_slice()
    else {
        return String::new();
    };
    match compiler.objects(objects) {
        Ok(read) => format!("!({}).is_empty() && ", compiler.fetch(read)),
        Err(_) => String::new(),
    }
}

/// "Whenever you cast a spell using mana produced by this": that mana is told
/// apart from other mana, and spent only as its controller chooses (CR 106.3,
/// 601.2h; Barracks of the Thousand). The rule a card file lists, and its function.
pub(super) fn mana_told_apart(functions: &str) -> Option<(&'static str, &'static str)> {
    // "If {S} of any of that spell's colors was spent to cast it" (Boreal
    // Outrider): mana from snow sources.
    if functions.contains("q.chars(source).supertypes.snow && ") && functions.contains(".mana_spent_to_cast(") {
        return Some((
            "snow_mana",
            "fn snow_mana(q: &dyn Query, _: &Asker, source: ObjectId) -> bool { q.chars(source).supertypes.snow }\n",
        ));
    }
    functions
        .contains(".mana_spent_to_cast(cast.spell).any(|(source, _)| Some(asker.source) == Some(source))")
        .then_some((
            "its_own_mana",
            "fn its_own_mana(_: &dyn Query, asker: &Asker, source: ObjectId) -> bool { source == asker.source }\n",
        ))
}

/// A prevention's hint is what it does, as a card file's says it: "Prevent all
/// damage that would be dealt to it." is "prevents all damage that would be dealt
/// to it". `hint` is a quoted literal.
pub(super) fn prevention_hint(hint: String) -> String {
    match hint.strip_prefix("\"Prevent ") {
        Some(rest) if !rest.trim_end_matches(".\"").contains(". ") => {
            format!("\"prevents {}\"", rest.trim_end_matches(".\"").trim_end_matches('"'))
        }
        _ => hint,
    }
}

/// Whether a filter reads the exiled cards used to craft an object (CR 702.167c):
/// those its craft cost exiled, linked to the card it was in exile, which
/// `cantrip_mechanics::craft` follows to the permanent it became.
pub(super) fn crafted_with(line: Option<&str>) -> bool {
    line.is_some_and(|line| line.contains("used to craft"))
}

/// A history test of damage, the source read as it was as it dealt the damage
/// and controlled by whoever controlled it then (CR 120.3, 609.7; Temple of Power).
pub(super) fn as_dealt(test: &str) -> String {
    const DAMAGE: &str = "if let Event::Damage { source, target, amount, kind, .. } = event { let (source, amount, combat) = (*source, *amount, kind.is_combat());";
    if !test.contains(DAMAGE) {
        return test.to_string();
    }
    test.replace(DAMAGE, "if let Event::Damage { source, target, amount, kind, controller, known, .. } = event { let (source, amount, combat, controller) = (*source, *amount, kind.is_combat(), *controller); let dealt = DamageDealt { source, target: *target, amount, combat, controller, known: *known };")
        .replace("q.chars(source)", "dealt.chars(q)")
        .replace("q.controller(source)", "controller")
}

/// "If you attacked with two or more creatures this turn": creatures, each once
/// however many combats it attacked in (CR 508.1, 500.8; Wedding Announcement).
pub(super) fn attackers_counted(event: &EventSpec, hits: &str) -> Option<String> {
    matches!(
        event,
        EventSpec::Attack { .. }
            | EventSpec::Object {
                event: ObjectEvent::Attacks,
                ..
            }
    )
    .then(|| {
        format!("{{ let mut attackers: Vec<ObjectId> = {hits}.filter_map(|f: Fired| f.subject.object()).collect(); attackers.sort(); attackers.dedup(); attackers.len() as i64 }}")
    })
}

/// How many cards a "may" draws its player, if it does: a player who can't draw
/// them isn't offered it (CR 121.2b; Arc of Fortune's "each player may discard
/// their hand and draw seven cards").
pub(super) fn may_draws(body: &[Instruction], player: &PlayerRef) -> Option<i64> {
    body.iter().find_map(|instruction| match instruction {
        Instruction::Effect {
            operation:
                Operation::Draw {
                    players: PlayerSet::One(drawer),
                    amount: Expr::Constant(n),
                },
            ..
        } if format!("{drawer:?}") == format!("{player:?}") => Some(*n),
        _ => None,
    })
}

/// "Up to two target cards from a single graveyard" as Cantrip says it: two
/// slots, the second a card of the graveyard the first's card is in and not that
/// card, any card when the first is left empty (CR 115.1, 601.2c;
/// `cantrip_mechanics::targets::ANOTHER_CARD_IN_THE_SAME_GRAVEYARD`, Cease //
/// Desist and Unlicensed Hearse). Only of the first slot, which that names.
pub(super) fn two_from_one_graveyard(at: usize, spec: &TargetSpec) -> bool {
    at == 0
        && spec.distribute.is_none()
        && spec.chooser.is_none()
        && matches!(spec.count, Count::UpTo(Expr::Constant(2)))
        && matches!(
            spec.constraints.as_slice(),
            [SelectionConstraint::CommonProperty(DistinctProperty::Owners)]
        )
        && matches!(&spec.kind, TargetKind::Object(domain) if domain.kind == ObjectKind::Card && domain.zones == [EventZone::Graveyard])
}

/// The second slot's test of `two_from_one_graveyard`, `test` being the first's.
pub(super) fn same_graveyard_as_first(test: &str) -> String {
    format!("({test}) && match (t, q.chosen_targets().first()) {{ (Target::Object(id), Some(&Target::Object(first))) => first != id && q.state().obj(first).zone == q.state().obj(id).zone, _ => true }}")
}

impl Compiler {
    /// "{T}: Add X {G} or X {W}, where X is ...": X mana of one color of the
    /// player's choice, the color chosen as the ability is activated and X counted
    /// as it resolves (CR 605.3b; Brigid, Doun's Mind).
    pub(super) fn counted_mana_of_one_color(
        &mut self,
        body: &Program,
        label: &str,
    ) -> Result<Option<String>> {
        let [Instruction::Effect {
            operation: Operation::BindValue { value },
            result: bound,
        }, Instruction::ChooseOne {
            decider: PlayerRef::AbilityController,
            options,
            offered: None,
        }] = body.instructions.as_slice()
        else {
            return Ok(None);
        };
        let mut colors = Vec::new();
        for option in options {
            let [Instruction::Effect {
                operation:
                    Operation::AddMana {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        mana:
                            ManaProduction::Repeat {
                                amount: Expr::BoundValue(read),
                                production,
                                ..
                            },
                        restriction: None,
                    },
                ..
            }] = option.instructions.as_slice()
            else {
                return Ok(None);
            };
            let ManaProduction::Amount {
                color,
                amount: Expr::Constant(1),
            } = production.as_ref()
            else {
                return Ok(None);
            };
            if read.0 != bound.0 || matches!(color, ManaColor::Colorless) || colors.contains(color) {
                return Ok(None);
            }
            colors.push(color.clone());
        }
        if colors.len() < 2 {
            return Ok(None);
        }
        let mask = color_mask(&colors)?;
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        let count = self.number(value).map(|n| n.code);
        self.reader = reader;
        writeln!(self.functions, "fn {label}(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(), Suspend> {{ let Some(color) = chosen else {{ return Ok(()) }}; let amount: i64 = {}; if amount > 0 {{ g.add_mana(asker.controller, color.mana(), amount as u32); }} Ok(()) }}", self.fetch(count?)).unwrap();
        Ok(Some(format!(
            "add_by({label}).choose_color().colors(ColorSet({mask}))"
        )))
    }
    /// The exiled cards used to craft an object, for a filter of `crafted_with`.
    pub(super) fn used_to_craft(&self, filter: &ObjectFilter, id: &str) -> Result<Option<String>> {
        let ObjectFilter::ExiledWith(object) = filter else {
            return Ok(None);
        };
        if !crafted_with(self.line_text()) {
            return Ok(None);
        }
        Ok(Some(match (object, self.reader) {
            (ObjectRef::Source, Reader::Layer) => {
                format!("cantrip_mechanics::craft::used_to_craft(q.state(), asker.source).contains(&{id})")
            }
            (_, Reader::Layer) => return Ok(None),
            (object, _) => format!(
                "{}.is_some_and(|o: ObjectId| cantrip_mechanics::craft::exiled_cards_used_to_craft(q, o).contains(&{id}))",
                self.object(object)?
            ),
        }))
    }
    /// "This Vehicle can't be blocked as long as ...": the keyword its rule is,
    /// while the condition holds, so that the internal rule keyword is what it
    /// has (CR 604.2, 613.1f; Watertight Gondola and Glyph of Delusion).
    pub(super) fn keyword_as_long_as(
        &mut self,
        label: &str,
        word: &str,
        parts: &mut Parts,
    ) -> Result<bool> {
        let Some(condition) = self.static_guard.clone() else {
            return Ok(false);
        };
        // A level's lines are the stage's, said as the rule they were.
        if !matches!(word, "CantBeBlocked" | "CantBlock" | "DoesNotUntap") || !self.stages.is_empty() {
            return Ok(false);
        }
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Layer, EventAt::Nowhere);
        let test = self.condition(&condition);
        (self.reader, self.event_at) = saved;
        let test = test?;
        // Power and toughness are of layer 7, after the keyword's layer 6: a rule
        // of them stays the combat rule it is (Stature, Size Shifter; Technodrome).
        if test.contains(".power") || test.contains(".toughness") {
            return Ok(false);
        }
        let function = format!("{label}_keyword");
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn LayerQuery, source: ObjectId, chars: &mut Chars) {{ let asker = &Asker::new(source, q.controller(source)); if {test} {{ chars.keywords = chars.keywords.with(Keyword::{word}); }} }}").unwrap();
        let text = quoted(&self.said(lane_k::Said::Sentence, format!("Static: {word}.")));
        parts.statics.push(format!(
            "static_ability(Layer::Ability, cantrip_mechanics::statics::itself, {function}, {text})"
        ));
        Ok(true)
    }
}
