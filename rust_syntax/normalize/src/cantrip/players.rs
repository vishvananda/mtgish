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
    /// "Each enchantment card in your hand has miracle. Its miracle cost is equal
    /// to its mana cost reduced by {4}" (Aminatou, Veil Piercer; CR 702.94a): a
    /// `GrantedMiracle`, whose cost is the card's mana cost less the generic mana.
    pub(super) fn miracle_granted(
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
            mechanic: Mechanic::Miracle,
            cost: Some(cost),
            amount: None,
            filter: None,
        } = &**ability
        else {
            return Ok(false);
        };
        let less = match cost.parts.as_slice() {
            [CostPart::ManaCostReduced { object: None, reduction }] => {
                let words = format!("{reduction:?}");
                if !words.contains("CostReduceGeneric") {
                    return Ok(false);
                }
                let digits: String = words
                    .split("Integer(")
                    .nth(1)
                    .unwrap_or("")
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                match digits.parse::<u8>() {
                    Ok(n) => n,
                    Err(_) => return Ok(false),
                }
            }
            _ => return Ok(false),
        };
        let Some(test) = self.outside_subject(affected, &Candidate::of("card"))? else {
            return Ok(false);
        };
        let guard = self.guard()?;
        let has = format!("{label}_miracle");
        writeln!(self.functions, "#[allow(unused_variables)] fn {has}(q: &dyn Query, asker: &Asker, card: ObjectId) -> bool {{ {guard}{test} }}").unwrap();
        let name = self.card_name.clone();
        writeln!(self.functions, "fn {label}_miracle_cost(q: &dyn Query, card: CardId) -> Option<ManaCost> {{ let def = q.card(card); def.has_cost.then_some(def.cost) }}").unwrap();
        parts.table(
            "granted_miracles",
            format!("GrantedMiracle {{ text: {:?}, has: {has}, cost: {label}_miracle_cost, less: {less} }}", format!("{name}'s miracle: its mana cost reduced by {{{less}}}")),
        );
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
                self.costed_spell = true;
                let delta = self.cost_delta(modifier);
                self.costed_spell = false;
                let delta = delta?;
                // "The first noncreature spell each opponent casts each turn": the
                // player the rule is about is the one casting, `player` here.
                let outer = std::mem::replace(&mut self.rule_player, true);
                let test = self.filter(
                    spells,
                    &Candidate {
                        announced: Some("targets".into()),
                        ..cast_card()
                    },
                );
                self.rule_player = outer;
                let test = test?;
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "cost_changes", &function, format!(
                    "fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ {PRELUDE} let Costed::Spell(spell) = costed else {{ return CostDelta::NONE; }}; if {guard}{member} && {test} {{ {delta} }} else {{ CostDelta::NONE }} }}"
                ));
            }
            // "Spells and abilities your opponents control can't cause you to ...":
            // of the player who would do it, and whose spell or ability would make them.
            PlayerRule::CantBeCaused {
                action:
                    action @ (PlayerAction::DiscardCards
                    | PlayerAction::Sacrifice {
                        objects: ObjectFilter::Any,
                        paying: None,
                    }),
                by: ObjectFilter::ControlledBy(PlayerSet::OpponentsOf(PlayerRef::AbilityController)),
            } if yours => {
                let cause = match action {
                    PlayerAction::DiscardCards => "CauseKind::Discard",
                    _ => "CauseKind::Sacrifice",
                };
                let guard = self.guard()?;
                self.table_entry(parts, "cause_rules", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, causer: PlayerId, player: PlayerId, kind: CauseKind) -> bool {{ !({guard}kind == {cause} && player == asker.controller && causer != asker.controller) }}"
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
                let rule = match size {
                    HandSizeRule::Unlimited => "PlayRule::MaximumHandSize(None)".to_string(),
                    HandSizeRule::Set(Expr::Constant(n)) if u32::try_from(*n).is_ok() => {
                        format!("PlayRule::MaximumHandSize(Some({n}))")
                    }
                    other => self.hand_size_to_come(label, other)?,
                };
                self.play_rule(label, rule, parts)?;
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
                if matches!(spells, ObjectFilter::All(parts) if parts.iter().all(|part| {
                    matches!(part, ObjectFilter::Subtype(_))
                })) {
                    return Err(gap("spells of two subtypes read as spells of both"));
                }
                let test = self.filter(spells, &cast_card())?;
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "flash_rules", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, spell: ObjectId, player: PlayerId) -> bool {{ {guard}{member} && {test} }}"
                ));
            }
            // "Can cast spells only any time they could cast a sorcery" (307.1), asked
            // before the spell is on the stack, which it being there would make it not.
            PlayerRule::SpellTiming {
                spells: ObjectFilter::Any,
                timing: SpellTiming::SorceryOnly,
            } => {
                let (guard, member) = (self.guard()?, self.player_member(players)?);
                self.table_entry(parts, "cast_rules", &function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, from: ZoneKind) -> bool {{ let s = q.state(); !({guard}{member}) || (player == s.turn.active && s.turn.step().is_main() && s.stack.is_empty()) }}"
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
            // "You may play X additional lands this turn": asked as it resolves.
            if let (
                PlayerRule::AdditionalLandPlays(Count::Exactly(n)),
                PlayerSet::One(PlayerRef::AbilityController),
                crate::ir::Duration::EndOfTurn,
                true,
                false,
            ) = (rule, players, duration, self.stack_item, matches!(rule, PlayerRule::AdditionalLandPlays(Count::Exactly(Expr::Constant(_)))))
            {
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?);
                writeln!(out, "g.additional_land_plays(item, {n});").unwrap();
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
    pub(super) fn cost_delta(&mut self, modifier: &SpellCostModifier) -> Result<String> {
        Ok(match modifier {
            SpellCostModifier::Increase(cost) => {
                let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                    // Read apart, but the functions its cost names are the card's.
                    let mut apart = self.clone();
                    let made = apart.costlier(cost);
                    self.functions = std::mem::take(&mut apart.functions);
                    return made;
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
    /// `bool`: whether `card` is in the zones `scope` names, each its owner's.
    pub(super) fn in_scope(&self, scope: &ZoneScope, card: &str) -> Result<String> {
        Ok(match scope {
            ZoneScope::Shared(zone) | ZoneScope::Players { zone, players: PlayerSet::All } => {
                format!("q.obj({card}).zone.is({})", zone_kind(zone)?)
            }
            // "You can't play lands or cast spells from your hand": the hand of the
            // player the rule is asked of (Experimental Frenzy).
            ZoneScope::Players {
                zone,
                players: PlayerSet::One(PlayerRef::Affected),
            } => format!(
                "(q.obj({card}).zone.is({}) && q.obj({card}).owner == player)",
                zone_kind(zone)?
            ),
            ZoneScope::Players { zone, players } => format!(
                "(q.obj({card}).zone.is({}) && {{ let player = q.obj({card}).owner; {} }})",
                zone_kind(zone)?,
                self.player_member(players)?
            ),
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
                // "With mana value greater than the number of lands that player controls".
                let outer = std::mem::replace(&mut self.rule_player, true);
                let test = self.filter(filter, &cast_card());
                self.rule_player = outer;
                let test = test?;
                self.table_entry(parts, "spell_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, spell: ObjectId) -> bool {{ !({banned} && {test}) }}"
                ));
            }
            // "Players can't cast spells from graveyards" (CR 601.3): the zone is all
            // a rule of casting is told of the spell.
            PlayerAction::CastSpell {
                from:
                    Some(
                        ZoneScope::Shared(zone)
                        | ZoneScope::Players {
                            zone,
                            players: PlayerSet::All,
                        },
                    ),
                filter: ObjectFilter::Any,
            } => {
                let zone = zone_kind(zone)?;
                self.table_entry(parts, "cast_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, from: ZoneKind) -> bool {{ !({banned} && from == {zone}) }}"
                ));
            }
            // What of the spell a rule reads, it reads of the card where it is as it's
            // asked first; the spell it's asked of again is on the stack, and passes.
            PlayerAction::CastSpell {
                from: Some(scope),
                filter,
            } => {
                let test = self.filter(filter, &cast_card())?;
                let from = self.in_scope(scope, "spell")?;
                self.table_entry(parts, "spell_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, spell: ObjectId) -> bool {{ !({banned} && {from} && {test}) }}"
                ));
            }
            PlayerAction::PlayLand { from: None, filter } => {
                let test = self.filter(filter, &Candidate { controller: "player".into(), ..Candidate::of("land") })?;
                self.table_entry(parts, "land_play_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, land: ObjectId, face: CardId) -> bool {{ !({banned} && {test}) }}"
                ));
            }
            PlayerAction::PlayLand {
                from: Some(scope),
                filter,
            } => {
                let test = self.filter(filter, &Candidate { controller: "player".into(), ..Candidate::of("land") })?;
                let from = self.in_scope(scope, "land")?;
                self.table_entry(parts, "land_play_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, land: ObjectId, face: CardId) -> bool {{ !({banned} && {from} && {test}) }}"
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
            // Only a spell or ability can have a player search (CR 701.23): each asks.
            PlayerAction::SearchLibraries => self.table_entry(parts, "cause_rules", function, format!(
                "fn {function}(q: &dyn Query, asker: &Asker, causer: PlayerId, player: PlayerId, kind: CauseKind) -> bool {{ !({banned} && matches!(kind, CauseKind::Search {{ .. }})) }}"
            )),
            PlayerAction::BlockWith(filter) => {
                let test = self.filter(filter, &Candidate::of("blocker"))?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ let asker = &Asker::new(source, q.controller(source)); let player = q.controller(blocker); !({banned} && {test}) }}").unwrap();
                parts.block_rules.push(function.to_string());
            }
            // "Players can't pay life ... to cast spells or activate abilities" (CR 118.3):
            // what a cost is paid for is all a ban is told of what it pays for.
            PlayerAction::PayLife(on) => {
                let paying = self.cost_paid_for(on)?;
                self.table_entry(parts, "cost_bans", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, costing: CostFor, payment: CostPayment) -> bool {{ {banned} && {paying} && matches!(payment, CostPayment::Life) }}"
                ));
            }
            PlayerAction::Sacrifice {
                objects,
                paying: Some(on),
            } => {
                let paying = self.cost_paid_for(on)?;
                let test = self.filter(objects, &Candidate::of("permanent"))?;
                self.table_entry(parts, "cost_bans", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, costing: CostFor, payment: CostPayment) -> bool {{ {banned} && {paying} && matches!(payment, CostPayment::Sacrifice(permanent) if {test}) }}"
                ));
            }
            // "Can't cast spells from anywhere other than their hands" (CR 601.3):
            // the zone is all a rule of casting is told (Drannith Magistrate).
            PlayerAction::CastOutside(zone) => {
                let zone = zone_kind(zone)?;
                self.table_entry(parts, "cast_rules", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, from: ZoneKind) -> bool {{ !({banned} && from != {zone}) }}"
                ));
            }
            // "Players can't get counters", "you can't get poison counters" (CR
            // 122.1): a player's counters are poison, energy and experience.
            PlayerAction::ReceiveCounters(kind) => {
                use crate::counter::{CounterKind as K, CounterName as N};
                let kind = match kind {
                    None => "true",
                    Some(K::Named(N::PoisonCounter)) => "matches!(kind, PlayerCounter::Poison)",
                    Some(K::Named(N::ExperienceCounter)) => "matches!(kind, PlayerCounter::Experience)",
                    Some(other) => return Err(gap(format!("prohibited action ReceiveCounters {other:?}"))),
                };
                self.table_entry(parts, "player_counter_bans", function, format!(
                    "fn {function}(q: &dyn Query, asker: &Asker, player: PlayerId, kind: PlayerCounter) -> bool {{ {banned} && {kind} }}"
                ));
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
