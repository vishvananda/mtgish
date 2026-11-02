//! Static abilities on players: rules of the game their controller's permanent
//! changes (CR 101.2), as the tables of `CardDef` that ask about one player.
use super::*;

const PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";

/// The card a player would be casting, controlled by nobody yet: whoever casts
/// it is its controller as far as a filter asks (CR 601.2).
pub(super) fn cast_card() -> Candidate {
    Candidate {
        controller: "player".into(),
        announced: Some(String::new()),
        ..Candidate::of("spell")
    }
}

impl Compiler {
    /// Whether `player` is one this static ability is about. Layers read this
    /// too, so the commonest sets avoid the state a `LayerQuery` does not show.
    pub(super) fn player_member(&self, players: &PlayerSet) -> Result<String> {
        Ok(match players {
            PlayerSet::One(PlayerRef::AbilityController) => "player == asker.controller".into(),
            PlayerSet::All => "true".into(),
            PlayerSet::OpponentsOf(player) if matches!(player, PlayerRef::AbilityController) => {
                "player != asker.controller".into()
            }
            other if self.reader == Reader::Layer => {
                return Err(gap(format!("players in a layer {}", form(other))))
            }
            other => format!("({}).contains(&player)", self.players(other)?),
        })
    }
    pub(super) fn guard(&self) -> Result<String> {
        Ok(match &self.static_guard {
            Some(condition) => format!("({}) && ", self.condition(condition)?),
            None => String::new(),
        })
    }
    /// Generate a named callback and list it under one of `CardDef`'s tables.
    fn table_entry(
        &mut self,
        parts: &mut Parts,
        table: &'static str,
        function: &str,
        code: String,
    ) {
        writeln!(self.functions, "#[allow(unused_variables)] {code}").unwrap();
        parts.table(table, function.to_string());
    }
    /// "Creature spells you cast have affinity for artifacts" (CR 702.41a, 702.41b):
    /// a cost change of the source, each source its own, as Witherbloom prints it.
    /// The data says only "spells": the words "you cast" are the card's.
    pub(super) fn affinity_granted(
        &mut self,
        label: &str,
        affected: &ObjectSet,
        changes: &[Change],
        parts: &mut Parts,
    ) -> Result<bool> {
        let [Change::GrantAbility(ability)] = changes else {
            return Ok(false);
        };
        let Ability::Mechanic {
            mechanic: Mechanic::Affinity,
            cost: None,
            amount: None,
            filter: Some(counted),
        } = &**ability
        else {
            return Ok(false);
        };
        let ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Stack),
            filter: spells,
            ..
        } = affected
        else {
            return Ok(false);
        };
        let printed = self
            .line_text()
            .is_some_and(|line| line.contains("you cast have affinity for"));
        if !printed {
            return Ok(false);
        }
        let test = self.filter(
            spells,
            &Candidate {
                announced: Some("targets".into()),
                ..cast_card()
            },
        )?;
        let counted = self.filter(counted, &Candidate::of("id"))?;
        let guard = self.guard()?;
        let function = format!("{label}_affinity");
        self.table_entry(parts, "cost_changes", &function, format!(
            "fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ {PRELUDE} let Costed::Spell(spell) = costed else {{ return CostDelta::NONE; }}; if {guard}player == asker.controller && {test} {{ let count = q.permanents_of(player).into_iter().filter(|&id| {counted}).count(); CostDelta::less(count.min(255) as u8) }} else {{ CostDelta::NONE }} }}"
        ));
        Ok(true)
    }
    pub(super) fn player_static(
        &mut self,
        label: &str,
        players: &PlayerSet,
        changes: &[Change],
        parts: &mut Parts,
    ) -> Result<()> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let mut compiled = Ok(());
        for (i, change) in changes.iter().enumerate() {
            compiled = match change {
                Change::PlayerRule(rule) => {
                    let label = format!("{label}_{i}");
                    match self.player_rule(&label, players, rule, parts) {
                        Err(failed) => {
                            self.stated_player_rule(&label, players, rule, failed, parts)
                        }
                        compiled => compiled,
                    }
                }
                other => Err(gap(format!("player static change {}", form(other)))),
            };
            if compiled.is_err() {
                break;
            }
        }
        (self.reader, self.event_at) = saved;
        compiled
    }
    /// A `PlayRule` is its controller's, and has no callback to read a
    /// condition in, so `While` carries it.
    pub(super) fn play_rule(&mut self, label: &str, rule: String, parts: &mut Parts) -> Result<()> {
        let Some(condition) = self.static_guard.clone() else {
            parts.table("play_rules", rule);
            return Ok(());
        };
        let function = format!("{label}_while");
        let test = self.condition(&condition)?;
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ {test} }}").unwrap();
        parts.table(
            "play_rules",
            format!("PlayRule::While({function}, &[{rule}])"),
        );
        Ok(())
    }
    pub(super) fn player_rule(
        &mut self,
        label: &str,
        players: &PlayerSet,
        rule: &PlayerRule,
        parts: &mut Parts,
    ) -> Result<()> {
        let yours = matches!(players, PlayerSet::One(PlayerRef::AbilityController));
        let function = format!("{label}_rule");
        match rule {
            PlayerRule::Primitive { name, .. } => {
                return Err(gap(format!("primitive player rule {name}")))
            }
            PlayerRule::SpellCost {
                spells,
                modifier: SpellCostModifier::Alternate { cost, limit },
            } if matches!(limit, None | Some(PlayLimit::EachTurn(1)))
                && lane_j::instead_of_mana_cost(cost).is_some() =>
            {
                let (text, instead) = lane_j::instead_of_mana_cost(cost).expect("just seen");
                let test = self.filter(spells, &cast_card())?;
                // Used up by a cast for it, which the history records as this
                // permanent's: another, or this one come back, is another.
                let test = match limit {
                    Some(_) => format!("{test} && !cantrip_mechanics::history::permission_used_this_turn(q, asker.source)"),
                    None => test,
                };
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                writeln!(
                    self.functions,
                    "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, spell: ObjectId, player: PlayerId) -> bool {{ {guard}{member} && {test} }}"
                )
                .unwrap();
                parts.table(
                    "spell_alternatives",
                    format!(
                        "SpellAlternative {{ text: {}, applies: {function}, cost: {instead}, flash: false }}",
                        quoted(&text)
                    ),
                );
            }
            PlayerRule::SpellCost { spells, modifier } => {
                let delta = self.cost_delta(modifier)?;
                let test = self.filter(
                    spells,
                    &Candidate {
                        announced: Some("targets".into()),
                        ..cast_card()
                    },
                )?;
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "cost_changes", &function, format!(
                    "fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ {PRELUDE} let Costed::Spell(spell) = costed else {{ return CostDelta::NONE; }}; if {guard}{member} && {test} {{ {delta} }} else {{ CostDelta::NONE }} }}"
                ));
            }
            PlayerRule::Prohibit(action) => self.prohibit(&function, players, action, parts)?,
            PlayerRule::LimitPerTurn {
                action:
                    PlayerAction::CastSpell {
                        from: None,
                        filter: ObjectFilter::Any,
                    },
                amount: Expr::Constant(n),
            } if matches!(players, PlayerSet::All)
                && self.static_guard.is_none()
                && (1..=255).contains(n) =>
            {
                parts.field("cast_limit", n.to_string());
            }
            PlayerRule::LimitPerTurn {
                action: PlayerAction::Draw,
                amount: Expr::Constant(1),
            } => {
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "cant_draw", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, n: u32) -> bool {{ {guard}{member} && cantrip_mechanics::statics::more_than_one_card_drawn_this_turn(q, player, n) }}"
                ));
            }
            PlayerRule::HandSize(size) if yours => {
                let limit = match size {
                    HandSizeRule::Unlimited => "None".to_string(),
                    HandSizeRule::Set(Expr::Constant(n)) if u32::try_from(*n).is_ok() => {
                        format!("Some({n})")
                    }
                    other => self.hand_size_to_come(other)?,
                };
                self.play_rule(label, format!("PlayRule::MaximumHandSize({limit})"), parts)?;
            }
            PlayerRule::AdditionalLandPlays(Count::Exactly(Expr::Constant(n)))
                if yours && u8::try_from(*n).is_ok() =>
            {
                self.play_rule(label, format!("PlayRule::ExtraLandPlays({n})"), parts)?;
            }
            PlayerRule::AdditionalLandPlays(Count::AnyNumber) if yours => {
                self.play_rule(label, "PlayRule::AnyNumberOfLandPlays".into(), parts)?;
            }
            PlayerRule::SpellTiming {
                spells,
                timing: SpellTiming::Flash,
            } => {
                let test = self.filter(spells, &cast_card())?;
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "flash_rules", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, spell: ObjectId, player: PlayerId) -> bool {{ {guard}{member} && {test} }}"
                ));
            }
            PlayerRule::SpellTiming {
                spells,
                timing: SpellTiming::OwnTurnOnly,
            } => {
                let test = self.filter(spells, &cast_card())?;
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "spell_rules", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, spell: ObjectId) -> bool {{ !({guard}{member} && {test}) || q.state().turn.active == player }}"
                ));
            }
            PlayerRule::GrantKeyword(keyword) => {
                let keyword = match keyword {
                    PlayerKeyword::Hexproof => "Hexproof",
                    PlayerKeyword::Shroud => "Shroud",
                    other => return Err(gap(format!("player keyword {}", form(other)))),
                };
                let (reader, event_at) = (self.reader, self.event_at);
                self.reader = Reader::Layer;
                let tests = self
                    .guard()
                    .and_then(|g| Ok((g, self.player_member(players)?)));
                (self.reader, self.event_at) = (reader, event_at);
                let (guard, member) = tests?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn LayerQuery, source: ObjectId, player: PlayerId) -> bool {{ {PRELUDE} {guard}{member} }}").unwrap();
                parts.table(
                    "player_keywords",
                    format!(
                        "PlayerKeywordRule {{ keyword: Keyword::{keyword}, applies: {function} }}"
                    ),
                );
            }
            PlayerRule::LifeTotalLocked => {
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "cant_gain_life", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool {{ {guard}{member} }}"
                ));
                parts.table("cant_lose_life", function);
            }
            PlayerRule::PlayPermission {
                cards,
                from,
                action,
                filter,
            } if yours => {
                let top = |cards: &ObjectSet| {
                    matches!(cards, ObjectSet::Take { objects, count: Expr::Constant(1), from: SequenceEnd::Top }
                        if affected_zone(objects, EventZone::Library, false))
                };
                match (from, action) {
                    (EventZone::Graveyard, CardPlayAction::PlayLand)
                        if affected_zone(cards, EventZone::Graveyard, true)
                            && matches!(filter, ObjectFilter::Any) =>
                    {
                        self.play_rule(label, "PlayRule::LandsFromGraveyard".into(), parts)?;
                    }
                    (EventZone::Library, CardPlayAction::PlayLand)
                        if top(cards) && matches!(filter, ObjectFilter::Any) =>
                    {
                        self.play_rule(label, "PlayRule::LibraryTopLands".into(), parts)?;
                    }
                    (EventZone::Library, CardPlayAction::CastSpell) if top(cards) => {
                        let test = self.filter(filter, &Candidate::of("spell"))?;
                        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, spell: ObjectId) -> bool {{ {test} }}").unwrap();
                        self.play_rule(
                            label,
                            format!("PlayRule::LibraryTopSpells({function}, None)"),
                            parts,
                        )?;
                    }
                    _ => return self.more_player_rule(label, players, rule, parts),
                }
            }
            PlayerRule::Visibility {
                objects:
                    ObjectSet::Take {
                        objects,
                        count: Expr::Constant(1),
                        from: SequenceEnd::Top,
                    },
                audience: VisibilityAudience::AffectedPlayer,
            } if yours && affected_zone(objects, EventZone::Library, false) => {
                self.play_rule(label, "PlayRule::LookAtLibraryTop".into(), parts)?;
            }
            other => return self.more_player_rule(label, players, other, parts),
        }
        Ok(())
    }
    /// `Operation::CreateContinuousEffect` on players: rules a resolving
    /// ability changes for a while (CR 611.2).
    pub(super) fn continuous_on_players(
        &mut self,
        id: usize,
        source: &ObjectRef,
        players: &PlayerSet,
        changes: &[Change],
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        for (i, change) in changes.iter().enumerate() {
            let rule = match change {
                Change::PlayerRule(rule) => rule,
                other => return Err(gap(format!("continuous change on players {}", form(other)))),
            };
            if self.play_for_a_while(id, i, players, rule, duration, out)?
                || self.prohibit_for_a_while(id, i, source, players, rule, duration, out)?
                || self.look_for_a_while(id, i, players, rule, duration, out)?
                || self.keyword_for_a_while(id, i, source, players, rule, duration, out)?
                || self.cost_for_a_while(id, i, source, players, rule, duration, out)?
            {
                continue;
            }
            let kind = match rule {
                PlayerRule::AdditionalLandPlays(Count::Exactly(Expr::Constant(n)))
                    if u32::try_from(*n).is_ok() =>
                {
                    format!("EffectKind::ExtraLandPlays({n})")
                }
                PlayerRule::HandSize(HandSizeRule::Unlimited) => {
                    "EffectKind::MaximumHandSize(None)".to_string()
                }
                PlayerRule::HandSize(HandSizeRule::Set(Expr::Constant(n)))
                    if u32::try_from(*n).is_ok() =>
                {
                    format!("EffectKind::MaximumHandSize(Some({n}))")
                }
                PlayerRule::SpellTiming {
                    spells,
                    timing: SpellTiming::Flash,
                } => match listed_types(spells) {
                    Some(types) => {
                        let types: Vec<String> =
                            types.iter().map(|t| format!("CardType::{t}")).collect();
                        format!(
                            "EffectKind::CastAsThoughFlash(TypeSet::of(&[{}]))",
                            types.join(",")
                        )
                    }
                    None => {
                        let test = self.stated_filter(spells)?;
                        planned::planned_value(
                            "flash for spells other than by type",
                            "EffectKind::CastAsThoughFlash(TypeSet::NONE)",
                            &format!("EffectKind::CastAsThoughFlash({test})"),
                        )
                    }
                },
                other => return Err(gap(format!("player rule for a while {}", form(other)))),
            };
            let PlayerSet::One(PlayerRef::AbilityController) = players else {
                return Err(gap(format!("rule for a while on {}", form(players))));
            };
            let duration = self.duration(duration)?;
            writeln!(
                out,
                "if let Some(source) = {} {{ g.add_player_effect(asker.controller, source, {kind}, {duration}); }}",
                self.fetch(self.object(source)?)
            )
            .unwrap();
        }
        Ok(())
    }
    pub(super) fn cost_delta(&self, modifier: &SpellCostModifier) -> Result<String> {
        Ok(match modifier {
            SpellCostModifier::Increase(cost) => {
                let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                    return self.clone().costlier(cost);
                };
                let [ManaSymbol::Generic(n)] = symbols.as_slice() else {
                    return Ok(planned::planned_value(
                        "a spell costing colored mana more",
                        "CostDelta::NONE",
                        &format!("CostDelta::more_mana({})", mana(symbols)?),
                    ));
                };
                let more = u8::try_from(*n).map_err(|_| gap("spell cost increase beyond u8"))?;
                format!("CostDelta::more({more})")
            }
            SpellCostModifier::Reduce {
                reductions,
                spill_into_generic: _,
            } => self.cost_reduction(reductions)?,
            SpellCostModifier::Alternate { .. } => {
                return Err(gap("alternate spell cost rule"));
            }
            SpellCostModifier::AlternateDuring { .. }
            | SpellCostModifier::PayToReduce { .. }
            | SpellCostModifier::Scaled { .. }
            | SpellCostModifier::OptionalAdditional { .. } => {
                return Err(gap("spell cost rule with terms"));
            }
            SpellCostModifier::Minimum(amount) => {
                let amount = self.number(amount)?.fit(0, 255, "u8")?;
                format!("CostDelta::at_least({amount})")
            }
        })
    }
    pub(super) fn prohibit(
        &mut self,
        function: &str,
        players: &PlayerSet,
        action: &PlayerAction,
        parts: &mut Parts,
    ) -> Result<()> {
        let (guard, member) = (self.guard()?, self.player_member(players)?);
        let banned = format!("{guard}{member}");
        const ONE: &str = "(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool";
        match action {
            PlayerAction::GainLife => self.table_entry(parts, "cant_gain_life", function, format!("fn {function}{ONE} {{ {banned} }}")),
            PlayerAction::LoseLife => self.table_entry(parts, "cant_lose_life", function, format!("fn {function}{ONE} {{ {banned} }}")),
            PlayerAction::WinGame => self.table_entry(parts, "win_rules", function, format!("fn {function}{ONE} {{ {banned} }}")),
            PlayerAction::Draw => self.table_entry(parts, "cant_draw", function, format!(
                "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, n: u32) -> bool {{ {banned} }}"
            )),
            PlayerAction::LoseGame { reason } => {
                let reason = match reason {
                    None => "true",
                    Some(GameLossReason::NonpositiveLife) => "matches!(reason, LossReason::Life)",
                };
                self.table_entry(parts, "loss_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, reason: LossReason) -> bool {{ {banned} && {reason} }}"
                ));
            }
            PlayerAction::CastSpell { from: None, filter } => {
                let test = self.filter(filter, &cast_card())?;
                self.table_entry(parts, "spell_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, spell: ObjectId) -> bool {{ !({banned} && {test}) }}"
                ));
            }
            PlayerAction::PlayLand { from: None, filter } => {
                let test = self.filter(filter, &Candidate { controller: "player".into(), ..Candidate::of("land") })?;
                self.table_entry(parts, "land_play_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, land: ObjectId, face: CardId) -> bool {{ !({banned} && {test}) }}"
                ));
            }
            PlayerAction::Activate(filter) => {
                let test = self.barred_activation(filter)?;
                let (table, function) = ("activation_rules", function.to_string());
                let code = format!(
                    "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, object: ObjectId, ability: &ActivatedDef) -> bool {{ let player = q.controller(object); !({banned} && {test}) }}"
                );
                self.table_entry(parts, table, &function, code);
            }
            other => return Err(gap(format!("prohibited action {}", form(other)))),
        }
        Ok(())
    }
}

