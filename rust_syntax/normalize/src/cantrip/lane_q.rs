//! The last of the generator: what each player's run of a walk did, read back
//! by the player, and the forms that were left when everything else had an arm.
use super::*;

/// A binding for a walk the AST does not hold. Lowering numbers its own from 0.
const OWN_SHARE_BINDING: u32 = 1 << 20;

/// "The second chosen type": the choice of two a filter's `NthChosen(0)` reads the first of.
pub(super) fn second_chosen_land_type(land: &Operand) -> Option<String> {
    match land {
        Operand::Term { family, form, .. }
            if family == "LandTypeVariable" && form == "TheSecondChosenLandType" =>
        {
            Some("q.nth_chosen_value(asker, 1).subtype()".to_string())
        }
        _ => None,
    }
}

/// Whether a later instruction asks how many cards a surveil left on top.
pub(super) fn reads_kept_on_top(after: &[Instruction], result: &ResultId) -> bool {
    crate::result_reads::ResultReads::instructions(after, result).kept_on_top
}

/// What a surveil does before and after, to say how many of the cards stayed.
/// Those that left may not have reached the graveyard (CR 614), so ask the library.
pub(super) fn looked_and_kept(id: usize, kept: bool) -> (String, String) {
    if !kept {
        return (String::new(), String::new());
    }
    (
        format!("let looked_{id} = {{ let q: &dyn Query = g; cantrip_mechanics::top_of_library(q, player_{id}, amount_{id}) }}; "),
        format!(" let kept_{id} = {{ let q: &dyn Query = g; looked_{id}.iter().filter(|&&card| q.state().obj(card).zone.is(ZoneKind::Library)).count() as i64 }};"),
    )
}

/// The result a token count reads as each player's own, if that is all it is.
fn own_share(tokens: &[TokenSpec]) -> Option<&ResultId> {
    match tokens {
        [TokenSpec {
            amount: Expr::Performed(result),
            recipe: TokenRecipe::Defined(_),
            ..
        }] => Some(result),
        _ => None,
    }
}

/// The token counts `rest` reads as each player's own, instruction by instruction.
fn own_share_reads(rest: &[Instruction], found: &mut BTreeSet<u32>) {
    for instruction in rest {
        if let Instruction::Effect {
            operation:
                Operation::CreateTokens {
                    players: PlayerSet::All,
                    tokens,
                    ..
                },
            ..
        } = instruction
        {
            found.extend(own_share(tokens).map(|result| result.0));
        }
    }
}

