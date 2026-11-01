//! Reads and rules that needed checking against Cantrip one at a time. An object
//! state is a read of what Cantrip records. Where it records nothing the read is
//! named as a requirement, and the file holds `false` in its place, so the code
//! around it stays the card's own.
use super::*;

/// A read with no record behind it: `code` names the field or query Cantrip
/// should add, and `false` stands in until it does.
fn unrecorded(state: &str, code: String) -> String {
    planned::planned_value(&format!("object state {state}"), "false", &code)
}

impl Compiler {
    /// `&'static CardDef` of the `CardId` that `card` reads, by either reader.
    pub(super) fn card_row(&self, card: &str) -> String {
        match self.reader {
            Reader::Layer => format!("q.cards().get({card})"),
            _ => format!("q.card({card})"),
        }
    }
    /// `ManaCost` of the object, by either reader.
    pub(super) fn cost_of(&self, id: &str, chars: &str) -> String {
        match self.reader {
            Reader::Layer => format!(
                "cantrip_api::mana_cost_of(&|card| q.cards().get(card), q.state().obj({id}), &{chars})"
            ),
            _ => format!("q.mana_cost({id})"),
        }
    }
    /// Whether the object was cast by a way of its card that passes `test`,
    /// over `w: &WayDef`. The same read as `warp::was_cast_for_warp`, which a
    /// layer can't call.
    fn cast_by_way(&self, id: &str, test: &str) -> String {
        let card = self.card_row("o.card");
        format!(
            "{{ let o = q.state().obj({id}); o.cast_way > 0 && {card}.ways.get(o.cast_way as usize - 1).is_some_and(|w| {test}) }}"
        )
    }
    /// The states `read::object_state` doesn't answer from the object's row alone.
    pub(super) fn state_rest(&self, state: &ObjectState, c: &Candidate) -> Result<String> {
        use ObjectState as S;
        let (id, chars) = (c.id.as_str(), c.chars.as_str());
        let layer = self.reader == Reader::Layer;
        let printed = self.card_row(&format!("{chars}.card"));
        let own = self.card_row(&format!("q.state().obj({id}).card"));
        let effect = |kind: &str| {
            format!("q.state().effects.iter().any(|e| e.affected == {id} && e.kind == EffectKind::{kind})")
        };
        let sides = |test: &str| {
            format!("{chars}.power.zip({chars}.toughness).is_some_and(|(p, t)| {test})")
        };
        Ok(match state {
            // CR 701.15a: an effect, so it outlasts the abilities of what it's on.
            S::Goaded => effect("Goaded"),
            S::Harnessed => effect("Harnessed"),
            // CR 702.166b, and the same optional cost as CR 702.194b.
            S::Bargained | S::CostPaid(CastCost::Teamwork) => {
                format!("q.state().obj({id}).paid > 0")
            }
            S::KickedTwice => format!("q.state().obj({id}).kicked >= 2"),
            S::Warped => self.cast_by_way(id, "w.kind == WayKind::Warp"),
            S::CostPaid(CastCost::Surge) => self.cast_by_way(id, "w.kind == WayKind::Surge"),
            S::CostPaid(CastCost::WebSlinging) => {
                self.cast_by_way(id, "w.kind == WayKind::WebSlinging")
            }
            S::CostPaid(CastCost::Emerge) => self.cast_by_way(id, "w.cost.emerge"),
            // `mechanics` gives a spectacle way this text.
            S::CostPaid(CastCost::Spectacle) => self.cast_by_way(id, "w.text == \"Spectacle\""),
            S::CostPaid(CastCost::Sneak) if !layer => {
                format!("cantrip_mechanics::sneak::was_cast_for_sneak(q, {id})")
            }
            S::CostPaid(CastCost::Prowl) => planned::planned_value(
                "object state CostPaid Prowl",
                "false",
                &self.cast_by_way(id, "w.kind == WayKind::Prowl"),
            ),
            S::CostPaid(CastCost::Madness) => planned::planned_value(
                "object state CostPaid Madness",
                "false",
                &format!("q.state().obj({id}).marks.cast_for_madness"),
            ),
            S::Escaped if !layer => format!("cantrip_mechanics::escape::escaped(q, {id})"),
            S::HasX => format!("{}.x > 0", self.cost_of(id, chars)),
            S::HybridInCost => {
                let cost = self.cost_of(id, chars);
                format!(
                    "{{ let cost = {cost}; cost.hybrid.iter().any(|h| h.is_some()) || cost.twobrid.iter().any(|&n| n > 0) }}"
                )
            }
            S::PhyrexianInCost => format!(
                "{}.phyrexian.iter().any(|&n| n > 0)",
                self.cost_of(id, chars)
            ),
            S::ManaSpentBelowManaValue => format!(
                "q.state().obj({id}).spent.total() < {}",
                match self.reader {
                    Reader::Layer => format!("q.mana_value({id}, &{chars})"),
                    _ => format!("q.mana_value({id})"),
                }
            ),
            S::HasAdventure => format!("{printed}.adventure.is_some()"),
            S::Modal => format!("{printed}.modal.is_some()"),
            // CR 712.2: what represents it, which a copy of one is not.
            S::DoubleFaced => format!(
                "q.state().obj({id}).represented_double_faced({own}.back.is_some())"
            ),
            // CR 702.62b.
            S::Suspended => format!(
                "(q.state().obj({id}).zone.is(ZoneKind::Exile) && q.state().obj({id}).counters(CounterKind::Time) > 0 && {own}.suspend.is_some())"
            ),
            // Keywords and the ability list are apart, and a protection,
            // hexproof from or toxic is a keyword ability with no bit of its own.
            S::NoAbilities => format!(
                "({chars}.ability_count() == 0 && {chars}.keywords.0 == 0 && !{chars}.has_protection() && {chars}.hexproof_from.0 == 0 && {chars}.toxic == 0 && {chars}.must_attack == 0)"
            ),
            S::Nameless => format!("{chars}.name.is_empty()"),
            S::PowerBelowToughness => sides("p < t"),
            S::PowerEqualsToughness => sides("p == t"),
            S::BlockingAlone => format!(
                "(q.state().blocks.iter().any(|b| b.blocker == {id}) && q.state().blocks.iter().all(|b| b.blocker == {id}))"
            ),
            S::BlockedThisTurn => format!(
                "q.history().this_turn().iter().any(|r| matches!(r.event, Event::Blocks {{ blocker, .. }} if blocker == {id}))"
            ),
            // CR 302.6: a change of control sets `controlled_since` too.
            S::ControlledSinceTurnBegan => {
                format!("q.state().obj({id}).controlled_since < q.state().turn.number")
            }
            S::ControlledSinceMostRecentTurn => format!(
                "q.state().obj({id}).controlled_since < q.state().player({}).turn_began",
                c.controller
            ),
            S::CastDuringOwnMainPhase if !layer => format!(
                "cast_of(q, {id}).is_some_and(|cast| q.history().all().iter().any(|r| matches!(r.event, Event::Cast(c) if c.spell == cast.spell) && r.active == cast.player && matches!(r.step, Step::PrecombatMain | Step::PostcombatMain)))"
            ),
            S::FirstLandPlayedThisTurn => format!(
                "{{ let first = q.history().this_turn().iter().find_map(|r| match r.event {{ Event::LandPlayed {{ land, player, .. }} if player == {} => Some(land), _ => None }}); first == Some({id}) }}",
                c.controller
            ),
            S::UnlockedDoor => format!("q.state().obj({id}).marks.doors != 0"),
            // The reminder text of Mjölnir's Equip says what this means.
            S::Worthy => format!(
                "({chars}.supertypes.legendary && !{chars}.has_subtype(Subtype::Villain) && ({chars}.colors.has(Color::Red) || {chars}.colors.has(Color::White)))"
            ),
            S::Renowned => unrecorded("Renowned", format!("q.state().obj({id}).marks.renowned")),
            S::Paired => unrecorded("Paired", format!("q.state().obj({id}).paired_with.is_some()")),
            S::HauntedBySource => unrecorded(
                "HauntedBySource",
                format!("q.state().obj({id}).haunted_by == Some(asker.source)"),
            ),
            S::TributeNotPaid => unrecorded(
                "TributeNotPaid",
                format!("q.state().obj({id}).marks.tribute_not_paid"),
            ),
            S::Foretold => unrecorded(
                "Foretold",
                format!("cantrip_mechanics::foretell::foretold(q.state(), {id})"),
            ),
            S::HasSticker => unrecorded(
                "HasSticker",
                format!("cantrip_mechanics::stickers::has_sticker(q.state(), {id})"),
            ),
            S::Sticker(kind) => unrecorded(
                "Sticker",
                format!(
                    "cantrip_mechanics::stickers::has(q.state(), {id}, StickerKind::{kind:?})"
                ),
            ),
            S::MostVotes => unrecorded("MostVotes", format!("votes.most({id})")),
            S::ReceivedAVote => unrecorded("ReceivedAVote", format!("votes.of({id}) > 0")),
            S::PowerAboveBase => unrecorded(
                "PowerAboveBase",
                format!("{chars}.power.map(i64::from) > Some(q.base_power({id}))"),
            ),
            S::PowerDiffersFromBase => unrecorded(
                "PowerDiffersFromBase",
                format!("{chars}.power.map(i64::from) != Some(q.base_power({id}))"),
            ),
            S::RingBearer => unrecorded(
                "RingBearer",
                format!("q.ring_bearer({}) == Some({id})", c.controller),
            ),
            S::Unearthed => unrecorded("Unearthed", format!("q.state().obj({id}).marks.unearthed")),
            S::Embalmed => unrecorded("Embalmed", format!("q.state().obj({id}).marks.embalmed")),
            S::DevouredACreature => unrecorded(
                "DevouredACreature",
                format!("q.state().obj({id}).marks.devoured > 0"),
            ),
            S::StartedTurnUntapped => unrecorded(
                "StartedTurnUntapped",
                format!("q.state().obj({id}).marks.untapped_as_turn_began"),
            ),
            S::ControlledSinceBeforeCombat => unrecorded(
                "ControlledSinceBeforeCombat",
                format!("q.state().obj({id}).controlled_since_step < Step::BeginCombat"),
            ),
            S::HadToAttack => unrecorded(
                "HadToAttack",
                format!("q.state().attackers.iter().any(|a| a.id == {id} && a.required)"),
            ),
            S::CouldntAttack => {
                unrecorded("CouldntAttack", format!("!q.could_have_attacked({id})"))
            }
            S::SnowManaOfOwnColorSpent => unrecorded(
                "SnowManaOfOwnColorSpent",
                format!("q.state().obj({id}).snow_spent.any_of({chars}.colors)"),
            ),
            other => return Err(gap(format!("object state {}", form(other)))),
        })
    }
}

