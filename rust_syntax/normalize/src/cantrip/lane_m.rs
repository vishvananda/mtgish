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
            S::KickedTwice if self.kickers.len() > 1 => format!(
                "{} >= 2",
                self.times_kicked("Some(asker.source)", id)
            ),
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
            S::CostPaidThisTurn(CastCost::Sneak) if !layer => format!(
                "cantrip_mechanics::sneak::sneak_cast_turn(q, {id}) == Some(q.history().turn)"
            ),
            S::CostPaid(CastCost::Prowl) => planned::planned_value(
                "object state CostPaid Prowl",
                "false",
                &self.cast_by_way(id, "w.kind == WayKind::Prowl"),
            ),
            // Cast by madness's own ability, its record says (CR 702.35a).
            S::CostPaid(CastCost::Madness) if !layer => format!(
                "{{ let records = q.history().all(); let spell = records.iter().rev().find_map(|r| match r.event {{ Event::ZoneChange {{ old, new, reason: ZoneReason::Resolve, .. }} if new == {id} => Some(old), _ => None }}).unwrap_or({id}); records.iter().rev().any(|r| matches!(r.event, Event::Cast(cast) if cast.spell == spell) && matches!(r.cause, Cause::Ability {{ ability, .. }} if ability.is_madness())) }}"
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
            S::FirstLandPlayedThisTurn if lane_k::printed_otherwise(self.line_text(), "wasn't the first") => {
                return Err(gap("the first land played, where the card says it wasn't"))
            }
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
            // Foretelling is a player's own action, so what lets the card be cast
            // names no spell or ability: the permission is the card's own, and a
            // spell's cast keeps it (CR 702.143a).
            S::Foretold => {
                let row = self.card_row("o.card");
                let cast = match layer {
                    true => String::new(),
                    false => " || q.history().cast_of(id).is_some_and(|cast| cast.granted_by().is_some_and(|by| by.ability == own))".to_string(),
                };
                format!(
                    "{{ let id = {id}; let o = q.state().obj(id); let own = AbilityRef::spell(o.card); {row}.foretell.is_some() && (q.state().permissions.iter().any(|p| p.by.object == id && p.by.ability == own){cast}) }}"
                )
            }
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
            S::MostVotes => unrecorded(
                "MostVotes",
                format!("q.votes().most(cantrip_mechanics::vote::Vote::Object({id}), true)"),
            ),
            S::ReceivedAVote => unrecorded(
                "ReceivedAVote",
                format!("q.votes().count(cantrip_mechanics::vote::Vote::Object({id})) > 0"),
            ),
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
            // "If {S} of any of that spell's colors was spent to cast it": mana from
            // a snow source of one of its colors, by the source that made it (CR
            // 106.3, 107.4h). The card tells snow mana apart (`faces::mana_told_apart`),
            // so its payer chooses which mana is spent (CR 601.2h).
            S::SnowManaOfOwnColorSpent => format!(
                "q.history().mana_spent_to_cast({id}).any(|(source, mana)| source != ObjectId::NONE && q.chars(source).supertypes.snow && {chars}.colors.iter().any(|c: Color| mana.get(c.mana()) > 0))"
            ),
            // CR 305.8: the five basic land types, and no other.
            S::NonBasicLandType => format!(
                "{chars}.subtypes.iter().any(|kind| kind.card_types().has(CardType::Land) && !matches!(kind, Subtype::Plains | Subtype::Island | Subtype::Swamp | Subtype::Mountain | Subtype::Forest))"
            ),
            other => return Err(gap(format!("object state {}", form(other)))),
        })
    }
}

