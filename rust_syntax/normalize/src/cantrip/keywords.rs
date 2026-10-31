//! Keyword actions the rules define (CR 701): each is what `Game` or
//! `cantrip_mechanics` calls it, so this says only who and what it is given.
use super::*;

const M: &str = "cantrip_mechanics";
/// The card's first delayed ability, which `earthbend::effect` gives the land.
const EARTHBEND_RETURN: &str = "cantrip_mechanics::earthbend::RETURN";

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
    fn actor(&self, action: &Action) -> Result<String> {
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
    fn acted_object(&self, object: Option<&ObjectRef>) -> Result<String> {
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
    fn action_amount(&mut self, action: &Action, max: i64, ty: &str) -> Result<String> {
        let amount = action
            .amount
            .ok_or_else(|| gap("keyword action without its amount"))?;
        let number = self.number(amount)?.amount(max.into(), ty)?;
        Ok(self.fetch(number))
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
            K::Amass | K::Connive | K::Earthbend | K::Monstrosity | K::Discover | K::GainEnergy
        );
        if action.amount.is_some() && !counted {
            return Err(gap(format!(
                "keyword action {:?} with an amount",
                action.action
            )));
        }
        match action.action {
            K::Transform if self.stack_item => {
                self.object_action(&action, "g.transform_by(object, item)?", out)?
            }
            K::Transform => self.object_action(&action, "g.transform(object)?", out)?,
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
            K::CopySpell => {
                let copier = self.actor(&action)?;
                let spell = self.acted_object(action.object)?;
                writeln!(out, "let copier_{id} = {copier}; if let Some(spell) = {spell} {{ g.copy_spell(spell, copier_{id}, true)?; }}").unwrap();
            }
            K::Investigate => {
                let clue = format!("{M}::tokens::CLUE");
                if !self.extras.contains(&clue) {
                    self.extras.push(clue);
                }
                let call = format!("{M}::tokens::investigate(g, player)?");
                self.actor_action(&action, &call, out)?;
            }
            K::Proliferate => {
                self.actor_action(&action, "g.proliferate(player, asker.source)?", out)?
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
                    supertypes: Vec::new(),
                    card_types: vec![crate::mtg_types::CardType::Creature],
                    subtypes: vec![subtype, S::Army],
                    power_toughness: Some((Expr::Constant(0), Expr::Constant(0))),
                    abilities: Vec::new(),
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
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; {M}::discover::discover(g, player_{id}, amount_{id})?;").unwrap();
            }
            K::GainEnergy => {
                let n = self.action_amount(&action, i32::MAX.into(), "i32")?;
                let player = self.actor(&action)?;
                writeln!(out, "let amount_{id} = {n}; let player_{id} = {player}; g.change_energy(player_{id}, amount_{id});").unwrap();
            }
            K::BecomeMonarch => self.actor_action(&action, "g.become_monarch(player)", out)?,
            K::LoseGame => self.actor_action(&action, "g.lose_game(player)", out)?,
            K::WinGame => self.actor_action(&action, "g.win_game(player)", out)?,
            K::TakeExtraTurn => self.actor_action(&action, "g.add_extra_turns(player, 1)", out)?,
            K::PhaseOut => self.object_action(&action, "g.phase_out(&[object])", out)?,
            K::Prepare => self.object_action(&action, "g.become_prepared(object)", out)?,
            K::TapOrUntap => {
                let player = self.actor(&action)?;
                self.tap_or_untap = true;
                let call = format!("let card = g.card_id(&CARD); match {M}::choose::one_of(g, player_{id}, asker.source, vec![Label::Card(card, 0), Label::Card(card, 1)])? {{ 0 => g.tap(object), _ => g.untap(object) }}");
                writeln!(out, "let player_{id} = {player};").unwrap();
                self.object_action(&action, &call, out)?;
            }
            K::RollDie(sides) => {
                let player = self.actor(&action)?;
                writeln!(out, "let player_{id} = {player}; let value_{id} = i64::from(g.roll_die(player_{id}, {sides}).result);").unwrap();
                self.values.insert(result.0, format!("value_{id}"));
            }
            K::Venture => {
                return Err(gap(
                    "engine: venture into the dungeon reaches only Undercity",
                ))
            }
            K::RingTempts => return Err(gap("engine: no Ring temptation")),
            other => return Err(gap(format!("keyword action {other:?}"))),
        }
        Ok(())
    }
}