/// What later reads ask of the instructions before them.
#[derive(Clone, Debug, Default)]
pub(super) struct Notes {
    /// What each instruction acted on, by the word `EarlierObjects` has for it:
    /// the last of these is what "that card" or "those" was.
    pub earlier: Vec<(&'static str, ResultId)>,
    /// For a step of a `ForEachPlayer`: the list of its players, and the test
    /// over `at`, an index into it, of whether that player's run did the step.
    pub took: BTreeMap<u32, (String, String)>,
    /// What a choice offered, and if it was one player's, whom it was bound to
    /// and the list of players it went round.
    pub offered: BTreeMap<u32, (ObjectSet, Option<(u32, String)>)>,
    /// The `ForEachPlayer` being walked: its binding, and its list of players.
    pub walking: Option<(u32, String)>,
    /// Steps of a walk where each player moves only what they own.
    pub own: BTreeSet<u32>,
    /// A choice among filters: the local holding its index, and the filters.
    pub filters: Option<(String, Vec<ObjectFilter>)>,
    /// Choices made among cards outside the game.
    pub outside: BTreeSet<u32>,
    /// Those of them that also offer cards elsewhere.
    pub outside_shown: BTreeSet<u32>,
}

/// The words `EarlierObjects` has for what an operation acts on.
fn acted_on(op: &Operation) -> &'static [&'static str] {
    match op {
        Operation::ChooseObjects { .. } => &["Selected"],
        Operation::Sacrifice { .. } => &["Sacrificed"],
        Operation::Discard { .. } | Operation::DiscardObjects { .. } => &["Discarded"],
        Operation::Mill { .. } => &["Milled"],
        Operation::Reveal { .. }
        | Operation::RevealUntil { .. }
        | Operation::RevealFromHand { .. } => &["Revealed"],
        Operation::Search { .. } => &["Found"],
        Operation::Tap { .. } => &["Tapped"],
        Operation::CreateTokens { .. } => &["Created"],
        Operation::Move { destination, .. } => match destination {
            Destination::Exile => &["Exiled", "LastExiled"],
            Destination::OwnersGraveyard => &["PutInGraveyard"],
            Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                &["Returned", "Entered"]
            }
            Destination::OwnersHand | Destination::Hand(_) => &["Returned"],
            _ => &[],
        },
        _ => &[],
    }
}