/// What later reads ask of the instructions before them.
#[derive(Clone, Debug, Default)]
pub(super) struct Notes {
    /// The players this body chose, in order: "the second player".
    pub named_players: Vec<String>,
    /// Who each of these results ended up with: "the player who did", "that opponent".
    pub performers: std::collections::HashMap<u32, String>,
    /// The player last asked to pay: "if they don't, they ...".
    pub acting: Option<String>,
    /// Whose cards an offer to play is of, past what its pool reads.
    pub play_owner: Option<String>,
    /// Counters go on this per-player batch, by this player, and only on what that
    /// player controls if so, rather than on at once.
    pub gathering_counters: Option<(usize, String, bool)>,
    /// Reveals whose cards each went to exile as they were reached, so that the
    /// instruction that moves them has nothing left to do.
    pub exiled_going: BTreeSet<u32>,
    /// The X just paid as this resolves, where "X" is that number and not the
    /// stack item's: "you may pay {X}. X can't be greater than ...".
    pub paid_x: Option<String>,
    /// The instruction before this copied each spell twice over.
    pub copied_twice: bool,
    /// What a cost paid as this resolved exiled: "return this card" after "you may
    /// exile it", where the source has since left the object it was.
    pub exiled_source: Option<String>,
    /// What a cost paid as this resolved exiled, for an ability given later in
    /// it to read as "the exiled card" (Timothar, Baron of Bats).
    pub exiled_for_grant: Option<String>,
    /// What each instruction acted on, by the word `EarlierObjects` has for it:
    /// the last of these is what "that card" or "those" was.
    pub earlier: Vec<(&'static str, ResultId)>,
    /// For a step of a `ForEachPlayer`: the list of its players, and the test
    /// over `at`, an index into it, of whether that player's run did the step.
    pub took: BTreeMap<u32, (String, String)>,
    /// A step of a walk: its players, and the list of what each of them did.
    pub each: BTreeMap<u32, (String, String)>,
    /// A step of a walk that counted: its players, and each one's number.
    pub each_done: BTreeMap<u32, (String, String)>,
    /// A step on what an earlier choice chose: that choice.
    pub acted_of: BTreeMap<u32, u32>,
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
    /// What each player's runs of a walk did, by the walk's step, where a later
    /// instruction reads it player by player.
    pub shares: BTreeMap<u32, String>,
    /// Amounts a walk made that are read a player at a time, as their own.
    pub read_as_own: BTreeSet<u32>,
    /// The life the last walk's players paid, together.
    pub life_paid: Option<String>,
    /// The player a rule for a while is on, where it lasts until their own next
    /// turn and they can only be read inside the walk that puts it on them.
    pub until_own_turn: Option<String>,
}

/// The words `EarlierObjects` has for what an operation acts on.
fn acted_on(op: &Operation) -> &'static [&'static str] {
    match op {
        Operation::ChooseObjects { .. } | Operation::ChooseAndPay { .. } => &["Selected"],
        // "Chooses a card in your graveyard", "secretly chooses a creature": objects
        // chosen on terms, which "each creature chosen this way" reads back.
        Operation::ChooseValueWith {
            kind: ValueKind::Object,
            ..
        } => &["Selected"],
        Operation::Sacrifice { .. } => &["Sacrificed", "PutInGraveyard"],
        Operation::PayCost { cost, optional: true, .. }
            if matches!(cost.parts.as_slice(), [CostPart::Sacrifice { .. }]) =>
        {
            &["Sacrificed"]
        }
        Operation::PayCost { cost, optional: true, .. }
            if matches!(cost.parts.as_slice(), [CostPart::UnattachFrom { .. }]) =>
        {
            &["Unattached"]
        }
        Operation::Destroy { .. } => &["PutInGraveyard"],
        Operation::Counter { .. } => &["Countered"],
        Operation::Discard { .. } | Operation::DiscardObjects { .. } => &["Discarded"],
        Operation::Mill { .. } => &["Milled"],
        Operation::Manifest { .. } => &["Manifested"],
        // "Until they reveal a historic permanent card ... that card" (Jhoira):
        // what a reveal until finds is the card found this way.
        Operation::RevealUntil { .. } => &["Revealed", "Found"],
        Operation::Reveal { .. } => &["Revealed"],
        // "Reveals a nonland card at random from their hand. It perpetually gains":
        // the card revealed is the card chosen in the hand.
        Operation::RevealFromHand { .. } => &["Revealed", "Selected"],
        Operation::Search { .. } => &["Found"],
        Operation::Tap { .. } => &["Tapped"],
        Operation::CreateTokens { .. } => &["Created"],
        Operation::CopyCards { .. } | Operation::CopyOnStack { .. } => &["CopiedSpell"],
        // "If you gain control of a creature this way, tap it" (Midnight Crusader
        // Shuttle).
        Operation::CreateContinuousEffect { changes, .. }
            if changes.iter().any(|change| matches!(change, Change::SetController(_))) =>
        {
            &["GainedControl"]
        }
        Operation::Move { destination, .. } => match destination {
            Destination::Exile => &["Exiled", "LastExiled"],
            Destination::OwnersGraveyard => &["PutInGraveyard"],
            Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                &["Returned", "Entered"]
            }
            Destination::OwnersHand | Destination::Hand(_) => &["Returned"],
            Destination::OwnersLibraryShuffled => &["Shuffled"],
            _ => &[],
        },
        _ => &[],
    }
}

