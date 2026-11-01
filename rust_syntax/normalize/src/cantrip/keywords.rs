//! Keyword actions the rules define (CR 701): each is what `Game` or
//! `cantrip_mechanics` calls it, so this says only who and what it is given.
use super::*;

const M: &str = "cantrip_mechanics";
/// The card's first delayed ability, which `earthbend::effect` gives the land.
pub(super) const EARTHBEND_RETURN: &str = "cantrip_mechanics::earthbend::RETURN";

/// One `Operation::Keyword`.
pub(super) struct Action<'a> {
    pub actor: &'a PlayerSet,
    pub action: KeywordAction,
    pub object: Option<&'a ObjectRef>,
    pub other: Option<&'a ObjectRef>,
    pub amount: Option<&'a Expr>,
    pub subtype: Option<&'a str>,
}

impl Compiler {
    /// The player doing this, for an action that hands them to the engine.
    pub(super) fn actor(&self, action: &Action) -> Result<String> {
        match action.actor {
            PlayerSet::One(player) => Ok(self.fetch(self.certain_player(player)?)),
            _ => Err(gap("keyword action by several players")),
        }
    }
    /// An action of the resolving item's controller, which `item` names.
    fn controlled_by_item(&self, action: &Action) -> Result<()> {
        match action.actor {
            PlayerSet::One(PlayerRef::AbilityController) if self.stack_item => Ok(()),
            _ => Err(gap(format!(
                "keyword action {:?} by another than its controller",
                action.action
            ))),
        }
    }
    /// `Option<ObjectId>` of an object still there to act on.
    pub(super) fn acted_object(&self, object: Option<&ObjectRef>) -> Result<String> {
        let object = object.ok_or_else(|| gap("keyword action without its object"))?;
        let list = self.acted_on(&ObjectSet::One(object.clone()))?;
        Ok(format!("({list}).first().copied()"))
    }
    /// `Option<ObjectId>` of an object, what's left of it included: connive and
    /// explore still happen for a creature that left (CR 701.44c, 701.50c).
    fn last_known_object(&self, object: Option<&ObjectRef>) -> Result<String> {
        let object = object.ok_or_else(|| gap("keyword action without its object"))?;
        let list = self.fetch(self.objects(&ObjectSet::One(object.clone()))?);
        Ok(format!("({list}).first().copied()"))
    }
    pub(super) fn action_amount(&mut self, action: &Action, max: i64, ty: &str) -> Result<String> {
        let amount = action
            .amount
            .ok_or_else(|| gap("keyword action without its amount"))?;
        let number = self.number(amount)?.amount(max.into(), ty)?;
        Ok(self.fetch(number))
    }
    /// How many times to repeat an action that may state a number of times.
    pub(super) fn times(&mut self, action: &Action) -> Result<String> {
        match action.amount {
            Some(_) => self.action_amount(action, u8::MAX.into(), "u32"),
            None => Ok("1u32".into()),
        }
    }
    fn earthbend(&mut self, action: &Action, out: &mut String) -> Result<()> {
        self.controlled_by_item(action)?;
        let (Some(ObjectRef::Target { slot, index: 0 }), Some(Expr::Constant(n))) =
            (action.object, action.amount)
        else {
            return Err(gap("earthbend beyond one target and a number"));
        };
        if slot.0 != 0 || self.slot(slot)? != 0 {
            return Err(gap("earthbend of a target past the first"));
        }
        match self.delayed.first().map(String::as_str) {
            Some(EARTHBEND_RETURN) => {}
            None => self.delayed.push(EARTHBEND_RETURN.into()),
            Some(_) => return Err(gap("earthbend beside another first delayed ability")),
        }
        let n = i16::try_from(*n).map_err(|_| gap("earthbend exceeds i16"))?;
        writeln!(out, "{M}::earthbend::effect::<{n}>(g, item)?;").unwrap();
        Ok(())
    }
    /// A keyword action that only hands the engine a player.
    fn actor_action(&self, action: &Action, call: &str, out: &mut String) -> Result<()> {
        let player = self.actor(action)?;
        writeln!(out, "{{ let player = {player}; {call}; }}").unwrap();
        Ok(())
    }
    /// A keyword action done to one object still there, if it is.
    fn object_action(&self, action: &Action, call: &str, out: &mut String) -> Result<()> {
        let object = self.acted_object(action.object)?;
        writeln!(out, "if let Some(object) = {object} {{ {call}; }}").unwrap();
        Ok(())
    }
    /// `object` and `other`, when both are there to act on.
    fn pair_action(&self, action: &Action, id: usize, out: &mut String) -> Result<String> {
        let first = self.acted_object(action.object)?;
        let second = self.acted_object(action.other)?;
        writeln!(out, "let first_{id} = {first}; let second_{id} = {second};").unwrap();
        Ok(format!("(first_{id}, second_{id})"))
    }
    pub(super) fn keyword_action(
        &mut self,
        id: usize,
        action: Action,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        use KeywordAction as K;
        let paired = matches!(action.action, K::Fight | K::ExchangeControl);
        if action.other.is_some() && !paired {
            return Err(gap(format!(
                "keyword action {:?} with a second object",
                action.action
            )));
        }
        let counted = matches!(
            action.action,
            K::Amass
                | K::Connive
                | K::Earthbend
                | K::Monstrosity
                | K::Discover
                | K::GainEnergy
                | K::Incubate
                | K::EmpowerJace
                | K::Adapt
                | K::Bolster
        ) || (action.amount.is_some()
            && matches!(action.action, K::Investigate | K::Proliferate))
            || lane_g::takes_amount(action.action);
        if action.amount.is_some() && !counted {
            return self.action_with_amount(id, action, result, out);
        }
        match action.action {
            K::Transform | K::Convert if self.stack_item => {
                self.object_action(&action, "g.transform_by(object, item)?", out)?
            }
            K::Transform | K::Convert => {
                self.object_action(&action, "g.transform(object)?", out)?
            }
            K::Fight => {
                let pair = self.pair_action(&action, id, out)?;
                writeln!(
                    out,
                    "if let (Some(first), Some(second)) = {pair} {{ g.fight(first, second); }}"
                )
                .unwrap();
            }
            K::ExchangeControl => {
                let pair = self.pair_action(&action, id, out)?;
                writeln!(out, "let exchanged_{id} = match {pair} {{ (Some(first), Some(second)) => g.exchange_control(asker.source, first, second), _ => false }};").unwrap();
                self.performed.insert(result.0, format!("exchanged_{id}"));
            }
            K::Investigate => {
                let clue = format!("{M}::tokens::CLUE");
                if !self.extras.contains(&clue) {
                    self.extras.push(clue);
                }
                let times = self.times(&action)?;
                let call = format!("let times = {times}; for _ in 0..times {{ {M}::tokens::investigate(g, player)?; }}");
                self.actor_action(&action, &call, out)?;
            }
            K::Proliferate => {
                let times = self.times(&action)?;
                let call = format!("let times = {times}; for _ in 0..times {{ g.proliferate(player, asker.source)?; }}");
                self.actor_action(&action, &call, out)?;
            }
            K::Incubate => {
                for token in ["INCUBATOR", "PHYREXIAN"] {
                    let token = format!("{M}::tokens::{token}");
                    if !self.extras.contains(&token) {
                        self.extras.push(token);
                    }
                }
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                let call = format!("let amount = {n}; {M}::tokens::incubate(g, player, amount)?");
                self.actor_action(&action, &call, out)?;
            }
            K::EmpowerJace => {
                let token = format!("{M}::empower::JACE");
                if !self.extras.contains(&token) {
                    self.extras.push(token);
                }
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                let call = format!(
                    "let amount = {n}; {M}::empower::jace(g, player, asker.source, amount)?"
                );
                self.actor_action(&action, &call, out)?;
            }
            K::Adapt => {
                if action.object.is_some() {
                    return Err(gap("adapt of another object"));
                }
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                writeln!(
                    out,
                    "{{ let amount_{id} = {n}; {M}::adapt::adapt(g, asker.source, amount_{id}); }}"
                )
                .unwrap();
            }
            K::Bolster => {
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                let call = format!("let amount = {n}; let creatures: Vec<ObjectId> = g.permanents_of(player).into_iter().filter(|&o| g.chars(o).is(CardType::Creature)).collect(); let least = creatures.iter().map(|&o| g.chars(o).toughness.unwrap_or(0)).min(); let fewest: Vec<ObjectId> = creatures.into_iter().filter(|&o| Some(g.chars(o).toughness.unwrap_or(0)) == least).collect(); if let Some(creature) = {M}::choose::card(g, player, asker.source, Purpose::ChooseAsEnters, fewest)? {{ g.add_counters(creature, CounterKind::PlusOne, amount); }}");
                self.actor_action(&action, &call, out)?;
            }
            K::Populate => {
                let call = format!("let tokens: Vec<ObjectId> = g.permanents_of(player).into_iter().filter(|&o| g.obj(o).token && g.chars(o).is(CardType::Creature)).collect(); if let Some(token) = {M}::choose::card(g, player, asker.source, Purpose::CopyWhich, tokens)? {{ let values = g.copiable_values(token); g.create_token_copies_tapped(values, player, 1, false)?; }}");
                self.actor_action(&action, &call, out)?;
            }
            K::Learn => {
                self.controlled_by_item(&action)?;
                writeln!(out, "{M}::learn::learn(g, item)?;").unwrap();
            }
            K::TakeInitiative => self.actor_action(&action, "g.take_initiative(player)", out)?,
            K::Suspect => self.object_action(&action, "g.suspect(object)", out)?,
            K::TurnFaceUp => {
                let player = self.actor(&action)?;
                let call = format!("g.turn_face_up(object, {player})");
                self.object_action(&action, &call, out)?;
            }
            K::Unattach => self.object_action(&action, "g.unattach(object)", out)?,
            K::Airbend => {
                let object = self.acted_object(action.object)?;
                writeln!(
                    out,
                    "if let Some(object) = {object} {{ {M}::airbend_all(g, &[object]); }}"
                )
                .unwrap();
            }
            K::EndTheTurn => writeln!(out, "g.end_the_turn()?;").unwrap(),
            K::BecomeDay => writeln!(out, "g.become_day()?;").unwrap(),
            K::BecomeNight => writeln!(out, "g.become_night()?;").unwrap(),
            K::AdditionalPhase(AddedPhases::Combat) => {
                writeln!(out, "g.add_combat_phase();").unwrap()
            }
            K::Skip(SkippedPart::Turn) => {
                let call = "g.skip_next_turns(asker.source, player, 1)";
                self.actor_action(&action, call, out)?;
            }
            K::RemoveFromCombat => {
                let mut code = String::new();
                self.object_action(&action, "g.remove_from_combat(object)", &mut code)?;
                self.plan_code("engine: no effect removes from combat", code.trim(), out);
            }
            K::Amass => {
                use crate::mtg_types::SubType as S;
                let (word, subtype) = match action.subtype {
                    Some("Goblin") => ("Goblin", S::Goblin),
                    Some("Orc") => ("Orc", S::Orc),
                    Some("Sliver") => ("Sliver", S::Sliver),
                    Some("Zombie") => ("Zombie", S::Zombie),
                    other => return Err(gap(format!("amass subtype {other:?}"))),
                };
                let token = self.token_definition(&TokenDefinition {
                    name: TokenName::FromSubtypes,
                    colors: vec![ManaColor::Black],
                    chosen_color: false,
                    chosen_creature_type: false,
                    supertypes: Vec::new(),
                    card_types: vec![crate::mtg_types::CardType::Creature],
                    subtypes: vec![subtype, S::Army],
                    power_toughness: Some((Expr::Constant(0), Expr::Constant(0))),
                    abilities: Vec::new(),
                    abilities_of: None,
                })?;
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                let player = self.actor(&action)?;
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; let army_{id}: Vec<ObjectId> = {M}::amass::amass_army(g, player_{id}, asker.source, &{token}, Subtype::{word}, amount_{id})?.into_iter().collect();").unwrap();
                self.moved.insert(result.0, format!("army_{id}"));
            }
            K::Explore => {
                let object = self.last_known_object(action.object)?;
                writeln!(
                    out,
                    "if let Some(object) = {object} {{ {M}::explore(g, object)?; }}"
                )
                .unwrap();
            }
            K::Connive => {
                let object = self.last_known_object(action.object)?;
                let n = match action.amount {
                    Some(_) => self.action_amount(&action, u32::MAX.into(), "u32")?,
                    None => "1".into(),
                };
                writeln!(out, "let amount_{id}: u32 = {n}; if let Some(object) = {object} {{ {M}::connive_n(g, object, asker.source, amount_{id})?; }}").unwrap();
            }
            K::Goad => {
                self.controlled_by_item(&action)?;
                let call = format!("{M}::goad::goad(g, item, object)");
                self.object_action(&action, &call, out)?;
            }
            K::Earthbend => self.earthbend(&action, out)?,
            K::ManifestDread => {
                let player = self.actor(&action)?;
                writeln!(out, "let player_{id} = {player}; let entered_{id}: Vec<ObjectId> = {M}::manifest_dread::manifest_dread_object(g, player_{id}, asker.source)?.into_iter().collect();").unwrap();
                self.moved.insert(result.0, format!("entered_{id}"));
            }
            K::Monstrosity => {
                if action.object.is_some() {
                    return Err(gap("monstrosity of another object"));
                }
                let n = self.action_amount(&action, i16::MAX.into(), "i16")?;
                writeln!(
                    out,
                    "let amount_{id} = {n}; g.monstrosity(asker.source, amount_{id});"
                )
                .unwrap();
            }
            K::Discover => {
                let n = self.action_amount(&action, u32::MAX.into(), "u32")?;
                let player = self.actor(&action)?;
                let discover = format!("{M}::discover::discover(g, player_{id}, amount_{id})?");
                if lane_n::read_as_objects(&self.after, result) {
                    let found = planned::planned_value(
                        "the card a discover found",
                        &format!("{{ {discover}; Vec::<ObjectId>::new() }}"),
                        &format!("{M}::discover::discover_card(g, player_{id}, amount_{id})?.into_iter().collect::<Vec<ObjectId>>()"),
                    );
                    writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; let discovered_{id}: Vec<ObjectId> = {found};").unwrap();
                    self.moved.insert(result.0, format!("discovered_{id}"));
                } else {
                    writeln!(
                        out,
                        "let amount_{id} = {n}; let player_{id} = {player}; {discover};"
                    )
                    .unwrap();
                }
            }
            K::GainEnergy => {
                let n = self.action_amount(&action, i32::MAX.into(), "i32")?;
                let player = self.actor(&action)?;
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; g.change_energy(player_{id}, amount_{id});").unwrap();
            }
            K::BecomeMonarch => self.actor_action(&action, "g.become_monarch(player)", out)?,
            K::LoseGame => self.actor_action(
                &action,
                "g.lose_game(player); if g.state().is_over() { return Ok(()); }",
                out,
            )?,
            K::WinGame => self.actor_action(
                &action,
                "g.win_game(player); if g.state().is_over() { return Ok(()); }",
                out,
            )?,
            K::TakeExtraTurn
                if format!("{:?}", self.after)
                    .contains(&format!("ExtraTurn(ResultId({}))", result.0)) =>
            {
                let player = self.actor(&action)?;
                writeln!(out, "let turn_{id} = g.add_extra_turn({player});").unwrap();
                self.performed.insert(result.0, format!("turn_{id}"));
            }
            K::TakeExtraTurn => {
                let times = self.times(&action)?;
                let call = format!("let times = {times}; g.add_extra_turns(player, times)");
                self.actor_action(&action, &call, out)?;
            }
            K::PhaseOut => self.object_action(&action, "g.phase_out(&[object])", out)?,
            K::Prepare => self.object_action(&action, "g.become_prepared(object)", out)?,
            K::Flip => {
                let object = self.acted_object(action.object)?;
                writeln!(out, "if let Some(object) = {object} {{").unwrap();
                self.plan_code("keyword action Flip", "g.flip(object);", out);
                writeln!(out, "}}").unwrap();
            }
            K::TapOrUntap => {
                let player = self.actor(&action)?;
                self.tap_or_untap = true;
                let (nothing, rest) = match std::mem::take(&mut self.may_tap_or_untap) {
                    true => (", Label::Card(card, 2)", "1 => g.untap(object), _ => {}"),
                    false => ("", "_ => g.untap(object)"),
                };
                let call = format!("let card = g.card_id(&CARD); match {M}::choose::one_of(g, player_{id}, asker.source, vec![Label::Card(card, 0), Label::Card(card, 1){nothing}])? {{ 0 => g.tap(object), {rest} }}");
                writeln!(out, "let player_{id} = {player};").unwrap();
                self.object_action(&action, &call, out)?;
            }
            K::RollDie(sides) => {
                let player = self.actor(&action)?;
                writeln!(out, "let player_{id} = {player}; let value_{id} = i64::from(g.roll_die(player_{id}, {sides}).result);").unwrap();
                self.values.insert(result.0, format!("value_{id}"));
            }
            K::Venture => {
                let mut code = String::new();
                self.actor_action(&action, "g.venture(player)?", &mut code)?;
                let feature = "engine: venture into the dungeon reaches only Undercity";
                self.plan_code(feature, code.trim(), out);
            }
            K::RingTempts => {
                let mut code = String::new();
                let call = format!("{M}::ring::tempt(g, player, asker.source)?");
                self.actor_action(&action, &call, &mut code)?;
                self.plan_code("engine: no Ring temptation", code.trim(), out);
            }
            _ => self.keyword_action_beyond(id, &action, result, out)?,
        }
        Ok(())
    }
    /// What the rules define and `Game` does not have, as the call it will be.
    pub(super) fn plan_for_actor(
        &mut self,
        feature: &str,
        action: &Action,
        call: &str,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        self.actor_action(action, call, &mut code)?;
        self.plan_code(feature, code.trim(), out);
        Ok(())
    }
    fn plan_for_object(
        &mut self,
        feature: &str,
        action: &Action,
        call: &str,
        out: &mut String,
    ) -> Result<()> {
        let mut code = String::new();
        self.object_action(action, call, &mut code)?;
        self.plan_code(feature, code.trim(), out);
        Ok(())
    }
    fn keyword_action_beyond(
        &mut self,
        id: usize,
        action: &Action,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        use KeywordAction as K;
        match action.action {
            K::Blight => {
                let n = self.action_amount(action, u8::MAX.into(), "u8")?;
                let player = self.actor(action)?;
                writeln!(out, "let amount_{id}: u8 = {n}; let player_{id} = {player}; let blighted_{id}: Vec<ObjectId> = {M}::blight::in_effect(g, player_{id}, asker.source, amount_{id})?.into_iter().collect();").unwrap();
                self.moved.insert(result.0, format!("blighted_{id}"));
            }
            K::Recruit => {
                let token = format!("{M}::tokens::HUMAN_SOLDIER");
                if !self.extras.contains(&token) {
                    self.extras.push(token);
                }
                self.actor_action(
                    action,
                    &format!("{M}::recruit::recruit(g, player, asker.source)?"),
                    out,
                )?;
            }
            K::Saddle => self.object_action(action, "g.become_saddled(object)", out)?,
            K::RemoveAllDamage => self.object_action(action, "g.heal(object)", out)?,
            K::Harness => self.plan_for_object(
                "engine: no Harness",
                action,
                &format!("{M}::harness::harness(g, object)"),
                out,
            )?,
            K::Assimilate => self.plan_for_object(
                "engine: no Assimilate",
                action,
                &format!("{M}::assimilate::assimilate(g, asker.source, object)"),
                out,
            )?,
            K::UnlockDoor => self.plan_for_object(
                "engine: no effect picks a door",
                action,
                "g.unlock_a_door(object, asker.controller)?",
                out,
            )?,
            K::LockOrUnlockDoor => self.plan_for_object(
                "engine: no effect picks a door",
                action,
                "g.lock_or_unlock_a_door(object, asker.controller)?",
                out,
            )?,
            K::ReverseTurnOrder => self.plan_code(
                "engine: no reversed turn order",
                "g.reverse_turn_order();",
                out,
            ),
            K::EndCombat => {
                self.plan_code("engine: no effect ends combat", "g.end_combat()?;", out)
            }
            K::DrawGame => {
                self.plan_code("engine: no effect draws the game", "g.draw_game();", out)
            }
            K::ShuffleGraveyard => self.plan_for_actor(
                "engine: no order of a graveyard",
                action,
                "g.shuffle_graveyard(player)",
                out,
            )?,
            K::ReorderGraveyard => self.plan_for_actor(
                "engine: no order of a graveyard",
                action,
                "g.reorder_graveyard(player)?",
                out,
            )?,
            K::TakeExtraTurnAfterNext => self.plan_for_actor(
                "engine: no extra turn but the next",
                action,
                "g.add_extra_turn_after_next(player)",
                out,
            )?,
            K::TakeExtraTurnSkippingUntap => self.plan_for_actor(
                "engine: no extra turn but the next",
                action,
                "g.add_extra_turn_skipping(player, Step::Untap)",
                out,
            )?,
            K::DiscardAnyNumberAtRandom => self.plan_for_actor(
                "engine: no choice of how many to discard at random",
                action,
                &format!("{M}::hand::discard_any_number_at_random(g, player, asker.source)?"),
                out,
            )?,
            K::RollPlanarDie => self.plan_for_actor(
                "engine: no planar deck",
                action,
                "g.roll_planar_die(player)",
                out,
            )?,
            K::PlaySubgame => self.plan_code(
                "engine: no subgame",
                &format!("{M}::subgame::play(g, asker.source)?;"),
                out,
            ),
            K::Paradigm => {
                self.controlled_by_item(action)?;
                self.paradigm = true;
            }
            K::CollectEvidence => {
                let n = self.action_amount(action, u8::MAX.into(), "u8")?;
                let player = self.actor(action)?;
                self.helpers.insert("pay_question");
                writeln!(out, "let amount_{id}: u8 = {n}; let payer_{id} = {player}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, Cost {{ collect_evidence: amount_{id}, ..Cost::NONE }}, &PAY_QUESTION)?;").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            K::OpenAttraction => self.plan_for_actor(
                "engine: no Attractions",
                action,
                &format!("{M}::attractions::open(g, player)?"),
                out,
            )?,
            K::VisitAttractions => self.plan_for_actor(
                "engine: no Attractions",
                action,
                &format!("{M}::attractions::visit(g, player)?"),
                out,
            )?,
            K::GetTickets => {
                let n = self.action_amount(action, i32::MAX.into(), "i32")?;
                let call = format!("let amount = {n}; g.change_tickets(player, amount)");
                self.plan_for_actor("engine: no tickets", action, &call, out)?;
            }
            K::Awaken => {
                let n = self.action_amount(action, i16::MAX.into(), "i16")?;
                let call = format!(
                    "let amount = {n}; {M}::awaken::awaken(g, asker.source, object, amount)?"
                );
                self.plan_for_object("engine: no Awaken", action, &call, out)?;
            }
            K::Endure => {
                let n = self.action_amount(action, i16::MAX.into(), "i16")?;
                let call = format!(
                    "let amount = {n}; {M}::endure::endure(g, asker.controller, object, amount)?"
                );
                self.plan_for_object("engine: no Endure", action, &call, out)?;
            }
            K::Detain => self.plan_for_object(
                "engine: no Detain",
                action,
                &format!("{M}::detain::detain(g, asker.source, object)"),
                out,
            )?,
            K::TurnFaceDown => self.plan_for_object(
                "engine: no effect turns a permanent face down",
                action,
                "g.turn_face_down(object)",
                out,
            )?,
            K::BecomeBlocked => self.plan_for_object(
                "engine: no effect makes a creature blocked",
                action,
                "g.become_blocked(object)",
                out,
            )?,
            K::RemoveFromCombatUnblocking => self.plan_for_object(
                "engine: no effect removes from combat",
                action,
                "g.remove_from_combat_and_unblock(object)",
                out,
            )?,
            K::PhaseIn => self.plan_for_object(
                "engine: no effect phases a permanent in",
                action,
                "g.phase_in(&[object])",
                out,
            )?,
            K::Flip => self.plan_for_object(
                "engine: no flipping a permanent",
                action,
                "g.flip(object)?",
                out,
            )?,
            K::TimeTravel => self.plan_for_actor(
                "engine: no Time Travel",
                action,
                &format!("{M}::time_travel::time_travel(g, player, asker.source)?"),
                out,
            )?,
            K::Forage => self.plan_for_actor(
                "engine: no Forage",
                action,
                &format!("{M}::forage::forage(g, player, asker.source)?"),
                out,
            )?,
            K::Ascend => self.plan_for_actor(
                "engine: no effect gives the city's blessing",
                action,
                "g.ascend(player)",
                out,
            )?,
            K::Epic => {
                self.controlled_by_item(action)?;
                self.plan_code(
                    "engine: no Epic",
                    &format!("{M}::epic::epic(g, item)?;"),
                    out,
                );
            }
            K::Intensify => {
                let n = match action.amount {
                    Some(_) => self.action_amount(action, i16::MAX.into(), "i16")?,
                    None => "1".to_string(),
                };
                let call = format!(
                    "let amount = {n}; {M}::intensity::intensify(g, player, asker.source, amount)"
                );
                self.plan_for_actor("engine: no Intensity", action, &call, out)?;
            }
            K::Fateseal => {
                let n = self.action_amount(action, u32::MAX.into(), "u32")?;
                let object = action.object.map(|o| self.object(o)).transpose()?;
                let call = format!(
                    "let amount = {n}; {M}::fateseal::fateseal(g, player, {}, amount)?",
                    object.unwrap_or_else(|| "None".into())
                );
                self.plan_for_actor("engine: no Fateseal", action, &call, out)?;
            }
            K::Skip(part) => {
                let call = format!("g.skip_next(player, Skipped::{part:?})");
                self.plan_for_actor("engine: no skipped step or phase", action, &call, out)?;
            }
            K::AdditionalPhase(phases) => {
                let code = format!("g.add_phases(AddedPhases::{phases:?});");
                self.plan_code("engine: no additional phase but combat", &code, out);
            }
            K::Planeswalk => self.plan_for_actor(
                "engine: no planar deck",
                action,
                "g.planeswalk(player)",
                out,
            )?,
            K::LoseUnspentMana => self.plan_for_actor(
                "engine: no effect empties a mana pool",
                action,
                "g.empty_mana_pool(player)",
                out,
            )?,
            K::DoubleUnspentMana => self.plan_for_actor(
                "engine: no effect empties a mana pool",
                action,
                "g.double_mana_pool(player)",
                out,
            )?,
            other => return self.stated_action(other, action, out),
        }
        Ok(())
    }
}