/// The card types a timing permission names, if that is all it names.
fn listed_types(spells: &ObjectFilter) -> Option<Vec<String>> {
    match spells {
        // Every spell has one of the types a spell can be (CR 300.1, 305.9).
        ObjectFilter::Any => Some(
            [
                "Artifact",
                "Battle",
                "Creature",
                "Enchantment",
                "Instant",
                "Kindred",
                "Planeswalker",
                "Sorcery",
            ]
            .map(String::from)
            .to_vec(),
        ),
        ObjectFilter::CardType(t) => Some(vec![card_type(t).ok()?.to_string()]),
        ObjectFilter::Either(filters) => {
            let mut types = Vec::new();
            for filter in filters {
                types.extend(listed_types(filter)?);
            }
            Some(types)
        }
        _ => None,
    }
}

/// The cards of the player the rule is about in `zone`: all of them, or their
/// land cards.
fn affected_zone(objects: &ObjectSet, zone: EventZone, lands: bool) -> bool {
    let ObjectSet::Zone {
        scope,
        kind: ObjectKind::Card,
        filter,
    } = objects
    else {
        return false;
    };
    let ZoneScope::Players {
        zone: found,
        players,
    } = scope
    else {
        return false;
    };
    found == &zone
        && matches!(players, PlayerSet::One(PlayerRef::Affected))
        && match filter {
            ObjectFilter::Any => !lands,
            ObjectFilter::CardType(found) => lands && name(found) == "Land",
            _ => false,
        }
}