impl Compiler {
    /// What a loop's walks leave for a reader after the loop: a run a player
    /// has in each pass of it adds to their own share.
    pub(super) fn hoist_shares(&mut self, rest: &[Instruction], out: &mut String) {
        let mut read = BTreeSet::new();
        own_share_reads(rest, &mut read);
        for result in read {
            let id = self.fresh();
            writeln!(
                out,
                "let mut shares_{id}: Vec<(PlayerId, i64)> = Vec::new();"
            )
            .unwrap();
            self.notes.shares.insert(result, format!("shares_{id}"));
        }
    }
    /// One player's run of a walk adds what it did to their share, if a later
    /// instruction reads it player by player.
    pub(super) fn tally_share(
        &mut self,
        result: u32,
        local: &str,
        bound: &str,
        rest: &[Instruction],
        declared: &mut String,
        tail: &mut String,
    ) {
        let mut read = BTreeSet::new();
        own_share_reads(rest, &mut read);
        if !read.contains(&result) && !self.notes.shares.contains_key(&result) {
            return;
        }
        if !self.notes.shares.contains_key(&result) {
            let id = self.fresh();
            writeln!(
                declared,
                "let mut shares_{id}: Vec<(PlayerId, i64)> = Vec::new();"
            )
            .unwrap();
            self.notes.shares.insert(result, format!("shares_{id}"));
        }
        let shares = &self.notes.shares[&result];
        writeln!(tail, "{shares}.push(({bound}, ({local}) as i64));").unwrap();
    }
    /// Whether a token count is each player's own share of an earlier walk.
    pub(super) fn reads_own_share(&self, tokens: &[TokenSpec]) -> bool {
        own_share(tokens).is_some_and(|result| self.notes.shares.contains_key(&result.0))
    }
    /// "Each player creates a token for each ... they paid this way": a walk of
    /// its own over the players, reading what each one's run did.
    pub(super) fn own_share_tokens(
        &mut self,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        out: &mut String,
    ) -> Result<()> {
        let result = own_share(tokens).expect("a read of an own share");
        let shares = self.notes.shares[&result.0].clone();
        let binding = Binding(OWN_SHARE_BINDING + result.0);
        let saved = self.performed.insert(
            result.0,
            format!(
                "{shares}.iter().filter(|share| share.0 == bound_{}).map(|share| share.1).sum::<i64>()",
                binding.0
            ),
        );
        let step = self.fresh();
        let body = Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(OWN_SHARE_BINDING + step as u32),
                operation: Operation::CreateTokens {
                    players: PlayerSet::One(PlayerRef::Bound(binding.clone())),
                    tokens: tokens.to_vec(),
                    entry: entry.clone(),
                },
            }],
        };
        let walked = self.each_player(
            lane_i::EachPlayer {
                players: &PlayerSet::All,
                binding: &binding,
                starting_with: None,
                in_chosen_direction: false,
                body: &body,
                rest: &[],
            },
            out,
        );
        match saved {
            Some(saved) => self.performed.insert(result.0, saved),
            None => self.performed.remove(&result.0),
        };
        walked
    }
    /// "If a number was chosen this way" reads the number a choice bound.
    pub(super) fn chosen_number_performed(&mut self, kind: ValueKind, result: &ResultId) {
        let local = self.chosen.iter().rev().find(|(k, _)| *k == kind);
        if let (ValueKind::Number, Some((_, local))) = (kind, local) {
            let read = format!("i64::from({local})");
            self.performed.insert(result.0, read);
        }
    }
    /// "That player ... until that player's next turn": the walk that puts the rule
    /// on them already holds them, though the reference alone might name nobody.
    pub(super) fn note_own_turn(&mut self, players: &PlayerSet, duration: &crate::ir::Duration) {
        if let (PlayerSet::One(on), crate::ir::Duration::UntilNextTurn(until)) = (players, duration)
        {
            let same = format!("{on:?}") == format!("{until:?}");
            if same && self.certain_player(until).is_err() {
                self.notes.until_own_turn = Some(format!("{on:?}"));
            }
        }
    }
    /// A delayed trigger on an activation reads "that ability" of the event it
    /// waited for, as lowering reads it (`delayed_reads_event`).
    pub(super) fn reads_stack_object_of(&mut self, event: &EventSpec, frame: Frame) {
        if matches!(
            event,
            EventSpec::Activated { .. } | EventSpec::SpellOrActivation { .. }
        ) {
            self.frame.stack_object = frame.stack_object;
            self.frame.activated = frame.activated;
            self.frame.spell_or_ability = frame.spell_or_ability;
        }
    }
    /// "Counter all spells and all abilities they control": what is on the stack,
    /// spell or ability, by whose it is. Those still on the stack weren't countered.
    pub(super) fn counter_all_on_stack(
        &mut self,
        id: usize,
        players: &PlayerSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let theirs = self.fetch(self.players(players)?);
        writeln!(
            out,
            "let players_{id} = {theirs}; let items_{id}: Vec<(ObjectId, usize)> = {{ let stack = &g.state().stack; stack.iter().enumerate().filter(|(_, e)| players_{id}.contains(&e.controller)).map(|(at, e)| (e.object, at)).collect() }}; g.counter_items(&items_{id}); let mut objects_{id}: Vec<ObjectId> = items_{id}.iter().map(|item| item.0).collect(); for left in g.state().stack.iter() {{ if let Some(at) = objects_{id}.iter().position(|o| *o == left.object) {{ objects_{id}.remove(at); }} }}"
        )
        .unwrap();
        self.selections.insert(result.0, format!("objects_{id}"));
        Ok(())
    }
    /// "Put the same number and kind of counters on ...", of those the event put.
    pub(super) fn duplicate_what_the_event_put(
        &mut self,
        id: usize,
        to: &ObjectSet,
        copies: CounterCopies,
        out: &mut String,
    ) -> Result<()> {
        let amount = match copies {
            CounterCopies::All => "amount",
            CounterCopies::OneOfEachKind => "1i16",
            _ => return Err(gap("counters the event put, of some kinds")),
        };
        let to = self.fetch(self.objects(to)?);
        let code = format!("let to_{id}: Vec<ObjectId> = {to}; let put_{id}: Vec<(CounterKind, i16)> = g.counters_put_by(item); let puts: Vec<(ObjectId, CounterKind, i16)> = to_{id}.iter().flat_map(|&o| put_{id}.iter().map(move |&(kind, amount)| (o, kind, {amount}))).collect(); g.add_counters_together(&puts);");
        self.plan_code("counters the event put", &code, out);
        Ok(())
    }
    /// "For each player, destroy up to one nonbasic land that player controls": one
    /// choice for each, among what is theirs. One asks all the choices of one player.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_from_each(
        &mut self,
        chooser: &str,
        objects: &ObjectSet,
        players: &PlayerSet,
        each: &Count,
        purpose: &str,
        out: &mut String,
    ) -> Result<String> {
        let each = self.offered_count(each);
        let count = self.fetch(self.select_count(&each)?);
        let pool = self.fetch(self.objects(objects)?);
        let players = self.fetch(self.players(players)?);
        let local = format!("selected_{}", self.fresh());
        writeln!(out, "let {local}_player = {chooser}; let {local}_pool: Vec<ObjectId> = {pool}; let mut {local}: Vec<ObjectId> = Vec::new(); for each_player in {players} {{ let candidates: Vec<ObjectId> = {{ let q: &dyn Query = g; {local}_pool.iter().copied().filter(|&o| (if q.state().obj(o).zone.is(ZoneKind::Battlefield) {{ q.controller(o) }} else {{ q.state().obj(o).owner }}) == each_player).collect() }}; let count = {count}; {local}.extend(cantrip_mechanics::choose::select(g, {local}_player, asker.source, Purpose::{purpose}, candidates, cantrip_mechanics::select::Selection::of(count, &[]))?); }}").unwrap();
        self.open_guard(&local, out);
        Ok(local)
    }
    /// "Instead create that many tokens that are copies of it": the same event, the
    /// tokens now each a copy of its copiable values (CR 707.2).
    pub(super) fn tokens_become_copies(
        &mut self,
        copies: u32,
        tokens: &[TokenSpec],
        frame: Frame,
    ) -> Result<Option<String>> {
        let [TokenSpec {
            amount: Expr::EventAmount,
            recipe:
                TokenRecipe::Copy {
                    object,
                    exceptions,
                },
            ..
        }] = tokens
        else {
            return Ok(None);
        };
        if copies != 0
            || !exceptions.is_empty()
            || matches!(object, ObjectRef::Result { .. })
        {
            return Ok(None);
        }
        let object = self.matching(frame, |c| c.object(object))?;
        Ok(Some(format!(
            "if let Some(object) = {object} {{ let values = q.copiable_values(object); t.token = values.card; t.copy = Some(values); }} "
        )))
    }
}