/// A walk of players who each just discard, as the one discard it is.
pub(super) fn each_discards(
    players: &PlayerSet,
    binding: &Binding,
    body: &Program,
) -> Option<Instruction> {
    let [Instruction::Effect {
        result,
        operation:
            Operation::PayCost {
                player: PlayerRef::Bound(payer),
                cost,
                optional: false,
            },
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    let [CostPart::Discard {
        count,
        filter: ObjectFilter::Any,
        random: false,
    }] = cost.parts.as_slice()
    else {
        return None;
    };
    (payer.0 == binding.0).then(|| Instruction::Effect {
        result: result.clone(),
        operation: Operation::Discard {
            players: players.clone(),
            amount: count.clone(),
            random: false,
        },
    })
}

/// A player's move of what they choose among several zones, as the choice and the
/// move of what was chosen: `fresh` numbers the choice.
pub(super) fn chosen_then_moved(body: &Program, fresh: u32) -> Option<Program> {
    let [Instruction::Effect {
        result,
        operation:
            Operation::Move {
                objects:
                    ObjectSet::Chosen {
                        chooser: PlayerSet::One(chooser @ PlayerRef::Bound(_)),
                        objects,
                        count,
                    },
                destination,
                entry,
                face_down,
            },
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    let count = match count {
        Count::Exactly(n) => Count::AsManyAsCan(n.clone()),
        other => other.clone(),
    };
    let chosen = ResultId(fresh);
    Some(Program {
        instructions: vec![
            Instruction::Effect {
                result: chosen.clone(),
                operation: Operation::ChooseObjects {
                    player: chooser.clone(),
                    objects: (**objects).clone(),
                    count,
                    constraints: Vec::new(),
                },
            },
            Instruction::Effect {
                result: result.clone(),
                operation: Operation::Move {
                    objects: ObjectSet::Selected(chosen),
                    destination: destination.clone(),
                    entry: entry.clone(),
                    face_down: *face_down,
                },
            },
        ],
    })
}

impl Compiler {
    /// What an instruction leaves for later ones to read back.
    pub(super) fn note_step(&mut self, instruction: &Instruction) {
        // "You may choose a creature": offered, taken or not.
        if let Instruction::Optional { body, .. } = instruction {
            for step in &body.instructions {
                if let Instruction::Effect {
                    operation: Operation::ChooseObjects { objects, .. },
                    result,
                } = step
                {
                    let walking = self.notes.walking.clone();
                    self.notes
                        .offered
                        .insert(result.0, (objects.clone(), walking));
                }
            }
        }
        let Instruction::Effect { operation, result } = instruction else {
            return;
        };
        for &what in acted_on(operation) {
            self.notes.earlier.push((what, result.clone()));
        }
        let chosen = match operation {
            Operation::Move { objects, .. }
            | Operation::Reveal { objects }
            | Operation::Tap { objects, .. } => match objects {
                ObjectSet::Selected(chosen) => Some(chosen.0),
                _ => None,
            },
            _ => None,
        };
        if let Some(chosen) = chosen {
            self.notes.acted_of.insert(result.0, chosen);
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
                Instruction::Effect {
                    result,
                    operation: Operation::Move { objects, .. },
                } if lane_p::only_their_own(objects) => {
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
        let stated = planned::planned_value(
            "object collection MovedBy",
            "Vec::<ObjectId>::new()",
            &format!("q.moved_by(result_{}, {who})", result.0),
        );
        if !self.notes.own.contains(&result.0) {
            return Ok(stated);
        }
        let Ok(moved) = self.transition(result, phase) else {
            return Ok(stated);
        };
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
    pub(super) fn earlier_objects(&self, what: &EarlierObjects) -> Result<String> {
        if let (EarlierObjects::It, Some(one)) = (what, &self.moving_one) {
            return Ok(format!("vec![{one}]"));
        }
        let word = form(what);
        // "The permanents this spell targets", read as its total cost is worked
        // out (CR 601.2f): each permanent once, however many slots chose it.
        if matches!(what, EarlierObjects::Targeted) && self.costing_targets {
            return Ok("{ let mut chosen: Vec<ObjectId> = targets.iter().filter_map(|t| t.object()).collect(); chosen.sort_unstable(); chosen.dedup(); chosen }".into());
        }
        let step = self.notes.earlier.iter().rev().find(|(w, _)| *w == word);
        // "When you do, tap the copied creature": what the ability that made this
        // one chose, carried to it as its other objects are.
        if step.is_none() && matches!(what, EarlierObjects::Selected) {
            let outer = self.carrying().and_then(|outer| {
                outer.compiler.notes.earlier.iter().rev().find(|(w, _)| *w == word)
            });
            if let Some((_, result)) = outer {
                if let Some(code) = self.outside_objects(&ObjectSet::Selected(result.clone()))? {
                    return Ok(code);
                }
            }
        }
        let Some((_, result)) = step.filter(|_| self.reader == Reader::Resolution) else {
            return self.earlier_paid(what);
        };
        if let (EarlierObjects::Revealed, Some(found)) = (what, self.reveal_matches.get(&result.0))
        {
            return Ok(format!("{found}.clone()"));
        }
        // The card a reveal until found, and the permanent it became if it was put
        // onto the battlefield: "that permanent's mana value" (Jhoira, CR 400.7).
        if let (EarlierObjects::Found, Some(found)) = (what, self.reveal_matches.get(&result.0)) {
            // Jhoira: "You put that card onto the battlefield under your control and
            // lose life" is in the data the revealing opponent's own action.
            if self.line_text().is_some_and(|line| line.contains("under your control and lose life")) {
                return Err(gap("the life you lose read as the revealing player's, in the data"));
            }
            return Ok(format!("{found}.iter().map(|&id| q.state().obj(id).became.filter(|&now| q.state().obj(now).zone.is(ZoneKind::Battlefield)).unwrap_or(id)).collect::<Vec<ObjectId>>()"));
        }
        // What was sacrificed or countered is read as it last existed.
        if matches!(what, EarlierObjects::Sacrificed | EarlierObjects::Countered)
            && self.before.contains_key(&result.0)
        {
            return self.transition(result, &ObjectPhase::Before);
        }
        self.named(result)
    }
    /// What a mechanic of this object's own spell paid with, from the records just
    /// before it became cast: the whole is paid after any mana is made (CR
    /// 601.2g-h), so what a mana ability cost comes before it. A copy's is the
    /// original's (CR 707.10).
    fn paid_as_cast(&self, what: &EarlierObjects) -> Option<String> {
        let event = match what {
            EarlierObjects::Sacrificed if self.cast_pays.contains(&"Sacrificed") => {
                "Event::ZoneChange { old, from, reason: ZoneReason::Sacrifice, .. } if from.is(ZoneKind::Battlefield) => Some(old)"
            }
            EarlierObjects::Discarded if self.cast_pays.contains(&"Discarded") => {
                "Event::Discarded { card, .. } => Some(card)"
            }
            // Web-slinging's return is the spell's only cost that returns a
            // permanent, read as it last existed (CR 702.188a).
            EarlierObjects::Webslung => {
                "Event::ZoneChange { old, from, to, .. } if from.is(ZoneKind::Battlefield) && to.is(ZoneKind::Hand) => Some(old)"
            }
            _ => return None,
        };
        let me = if self.stack_item { "item.object" } else { "asker.source" };
        Some(format!(
            "{{ let records = q.history().all(); let spell = q.history().original_spell(records.iter().rev().find_map(|r| match r.event {{ Event::ZoneChange {{ old, new, reason: ZoneReason::Resolve, .. }} if new == {me} => Some(old), _ => None }}).unwrap_or({me})); records.iter().rposition(|r| matches!(r.event, Event::Cast(c) if c.spell == spell)).map_or_else(Vec::new, |cast| records[..cast].iter().rev().filter_map(|r| match r.event {{ {event}, _ => None }}).take(1).collect::<Vec<ObjectId>>()) }}"
        ))
    }
    /// The sacrifice of several players that put what's last read in a graveyard.
    pub(super) fn earlier_sacrifice(&self) -> Option<ResultId> {
        let word = "PutInGraveyard";
        let (_, result) = self.notes.earlier.iter().rev().find(|(w, _)| *w == word)?;
        self.shares
            .get(&result.0)
            .is_some_and(|(_, share)| matches!(share, Share::Picks(_)))
            .then(|| result.clone())
    }
    /// A walk's count of what each player moved: its players, the counts, and whom.
    pub(super) fn each_done<'a>(
        &'a self,
        objects: &'a ObjectSet,
    ) -> Option<(&'a String, &'a String, &'a PlayerRef)> {
        let ObjectSet::MovedBy {
            result,
            phase: ObjectPhase::After,
            player,
        } = objects
        else {
            return None;
        };
        let (players, each) = self.notes.each_done.get(&result.0)?;
        Some((players, each, player))
    }
    /// What one player of an earlier walk acted on, as chosen, so what has left
    /// reads as it last was (CR 608.2h). Cards revealed outside a walk came from a
    /// hand or a library, which hold only their owner's cards (CR 400.3).
    pub(super) fn earlier_by(&self, what: &EarlierObjects, binding: u32) -> Option<String> {
        if self.reader != Reader::Resolution {
            return None;
        }
        let word = form(what);
        let (_, result) = self.notes.earlier.iter().rev().find(|(w, _)| *w == word)?;
        let chosen = self.notes.acted_of.get(&result.0).unwrap_or(&result.0);
        let walked = self.notes.each.get(chosen).or(self.notes.each.get(&result.0));
        // "Choose a different opponent": each pick shuts out the rest, which a walk
        // of independent picks doesn't.
        if lane_k::printed_otherwise(self.line_text(), "a different") {
            return None;
        }
        let Some((players, each)) = walked else {
            // What a player shuffled into their library is what they own of it
            // (CR 400.3), when the players' cards went together.
            return matches!(what, EarlierObjects::Revealed | EarlierObjects::Shuffled).then(|| {
                let list = self.earlier_objects(what).ok()?;
                Some(format!("({list}).into_iter().filter(|&id: &ObjectId| q.state().obj(id).owner == bound_{binding}).collect::<Vec<ObjectId>>()"))
            })?;
        };
        let list = format!("{players}.iter().position(|&p| p == bound_{binding}).map_or_else(Vec::new, |at| {each}[at].clone())");
        Some(match what {
            // A commander that went elsewhere instead wasn't exiled (CR 903.9b).
            EarlierObjects::Exiled => format!("({list}).into_iter().filter(|&o| q.state().obj(o).became.is_some_and(|now| q.state().obj(now).zone.is(ZoneKind::Exile))).collect::<Vec<ObjectId>>()"),
            _ => list,
        })
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
            (
                EarlierObjects::Returned,
                CostPart::Move {
                    destination: Destination::OwnersHand,
                    ..
                },
            ) => true,
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
        if let Some(paid) = self.paid_as_cast(what) {
            return Ok(paid);
        }
        // "{T}, Unattach Shuriken": an ability Shuriken gave the creature it
        // equips, so what Shuriken was unattached from is that ability's source
        // (Shuriken, "unless it was unattached from a Ninja").
        let unattaches_granter = self.activation_cost.as_ref().is_some_and(|cost| {
            cost.parts
                .iter()
                .any(|part| matches!(part, CostPart::Unattach(ObjectRef::Granter)))
        });
        if matches!(what, EarlierObjects::Unattached) && unattaches_granter {
            return Ok("vec![asker.source]".into());
        }
        // "As an additional cost to cast this spell, choose a creature you control":
        // what was chosen is what that cost part chose, and the part's own hole
        // (cost part PlayerAction) already says what the file waits on.
        let chooses = |cost: &Cost| {
            let cost = format!("{cost:?}");
            cost.contains("PlayerAction") && cost.contains("ChooseObjects")
        };
        let chose_as_cost = self.activation_cost.as_ref().is_some_and(chooses)
            || self.casting_costs.values().any(chooses)
            || self.option_cost.as_ref().is_some_and(chooses);
        if matches!(what, EarlierObjects::Selected) && chose_as_cost {
            return Ok(planned::planned_value(FOLLOWS_GAP, NO_OBJECTS, "Vec::<ObjectId>::new()"));
        }
        let Some((payment, at)) = by_activation.or(by_casting) else {
            if exiled {
                return Ok(format!("q.exiled_with({})", self.linked()));
            }
            // The turn's own record of it.
            let last = |event: &str, card: &str| {
                format!("q.history().this_turn().iter().rev().find_map(|r| match r.event {{ {event} => Some({card}), _ => None }}).into_iter().collect::<Vec<ObjectId>>()")
            };
            match what {
                EarlierObjects::LastDrawn => {
                    return Ok(last(
                        "Event::Drew { player, card } if player == asker.controller",
                        "card",
                    ))
                }
                EarlierObjects::LastCast => return Ok(last("Event::Cast(cast)", "cast.spell")),
                // What a "one or more" ability triggered on (CR 603.2c).
                EarlierObjects::LeftGraveyard if self.stack_item => {
                    return Ok("q.event_objects(item).to_vec()".into())
                }
                // No step of its own: what the ability is about went there.
                EarlierObjects::PutInGraveyard if self.stack_item => {
                    if let Ok(card) = self.objects(&ObjectSet::One(ObjectRef::EventDestination)) {
                        return Ok(card);
                    }
                }
                _ => {}
            }
            // "The last chosen card", read by a static ability: an ability that
            // resolved chose it, and a resolving ability can keep only a name on
            // its permanent (`Game::remember_chosen_name`), never an object.
            // "As this Aura enters, choose a creature. Enchanted creature is a copy
            // of the chosen creature": what it chose as it entered, which it keeps.
            if matches!(what, EarlierObjects::Selected)
                && self.chose_object_as_enters
                && self.reader != Reader::Resolution
            {
                return Ok(format!(
                    "q.state().chosen_object({}).into_iter().collect::<Vec<ObjectId>>()",
                    self.linked()
                ));
            }
            // "Choose an exiled creature card used to craft this Equipment" as it
            // becomes attached: the choice is that ability's, whose own hole says
            // what the file waits on.
            if matches!(what, EarlierObjects::Selected)
                && self.functions.contains("AttachAction::ChooseAnExiledCardToCopy")
            {
                return Ok(planned::planned_value(FOLLOWS_GAP, NO_OBJECTS, "Vec::<ObjectId>::new()"));
            }
            // Read by a static or a trigger's event: an ability that resolved chose
            // it ("When Zenos enters, choose a creature ... When the chosen creature
            // leaves the battlefield"), and a resolving ability can keep only a name
            // on its permanent (`Game::remember_chosen_name`), never an object.
            if matches!(what, EarlierObjects::Selected) && self.reader != Reader::Resolution {
                return Err(gap("a chosen object outside its body"));
            }
            // "Each player turns face up all cards they own exiled with this, then
            // puts all permanent cards among them onto the battlefield" (Pyxis of
            // Pandemonium): each player's own cards of an earlier walk, which no
            // walk keeps for one that turns cards face up.
            if let (EarlierObjects::It, Some((walked, turned))) = (what, &self.turned_up) {
                if self.bound_players.contains(walked) {
                    return self.objects(&turned.clone());
                }
            }
            // "Pay {X} rather than its mana cost, where X is its mana value": the
            // card the cost is paid for, which a fixed cost can't read (a play
            // grant's `instead` and a spell alternative's `InsteadOfManaCost`).
            if matches!(what, EarlierObjects::It) && self.card_costed {
                return Ok(planned::planned_value(
                    lane_o::CARD_IT_IS_PAID_FOR,
                    "Vec::<ObjectId>::new()",
                    "vec![card]",
                ));
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
                let player = match chooser {
                    PlayerSet::One(player) => player,
                    // "Of an opponent's choice": the effect names no opponent, so the
                    // controller says which one chooses (CR 608.2d), asked only when
                    // there is more than one (`choose::opponent_chooses`).
                    PlayerSet::OpponentsOf(_) => {
                        let binding = ACTING + 100 + id as u32;
                        let players = self.fetch(self.players(&chooser)?);
                        writeln!(out, "let choosers_{id}: Vec<PlayerId> = {players}; let bound_{binding}: PlayerId = match choosers_{id}.len() {{ 0 => asker.controller, 1 => choosers_{id}[0], _ => cantrip_mechanics::choose::player(g, asker.controller, asker.source, choosers_{id}.clone(), \"Choose\", 0)?.unwrap_or(choosers_{id}[0]) }};").unwrap();
                        self.bound_players.insert(binding);
                        PlayerRef::Bound(Binding(binding))
                    }
                    _ => return Err(gap("object collection Chosen")),
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
        if !gives_flashback(changes) && flashback_price(changes).is_none() {
            return Ok(false);
        }
        let (EffectSubjects::Objects(cards), Duration::EndOfTurn) = (affected, duration) else {
            return Err(gap("granted Casting"));
        };
        // "Flashback {2}{R}{G}", where the words name a price of their own (CR 118.9).
        let instead = match flashback_price(changes) {
            Some(symbols) => format!("instead: Some({}), ", mana(symbols)?),
            None => String::new(),
        };
        let cards = self.fetch(self.objects(cards)?);
        writeln!(
            out,
            "let cards_{id}: Vec<ObjectId> = {cards}; for card in cards_{id} {{ let owner = g.obj(card).owner; g.allow_play(card, owner, PlayGrant {{ {instead}expiry: Expiry::EndOfTurn, flashback: true, ..PlayGrant::CAST }}); }}"
        )
        .unwrap();
        Ok(true)
    }
    /// "Each legendary card in your graveyard gains 'You may play this card from
    /// your graveyard' until end of turn": the cards there as this resolves (CR
    /// 611.2c), each playable from there by its owner, the "you" of an ability of
    /// a card in a graveyard (CR 109.5), for its own cost and at its usual time
    /// (CR 601.3, 305.1). A card that leaves is a new object without it (CR 400.7).
    /// Case of the Uneaten Feast is written so. Whether the effect was that.
    pub(super) fn play_this_until(
        &mut self,
        id: usize,
        affected: &EffectSubjects,
        changes: &[Change],
        duration: &Duration,
        out: &mut String,
    ) -> Result<bool> {
        let (EffectSubjects::Objects(cards), Duration::EndOfTurn, [Change::GrantAbility(ability)]) =
            (affected, duration, changes)
        else {
            return Ok(false);
        };
        let Ability::Scoped { zones, condition: None, ability } = &**ability else {
            return Ok(false);
        };
        let Ability::Static {
            kind: StaticKind::Ordinary,
            affected: EffectSubjects::Players(PlayerSet::One(PlayerRef::AbilityController)),
            changes,
        } = &**ability
        else {
            return Ok(false);
        };
        let [Change::PlayerRule(rule)] = changes.as_slice() else {
            return Ok(false);
        };
        let (permission, plain) = match rule {
            PlayerRule::PlayPermissionWith { permission, terms } => {
                let plain = terms.limit.is_none()
                    && terms.during.is_none()
                    && matches!(terms.cost, PlayCost::Normal)
                    && terms.spend_as.is_none()
                    && !terms.into_exile
                    && terms.condition.is_none()
                    && terms.effects.is_empty()
                    && !terms.flash
                    && !terms.may_look
                    && terms.then.is_none()
                    && terms.face_down.is_none();
                (&**permission, plain)
            }
            other => (other, true),
        };
        let PlayerRule::PlayPermission {
            cards: ObjectSet::One(ObjectRef::Source),
            from,
            action,
            filter: ObjectFilter::Any,
        } = permission
        else {
            return Ok(false);
        };
        if !plain || zones.as_slice() != [from.clone()] || !matches!(from, EventZone::Graveyard) {
            return Ok(false);
        }
        let lands = match action {
            CardPlayAction::CastSpell => "",
            CardPlayAction::Either => "lands: true, ",
            CardPlayAction::PlayLand => return Ok(false),
        };
        let cards = self.fetch(self.objects(cards)?);
        writeln!(
            out,
            "let cards_{id}: Vec<ObjectId> = {cards}; for card in cards_{id} {{ let owner = g.obj(card).owner; g.allow_play(card, owner, PlayGrant {{ {lands}expiry: Expiry::EndOfTurn, ..PlayGrant::CAST }}); }}"
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
        // "Flashback {1}": a price of its own (CR 118.9; Iroh, Grand Lotus).
        let price = flashback_price(changes);
        let escape = granted_escape(changes);
        if !gives_flashback(changes) && price.is_none() && escape.is_none() {
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
        // "Each nonland card in your graveyard has escape. The escape cost is
        // equal to the card's mana cost plus exile three other cards from your
        // graveyard" (Underworld Breach; CR 702.138a, 113.6): the cost of the
        // face cast, for a card that has a mana cost.
        if let Some(exiled) = escape {
            let body = format!(
                "({} && {printed}.has_cost).then(|| cantrip_mechanics::escape::cost({printed}.cost, {exiled}))",
                tests?.join(" && ")
            );
            self.permission_entry(
                None,
                "granted_ways",
                &format!("{label}_escape_cost"),
                (
                    "q: &dyn Query, asker: &Asker, card: ObjectId, player: PlayerId",
                    "q, asker, card, player",
                    "Option<Cost>",
                ),
                &body,
                |cost| format!("cantrip_mechanics::escape::granted({cost})"),
                parts,
            );
            return Ok(true);
        }
        let body = match price {
            Some(symbols) => format!(
                "({}).then(|| Cost::mana({}))",
                tests?.join(" && "),
                mana(symbols)?
            ),
            None => format!(
                "({} && {printed}.has_cost).then(|| Cost::mana({printed}.cost))",
                tests?.join(" && ")
            ),
        };
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
            // "That player" of an event that binds one, outside any row of players.
            PlayerRef::Prior(PriorPlayer::That)
                if self.row_player().is_err() && self.frame.player.is_some() =>
            {
                self.carried_player(self.frame.player)?
            }
            PlayerRef::Named { index, .. }
                if self.notes.named_players.get(*index as usize).is_some() =>
            {
                self.notes.named_players[*index as usize].clone()
            }
            PlayerRef::Performer(result) if self.notes.performers.contains_key(&result.0) => {
                self.notes.performers[&result.0].clone()
            }
            PlayerRef::Acting | PlayerRef::Prior(PriorPlayer::That) => match self.row_player() {
                Ok(row) => row,
                Err(_) if matches!(player, PlayerRef::Acting) => {
                    if let Some(payer) = &self.notes.acting {
                        return Ok(payer.clone());
                    }
                    return self.player_stated(player);
                }
                Err(_) => return Err(gap(feature)),
            },
            // "That player" of another ability of the object: nothing keeps it.
            PlayerRef::Prior(PriorPlayer::Remembered) => planned::planned_value(
                "player reference Prior",
                "None::<PlayerId>",
                "q.remembered_player(asker)",
            ),
            // "The player who cast Hellish Rebuke": the effect that gave the ability
            // has that player for its controller, and is named by the ability's stamp.
            PlayerRef::Prior(PriorPlayer::GrantedBy) if !self.rule_player => {
                "granted_by(q, asker)".to_string()
            }
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
            other => return self.player_stated(other),
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
            // "The number of Forests sacrificed as it entered": all it sacrificed.
            Expr::Earlier {
                what: EarlierObjects::Sacrificed,
                measure: ObjectMeasure::Count,
                result: None,
                by: None,
                ..
            } if self.enters_sacrifices && self.reader != Reader::Resolution => counted(format!(
                "q.state().chosen_number({}).map_or(0i64, i64::from)",
                self.linked()
            )),
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
                    // The roll that triggered this.
                    None if self.frame.amount.is_some() => self.number(&Expr::EventAmount)?,
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
            // "As you cast this spell" (CR 601.2i): kept in its cast record, so what
            // happens to the board after changes nothing, and a copy has none.
            Expr::AsOf {
                moment: Moment::ThisSpellCast,
                value,
            } if self.reader == Reader::Resolution && self.stack_item => {
                let mut asked = self.clone();
                asked.reader = Reader::Query;
                asked.stack_item = false;
                let code = asked.number(value)?.code;
                let mut fact = self.cast_fact.borrow_mut();
                if fact.as_ref().is_some_and(|other| *other != code) {
                    return Err(gap("two facts kept as a spell is cast"));
                }
                *fact = Some(code);
                counted("i64::from(q.history().fact_as_cast(item.object))".into())
            }
            // "Cast before that spell this turn": the casts as they were, in order.
            Expr::AsOf {
                moment: Moment::Cast(object),
                value,
            } if matches!(
                &**value,
                Expr::EventHistory {
                    event,
                    window: HistoryWindow::ThisTurn,
                    measure: HistoryMeasure::Occurrences,
                } if matches!(**event, EventSpec::Spell { event: SpellEvent::Cast, .. })
            ) =>
            {
                let Expr::EventHistory { event, .. } = &**value else {
                    unreachable!("matched above");
                };
                let EventSpec::Spell { players, spells, .. } = &**event else {
                    unreachable!("matched above");
                };
                let spell = self.object(object)?;
                let who = self.player_in(players, "cast.player")?;
                let earlier = self
                    .filter(spells, &Candidate::of("cast.spell"))?
                    .replace("q.chars(cast.spell)", "cast.chars(q)");
                counted(format!(
                    "q.history().casts().take_while(|cast| Some(cast.spell) != {spell}).filter(|cast| {who} && {earlier}).count() as i64"
                ))
            }
            Expr::AsOf {
                moment: Moment::TurnBegan,
                ..
            } => held_number(
                "engine: no record of the board as the turn began".into(),
                &format!("q.as_of(asker, {})", one_line(n)),
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
                &format!("q.draft_note_count(asker.controller, {})", one_line(n)),
            ),
            Expr::HeadsFlipped(result) if self.performed.contains_key(&result.0) => {
                counted(self.performed[&result.0].clone())
            }
            Expr::CoinFlipsWon(won) if self.coins.is_some() => {
                let (wins, losses) = self.coins.as_ref().expect("flipped");
                counted(if *won { wins.clone() } else { losses.clone() })
            }
            Expr::CoinFlipsWon(won) => held_number(
                "numeric expression CoinFlipsWon".into(),
                &format!("g.coin_flips(asker.controller, {won})"),
            ),
            Expr::TimesMutated(object) => held_number(
                "numeric expression TimesMutated".into(),
                &format!(
                    "{}.map_or(0i64, |o: ObjectId| cantrip_mechanics::mutate::times_mutated(q, o) as i64)",
                    self.object(object)?
                ),
            ),
            Expr::Votes(option) => held_number(
                "numeric expression Votes".into(),
                &format!("q.votes().count({})", self.vote_option(option)?),
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
                &format!("q.draft_noted_card({id}, {})", one_line(f)),
            ),
            other => return self.predicate_p(other, c),
        })
    }
    /// The rest of `condition`.
    pub(super) fn condition_more(&self, condition: &Condition) -> Result<String> {
        Ok(match condition {
            Condition::FirstDrawInOwnDrawStep(player) => format!(
                "{}.is_some_and(|p: PlayerId| cantrip_mechanics::history::first_draw_in_own_draw_step(q, p))",
                self.player(player)?
            ),
            Condition::Outcome(PriorOutcome::NoLifePaid) if self.notes.life_paid.is_some() => {
                format!("({} == 0)", self.notes.life_paid.as_deref().unwrap_or_default())
            }
            Condition::Outcome(outcome) => planned::planned_value(
                "condition Outcome",
                "false",
                &format!("q.outcome(asker, Outcome::{outcome:?})"),
            ),
            // "If you ... controlled a Dragon as you cast this spell" (CR 601.2i):
            // the board as the spell became cast, kept in its cast record
            // (`CardDef::cast_fact`, as Faerie Fencing keeps it), so a copy, which
            // was not cast (CR 707.10), never did.
            // The permanent the spell became reads its spell's cast record: the
            // record names the spell, whose row says what it became (CR 400.7).
            Condition::AsCast {
                player,
                action: AsCastAction::Controlled(set),
            } if matches!(
                format!("{player:?}").as_str(),
                "AbilityController" | "ControllerOf(EventSubject)" | "ControllerOf(Source)"
            ) =>
            {
                // Asked as the spell becomes cast: "you" is its caster.
                let mut asked = self.clone();
                asked.reader = Reader::Query;
                asked.stack_item = false;
                let objects = asked.objects(set)?;
                let code = format!(
                    "i64::from(({objects}).into_iter().any(|o: ObjectId| q.controller(o) == asker.controller))"
                );
                let mut fact = self.cast_fact.borrow_mut();
                if fact.as_ref().is_some_and(|other| *other != code) {
                    return Err(gap("two facts kept as a spell is cast"));
                }
                *fact = Some(code);
                // The spell itself, or the permanent it became; a copy, which was
                // not cast (CR 707.10), has no record.
                "q.history().casts().any(|c| (c.spell == asker.source || q.state().obj(c.spell).became == Some(asker.source)) && c.fact != 0)".into()
            }
            // "If you revealed a Dragon card as you cast this spell": what the
            // optional reveal cost showed, which waits on that cost.
            Condition::AsCast {
                action: AsCastAction::Revealed(_) | AsCastAction::Chose(_),
                ..
            } => planned::planned_value(
                "engine: no reveal cost",
                "false",
                &format!("q.as_cast(asker, {})", one_line(condition)),
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
                    None => planned::planned_value(
                        "condition PlayerTook",
                        "false",
                        &format!("q.took_part(asker, {}, {})", self.player(player)?, result.0),
                    ),
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
        TurnWindow::AfterUpkeep => format!("{step} > (Step::Upkeep as u8)"),
        TurnWindow::BeforeEndStep => format!("{step} < (Step::End as u8)"),
        TurnWindow::UntapStep => "q.state().turn.step() == Step::Untap".into(),
        TurnWindow::ExtraTurn => {
            "q.history().this_turn().iter().any(|r| matches!(r.event, Event::TurnBegan { extra: true, .. }))".into()
        }
        TurnWindow::BeforeFirstCombat => {
            "!q.history().this_turn().iter().any(|r| matches!(r.event, Event::StepBegan { step: Step::BeginCombat }))".into()
        }
        // Read as the ability that asks triggers, this step's beginning isn't history yet.
        TurnWindow::FirstEndStep => {
            "q.state().turn.step() == Step::End && q.history().this_turn().iter().filter(|r| matches!(r.event, Event::StepBegan { step: Step::End })).count() <= 1".into()
        }
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
/// The mana a grant of flashback names as its cost, when it names one.
/// "Has escape. The escape cost is equal to the card's mana cost plus exile N
/// other cards from your graveyard": N.
fn granted_escape(changes: &[Change]) -> Option<u8> {
    let [Change::GrantAbility(ability)] = changes else {
        return None;
    };
    let Ability::Mechanic {
        mechanic: Mechanic::Escape,
        cost: Some(cost),
        amount: None,
        filter: None,
    } = &**ability
    else {
        return None;
    };
    let [CostPart::ManaCostOf(ObjectRef::Source), CostPart::Move {
        selections,
        destination: Destination::Exile,
        ..
    }] = cost.parts.as_slice()
    else {
        return None;
    };
    let [selection] = selections.as_slice() else {
        return None;
    };
    let Some(Count::Exactly(Expr::Constant(exiled))) = &selection.choose else {
        return None;
    };
    let shape = format!("{:?}", selection.objects);
    let others = shape.contains("Graveyard")
        && shape.contains("Not(SameObject(Source))")
        && shape.contains("AbilityController");
    others.then_some(u8::try_from(*exiled).ok()?)
}
fn flashback_price(changes: &[Change]) -> Option<&[ManaSymbol]> {
    let [Change::GrantAbility(ability)] = changes else {
        return None;
    };
    let Ability::Casting(CastingRule::Permission {
        origin: CastingOrigin::Flashback,
        cost,
        leave_stack: Some(Destination::Exile),
        ..
    }) = &**ability
    else {
        return None;
    };
    match cost.parts.as_slice() {
        [CostPart::Mana(symbols)] => Some(symbols),
        _ => None,
    }
}

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
/// A value the generator can't write yet, as its words: a string, as
/// `planned::source` says one, not the IR's names standing as code.
fn one_line(value: &impl std::fmt::Debug) -> String {
    quoted(
        &format!("{value:?}")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
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