impl Compiler {
    /// What an instruction leaves for later ones to read back.
    pub(super) fn note_step(&mut self, instruction: &Instruction) {
        let Instruction::Effect { operation, result } = instruction else {
            return;
        };
        for &what in acted_on(operation) {
            self.notes.earlier.push((what, result.clone()));
        }
        if format!("{operation:?}").contains("RollDie") {
            self.notes.earlier.push(("Rolled", result.clone()));
        }
        if let Operation::ChooseObjects { objects, .. } = operation {
            let walking = self.notes.walking.clone();
            self.notes
                .offered
                .insert(result.0, (objects.clone(), walking));
        }
    }
    /// The steps of a walk where a player can only move what they own: nothing in
    /// a hand or a library is anyone else's (CR 400.3).
    pub(super) fn note_own(&mut self, body: &Program) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect {
                    result,
                    operation:
                        Operation::Discard { .. }
                        | Operation::DiscardObjects { .. }
                        | Operation::Mill { .. },
                } => {
                    self.notes.own.insert(result.0);
                }
                Instruction::Group(body) | Instruction::Optional { body, .. } => {
                    self.note_own(body)
                }
                _ => {}
            }
        }
    }
    /// What this player's run of a walk moved, of what the runs moved together.
    pub(super) fn moved_by(
        &self,
        result: &ResultId,
        phase: &ObjectPhase,
        who: &str,
    ) -> Result<String> {
        if !self.notes.own.contains(&result.0) {
            return Err(gap("object collection MovedBy"));
        }
        let moved = self
            .transition(result, phase)
            .map_err(|_| gap("object collection MovedBy"))?;
        Ok(format!(
            "({moved}).into_iter().filter(|&o| Some(q.state().obj(o).owner) == {who}).collect::<Vec<ObjectId>>()"
        ))
    }
    /// What the steps of a walk leave, which went by the way of `walk`.
    pub(super) fn note_walked(&mut self, body: &Program) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect { .. } => self.note_step(instruction),
                Instruction::Group(body) | Instruction::Optional { body, .. } => {
                    self.note_walked(body)
                }
                Instruction::If {
                    then, otherwise, ..
                } => {
                    self.note_walked(then);
                    self.note_walked(otherwise);
                }
                _ => {}
            }
        }
    }
    /// `Vec<ObjectId>` of what an earlier step acted on, as the words of the card
    /// leave it to be found. A step that couldn't be written reads as nothing, and
    /// its own hole says why.
    fn earlier_objects(&self, what: &EarlierObjects) -> Result<String> {
        let word = form(what);
        let step = self.notes.earlier.iter().rev().find(|(w, _)| *w == word);
        let Some((_, result)) = step.filter(|_| self.reader == Reader::Resolution) else {
            return self.earlier_paid(what);
        };
        if let (EarlierObjects::Revealed, Some(found)) = (what, self.reveal_matches.get(&result.0))
        {
            return Ok(format!("{found}.clone()"));
        }
        // What was sacrificed is read as it last existed.
        if matches!(what, EarlierObjects::Sacrificed) && self.before.contains_key(&result.0) {
            return self.transition(result, &ObjectPhase::Before);
        }
        self.named(result)
    }
    /// What paid a cost of the ability or spell being resolved, or, read outside
    /// a resolution, what the ability that exiled it linked to it (CR 607.2a).
    fn earlier_paid(&self, what: &EarlierObjects) -> Result<String> {
        let word = form(what);
        let exiled = matches!(what, EarlierObjects::Exiled | EarlierObjects::LastExiled);
        if exiled && self.reader != Reader::Resolution {
            return Ok(format!("q.exiled_with({})", self.linked()));
        }
        let pays = |part: &CostPart| match (what, part) {
            (EarlierObjects::Sacrificed, CostPart::Sacrifice { .. }) => true,
            (EarlierObjects::Revealed, CostPart::Reveal { .. }) => true,
            (EarlierObjects::Discarded, CostPart::Discard { .. }) => true,
            (EarlierObjects::Unattached, CostPart::UnattachFrom { .. }) => true,
            (
                EarlierObjects::Exiled,
                CostPart::Move {
                    destination: Destination::Exile,
                    ..
                },
            ) => true,
            _ => false,
        };
        let by_activation = self.activation_cost.as_ref().and_then(|cost| {
            let at = cost.parts.iter().position(pays)?;
            Some((PaymentRef::Activation, at))
        });
        let by_casting = self.casting_costs.iter().find_map(|(ability, cost)| {
            let at = cost.parts.iter().position(pays)?;
            Some((PaymentRef::Casting { ability: *ability }, at))
        });
        let Some((payment, at)) = by_activation.or(by_casting) else {
            if exiled {
                return Ok(format!("q.exiled_with({})", self.linked()));
            }
            // Cantrip keeps no record of this for a read to find.
            return Ok(planned::planned_value(
                &format!("earlier objects {word}"),
                "Vec::<ObjectId>::new()",
                &format!("q.acted_on(asker, Acted::{word})"),
            ));
        };
        self.objects(&ObjectSet::CostObjects {
            payment,
            component: vec![at as u32],
            view: CostObjectView::Performed,
            phase: ObjectPhase::Before,
        })
    }
    /// The player "their" means in a part of a row about several, or, for a row
    /// about an event, the event's.
    fn row_player(&self) -> Result<String> {
        match self.bound_players.iter().collect::<Vec<&u32>>().as_slice() {
            [one] => Ok(format!("Some(bound_{one})")),
            [] => self.carried_player(self.frame.player),
            _ => Err(gap("a row of several players")),
        }
    }
    /// The instruction one player at a time, where a part of it reads the player
    /// that part is for: "each player loses half their life". The set the operation
    /// names moves out to a walk, and inside it the operation names the player
    /// walked.
    pub(super) fn one_at_a_time(&self, instruction: &Instruction) -> Option<Instruction> {
        let Instruction::Effect { result, operation } = instruction else {
            return None;
        };
        if !self.bound_players.is_empty() {
            return None;
        }
        let words = format!("{operation:?}");
        if !(words.contains("Acting") || words.contains("Prior(That)")) {
            return None;
        }
        let mut value = serde_json::to_value(operation).ok()?;
        let fields = value.get_mut("args")?.as_object_mut()?;
        let binding = Binding(ACTING);
        let one = PlayerSet::One(PlayerRef::Bound(binding.clone()));
        let players: PlayerSet = if let Some(set) = fields.get("players") {
            let players = serde_json::from_value(set.clone()).ok()?;
            fields.insert("players".into(), serde_json::to_value(&one).ok()?);
            players
        } else {
            let Recipients::Players(players) =
                serde_json::from_value(fields.get("recipients")?.clone()).ok()?
            else {
                return None;
            };
            let to = Recipients::Players(one);
            fields.insert("recipients".into(), serde_json::to_value(&to).ok()?);
            players
        };
        // A set that itself names the row's player can't be walked.
        if format!("{players:?}").contains("Acting") {
            return None;
        }
        let operation: Operation = serde_json::from_value(value).ok()?;
        Some(Instruction::ForEachPlayer {
            players,
            binding,
            starting_with: None,
            in_chosen_direction: false,
            body: Program {
                instructions: vec![Instruction::Effect {
                    result: result.clone(),
                    operation,
                }],
            },
        })
    }
    /// The instruction with what it leaves to be picked already picked, by lot or
    /// by a player: a read can't roll or ask, so the pick comes ahead of it and the
    /// instruction reads what was picked.
    pub(super) fn picked_first(
        &mut self,
        instruction: &Instruction,
        body: &Program,
        i: usize,
        out: &mut String,
    ) -> Result<Option<Instruction>> {
        let Instruction::Effect { operation, .. } = instruction else {
            return Ok(None);
        };
        let words = format!("{operation:?}");
        if !(words.contains("Random {") || words.contains("Chosen {")) {
            return Ok(None);
        }
        let Ok(mut value) = serde_json::to_value(instruction) else {
            return Ok(None);
        };
        let id = self.fresh();
        let picked = ResultId(PICKED + id as u32);
        let Some(pick) = take_pick(&mut value, &picked) else {
            return Ok(None);
        };
        let Ok(rewritten) = serde_json::from_value::<Instruction>(value) else {
            return Ok(None);
        };
        match pick {
            Pick::Random(objects, count) => {
                let pool = self.objects(&objects)?;
                let count = self.number(&count)?.code;
                writeln!(
                    out,
                    "let mut pool_{id}: Vec<ObjectId> = {};",
                    self.fetch(pool)
                )
                .unwrap();
                writeln!(out, "let count_{id} = {};", self.fetch(count)).unwrap();
                writeln!(
                    out,
                    "let mut random_{id}: Vec<ObjectId> = Vec::new(); for _ in 0..count_{id} {{ let Some(one) = cantrip_mechanics::library::random_of(g, &pool_{id}) else {{ break }}; pool_{id}.retain(|&o| o != one); random_{id}.push(one); }}"
                )
                .unwrap();
                self.selections.insert(picked.0, format!("random_{id}"));
            }
            Pick::Chosen(chooser, objects, count) => {
                let PlayerSet::One(player) = chooser else {
                    return Err(gap("object collection Chosen"));
                };
                let choice = Instruction::Effect {
                    result: picked,
                    operation: Operation::ChooseObjects {
                        player,
                        objects,
                        count,
                        constraints: Vec::new(),
                    },
                };
                self.instruction(body, i, &choice, out)?;
            }
        }
        Ok(Some(rewritten))
    }
    /// The choices `choose_value` leaves to this lane.
    pub(super) fn choose_value_more(
        &mut self,
        kind: ValueKind,
        id: usize,
        player: &str,
        _options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        match kind {
            ValueKind::Parity if self.pending == Pending::Entering => self.entry_option(
                PARITY.iter().map(|word| word.to_string()).collect(),
                vec![String::new(); PARITY.len()],
                out,
            ),
            ValueKind::Parity if self.stack_item => {
                let base = self.choice_labels.len();
                self.choice_labels
                    .extend(PARITY.iter().map(|word| word.to_string()));
                let end = self.choice_labels.len();
                let words: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", words.join(", ")));
                writeln!(
                    out,
                    "let player_{id} = {player}; let chosen_{id} = cantrip_mechanics::choose::option(g, player_{id}, asker.source, item.ability.card, {base}..{end})? - {base};"
                )
                .unwrap();
                Ok(())
            }
            other => Err(gap(format!("a choice of {other:?}"))),
        }
    }
    /// "Target card gains flashback until end of turn": a grant of each card to
    /// its owner, of the cards there as this resolves. Whether the effect was that.
    pub(super) fn flashback_until(
        &mut self,
        id: usize,
        affected: &EffectSubjects,
        changes: &[Change],
        duration: &Duration,
        out: &mut String,
    ) -> Result<bool> {
        if !gives_flashback(changes) {
            return Ok(false);
        }
        let (EffectSubjects::Objects(cards), Duration::EndOfTurn) = (affected, duration) else {
            return Err(gap("granted Casting"));
        };
        let cards = self.fetch(self.objects(cards)?);
        writeln!(
            out,
            "let cards_{id}: Vec<ObjectId> = {cards}; for card in cards_{id} {{ let owner = g.obj(card).owner; g.allow_play(card, owner, PlayGrant {{ expiry: Expiry::EndOfTurn, flashback: true, ..PlayGrant::CAST }}); }}"
        )
        .unwrap();
        Ok(true)
    }
    /// "Each instant and sorcery card in your graveyard has flashback" (CR 702.34a):
    /// a way of casting them, for as long as this is here. A card with no mana cost
    /// has one that can't be paid (CR 118.6). Whether the ability was that.
    pub(super) fn flashback_way(
        &mut self,
        label: &str,
        affected: &EffectSubjects,
        changes: &[Change],
        parts: &mut Parts,
    ) -> Result<bool> {
        if !gives_flashback(changes) {
            return Ok(false);
        }
        let EffectSubjects::Objects(ObjectSet::Zone {
            scope:
                ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: players @ PlayerSet::One(PlayerRef::AbilityController),
                },
            filter,
            ..
        }) = affected
        else {
            return Err(gap("granted Casting"));
        };
        let reader = std::mem::replace(&mut self.reader, Reader::Query);
        let tests = self.permission_tests(players, &ObjectFilter::Any, filter, "card");
        self.reader = reader;
        let printed = "q.card(q.chars(card).card)";
        let body = format!(
            "({} && {printed}.has_cost).then(|| Cost::mana({printed}.cost))",
            tests?.join(" && ")
        );
        self.permission_entry(
            None,
            "granted_ways",
            &format!("{label}_cost"),
            (
                "q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId",
                "q, asker, card, player",
                "Option<Cost>",
            ),
            &body,
            |cost| {
                format!("GrantedWay {{ text: \"Flashback\", flashback: true, cost: {cost}, ..GrantedWay::CAST }}")
            },
            parts,
        );
        Ok(true)
    }
    /// The rest of `player`.
    pub(super) fn player_more(&self, player: &PlayerRef) -> Result<String> {
        let feature = format!("player reference {}", form(player));
        Ok(match player {
            PlayerRef::Acting | PlayerRef::Prior(PriorPlayer::That) => {
                self.row_player().map_err(|_| gap(feature))?
            }
            // "That player" of another ability of the object: nothing keeps it.
            PlayerRef::Prior(PriorPlayer::Remembered) => planned::planned_value(
                "player reference Prior",
                "None::<PlayerId>",
                "q.remembered_player(asker)",
            ),
            PlayerRef::Prior(PriorPlayer::GrantedBy) => planned::planned_value(
                "player reference Prior",
                "None::<PlayerId>",
                "q.granted_by(asker)",
            ),
            PlayerRef::Prior(PriorPlayer::WinningBidder) => planned::planned_value(
                "player reference Prior",
                "None::<PlayerId>",
                "g.winning_bidder(item)",
            ),
            // Turn order goes to the left (CR 101.4), so these are the seats
            // next to a player among those left in the game (CR 800.4a).
            PlayerRef::Seat { of, side } => {
                let order = "q.state().turn_order_from(p).filter(|&next| next != p)";
                let read = match side {
                    SeatSide::Left => format!("{order}.next()"),
                    SeatSide::Right => format!("{order}.last()"),
                    SeatSide::Next => {
                        format!("{order}.find(|&next| q.state().opponents(p).any(|o| o == next))")
                    }
                };
                format!("{}.and_then(|p: PlayerId| {read})", self.player(of)?)
            }
            // A tie names nobody: the card's own words say what then.
            PlayerRef::Extreme {
                players,
                number,
                highest,
            } => {
                let value = self.player_number("p", number)?;
                let pick = if *highest { "max" } else { "min" };
                format!(
                    "{{ let rows: Vec<(PlayerId, i64)> = ({}).into_iter().map(|p: PlayerId| (p, {value})).collect(); let best = rows.iter().map(|r| r.1).{pick}(); let mut top = rows.iter().filter(|r| Some(r.1) == best).map(|r| r.0); match (top.next(), top.next()) {{ (Some(p), None) => Some(p), _ => None }} }}",
                    self.players(players)?
                )
            }
            other => return Err(gap(format!("player reference {}", form(other)))),
        })
    }
    /// The rest of `object`.
    pub(super) fn object_more(&self, object: &ObjectRef) -> Result<String> {
        Ok(match object {
            // `exiled_with` goes in exile order.
            ObjectRef::Earlier {
                what: what @ EarlierObjects::LastExiled,
                by: None | Some(PlayerRef::AbilityController),
            } => format!("({}).last().copied()", self.earlier_objects(what)?),
            ObjectRef::Earlier {
                what,
                by: None | Some(PlayerRef::AbilityController),
            } => format!("({}).first().copied()", self.earlier_objects(what)?),
            other => return self.object_p(other),
        })
    }
    /// The rest of `objects`.
    pub(super) fn objects_more(&self, objects: &ObjectSet) -> Result<String> {
        Ok(match objects {
            ObjectSet::Earlier { what, by: None } => self.earlier_objects(what)?,
            ObjectSet::Copies(result) => self.named(result)?,
            ObjectSet::Unselected(result) => self.unselected(result)?,
            other => return self.objects_p(other),
        })
    }
    /// What a choice offered and didn't take, all the players' for a choice each
    /// of them made.
    fn unselected(&self, result: &ResultId) -> Result<String> {
        let (offered, walked) = self
            .notes
            .offered
            .get(&result.0)
            .ok_or_else(|| gap("object collection Unselected"))?;
        let chosen = self.selected(result)?;
        let list = match walked {
            None => self.objects(offered)?,
            Some((binding, players)) => {
                let mut each = self.clone();
                each.bound_players.insert(*binding);
                format!(
                    "{players}.iter().copied().flat_map(|bound_{binding}: PlayerId| {}).collect::<Vec<ObjectId>>()",
                    each.objects(offered)?
                )
            }
        };
        Ok(format!(
            "{{ let chosen: Vec<ObjectId> = {chosen}; ({list}).into_iter().filter(|o| !chosen.contains(o)).collect::<Vec<ObjectId>>() }}"
        ))
    }
    /// The rest of `number`.
    pub(super) fn number_more(&self, n: &Expr) -> Result<Number> {
        use lane_j::{counted, held_number};
        Ok(match n {
            // What the card has as its optional cost is the evidence it collects.
            Expr::Earlier {
                what: EarlierObjects::Evidence,
                measure: ObjectMeasure::Count,
                ..
            } => counted("i64::from(q.state().obj(asker.source).paid)".into()),
            Expr::Earlier {
                what,
                measure,
                result: None,
                filter,
                by: None | Some(PlayerRef::AbilityController),
            } => {
                let list = self.earlier_objects(what)?;
                let keep = match filter {
                    Some(filter) => format!(
                        ".filter(|&id| {})",
                        self.filter(filter, &Candidate::of("id"))?
                    ),
                    None => String::new(),
                };
                let those = format!("({list}).into_iter(){keep}");
                match measure {
                    ObjectMeasure::Count => counted(format!("{those}.count() as i64")),
                    ObjectMeasure::Of(property) => {
                        let value = self.property("id", property)?.code;
                        counted(format!("{those}.next().map_or(0i64, |id: ObjectId| {value})"))
                    }
                    ObjectMeasure::Total(property) => {
                        let value = self.property("id", property)?.code;
                        counted(format!("{those}.map(|id: ObjectId| {value}).sum::<i64>()"))
                    }
                    ObjectMeasure::Greatest(property) => {
                        let value = self.property("id", property)?.code;
                        counted(format!("{those}.map(|id: ObjectId| {value}).max().unwrap_or(0)"))
                    }
                }
            }
            Expr::Power(base, exponent) => {
                let (base, exponent) = (self.number(base)?, self.number(exponent)?);
                Number {
                    code: format!(
                        "({}).saturating_pow(({}).clamp(0, 62) as u32)",
                        base.code, exponent.code
                    ),
                    min: i64::MIN.into(),
                    max: i64::MAX.into(),
                    dynamic: true,
                }
            }
            Expr::Difference(a, b) => {
                let (a, b) = (self.number(a)?, self.number(b)?);
                counted(format!("(({}).saturating_sub({})).abs()", a.code, b.code))
            }
            Expr::TurnNumber => counted("i64::from(q.state().turn.number)".into()),
            // The count of the record leaves out the resolution under way.
            Expr::Game(GameValue::ResolutionsThisTurn)
                if self.stack_item && self.reader == Reader::Resolution =>
            {
                counted("(q.history().resolutions(item.object, item.ability) as i64 + 1)".into())
            }
            // The roll just made: `RollDie` is one die.
            Expr::Dice(DiceMeasure::Result | DiceMeasure::Total) => {
                match self.notes.earlier.iter().rev().find(|(w, _)| *w == "Rolled") {
                    Some((_, result)) => self.number(&Expr::DieResult(result.clone()))?,
                    None => held_number(
                        "numeric expression Dice".into(),
                        "g.last_roll(asker.controller).result",
                    ),
                }
            }
            // Odd is the first word, and even the second; the AST has it 1 for odd.
            Expr::ChosenParity => match self.chosen_local(ValueKind::Parity) {
                Ok(local) => counted(format!("i64::from({local} == 0)")),
                Err(_) if self.reader == Reader::Layer => counted(
                    "q.state().chosen_option(q.linked_to(asker.source)).filter(|c| std::ptr::eq(q.cards().get(c.card), &CARD)).map_or(-1i64, |c| i64::from(c.index == 0))".into(),
                ),
                Err(_) => counted(
                    "cantrip_mechanics::choose::chosen_option(q, asker, &CARD).map_or(-1i64, |index| i64::from(index == 0))".into(),
                ),
            },
            Expr::Noted { .. } => held_number(
                "numeric expression Noted".into(),
                &format!("q.noted(asker, {})", one_line(n)),
            ),
            Expr::AsOf { .. } => held_number(
                "numeric expression AsOf".into(),
                &format!("q.as_of(asker, {})", one_line(n)),
            ),
            Expr::DieResultAt { index, .. } => held_number(
                "numeric expression DieResultAt".into(),
                &format!("g.last_roll(asker.controller).dice[{index}]"),
            ),
            Expr::DraftNoted { .. } => held_number(
                "numeric expression DraftNoted".into(),
                &format!("q.draft_notes(asker.controller, {})", one_line(n)),
            ),
            Expr::CoinFlipsWon(won) => held_number(
                "numeric expression CoinFlipsWon".into(),
                &format!("g.coin_flips(asker.controller, {won})"),
            ),
            Expr::TimesMutated(object) => held_number(
                "numeric expression TimesMutated".into(),
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| q.state().obj(o).mutations as i64)",
                    self.object(object)?
                ),
            ),
            Expr::Votes(option) => held_number(
                "numeric expression Votes".into(),
                &format!("g.votes().count({})", form(option)),
            ),
            Expr::ManaSpentFrom { object, sources } if self.reader != Reader::Layer => counted(format!(
                "{}.and_then(|o: ObjectId| cast_of(q, o)).map_or(0i64, |cast| q.history().mana_spent_to_cast(cast.spell).filter(|&(source, _)| {}).map(|(_, spent)| i64::from(spent.total())).sum::<i64>())",
                self.object(object)?,
                self.filter(sources, &Candidate::of("source"))?
            )),
            other => return self.number_p(other),
        })
    }
    /// The rest of `predicate`.
    pub(super) fn predicate_more(&self, f: &ObjectFilter, c: &Candidate) -> Result<String> {
        let (id, chars) = (&c.id, &c.chars);
        Ok(match f {
            ObjectFilter::AttachedTo(filter) => format!(
                "q.state().obj({id}).attached_to.is_some_and(|host: ObjectId| {})",
                self.filter(filter, &Candidate::of("host"))?
            ),
            ObjectFilter::HasAttached(filter) => format!(
                "q.state().battlefield().iter().any(|&attached| q.state().obj(attached).attached_to == Some({id}) && {})",
                self.filter(filter, &Candidate::of("attached"))?
            ),
            // A nameless object shares its name with nothing (CR 201.2c).
            ObjectFilter::SharesNameWith(object) => format!(
                "{}.is_some_and(|other: ObjectId| !{chars}.name.is_empty() && q.chars(other).name == {chars}.name)",
                self.object(object)?
            ),
            ObjectFilter::SharesTypeFrom { types, with } => {
                let list = types
                    .iter()
                    .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                    .collect::<Result<Vec<_>>>()?;
                format!(
                    "{}.is_some_and(|other: ObjectId| [{}].into_iter().any(|t: CardType| q.chars(other).is(t) && {chars}.is(t)))",
                    self.object(with)?,
                    list.join(", ")
                )
            }
            ObjectFilter::SharesColorWithAny(objects) => format!(
                "({}).into_iter().any(|other: ObjectId| q.chars(other).colors.iter().any(|color| {chars}.colors.has(color)))",
                self.objects(objects)?
            ),
            ObjectFilter::Extreme {
                among,
                property,
                highest,
            } => {
                let value = self.property("row", property)?.code;
                let pick = if *highest { "max" } else { "min" };
                format!(
                    "{{ let rows: Vec<(ObjectId, i64)> = ({}).into_iter().map(|row: ObjectId| (row, {value})).collect(); let best = rows.iter().map(|r| r.1).{pick}(); rows.iter().any(|r| r.0 == {id} && Some(r.1) == best) }}",
                    self.objects(among)?
                )
            }
            ObjectFilter::DraftNoted { .. } => planned::planned_value(
                "object filter DraftNoted",
                "false",
                &format!("q.draft_notes({id}, {})", one_line(f)),
            ),
            other => return self.predicate_p(other, c),
        })
    }
    /// The rest of `condition`.
    pub(super) fn condition_more(&self, condition: &Condition) -> Result<String> {
        Ok(match condition {
            Condition::Outcome(outcome) => planned::planned_value(
                "condition Outcome",
                "false",
                &format!("q.outcome(asker, Outcome::{outcome:?})"),
            ),
            Condition::AsCast { .. } => planned::planned_value(
                "condition AsCast",
                "false",
                &format!("q.as_cast(asker, {})", one_line(condition)),
            ),
            // A color no permanent has isn't the most common one.
            Condition::MostCommonColor {
                color,
                objects,
                tied,
            } => {
                let Operand::Vocabulary { form: color, .. } = &**color else {
                    return Err(gap("condition MostCommonColor"));
                };
                let wanted = match color.as_str() {
                    "TheChosenColor" => self.chosen_value(ValueKind::Color)?,
                    "White" | "Blue" | "Black" | "Red" | "Green" => format!("Some(Color::{color})"),
                    _ => return Err(gap("condition MostCommonColor")),
                };
                let alone = match tied {
                    true => "",
                    false => " && Color::ALL.into_iter().filter(|&c| count(c) == best).count() == 1",
                };
                format!(
                    "{{ let those: Vec<ObjectId> = {}; let count = |c: Color| those.iter().filter(|&&o| q.chars(o).colors.has(c)).count(); let best = Color::ALL.into_iter().map(count).max().unwrap_or(0); {wanted}.is_some_and(|wanted: Color| count(wanted) > 0 && count(wanted) == best{alone}) }}",
                    self.objects(objects)?
                )
            }
            // A set of N with different values exists when there are N values.
            Condition::SubsetMeets {
                objects,
                comparison: Comparison::GreaterOrEqual,
                amount,
                constraints,
            } if matches!(constraints.as_slice(), [SelectionConstraint::DifferentProperty(_)]) => {
                let [SelectionConstraint::DifferentProperty(property)] = constraints.as_slice()
                else {
                    return Err(gap("condition SubsetMeets"));
                };
                let value = self.property("id", property)?.code;
                format!(
                    "{{ let mut values: Vec<i64> = ({}).into_iter().map(|id: ObjectId| {value}).collect(); values.sort_unstable(); values.dedup(); (values.len() as i64) >= ({}) }}",
                    self.objects(objects)?,
                    self.number(amount)?.code
                )
            }
            // The event's own cast: it is in the history only once it is over.
            Condition::WasCastFrom {
                object: ObjectRef::EventSubject,
                zone,
            } if self.event_at == EventAt::Matcher || self.history.is_some() => format!(
                "matches!(event, Event::Cast(cast) if cast.from == {})",
                read::zone_kind(zone)?
            ),
            Condition::WasCastFrom { object, zone } if self.reader != Reader::Layer => format!(
                "{}.and_then(|o: ObjectId| cast_of(q, o)).is_some_and(|cast| cast.from == {})",
                self.object(object)?,
                read::zone_kind(zone)?
            ),
            // Whether the run of a `ForEachPlayer` this player had did that step.
            Condition::PlayerTook { player, result } => {
                match self.notes.took.get(&result.0) {
                    Some((players, test)) => format!(
                        "{}.and_then(|p: PlayerId| {players}.iter().position(|&next| next == p)).is_some_and(|at| {test})",
                        self.player(player)?
                    ),
                    None if self.failed.contains(&result.0) => {
                        planned::unreported("false", result)
                    }
                    None => return Err(gap("condition PlayerTook")),
                }
            }
            other => return self.condition_p(other),
        })
    }
}

