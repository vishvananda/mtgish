//! What a resolving effect does past the arms of `effects` and `lane_i`: the
//! operations the first round left, each the code Cantrip runs today, or the code
//! it will run once the requirement it names exists.
use super::*;

const M: &str = "cantrip_mechanics";

/// The prompt of a choice that nothing does anything to by name: "chooses three
/// permanents, then sacrifices the rest" keeps what it chooses, and any other
/// choice a later instruction reads is a plain choice.
pub(super) fn plain_purpose(result: &ResultId, rest: &[Instruction]) -> Option<&'static str> {
    let unselected = format!("Unselected(ResultId({}))", result.0);
    let earlier = "Earlier { what: Selected";
    rest.iter().find_map(|instruction| {
        let text = format!("{instruction:?}");
        if text.contains(&unselected) {
            Some("Keep")
        } else if text.contains(earlier) {
            Some("ChooseAsEnters")
        } else {
            None
        }
    })
}

impl Compiler {
    /// The one player an instruction names, or the gap `feature`.
    fn one_actor(&self, actor: &PlayerSet, feature: &str) -> Result<String> {
        let PlayerSet::One(player) = actor else {
            return Err(gap(feature));
        };
        Ok(self.fetch(self.certain_player(player)?))
    }
    /// What `operation` has no arm for of its own.
    pub(super) fn resolving(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let unsupported = || gap(format!("operation {}", form(op)));
        match op {
            // "Those creatures": what the set holds now, whatever happens to it.
            Operation::NoteObjects { objects } => {
                writeln!(
                    out,
                    "let noted_{id}: Vec<ObjectId> = {};",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.selections.insert(result.0, format!("noted_{id}"));
            }
            Operation::DealDamageFromEach {
                sources,
                recipients,
                amount,
            } => self.damage_from_each(id, sources, recipients, amount, result, out)?,
            Operation::RevealFromHand {
                actor,
                cards,
                count,
                at_random,
            } => self.reveal_from_hand(id, actor, cards, count.as_ref(), *at_random, result, out)?,
            Operation::ChooseAtRandom { kind, options } => {
                self.choose_at_random(id, *kind, options, out)?
            }
            Operation::Exchange {
                kind,
                first,
                second,
                until,
            } => self.exchange(id, kind, first, second, until.as_ref(), out)?,
            Operation::Wish {
                actor,
                filter,
                reveal,
            } => {
                let player = self.one_actor(actor, "operation Wish")?;
                let test = self.filter(filter, &Candidate::of("id"))?;
                let wished = self.fetch(format!("q.outside_the_game(player_{id}).into_iter().filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
                let bring = if *reveal {
                    "bring_into_hand_revealed"
                } else {
                    "bring_into_hand"
                };
                writeln!(out, "let player_{id} = {player}; let wished_{id} = {wished}; let chosen_{id} = {M}::choose::cards(g, player_{id}, asker.source, Purpose::PutIntoHand, wished_{id}, 1, true)?; let brought_{id} = g.{bring}(&chosen_{id});").unwrap();
                self.moved.insert(result.0, format!("brought_{id}"));
                self.left_behind(result, format!("chosen_{id}.clone()"));
            }
            Operation::SeparatePiles {
                separator,
                chooser,
                objects,
            } => {
                let cards = self.fetch(self.objects(objects)?);
                let separator = self.fetch(self.certain_player(separator)?);
                let chooser = self.fetch(self.certain_player(chooser)?);
                writeln!(out, "let cards_{id} = {cards}; let separator_{id} = {separator}; let chooser_{id} = {chooser}; let (first_{id}, second_{id}) = {M}::choose::separate(g, Some(separator_{id}), asker.source, cards_{id})?; let (pile_{id}, other_{id}) = {M}::choose::pile(g, chooser_{id}, asker.source, first_{id}, second_{id})?;").unwrap();
                self.selections.insert(result.0, format!("pile_{id}"));
                self.unchosen.insert(result.0, format!("other_{id}"));
            }
            Operation::DrawUpTo { players, amount } => {
                let most = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let most_{id} = {}; let mut done_{id}: i64 = 0; for player in {} {{ let n = {M}::choose::number(g, player, asker.source, 0, most_{id}, \"Draw\")?; let before = g.history().drawn(player) as i64; g.draw(player, n); done_{id} += g.history().drawn(player) as i64 - before; }}", self.fetch(most), self.fetch(self.players(players)?)).unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
            }
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } => {
                let keep = self.number(keep)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let objects_{id} = {}; let keep_{id} = {}; let mut picks_{id}: Vec<(PlayerId, ObjectId)> = Vec::new(); for player in {} {{ let own: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.controller(o) == player).collect(); let kept = {M}::choose::select(g, player, asker.source, Purpose::Keep, own.clone(), {M}::select::Selection::of({M}::select::Count::AsManyAsCan(keep_{id}), &[]))?; for o in own {{ if !kept.contains(&o) && g.can_sacrifice(player, o) {{ picks_{id}.push((player, o)); }} }} }} let sacrificed_{id} = g.sacrifice_each(&picks_{id});", self.fetch(self.objects(objects)?), self.fetch(keep), self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("sacrificed_{id}"));
                self.left_behind(result, effects::gone(&format!("objects_{id}")));
            }
            Operation::DiscardAllBut { players, keep } => {
                let keep = self.number(keep)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let keep_{id} = {}; let mut cards_{id}: Vec<ObjectId> = Vec::new(); for player in {} {{ let hand = g.state().zone(Zone::hand(player)).to_vec(); let extra = (hand.len() as u32).saturating_sub(keep_{id}); cards_{id}.extend({M}::choose_from_hand(g, player, asker.source, extra)?); }} let discarded_{id} = g.discard_together(&cards_{id});", self.fetch(keep), self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("discarded_{id}"));
                self.left_behind(result, format!("discarded_{id}.clone()"));
            }
            Operation::DrawThenDiscard {
                players,
                draw,
                discard,
            } => {
                let draw = self.number(draw)?.amount(u32::MAX.into(), "u32")?;
                let discard = self.number(discard)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let draw_{id} = {}; let discard_{id} = {}; for player in {} {{ g.draw(player, draw_{id}); let cards = {M}::choose_from_hand(g, player, asker.source, discard_{id})?; g.discard_together(&cards); }}", self.fetch(draw), self.fetch(discard), self.fetch(self.players(players)?)).unwrap();
            }
            Operation::PutCountersEach {
                amount,
                kind,
                objects,
                up_to: false,
            } => {
                let kind = counter_kind(kind)?;
                let saved = self.affected_object.replace("object");
                let amount = self.number(amount).and_then(|n| n.amount(i16::MAX.into(), "i16"));
                self.affected_object = saved;
                let amount = self.fetch(amount?);
                writeln!(out, "let objects_{id} = {}; let mut counters_{id}: Vec<(ObjectId, CounterKind, i16)> = Vec::new(); for &object in &objects_{id} {{ let amount = {amount}; counters_{id}.push((object, {kind}, amount)); }} g.add_counters_together(&counters_{id});", self.acted_on(objects)?).unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
            }
            Operation::PutCountersCapped {
                amount,
                kind,
                object,
                at_most,
            } => {
                let kind = counter_kind(kind)?;
                let amount = self.number(amount)?.amount(i16::MAX.into(), "i16")?;
                let cap = self.number(at_most)?.amount(i16::MAX.into(), "i16")?;
                writeln!(out, "let object_{id} = {}; let amount_{id} = {}; let cap_{id} = {}; if let Some(object) = object_{id} {{ let room = cap_{id}.saturating_sub(g.state().obj(object).counters({kind})).max(0); g.add_counters(object, {kind}, amount_{id}.min(room)); }}", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(cap)).unwrap();
            }
            Operation::Repeat { times, body } => {
                let runs = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let runs_{id} = {}; for _ in 0..runs_{id} {{", self.fetch(runs)).unwrap();
                let scope = self.results();
                let done = self.operation(body, result, out);
                self.restore_results(scope);
                done?;
                writeln!(out, "}}").unwrap();
            }
            Operation::AttachChosen {
                actor,
                attachments,
                count,
                hosts,
            } => {
                let ObjectSet::One(host) = hosts else {
                    return Err(unsupported());
                };
                let player = self.one_actor(actor, "operation AttachChosen")?;
                let count = self.fetch(self.select_count(count)?);
                writeln!(out, "let player_{id} = {player}; let eligible_{id} = {}; let host_{id} = {}; let count_{id} = {count}; let chosen_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::Attach, eligible_{id}, {M}::select::Selection::of(count_{id}, &[]))?; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for &attachment in &chosen_{id} {{ g.attach(attachment, host); }} }}); }}", self.acted_on(attachments)?, self.fetch(self.object(host)?)).unwrap();
                self.selections.insert(result.0, format!("chosen_{id}"));
            }
            Operation::FightEach { fighters, others } => {
                writeln!(out, "let fighters_{id} = {}; let others_{id} = {}; for &first in &fighters_{id} {{ for &second in &others_{id} {{ g.fight(first, second); }} }}", self.fetch(self.objects(fighters)?), self.fetch(self.objects(others)?)).unwrap();
            }
            Operation::FlipCoins { actor, count } => {
                let player = self.one_actor(actor, "operation FlipCoins")?;
                let count = self.number(count)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let player_{id} = {player}; let count_{id} = {}; let mut done_{id}: i64 = 0; for _ in 0..count_{id} {{ if g.flip_coin(player_{id}) {{ done_{id} += 1; }} }}", self.fetch(count)).unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
            }
            Operation::EachObject { objects, action } => {
                writeln!(out, "for each_object in {} {{", self.fetch(self.objects(objects)?)).unwrap();
                let saved = self.affected_object.replace("each_object");
                let scope = self.results();
                let done = self.operation(action, result, out);
                self.restore_results(scope);
                self.affected_object = saved;
                done?;
                writeln!(out, "}}").unwrap();
            }
            other => return self.resolving_rest(id, other, result, out),
        }
        Ok(())
    }
    /// The requirements: code that waits on Cantrip.
    fn resolving_rest(
        &mut self,
        _id: usize,
        op: &Operation,
        _result: &ResultId,
        _out: &mut String,
    ) -> Result<()> {
        Err(gap(format!("operation {}", form(op))))
    }
    /// "Each of them deals damage": one event, with what each deals read for it.
    fn damage_from_each(
        &mut self,
        id: usize,
        sources: &ObjectSet,
        recipients: &Recipients,
        amount: &Expr,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let saved = self.affected_object.replace("source");
        let amount = self.number(amount).and_then(|n| n.amount(u16::MAX.into(), "u16"));
        let targets = self.recipients(recipients);
        self.affected_object = saved;
        let (amount, targets) = (self.fetch(amount?), self.fetch(targets?));
        writeln!(out, "let sources_{id} = {}; let mut batch_{id}: Vec<(ObjectId, Target, u16)> = Vec::new(); for &source in &sources_{id} {{ let amount = {amount}; for t in {targets} {{ batch_{id}.push((source, t, amount)); }} }} let dealt_{id} = g.damage_together(&batch_{id}); let done_{id}: i64 = dealt_{id}.iter().map(|&n| i64::from(n)).sum();", self.fetch(self.objects(sources)?)).unwrap();
        self.performed.insert(result.0, format!("done_{id}"));
        Ok(())
    }
    /// "Reveals a card from their hand": at random, of their choice, or all.
    #[allow(clippy::too_many_arguments)]
    fn reveal_from_hand(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        cards: &ObjectFilter,
        count: Option<&Expr>,
        at_random: bool,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let player = self.one_actor(actor, "operation RevealFromHand")?;
        let test = self.filter(cards, &Candidate::of("id"))?;
        let hand = self.fetch(format!("q.state().zone(Zone::hand(player_{id})).iter().copied().filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
        writeln!(out, "let player_{id} = {player}; let mut pool_{id}: Vec<ObjectId> = {hand};").unwrap();
        match (count, at_random) {
            (None, _) => writeln!(out, "let shown_{id} = pool_{id};").unwrap(),
            (Some(n), true) => {
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "usize")?);
                writeln!(out, "let count_{id} = {n}; let mut shown_{id}: Vec<ObjectId> = Vec::new(); while shown_{id}.len() < count_{id} && !pool_{id}.is_empty() {{ let at = g.choose_at_random(pool_{id}.len()); shown_{id}.push(pool_{id}.remove(at)); }}").unwrap();
            }
            (Some(n), false) => {
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?);
                writeln!(out, "let count_{id} = {n}; let shown_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::Reveal, pool_{id}, {M}::select::Selection::of({M}::select::Count::AsManyAsCan(count_{id}), &[]))?;").unwrap();
            }
        }
        writeln!(out, "g.reveal(player_{id}, &shown_{id});").unwrap();
        self.revealed = true;
        if matches!(count, Some(Expr::Constant(1))) {
            self.single.insert(result.0);
        }
        self.selections.insert(result.0, format!("shown_{id}"));
        Ok(())
    }
    /// "Chosen at random", which the engine records as one (`Event::ChosenAtRandom`).
    fn choose_at_random(
        &mut self,
        id: usize,
        kind: ValueKind,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let entering = self.pending == Pending::Entering;
        let local = format!("chosen_{id}");
        match (kind, options) {
            (ValueKind::Player, [Operand::Players(players)]) => {
                let players = self.fetch(self.players(players)?);
                writeln!(out, "let pool_{id}: Vec<PlayerId> = {players}; if pool_{id}.is_empty() {{ return Ok(()); }} let {local} = pool_{id}[g.choose_at_random(pool_{id}.len())];").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_player = Some({local});").unwrap();
                }
            }
            (ValueKind::Number, options) => {
                let number = match options {
                    [Operand::Comparison(Comparison::GreaterOrEqual, least), Operand::Comparison(Comparison::LessOrEqual, most)] =>
                    {
                        let least = self.number(least)?.amount(u32::MAX.into(), "u32")?;
                        let most = self.number(most)?.amount(u32::MAX.into(), "u32")?;
                        format!("{{ let least = {}; let most = {}; least + g.choose_at_random((most.saturating_sub(least) + 1) as usize) as u32 }}", self.fetch(least), self.fetch(most))
                    }
                    numbers => {
                        let numbers = numbers
                            .iter()
                            .map(|n| match n {
                                Operand::Integer(n) if *n >= 0 => Ok(format!("{n}u32")),
                                _ => Err(gap("operation ChooseAtRandom")),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        format!("{{ let pool = [{}]; pool[g.choose_at_random(pool.len())] }}", numbers.join(", "))
                    }
                };
                writeln!(out, "let {local}: u32 = {number};").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_number = Some({local});").unwrap();
                }
            }
            _ => return Err(gap("operation ChooseAtRandom")),
        }
        self.chosen.push((kind, local));
        Ok(())
    }
    /// Two things trade places (CR 701.12).
    fn exchange(
        &mut self,
        id: usize,
        kind: &ExchangeKind,
        first: &Operand,
        second: &Operand,
        until: Option<&crate::ir::Duration>,
        out: &mut String,
    ) -> Result<()> {
        let unsupported = || gap("operation Exchange");
        match (kind, first, second, until) {
            // Each gains or loses what makes the two equal (701.12c, 119.5).
            (ExchangeKind::LifeTotals, Operand::Player(a), Operand::Player(b), None) => {
                writeln!(out, "let (first_{id}, second_{id}) = ({}, {}); if let (Some(first), Some(second)) = (first_{id}, second_{id}) {{ let (a, b) = (g.state().player(first).life, g.state().player(second).life); cantrip_api::instruction(g, |g| {{ g.set_life_total(first, b); g.set_life_total(second, a); }}); }}", self.fetch(self.player(a)?), self.fetch(self.player(b)?)).unwrap();
            }
            // Control of two permanents, for good (701.12b).
            (ExchangeKind::Control, Operand::Object(a), Operand::Object(b), None) => {
                writeln!(out, "let (first_{id}, second_{id}) = ({}, {}); if let (Some(first), Some(second)) = (first_{id}, second_{id}) {{ g.exchange_control(asker.source, first, second); }}", self.fetch(self.object(a)?), self.fetch(self.object(b)?)).unwrap();
            }
            _ => return Err(unsupported()),
        }
        Ok(())
    }
}