/// The rest of `TurnWindow`. `step` is the step as a number.
pub(super) fn turn_window(window: &TurnWindow, step: &str) -> Result<String> {
    Ok(match window {
        TurnWindow::DrawStep => "q.state().turn.step() == Step::Draw".into(),
        TurnWindow::MainPhase => {
            "matches!(q.state().turn.step(), Step::PrecombatMain | Step::PostcombatMain)".into()
        }
        TurnWindow::BeforeCombat => format!("{step} < (Step::BeginCombat as u8)"),
        TurnWindow::AfterCombat => format!("{step} > (Step::EndOfCombat as u8)"),
        TurnWindow::EndOfCombatStep => "q.state().turn.step() == Step::EndOfCombat".into(),
        TurnWindow::BeforeEndOfCombatStep => format!("{step} < (Step::EndOfCombat as u8)"),
        other => planned::planned_value(
            &format!("turn window {other:?}"),
            "false",
            &format!("q.state().turn.is_in(TurnWindow::{other:?})"),
        ),
    })
}

/// The rest of what a resolving effect can change.
pub(super) fn continuous_more(change: &Change) -> Result<Vec<String>> {
    Ok(match change {
        Change::SetName(word) => vec![format!("EffectKind::SetName({})", quoted(word))],
        Change::AddArtifactType(t) => vec![format!("EffectKind::AddSubtype(Subtype::{})", name(t))],
        // It keeps no other artifact type (CR 205.1a).
        Change::SetArtifactType(t) => vec![
            "EffectKind::LoseSubtypesOf(CardType::Artifact)".into(),
            format!("EffectKind::AddSubtype(Subtype::{})", name(t)),
        ],
        Change::RemoveSubtype(t) => vec![lane_g::planned_kind(
            "continuous change RemoveSubtype",
            format!("EffectKind::LoseSubtype(Subtype::{})", name(t)),
        )],
        other => return Err(gap(format!("continuous change {}", form(other)))),
    })
}

/// The rest of the numbers an object has.
pub(super) fn property_more(property: &Property, id: &str) -> Option<Number> {
    let code = match property {
        Property::TotalPowerAndToughness => format!(
            "{{ let chars = q.chars({id}); i64::from(chars.power.unwrap_or(0)) + i64::from(chars.toughness.unwrap_or(0)) }}"
        ),
        Property::ToxicValue => format!("i64::from(q.chars({id}).toxic)"),
        _ => return None,
    };
    Some(lane_j::counted(code))
}

/// A duration whose end Cantrip doesn't track: the effect would go on, until it
/// has the `Duration` this names.
pub(super) fn duration_more(duration: &crate::ir::Duration) -> Result<String> {
    use crate::ir::Duration as D;
    let feature = format!("duration {}", form(duration));
    if !matches!(
        duration,
        D::WhileOnBattlefield(_)
            | D::UntilCast(_)
            | D::UntilEvent(_)
            | D::UntilNextUntapStep(_)
            | D::DuringNextTurn(_)
    ) {
        return Err(gap(feature));
    }
    let words = format!("{duration:?}");
    let words: String = words.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(planned::planned_value(
        &feature,
        "Duration::Indefinite",
        &format!("Duration::{words}"),
    ))
}

/// A spell whose text comes in paragraphs is one spell, read in order (CR 608.2c):
/// the card with them joined, when at most one announces anything and no other
/// ability counts past them.
pub(super) fn spells_joined(card: &Card) -> Option<Card> {
    let spells: Vec<usize> = (card.abilities.iter().enumerate())
        .filter(|(_, a)| matches!(a, Ability::Spell { .. }))
        .map(|(i, _)| i)
        .collect();
    let (&first, rest) = spells.split_first()?;
    rest.last()?;
    let counted = |a: &Ability| matches!(a, Ability::Casting(CastingRule::AdditionalCost { .. }));
    if card.abilities[first..].iter().any(counted) {
        return None;
    }
    let mut announced: Option<Announcement> = None;
    let mut instructions = Vec::new();
    for &at in &spells {
        let Ability::Spell { announcement, body } = &card.abilities[at] else {
            return None;
        };
        if announcement.modes.is_some() {
            return None;
        }
        if !announcement.targets.is_empty() {
            if announced.is_some() {
                return None;
            }
            announced = Some(announcement.clone());
        }
        instructions.extend(body.instructions.iter().cloned());
    }
    let Ability::Spell { announcement, .. } = &card.abilities[first] else {
        return None;
    };
    let joined = Ability::Spell {
        announcement: announced.unwrap_or_else(|| announcement.clone()),
        body: Program { instructions },
    };
    let mut joined_card = card.clone();
    joined_card.abilities[first] = joined;
    for &at in rest.iter().rev() {
        joined_card.abilities.remove(at);
    }
    Some(joined_card)
}

/// Whether all these do is give flashback for the card's own mana cost.
fn gives_flashback(changes: &[Change]) -> bool {
    let [Change::GrantAbility(ability)] = changes else {
        return false;
    };
    let Ability::Casting(CastingRule::Permission {
        origin: CastingOrigin::Flashback,
        cost,
        leave_stack: Some(Destination::Exile),
        ..
    }) = &**ability
    else {
        return false;
    };
    matches!(
        cost.parts.as_slice(),
        [CostPart::ManaCostOf(ObjectRef::Source)]
    )
}

/// These instructions, with a group's in its place.
pub(super) fn flat_groups(instructions: &[Instruction], flat: &mut Vec<Instruction>) {
    for instruction in instructions {
        match instruction {
            Instruction::Group(body) => flat_groups(&body.instructions, flat),
            other => flat.push(other.clone()),
        }
    }
}

/// The words of "choose odd or even", odd first.
const PARITY: [&str; 2] = ["odd", "even"];

/// A form of the AST on one line, to name it in a planned call.
fn one_line(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// What an instruction leaves to be picked.
enum Pick {
    Random(ObjectSet, Expr),
    Chosen(PlayerSet, ObjectSet, Count),
}

/// Takes the first pick out of an instruction's JSON, leaving the selection
/// `picked` where it was.
fn take_pick(value: &mut serde_json::Value, picked: &ResultId) -> Option<Pick> {
    use serde_json::{from_value, json, Value};
    match value {
        Value::Object(fields) => {
            let pick = (|| {
                let args = fields.get("args")?.as_object()?;
                let objects = args.get("objects")?.clone();
                let count = args.get("count")?.clone();
                match fields.get("kind")?.as_str()? {
                    "Random" => Some(Pick::Random(
                        from_value(objects).ok()?,
                        from_value(count).ok()?,
                    )),
                    "Chosen" => Some(Pick::Chosen(
                        from_value(args.get("chooser")?.clone()).ok()?,
                        from_value(objects).ok()?,
                        from_value(count).ok()?,
                    )),
                    _ => None,
                }
            })();
            if pick.is_some() {
                *value = json!({ "kind": "Selected", "args": picked.0 });
                return pick;
            }
            fields
                .values_mut()
                .find_map(|inner| take_pick(inner, picked))
        }
        Value::Array(items) => items.iter_mut().find_map(|inner| take_pick(inner, picked)),
        _ => None,
    }
}

/// Where a pick keeps its result, past any card's own.
const PICKED: u32 = 1_000_000;

/// A binding no card's AST uses, for an instruction taken one player at a time.
const ACTING: u32 = 9_000;
