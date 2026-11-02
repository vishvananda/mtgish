//! Trigger events beyond the plain ones of `triggers.rs`. An event Cantrip has
//! is an arm like any other. An event it can't fire yet still gets its arm, the
//! matcher it will have, and `Matched::planned` holds that arm out of the file's
//! compiled code: the whole ability goes in a `planned_item!`, with the body it
//! triggers compiled, so the event is all the card waits on.
use super::casting::target_range;
use super::costs::constant_u8;
use super::read::compare;
use super::triggers::{object_fired, single, Arm, Matched};
use super::*;
use crate::mtg_types::CardType;

const ONE: &str = "let subject = Target::Player(*player); let data = [0u32; 2];";

/// Frames of the rows below.
fn plain() -> Frame {
    Frame {
        object: Some(Carried::Subject),
        ..Frame::default()
    }
}
fn left() -> Frame {
    Frame {
        destination: Some(Carried::Data(0)),
        ..plain()
    }
}
fn about_player() -> Frame {
    Frame {
        player: Some(Carried::Subject),
        ..Frame::default()
    }
}
/// A player did something to `id`, the subject.
fn acted() -> Frame {
    Frame {
        object: Some(Carried::Subject),
        player: Some(Carried::Data(0)),
        ..Frame::default()
    }
}
/// A permanent's move that a player's event names, `data` holding where it went
/// and that player.
fn moved_by() -> Frame {
    Frame {
        object: Some(Carried::Subject),
        destination: Some(Carried::Data(0)),
        player: Some(Carried::Data(1)),
        ..Frame::default()
    }
}

/// A row of `object_event_rest`.
struct ObjectRow {
    mask: &'static str,
    pattern: &'static str,
    id: &'static str,
    data: &'static str,
    frame: Frame,
    extra: Option<&'static str>,
    phrase: &'static str,
    batch: Option<bool>,
    /// Cantrip has no such event yet.
    planned: bool,
    /// Cantrip files the event only in the history, where a question can read it.
    recorded: bool,
}
impl ObjectRow {
    fn new(
        mask: &'static str,
        pattern: &'static str,
        id: &'static str,
        phrase: &'static str,
    ) -> Self {
        Self {
            mask,
            pattern,
            id,
            data: "u32::MAX",
            frame: plain(),
            extra: None,
            phrase,
            batch: None,
            planned: false,
            recorded: false,
        }
    }
    fn planned(self) -> Self {
        Self {
            planned: true,
            ..self
        }
    }
    fn recorded(self) -> Self {
        Self {
            recorded: true,
            ..self
        }
    }
}

const ZONE: &str = "Event::ZoneChange { old, new, from, .. }";
const LEFT_BATTLEFIELD: &str = "from.is(ZoneKind::Battlefield)";

impl Compiler {
    /// The rest of `ObjectEvent`.
    pub(super) fn object_event_rest(
        &mut self,
        event: &ObjectEvent,
        objects: &ObjectFilter,
    ) -> Result<Matched> {
        type E = ObjectEvent;
        let departed = |pattern, extra, phrase| ObjectRow {
            data: "new.0",
            frame: left(),
            extra: Some(extra),
            ..ObjectRow::new("ZONE_CHANGE", pattern, "*old", phrase)
        };
        let row = match event {
            // The words are what make it look back on a card put onto the
            // battlefield from a graveyard (CR 603.10a).
            E::LeavesGraveyard => departed(
                ZONE,
                "from.is(ZoneKind::Graveyard) && !q.obj(*old).token && !q.obj(*old).card_copy",
                "Whenever this leaves a graveyard",
            ),
            E::LeavesWithoutDying => departed(
                "Event::ZoneChange { old, new, from, to, .. }",
                "from.is(ZoneKind::Battlefield) && !to.is(ZoneKind::Graveyard)",
                "Whenever this leaves the battlefield",
            ),
            E::Sacrificed => departed(
                "Event::ZoneChange { old, new, from, reason: ZoneReason::Sacrifice, .. }",
                LEFT_BATTLEFIELD,
                "Whenever this is sacrificed",
            ),
            E::Destroyed => departed(
                "Event::ZoneChange { old, new, from, reason: ZoneReason::Destroy | ZoneReason::DestroyNoRegen, .. }",
                LEFT_BATTLEFIELD,
                "Whenever this is destroyed",
            ),
            // One record for each creature that crewed it, at one cost.
            E::Crewed => ObjectRow {
                batch: Some(false),
                ..ObjectRow::new(
                    "CREWED",
                    "Event::Crewed { vehicle, .. }",
                    "*vehicle",
                    "Whenever this becomes crewed",
                )
            }
            .recorded(),
            E::Countered => ObjectRow::new(
                "COUNTERED",
                "Event::Countered { spell }",
                "*spell",
                "Whenever this is countered",
            ),
            E::TappedForTeamwork => ObjectRow::new(
                "TAPPED_FOR_TEAMWORK",
                "Event::TappedForTeamwork { creature, .. }",
                "*creature",
                "Whenever this becomes tapped to pay a teamwork cost",
            ),
            E::PhasesIn => ObjectRow::new(
                "PHASED_IN",
                "Event::PhasedIn { permanent }",
                "*permanent",
                "Whenever this phases in",
            )
            .recorded(),
            E::PhasesOut => ObjectRow::new(
                "PHASED_OUT",
                "Event::PhasedOut { permanent, .. }",
                "*permanent",
                "Whenever this phases out",
            )
            .recorded(),
            E::Explores => ObjectRow::new(
                "EXPLORED",
                "Event::Explored { permanent, .. }",
                "*permanent",
                "Whenever this explores",
            )
            .planned(),
            E::Connives => ObjectRow::new(
                "CONNIVED",
                "Event::Connived { creature, .. }",
                "*creature",
                "Whenever this connives",
            )
            .planned(),
            E::Fights => ObjectRow::new(
                "FOUGHT",
                "Event::Fought { creature, .. }",
                "*creature",
                "Whenever this fights",
            )
            .planned(),
            E::BecomesRenowned => ObjectRow::new(
                "RENOWNED",
                "Event::Renowned { permanent }",
                "*permanent",
                "Whenever this becomes renowned",
            )
            .planned(),
            E::BecomesSaddled => ObjectRow::new(
                "SADDLED",
                "Event::Saddled { permanent }",
                "*permanent",
                "Whenever this becomes saddled",
            )
            .planned(),
            E::Evolves => ObjectRow::new(
                "EVOLVED",
                "Event::Evolved { creature }",
                "*creature",
                "Whenever this evolves",
            )
            .planned(),
            E::Trains => ObjectRow::new(
                "TRAINED",
                "Event::Trained { creature }",
                "*creature",
                "Whenever this trains",
            )
            .planned(),
            E::Devoured => ObjectRow {
                data: "by.0",
                ..ObjectRow::new(
                    "DEVOURED",
                    "Event::Devoured { creature, by }",
                    "*creature",
                    "Whenever this is devoured",
                )
            }
            .planned(),
            E::Plotted => ObjectRow::new(
                "PLOTTED",
                "Event::Plotted { card }",
                "*card",
                "Whenever this becomes plotted",
            )
            .planned(),
            E::EchoPaid => ObjectRow::new(
                "ECHO_PAID",
                "Event::EchoPaid { permanent, .. }",
                "*permanent",
                "Whenever this creature's echo cost is paid",
            )
            .planned(),
            E::CumulativeUpkeepPaid => ObjectRow::new(
                "CUMULATIVE_UPKEEP_PAID",
                "Event::CumulativeUpkeepPaid { permanent, .. }",
                "*permanent",
                "Whenever this permanent's cumulative upkeep is paid",
            )
            .planned(),
            // A Saga's last chapter is known by its lore counters at the time
            // only to the core, which sacrifices it after this resolves.
            E::FinalChapterResolves => ObjectRow::new(
                "FINAL_CHAPTER",
                "Event::FinalChapterResolved { saga }",
                "*saga",
                "Whenever the final chapter ability of a Saga resolves",
            )
            .planned(),
            E::FinalChapterTriggers => ObjectRow::new(
                "FINAL_CHAPTER",
                "Event::FinalChapterTriggered { saga }",
                "*saga",
                "Whenever the final chapter ability of a Saga triggers",
            )
            .planned(),
            other => return Err(gap(format!("trigger event Object {}", form(other)))),
        };
        let mut tests: Vec<String> = row.extra.map(Into::into).into_iter().collect();
        tests.push(self.matching(row.frame, |c| c.filter(objects, &Candidate::of("id")))?);
        let mut matched = single(
            row.mask,
            Arm {
                pattern: row.pattern.into(),
                locals: object_fired(row.id, row.data),
                tests,
            },
            row.frame,
            row.phrase,
        );
        matched.batch = row.batch;
        if row.planned || (row.recorded && self.history.is_none()) {
            matched
                .planned
                .push(format!("trigger event Object {}", form(event)));
        }
        Ok(matched)
    }

    /// The rest of `PlayerEvent`.
    pub(super) fn player_event_rest(
        &mut self,
        event: &PlayerEvent,
        players: &PlayerSet,
        objects: Option<&ObjectFilter>,
    ) -> Result<Matched> {
        type E = PlayerEvent;
        // What a Room's doors are now says nothing of a past unlock.
        if matches!(event, E::FullyUnlocksRoom) && self.history.is_some() {
            return Err(gap(
                "trigger event Player FullyUnlocksRoom in a history question",
            ));
        }
        let about = about_player();
        const MOVED: &str = "let id = *new; let player = q.controller(id); let subject = Target::Object(id); let data = [new.0, u32::from(player.0)];";
        const LAND: &str = "Event::ZoneChange { new, from, to, reason: ZoneReason::PlayLand, .. }";
        const ON_BATTLEFIELD: &str = "to.is(ZoneKind::Battlefield)";
        const MILLED: &str = "let id = *old; let player = q.state().obj(id).owner; let subject = Target::Object(id); let data = [new.0, u32::from(player.0)];";
        type Row<'a> = (
            &'static str,
            &'a str,
            &'a str,
            &'a str,
            Frame,
            Option<&'a str>,
            &'static str,
            bool,
        );
        // Cantrip has no such event: the row is what the matcher will read.
        let hold = |mask, pattern, phrase| -> Row {
            (mask, pattern, ONE, "*player", about, None, phrase, true)
        };
        let row: Row = match event {
            E::BecomesMonarch => (
                "BECAME_MONARCH",
                "Event::BecameMonarch { player }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player becomes the monarch",
                false,
            ),
            E::RollsADie => (
                "DIE_ROLLED",
                "Event::DieRolled { player, result, .. }",
                "let subject = Target::Player(*player); let data = [(*result).max(0) as u32, 0];",
                "*player",
                Frame {
                    amount: Some(Carried::Data(0)),
                    ..about
                },
                None,
                "Whenever a player rolls a die",
                false,
            ),
            E::RollsNatural20 => (
                "DIE_ROLLED",
                "Event::DieRolled { player, natural: 20, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player rolls a natural 20",
                false,
            ),
            E::RollsHighestNatural => (
                "DIE_ROLLED",
                "Event::DieRolled { player, sides, natural, .. }",
                ONE,
                "*player",
                about,
                Some("natural == sides"),
                "Whenever a player rolls the highest number on a die",
                false,
            ),
            E::LosesCoinFlip => (
                "COIN_FLIPPED",
                "Event::CoinFlipped { player, result: CoinFlip::Lost }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player loses a coin flip",
                false,
            ),
            E::FlipsCoins => (
                "COIN_FLIPPED",
                "Event::CoinFlipped { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player flips a coin",
                false,
            ),
            E::GivesGift => (
                "GAVE_GIFT",
                "Event::GaveGift { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player gives a gift",
                false,
            ),
            E::FacesDilemma => (
                "MODES_CHOSEN",
                "Event::ModesChosen { player, modes, .. }",
                ONE,
                "*player",
                about,
                Some("*modes != 0"),
                "Whenever a player faces a dilemma",
                false,
            ),
            E::SolvesCase => (
                "SOLVED",
                "Event::Solved { permanent }",
                "let id = *permanent; let player = q.controller(id); let subject = Target::Player(player); let data = [0u32; 2];",
                "player",
                about,
                None,
                "Whenever a player solves a Case",
                false,
            ),
            E::TurnsFaceUp => (
                "TURNED_FACE_UP",
                "Event::TurnedFaceUp { permanent, player }",
                "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "*player",
                acted(),
                None,
                "Whenever a player turns a permanent face up",
                false,
            ),
            E::DrawsCard | E::DrawsRevealedCard if objects.is_some() => (
                "DREW",
                "Event::Drew { player, card }",
                "let id = *card; let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "*player",
                acted(),
                None,
                "Whenever a player draws a card",
                false,
            ),
            E::DrawsCard | E::DrawsRevealedCard => (
                "DREW",
                "Event::Drew { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player draws a card",
                false,
            ),
            E::EntersUnderControl | E::PutsOntoBattlefield => (
                "ZONE_CHANGE",
                "Event::ZoneChange { new, to, .. }",
                MOVED,
                "player",
                moved_by(),
                Some(ON_BATTLEFIELD),
                "Whenever a permanent enters",
                false,
            ),
            E::EntersUnplayed => (
                "ZONE_CHANGE",
                "Event::ZoneChange { new, to, reason, .. }",
                MOVED,
                "player",
                moved_by(),
                Some("to.is(ZoneKind::Battlefield) && !matches!(reason, ZoneReason::PlayLand | ZoneReason::Resolve)"),
                "Whenever a permanent enters",
                false,
            ),
            E::CreatesToken => (
                "ZONE_CHANGE",
                "Event::ZoneChange { new, to, reason: ZoneReason::Created, .. }",
                MOVED,
                "player",
                moved_by(),
                Some(ON_BATTLEFIELD),
                "Whenever a player creates a token",
                false,
            ),
            E::PlaysLandFromExile => (
                "ZONE_CHANGE",
                LAND,
                MOVED,
                "player",
                moved_by(),
                Some("to.is(ZoneKind::Battlefield) && from.is(ZoneKind::Exile)"),
                "Whenever a player plays a land",
                false,
            ),
            E::PlaysLandNotFromHand => (
                "ZONE_CHANGE",
                LAND,
                MOVED,
                "player",
                moved_by(),
                Some("to.is(ZoneKind::Battlefield) && !from.is(ZoneKind::Hand)"),
                "Whenever a player plays a land",
                false,
            ),
            E::MillsCard => (
                "ZONE_CHANGE",
                "Event::ZoneChange { old, new, from, reason: ZoneReason::Mill, .. }",
                MILLED,
                "player",
                moved_by(),
                Some("from.is(ZoneKind::Library)"),
                "Whenever a player mills a card",
                false,
            ),
            E::UnlocksDoor => (
                "DOOR_UNLOCKED",
                "Event::DoorUnlocked { permanent, door, player }",
                "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), u32::from(*door)];",
                "*player",
                acted(),
                Some("*door == THIS_FACES_DOOR"),
                "Whenever a player unlocks a door",
                true,
            ),
            E::FullyUnlocksRoom => (
                "DOOR_UNLOCKED",
                "Event::DoorUnlocked { permanent, player, .. }",
                "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "*player",
                acted(),
                Some("q.state().obj(id).marks.doors == DOOR_LEFT | DOOR_RIGHT"),
                "Whenever a player fully unlocks a Room",
                false,
            ),
            E::BecomesTarget => return self.player_targeted(players, objects),
            E::PlaysCard => return self.played(players, objects, None, event),
            E::PlaysCardFromExile => {
                return self.played(players, objects, Some("ZoneKind::Exile"), event)
            }
            E::Mills => (
                "ZONE_CHANGE",
                "Event::ZoneChange { old, new, from, reason: ZoneReason::Mill, .. }",
                MILLED,
                "player",
                moved_by(),
                Some("from.is(ZoneKind::Library)"),
                "Whenever a player mills one or more cards",
                false,
            ),
            E::Exerts => (
                "PAID_TO_ATTACK",
                "Event::PaidToAttack { attacker, option: AttackOption::Exert, .. }",
                "let id = *attacker; let player = q.controller(id); let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "player",
                acted(),
                None,
                "Whenever a player exerts a creature",
                false,
            ),
            E::TapsPermanent | E::LosesControl | E::RevealsFirstCardDrawn | E::PaysCumulativeUpkeep | E::DeclinesCumulativeUpkeep | E::SacrificesForEmerge | E::VisitsAttraction | E::UntapsDuringUntapStep => {
                let (mask, pattern, phrase) = match event {
                    E::TapsPermanent => ("TAPPED", "Event::Tapped { permanent, by: Some(player) }", "Whenever a player taps a permanent"),
                    E::LosesControl => ("CONTROL_CHANGED", "Event::ControlChanged { permanent, previous: player, .. }", "When a player loses control of this"),
                    E::RevealsFirstCardDrawn => ("REVEALED", "Event::RevealedAsDrawn { permanent, player, first: true }", "Whenever a player reveals the first card they drew this way"),
                    E::PaysCumulativeUpkeep => ("CUMULATIVE_UPKEEP_PAID", "Event::CumulativeUpkeepPaid { permanent, player }", "Whenever a player pays this permanent's cumulative upkeep"),
                    E::DeclinesCumulativeUpkeep => ("CUMULATIVE_UPKEEP_DECLINED", "Event::CumulativeUpkeepDeclined { permanent, player }", "When a player doesn't pay this permanent's cumulative upkeep"),
                    E::SacrificesForEmerge => ("SACRIFICED_FOR_EMERGE", "Event::SacrificedForEmerge { permanent, player, .. }", "When a player sacrifices this while casting a spell with emerge"),
                    E::VisitsAttraction => ("ATTRACTION_VISITED", "Event::AttractionVisited { permanent, player }", "Whenever a player visits an Attraction"),
                    _ => ("UNTAPPED_IN_UNTAP_STEP", "Event::UntappedInUntapStep { permanent, player }", "Whenever a player untaps a permanent during their untap step"),
                };
                (mask, pattern, "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), 0];", "*player", acted(), None, phrase, true)
            }
            E::Guesses => (
                "GUESSED",
                "Event::Guessed { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player guesses",
                true,
            ),
            E::GroupsIntoPiles => (
                "GROUPED_INTO_PILES",
                "Event::GroupedIntoPiles { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player groups objects into piles",
                true,
            ),
            E::CompletesDungeon => (
                "DUNGEON_COMPLETED",
                "Event::DungeonCompleted { player }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player completes a dungeon",
                self.history.is_none(),
            ),
            E::Shuffles => (
                "SHUFFLED",
                "Event::Shuffled { player }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player shuffles their library",
                self.history.is_none(),
            ),
            E::DayNightChanges => {
                hold("DAY_NIGHT", "Event::DayNightChanged { player, .. }", "Whenever day becomes night or night becomes day")
            }
            E::GetsEnergy => (
                "ENERGY",
                "Event::EnergyGained { player, amount }",
                "let subject = Target::Player(*player); let data = [*amount, 0];",
                "*player",
                Frame {
                    amount: Some(Carried::Data(0)),
                    ..about
                },
                None,
                "Whenever a player gets energy",
                true,
            ),
            E::PaysLife => (
                "LIFE_PAID",
                "Event::LifePaid { player, amount }",
                "let subject = Target::Player(*player); let data = [*amount, 0];",
                "*player",
                Frame {
                    amount: Some(Carried::Data(0)),
                    ..about
                },
                None,
                "Whenever a player pays life",
                true,
            ),
            E::Proliferates => hold("PROLIFERATED", "Event::Proliferated { player }", "Whenever a player proliferates"),
            E::Investigates => hold("INVESTIGATED", "Event::Investigated { player }", "Whenever a player investigates"),
            E::Discovers => hold("DISCOVERED", "Event::Discovered { player, .. }", "Whenever a player discovers"),
            E::Seeks => hold("SOUGHT", "Event::Sought { player, .. }", "Whenever a player seeks"),
            E::Conjures | E::ConjuresOther => hold("CONJURED", "Event::Conjured { player, .. }", "Whenever a player conjures a card"),
            E::Foretells => hold("FORETOLD", "Event::Foretold { player, .. }", "Whenever a player foretells a card"),
            E::Forages => hold("FORAGED", "Event::Foraged { player }", "Whenever a player forages"),
            E::CollectsEvidence => hold("EVIDENCE_COLLECTED", "Event::EvidenceCollected { player, .. }", "Whenever a player collects evidence"),
            E::ManifestsDread => hold("MANIFESTED_DREAD", "Event::ManifestedDread { player, .. }", "Whenever a player manifests dread"),
            E::Clashes => hold("CLASHED", "Event::Clashed { player, .. }", "Whenever a player clashes"),
            E::WinsClash => hold("CLASHED", "Event::Clashed { player, won: true, .. }", "Whenever a player wins a clash"),
            E::Bends => hold("BENT", "Event::Bent { player, .. }", "Whenever a player waterbends, earthbends, firebends or airbends"),
            E::RingTempts => hold("RING_TEMPTED", "Event::RingTempted { player }", "Whenever the Ring tempts a player"),
            E::ChoosesRingBearer => hold("RING_BEARER_CHOSEN", "Event::RingBearerChosen { player, .. }", "Whenever a player chooses a creature as their Ring-bearer"),
            E::FinishVoting => hold("VOTED", "Event::VotingFinished { player, .. }", "Whenever players finish voting"),
            E::BottomsWhileScrying => hold("BOTTOMED", "Event::BottomedWhileScrying { player, .. }", "Whenever a player puts cards on the bottom of their library while scrying"),
            E::DiscardsAtRandom => hold("DISCARDED", "Event::DiscardedAtRandom { player, .. }", "Whenever a player discards at random"),
            E::SelectsTargetsAtRandom => hold("RANDOM_TARGETS", "Event::TargetsAtRandom { player, .. }", "Whenever a player selects targets at random"),
            E::SelectsModeAtRandom => hold("RANDOM_MODES", "Event::ModesAtRandom { player, .. }", "Whenever a player chooses modes at random"),
            E::OpensAttraction => hold("ATTRACTION_OPENED", "Event::AttractionOpened { player, .. }", "Whenever a player opens an Attraction"),
            E::ClaimsPrize => hold("PRIZE_CLAIMED", "Event::PrizeClaimed { player, .. }", "Whenever a player claims the prize of an Attraction"),
            E::RollsToVisit => hold("ROLLED_TO_VISIT", "Event::RolledToVisit { player, .. }", "Whenever a player rolls to visit their Attractions"),
            E::PutsSticker | E::PutsNameSticker | E::PutsAbilitySticker | E::PutsArtSticker | E::PlacesSticker => {
                hold("STICKER", "Event::StickerPut { player, .. }", "Whenever a player puts a sticker on a permanent")
            }
            E::ChaosEnsues => hold("CHAOS_ENSUED", "Event::ChaosEnsued { player }", "Whenever chaos ensues"),
            E::SetsSchemeInMotion => hold("SCHEME_SET", "Event::SchemeSetInMotion { player, .. }", "When you set this scheme in motion"),
            E::Planeswalks => hold("PLANESWALKED", "Event::Planeswalked { player, .. }", "When you planeswalk to this plane"),
            E::PlaneswalksAway => hold("PLANESWALKED", "Event::PlaneswalkedAway { player, .. }", "When you planeswalk away from this plane"),
            E::EncountersPhenomenon => hold("PHENOMENON", "Event::EncounteredPhenomenon { player, .. }", "When you encounter this phenomenon"),
            E::RollsPlanarDie => hold("PLANAR_DIE", "Event::PlanarDieRolled { player, .. }", "Whenever a player rolls the planar die"),
            E::RollsPlanarBlank => hold("PLANAR_DIE", "Event::PlanarDieRolled { player, blank: true, .. }", "Whenever a player rolls a blank on the planar die"),
            E::GameBegins => hold("GAME_BEGAN", "Event::GameBegan { player }", "At the beginning of the game"),
            other => return Err(gap(format!("trigger event Player {}", form(other)))),
        };
        let (mask, pattern, locals, who, mut frame, test, phrase, planned) = row;
        let mut pattern = pattern.to_string();
        let mut locals = locals.to_string();
        let wanted = objects.filter(|objects| !matches!(objects, ObjectFilter::Any));
        match (wanted, frame.object) {
            (Some(_), None) if planned => {
                // The event it will be names the card too.
                pattern = pattern.replacen("{ player", "{ player, object", 1);
                locals = "let id = *object; let subject = Target::Object(id); let data = [u32::from(player.0), 0];".into();
                frame = acted();
            }
            (Some(_), None) => {
                return Err(gap(format!("trigger event Player {} objects", form(event))))
            }
            _ => {}
        }
        // "An untapped creature" says what tapping it needs, not what it is after.
        let held;
        let objects = match objects {
            Some(objects) if matches!(event, E::TapsPermanent) => {
                held = without_untapped(objects);
                Some(&held)
            }
            other => other,
        };
        let mut tests: Vec<String> = test.map(Into::into).into_iter().collect();
        tests.push(self.matching(frame, |c| c.player_in(players, who))?);
        if let (Some(objects), Some(_)) = (objects, frame.object) {
            tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
        }
        let mut matched = single(
            mask,
            Arm {
                pattern,
                locals,
                tests,
            },
            frame,
            phrase,
        );
        // "One or more", one for each player whose cards they were.
        if matches!(event, E::Mills) {
            matched.batch = Some(true);
        }
        if planned {
            matched
                .planned
                .push(format!("trigger event Player {}", form(event)));
        }
        Ok(matched)
    }

    /// The rest of `PairEvent`, whose tests the caller makes.
    pub(super) fn pair_event_rest(
        &mut self,
        event: &PairEvent,
        objects: &ObjectFilter,
        others: &ObjectFilter,
    ) -> Result<Matched> {
        let about = Frame {
            object: Some(Carried::Subject),
            other: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let (mask, pattern, id, other, extra, phrase, planned): (
            &'static str,
            &str,
            &str,
            &str,
            Option<&str>,
            &str,
            bool,
        ) = match event {
            PairEvent::BecomesUnattachedFrom => (
                "UNATTACHED",
                "Event::Unattached { attachment, from }",
                "*attachment",
                "*from",
                None,
                "Whenever this becomes unattached from a permanent",
                false,
            ),
            PairEvent::EntersAttachedTo => (
                "ZONE_CHANGE",
                "Event::ZoneChange { new, to, .. }",
                "*new",
                "q.state().obj(*new).attached_to.unwrap_or(*new)",
                Some("to.is(ZoneKind::Battlefield) && q.state().obj(*new).attached_to.is_some()"),
                "When this enters attached to a permanent",
                false,
            ),
            PairEvent::Enlists => (
                "PAID_TO_ATTACK",
                "Event::PaidToAttack { attacker, option: AttackOption::Enlist, tapped, .. }",
                "*attacker",
                "*tapped",
                None,
                "Whenever this creature enlists a creature",
                false,
            ),
            // The object it was and what it became are one object (CR 701.27).
            PairEvent::TransformsInto => (
                "TRANSFORMED",
                "Event::Transformed { object }",
                "*object",
                "*object",
                None,
                "Whenever this transforms",
                false,
            ),
            PairEvent::Mentors => (
                "MENTORED",
                "Event::Mentored { creature, mentored }",
                "*creature",
                "*mentored",
                None,
                "Whenever this mentors a creature",
                true,
            ),
            PairEvent::Stations => (
                "STATIONED",
                "Event::Stationed { creature, spacecraft }",
                "*creature",
                "*spacecraft",
                None,
                "Whenever a creature stations this",
                true,
            ),
            PairEvent::ChampionedWith => (
                "CHAMPIONED",
                "Event::Championed { card, champion }",
                "*card",
                "*champion",
                None,
                "When a card is championed with this",
                true,
            ),
            other => return Err(gap(format!("trigger event Pair {}", form(other)))),
        };
        let mut tests: Vec<String> = extra.map(Into::into).into_iter().collect();
        tests.push(self.matching(about, |c| c.filter(objects, &Candidate::of("id")))?);
        tests.push(self.matching(about, |c| c.filter(others, &Candidate::of("other")))?);
        let mut matched = single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: format!("let id = {id}; let other = {other}; let subject = Target::Object(id); let data = [other.0, u32::MAX];"),
                tests,
            },
            about,
            phrase,
        );
        if planned {
            matched
                .planned
                .push(format!("trigger event Pair {}", form(event)));
        }
        Ok(matched)
    }

    /// The rest of `EventSpec`.
    pub(super) fn event_rest(&mut self, event: &EventSpec) -> Result<Matched> {
        match event {
            EventSpec::ZoneChangeFromElsewhere {
                objects,
                except,
                to,
            } => {
                let mut matched = self.zone_change(objects, None, to)?;
                for zone in except {
                    let test = format!("!from.is({})", zone_kind(zone)?);
                    matched.arms[0].tests.push(test);
                }
                Ok(matched)
            }
            EventSpec::NthCounter { kind, objects, nth } => {
                let kind = match &**kind {
                    Operand::Vocabulary { form, .. } => form.as_str(),
                    Operand::Term { form, operands, .. }
                        if form == "PTCounter"
                            && matches!(
                                operands.as_slice(),
                                [Operand::Integer(1), Operand::Integer(1)]
                            ) =>
                    {
                        "PlusOne"
                    }
                    _ => return Err(gap("trigger event NthCounter of a kind not named")),
                };
                let kind = kind.strip_suffix("Counter").unwrap_or(kind);
                let named = format!("CounterKind::{kind}");
                let kind = if crate::cantrip_vocab::COUNTERS.contains(&kind) {
                    named
                } else {
                    planned::planned_value(
                        &format!("counter kind {kind}"),
                        "CounterKind::Charge",
                        &named,
                    )
                };
                let frame = Frame {
                    amount: Some(Carried::Data(0)),
                    ..plain()
                };
                let (test, nth) = self.matching(frame, |c| {
                    Ok((
                        c.filter(objects, &Candidate::of("id"))?,
                        c.number(nth)?.code,
                    ))
                })?;
                Ok(single(
                    "COUNTERS",
                    Arm {
                        pattern: "Event::CountersPut { object, kind, amount, total, .. }".into(),
                        locals: "let id = *object; let subject = Target::Object(id); let data = [*amount as u32, id.0];".into(),
                        tests: vec![
                            format!("*kind == {kind}"),
                            test,
                            format!("i64::from(*total - *amount) < {nth}"),
                            format!("i64::from(*total) >= {nth}"),
                        ],
                    },
                    frame,
                    "Whenever a counter is put on this",
                ))
            }
            EventSpec::FirstForObjectEachTurn(event) => {
                self.first_of(event, "q.history().this_turn().iter()", true)
            }
            EventSpec::FirstEachGame(event) => {
                self.first_of(event, "q.history().all().iter()", false)
            }
            EventSpec::AttackUnblocked {
                attackers,
                defenders,
                count,
            } => self.attack_unblocked(attackers, defenders, count.as_ref()),
            EventSpec::AttacksWith {
                object,
                others,
                count,
                by,
                different_defenders,
            } => self.attacks_with(object, others, count, by.as_ref(), *different_defenders),
            EventSpec::AttacksAsGroup {
                by,
                attackers,
                group,
            } => self.attacks_as_group(by, attackers, group),
            EventSpec::AllAttack(_) if self.history.is_some() => {
                Err(gap("who attacked together, in a history question"))
            }
            EventSpec::AllAttack(filter) => {
                let frame = about_player();
                let every = self.matching(frame, |c| c.filter(filter, &Candidate::of("id")))?;
                let attacking = "q.state().attackers.iter().any(|a| a.id == id)";
                Ok(single(
                    "ATTACKERS_DECLARED",
                    Arm {
                        pattern: "Event::AttackersDeclared { player, count }".into(),
                        locals: ONE.into(),
                        tests: vec![
                            "*count > 0".into(),
                            format!("{{ let those: Vec<ObjectId> = q.state().battlefield().iter().copied().filter(|&id| {every}).collect(); !those.is_empty() && those.into_iter().all(|id| {attacking}) }}"),
                        ],
                    },
                    frame,
                    "Whenever all these creatures attack",
                ))
            }
            EventSpec::SpellTargeting {
                players,
                spells,
                targets,
                count: None,
            } => {
                let spell = EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: players.clone(),
                    spells: spells.clone(),
                };
                let mut matched = self.event(&spell)?;
                if let Some(targets) = targets {
                    let frame = matched.frame;
                    let set = self.matching(frame, |c| c.objects(targets))?;
                    let test = format!("q.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == id).is_some_and(|i| {{ let wanted: Vec<ObjectId> = {set}; q.state().targets(i).iter().filter_map(|t| t.object()).any(|o| wanted.contains(&o)) }})");
                    matched.arms[0].tests.push(test);
                }
                Ok(matched)
            }
            EventSpec::SpellTargeting {
                players,
                spells,
                targets: None,
                count: Some((comparison, value)),
            } => {
                let spell = EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: players.clone(),
                    spells: spells.clone(),
                };
                let mut matched = self.event(&spell)?;
                let frame = matched.frame;
                if frame.amount.is_some() || format!("{frame:?}").contains("Data(1)") {
                    return Err(gap("trigger event spell targeting"));
                }
                let value = self.matching(frame, |c| Ok(c.number(value)?.code))?;
                let arm = &mut matched.arms[0];
                arm.locals.push_str(" let many = q.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == id).map_or(0, |i| q.state().targets(i).len()) as i64; let data = [data[0], many as u32];");
                arm.tests.push(read::compare("many", comparison, &value));
                matched.frame.amount = Some(Carried::Data(1));
                Ok(matched)
            }
            EventSpec::SpellOrActivation {
                players,
                spells,
                abilities,
            } => {
                let mut matched = self.event(&EventSpec::Either(vec![
                    EventSpec::Spell {
                        event: SpellEvent::Cast,
                        players: players.clone(),
                        spells: spells.clone(),
                    },
                    EventSpec::Activated {
                        players: players.clone(),
                        abilities: abilities.clone(),
                    },
                ]))?;
                matched.frame.spell_or_ability = true;
                Ok(matched)
            }
            EventSpec::TappedForManaOf { objects, by, mana } => {
                self.tapped_for_mana_of(objects, by.as_ref(), mana)
            }
            EventSpec::ManaAdded {
                abilities, players, ..
            } => {
                let frame = about_player();
                let tests = self.matching(frame, |c| {
                    Ok(vec![
                        c.player_in(players, "*player")?,
                        c.filter(abilities, &Candidate::of("*source"))?,
                    ])
                })?;
                Ok(held_arm(
                    "MANA_ADDED",
                    "Event::ManaAdded { player, source, produced }",
                    ONE,
                    tests,
                    frame,
                    "Whenever an ability causes a player to add mana",
                    "trigger event ManaAdded",
                ))
            }
            EventSpec::PaysToActivate {
                players,
                abilities,
                payment,
            } => {
                let frame = Frame {
                    activated: Some(Carried::Subject),
                    object: Some(Carried::Data(0)),
                    amount: Some(Carried::Data(1)),
                    ..Frame::default()
                };
                let mut tests = vec![self.activation_test(abilities, frame)?];
                tests.push(self.matching(frame, |c| c.player_in(players, "*player"))?);
                let paid = match payment {
                    ActivationPayment::Life => "paid.life > 0",
                    _ => "!paid.sacrificed.is_empty()",
                };
                tests.push(paid.into());
                Ok(held_arm(
                    "ABILITY_ACTIVATED",
                    "Event::AbilityActivated { source, ability, player, .. }",
                    "let id = *source; let def = q.activated_ability(*ability); let paid = q.paid_to_activate(event); let subject = cantrip_mechanics::triggers::activated_ability_on_stack(q, event).unwrap_or(Target::None); let data = [id.0, paid.life];",
                    tests,
                    frame,
                    "Whenever a player activates an ability",
                    "trigger event PaysToActivate",
                ))
            }
            EventSpec::Caused { cause, event } => self.caused(cause, event),
            EventSpec::CausesTrigger { cause, .. } => {
                let frame = Frame {
                    activated: Some(Carried::Data(0)),
                    stack_controller: Some(Carried::Data(1)),
                    ..plain()
                };
                let mut matched = self.event(cause)?;
                let tests = std::mem::take(&mut matched.arms);
                let cause_test = tests
                    .iter()
                    .map(|arm| {
                        format!(
                            "(if let {} = cause {{ {} {} }} else {{ false }})",
                            arm.pattern,
                            arm.locals,
                            arm.tests.join(" && ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" || ");
                let mut held = single(
                    "TRIGGERED",
                    Arm {
                        pattern: "Event::Triggered { source, ability, cause }".into(),
                        locals: "let id = *source; let subject = Target::Object(id); let on_stack = cantrip_mechanics::triggers::triggered_ability_on_stack(q, event); let data = [on_stack.map_or(0, |i: StackItem| i.serial), on_stack.map_or(0, |i: StackItem| u32::from(i.controller.0))]; let cause = &*cause;".into(),
                        tests: vec![cause_test],
                    },
                    frame,
                    "Whenever an ability triggers",
                );
                held.planned.push("trigger event CausesTrigger".into());
                Ok(held)
            }
            EventSpec::ExploresCard { objects, cards } => {
                let frame = Frame {
                    other: Some(Carried::Data(0)),
                    ..plain()
                };
                let (id, other) = self.matching(frame, |c| {
                    Ok((
                        c.filter(objects, &Candidate::of("id"))?,
                        c.filter(cards, &Candidate::of("other"))?,
                    ))
                })?;
                Ok(held_arm(
                    "EXPLORED",
                    "Event::Explored { permanent, revealed }",
                    "let id = *permanent; let other = *revealed; let subject = Target::Object(id); let data = [other.0, u32::MAX];",
                    vec![id, other],
                    frame,
                    "Whenever this explores",
                    "trigger event ExploresCard",
                ))
            }
            EventSpec::ClassLevel { objects, level } => {
                let frame = plain();
                let tests = self.matching(frame, |c| {
                    let level = c.number(level)?.amount(u8::MAX.into(), "u8")?;
                    Ok(vec![
                        c.filter(objects, &Candidate::of("id"))?,
                        format!("*level == {level}"),
                    ])
                })?;
                Ok(single(
                    "CLASS_LEVEL",
                    Arm {
                        pattern: "Event::ClassLevel { object, level }".into(),
                        locals: object_fired("*object", "u32::MAX"),
                        tests,
                    },
                    frame,
                    "When this Class becomes that level",
                ))
            }
            EventSpec::ControlChanges { objects, to, from } => {
                let frame = plain();
                let tests = self.matching(frame, |c| {
                    Ok(vec![
                        c.filter(objects, &Candidate::of("id"))?,
                        c.player_in(to, "*controller")?,
                        c.player_in(from, "*previous")?,
                    ])
                })?;
                Ok(held_arm(
                    "CONTROL_CHANGED",
                    "Event::ControlChanged { permanent, controller, previous }",
                    &object_fired("*permanent", "u32::MAX"),
                    tests,
                    frame,
                    "Whenever control of this changes",
                    "trigger event ControlChanges",
                ))
            }
            EventSpec::CountersOnPlayer { by, players } => {
                let frame = about_player();
                let tests = self.matching(frame, |c| {
                    Ok(vec![
                        c.player_in(by, "*by")?,
                        c.player_in(players, "*player")?,
                    ])
                })?;
                Ok(held_arm(
                    "PLAYER_COUNTERS",
                    "Event::PlayerCountersPut { player, by, .. }",
                    ONE,
                    tests,
                    frame,
                    "Whenever counters are put on a player",
                    "trigger event CountersOnPlayer",
                ))
            }
            EventSpec::OntoStack(filter) => {
                let frame = Frame {
                    stack_object: Some(Carried::Subject),
                    ..plain()
                };
                let test = self.matching(frame, |c| c.filter(filter, &Candidate::of("id")))?;
                Ok(held_arm(
                    "ONTO_STACK",
                    "Event::PutOnStack { object, .. }",
                    &object_fired("*object", "u32::MAX"),
                    vec![test],
                    frame,
                    "Whenever a spell or ability is put onto the stack",
                    "trigger event OntoStack",
                ))
            }
            EventSpec::TargetsChosen { players, .. } => {
                let frame = Frame {
                    stack_object: Some(Carried::Subject),
                    ..plain()
                };
                let test = self.matching(frame, |c| c.player_in(players, "*player"))?;
                Ok(held_arm(
                    "TARGETS_CHOSEN",
                    "Event::TargetsChosen { object, player }",
                    &object_fired("*object", "u32::MAX"),
                    vec![test],
                    frame,
                    "Whenever a player chooses targets",
                    "trigger event TargetsChosen",
                ))
            }
            EventSpec::WhileActivating {
                event,
                players,
                abilities,
            } => self.while_activating(event, players, abilities),
            EventSpec::Resolves(filter) => {
                let frame = plain();
                let test = self.activation_test(filter, frame)?;
                Ok(held_arm(
                    "RESOLVED",
                    "Event::Resolved { source, ability }",
                    "let id = *source; let def = q.activated_ability(*ability); let subject = Target::Object(id); let data = [u32::MAX; 2];",
                    vec![test],
                    frame,
                    "Whenever an ability resolves",
                    "trigger event Resolves",
                ))
            }
            EventSpec::ObjectBecomes(filter) => {
                let frame = plain();
                let test = self.matching(frame, |c| c.filter(filter, &Candidate::of("id")))?;
                Ok(held_arm(
                    "OBJECT_STATE",
                    "Event::StateChecked { object }",
                    &object_fired("*object", "u32::MAX"),
                    vec![test],
                    frame,
                    "Whenever this comes to be so",
                    "trigger event ObjectBecomes",
                ))
            }
            other => Err(gap(format!("trigger event {}", form(other)))),
        }
    }

    /// The first such event in `scope`, of its own subject when `by_subject`.
    fn first_of(&mut self, event: &EventSpec, scope: &str, by_subject: bool) -> Result<Matched> {
        if self.history.is_some() {
            return Err(gap("an ordinal event in a history question"));
        }
        let mut matched = self.event(event)?;
        let mut asked = self.clone();
        asked.history = Some(History {
            outer: None,
            event_at: EventAt::Matcher,
            frame: Frame::default(),
        });
        let past = asked.event(event)?;
        let same = if by_subject {
            ".filter(|f: &Fired| f.subject == subject)"
        } else {
            ""
        };
        let test = format!(
            "{scope}.filter_map({}){same}.count() == 0",
            past.history_test()
        );
        // Summed events are one happening: the rest of it is no earlier one.
        let test = match matched.sums {
            true => planned::planned_value(
                "an ordinal of grouped events",
                "false",
                &format!(
                    "{scope}.filter(|r| !r.in_batch_of(event)).filter_map({}){same}.count() == 0",
                    past.history_test()
                ),
            ),
            false => test,
        };
        for arm in &mut matched.arms {
            arm.tests.push(test.clone());
        }
        Ok(matched)
    }

    /// `EventSpec::TappedForManaOf`, a triggered mana ability like any on
    /// `ObjectEvent::TappedForMana`.
    fn tapped_for_mana_of(
        &mut self,
        objects: &ObjectFilter,
        by: Option<&PlayerSet>,
        mana: &Operand,
    ) -> Result<Matched> {
        let frame = acted();
        let Operand::Term { form, .. } = mana else {
            return Err(gap("trigger event TappedForManaOf of unnamed mana"));
        };
        let test = match form.as_str() {
            "ManaProduceC" => "produced.get(Mana::C) > 0".to_string(),
            "ManaOfTheChosenColor" => {
                let chosen = self.matching(frame, |c| c.chosen_value(ValueKind::Color))?;
                format!("{chosen}.is_some_and(|chosen| Mana::ALL.iter().any(|m| m.color() == Some(chosen) && produced.get(*m) > 0))")
            }
            other => return Err(gap(format!("trigger event TappedForManaOf {other}"))),
        };
        let mut tests = vec![test];
        tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
        if let Some(by) = by {
            tests.push(self.matching(frame, |c| c.player_in(by, "*player"))?);
        }
        let mut matched = single(
            "TAPPED",
            Arm {
                pattern: "Event::TappedForMana { permanent, player, produced }".into(),
                locals: "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), 0];".into(),
                tests,
            },
            frame,
            "Whenever a permanent is tapped for mana",
        );
        matched.mana = true;
        Ok(matched)
    }

    /// `PlayerEvent::BecomesTarget`. A filter of `Any` is "a spell or ability":
    /// "of a spell" and "of an ability" each say so.
    fn player_targeted(
        &mut self,
        players: &PlayerSet,
        objects: Option<&ObjectFilter>,
    ) -> Result<Matched> {
        let by = match objects {
            None => {
                return Err(gap(
                    "trigger event Player BecomesTarget of what is not said",
                ))
            }
            Some(filter) => filter,
        };
        let frame = Frame {
            player: Some(Carried::Subject),
            stack_controller: Some(Carried::Data(1)),
            ..Frame::default()
        };
        let mut tests = self.matching(frame, |c| {
            Ok(vec![
                c.player_in(players, "*target")?,
                match by {
                    ObjectFilter::ControlledBy(controllers) => {
                        c.player_in(controllers, "controller")?
                    }
                    other => c.filter(other, &Candidate::of("by"))?,
                },
            ])
        })?;
        Ok(single(
            "BECAME_TARGET",
            Arm {
                pattern: "Event::BecameTarget { target: Target::Player(target), by, controller, .. }".into(),
                locals: "let by = *by; let controller = *controller; let subject = Target::Player(*target); let data = [by.0, u32::from(controller.0) | (q.state().stack.len().saturating_sub(1) as u32) << 8];".into(),
                tests,
            },
            frame,
            "Whenever a player becomes the target of a spell or ability",
        ))
    }

    /// "Plays a card": casts it or plays it as a land, `from` a zone if one is
    /// said. Only the zone can be read of a card that's left it.
    fn played(
        &mut self,
        players: &PlayerSet,
        objects: Option<&ObjectFilter>,
        from: Option<&str>,
        event: &PlayerEvent,
    ) -> Result<Matched> {
        let any = ObjectFilter::Any;
        let exiled_with = objects.and_then(lane_n::played_from_exile_with);
        let spells = match objects {
            None => &any,
            _ if exiled_with.is_some() => &any,
            Some(ObjectFilter::InSet(set))
                if matches!(
                    &**set,
                    ObjectSet::Zone {
                        filter: ObjectFilter::Any,
                        ..
                    }
                ) =>
            {
                &any
            }
            Some(filter) if from.is_none() => filter,
            Some(_) => {
                return Err(gap(format!(
                    "trigger event Player {} of a set of cards",
                    form(event)
                )))
            }
        };
        let mut matched = self.event(&EventSpec::Either(vec![
            EventSpec::Spell {
                event: SpellEvent::Cast,
                players: players.clone(),
                spells: spells.clone(),
            },
            EventSpec::Player {
                event: PlayerEvent::PlaysLand,
                players: players.clone(),
                objects: Some(spells.clone()),
            },
        ]))?;
        if let Some(zone) = from {
            matched.arms[0].tests.push(format!(
                "cast_of(q, id).is_some_and(|cast| cast.from == {zone})"
            ));
            matched.arms[1].pattern =
                "Event::ZoneChange { new, from, to, reason: ZoneReason::PlayLand, .. }".into();
            matched.arms[1].tests.push(format!("from.is({zone})"));
        }
        if let Some(object) = exiled_with {
            let by = self.matching(matched.frame, |c| c.object(object))?;
            let test = planned::planned_value(
                "a card played from among those exiled with an object",
                "false",
                &format!("{by}.is_some_and(|by: ObjectId| q.was_exiled_with(id, by))"),
            );
            for arm in &mut matched.arms {
                arm.tests.push(test.clone());
            }
        }
        Ok(matched)
    }

    /// "Whenever you expend N" (CR 700.14): the cast whose payment takes the mana
    /// spent to cast spells this turn from under N to N or more.
    pub(super) fn expends(
        &mut self,
        players: &PlayerSet,
        comparison: &Comparison,
        value: &Expr,
    ) -> Result<Matched> {
        if !matches!(comparison, Comparison::Equal) || self.history.is_some() {
            return Err(gap("trigger event Player Expends"));
        }
        let frame = about_player();
        let (who, value) = self.matching(frame, |c| {
            Ok((c.player_in(players, "*player")?, c.number(value)?.code))
        })?;
        let before = "q.history().this_turn().iter().filter_map(|r| match r.event { Event::ManaSpent { player: p, on, cast: true, mana, .. } if p == *player && on != *spell => Some(i64::from(mana.total())), _ => None }).sum::<i64>()";
        let paid = "q.history().mana_spent_to_cast(*spell).map(|(_, mana)| i64::from(mana.total())).sum::<i64>()";
        Ok(single(
            "SPELL_CAST",
            Arm {
                pattern: "Event::SpellCast { spell, player, .. }".into(),
                locals: ONE.into(),
                tests: vec![
                    who,
                    format!("{before} < {value}"),
                    format!("{before} + {paid} >= {value}"),
                ],
            },
            frame,
            "Whenever a player expends",
        ))
    }

    /// `EventSpec::AttackUnblocked`.
    fn attack_unblocked(
        &mut self,
        attackers: &ObjectFilter,
        defenders: &AttackDefenders,
        count: Option<&(Comparison, Expr)>,
    ) -> Result<Matched> {
        let grouped = match count {
            None => false,
            Some((Comparison::GreaterOrEqual, Expr::Constant(1))) => true,
            Some(_) => return Err(gap("trigger event AttackUnblocked of a count")),
        };
        // "That player" is the one attacked, or with "one or more creatures an
        // opponent controls attack you" the one attacking.
        let frame = Frame {
            object: (!grouped).then_some(Carried::Subject),
            other_player: Some(Carried::Data(0)),
            player: Some(Carried::Data(usize::from(grouped))),
            ..Frame::default()
        };
        let tests = self.matching(frame, |c| {
            Ok(vec![
                c.filter(attackers, &Candidate::of("id"))?,
                c.attacked(defenders)?,
            ])
        })?;
        let mut matched = single(
            "UNBLOCKED",
            Arm {
                pattern: "Event::Unblocked { attacker, defender }".into(),
                locals: "let id = *attacker; let subject = Target::Object(id); let data = [q.defending_player(id).map_or(u32::MAX, |p| u32::from(p.0)), u32::from(q.controller(id).0)];".into(),
                tests,
            },
            frame,
            "Whenever this attacks and isn't blocked",
        );
        if grouped {
            matched.batch = Some(true);
        }
        Ok(matched)
    }

    /// `EventSpec::AttacksWith`: the declaration is whole by the time its events
    /// are matched, so `q.state().attackers` says who attacks alongside it.
    fn attacks_with(
        &mut self,
        object: &ObjectRef,
        others: &ObjectFilter,
        count: &(Comparison, Expr),
        by: Option<&PlayerSet>,
        different_defenders: bool,
    ) -> Result<Matched> {
        if self.history.is_some() {
            return Err(gap("who attacked alongside whom, in a history question"));
        }
        let (comparison, value) = count;
        let alone = matches!(comparison, Comparison::Equal) && matches!(value, Expr::Constant(1));
        let frame = Frame {
            other_player: Some(Carried::Data(0)),
            other: alone.then_some(Carried::Data(1)),
            ..plain()
        };
        let mut alongside = String::new();
        let tests = self.matching(frame, |c| {
            let object = c.object(object)?;
            let other = c.filter(others, &Candidate::of("o"))?;
            let value = c.number(value)?.code;
            let apart = if different_defenders {
                " && a.defending != *defending"
            } else {
                ""
            };
            let many = format!(
                "q.state().attackers.iter().filter(|a| {{ let o = a.id; o != id{apart} && {other} }}).count() as i64"
            );
            if alone {
                alongside = format!(" let data = [data[0], q.state().attackers.iter().filter(|a| {{ let o = a.id; o != id{apart} && {other} }}).map(|a| a.id.0).next().unwrap_or(u32::MAX)];");
            }
            let mut tests = vec![format!("{object} == Some(id)"), compare(&many, comparison, &value)];
            if let Some(by) = by {
                tests.push(c.player_in(by, "q.controller(id)")?);
            }
            Ok(tests)
        })?;
        Ok(single(
            "ATTACKS",
            Arm {
                pattern: "Event::Attacks { attacker, defending, .. }".into(),
                locals: object_fired("*attacker", "u32::from(defending.0)") + &alongside,
                tests,
            },
            frame,
            "Whenever this attacks",
        ))
    }

    /// `EventSpec::AttacksAsGroup`, on the declaration.
    fn attacks_as_group(
        &mut self,
        by: &PlayerSet,
        attackers: &ObjectFilter,
        group: &Operand,
    ) -> Result<Matched> {
        if self.history.is_some() {
            return Err(gap("who attacked together, in a history question"));
        }
        let Operand::Term { form, operands, .. } = group else {
            return Err(gap("trigger event AttacksAsGroup of an unnamed group"));
        };
        let ("TotalPowerIs", [Operand::Comparison(comparison, value)]) =
            (form.as_str(), operands.as_slice())
        else {
            return Err(gap(format!("trigger event AttacksAsGroup {form}")));
        };
        let frame = about_player();
        let tests = self.matching(frame, |c| {
            let filter = c.filter(attackers, &Candidate::of("id"))?;
            let total = format!(
                "q.state().attackers.iter().map(|a| a.id).filter(|&id| {filter}).map(|id| i64::from(q.chars(id).power.unwrap_or(0))).sum::<i64>()"
            );
            Ok(vec![
                "*count > 0".to_string(),
                c.player_in(by, "*player")?,
                compare(&total, comparison, &c.number(value)?.code),
            ])
        })?;
        Ok(single(
            "ATTACKERS_DECLARED",
            Arm {
                pattern: "Event::AttackersDeclared { player, count }".into(),
                locals: ONE.into(),
                tests,
            },
            frame,
            "Whenever a player attacks",
        ))
    }

    /// `EventSpec::Caused`. The history keeps what caused each record; a matcher
    /// is told only the event, so only a history question can ask yet.
    fn caused(&mut self, cause: &ObjectFilter, event: &EventSpec) -> Result<Matched> {
        let mut matched = self.event(event)?;
        let frame = matched.frame;
        let test = self.matching(frame, |c| c.filter(cause, &Candidate::of("causer")))?;
        let test = format!(
            "match q.event_cause() {{ Cause::Spell(causer) | Cause::Ability {{ source: causer, .. }} => {test}, _ => false }}"
        );
        let free = !format!("{frame:?}").contains("Data(1)");
        for arm in &mut matched.arms {
            arm.tests.push(test.clone());
            if free {
                arm.locals.push_str(" let data = [data[0], match q.event_cause() { Cause::Spell(c) | Cause::Ability { source: c, .. } => u32::from(q.controller(c).0), _ => u32::MAX }];");
            }
        }
        if free {
            matched.frame.stack_controller = Some(Carried::Data(1));
        }
        if self.history.is_none() {
            if matched.planned.is_empty() {
                matched.planned.push("trigger event Caused".into());
            }
        }
        Ok(matched)
    }
}

/// The filter, less a requirement that a permanent is untapped.
fn without_untapped(filter: &ObjectFilter) -> ObjectFilter {
    let untapped = |f: &ObjectFilter| matches!(f, ObjectFilter::Not(inner) if matches!(&**inner, ObjectFilter::Tapped));
    match filter {
        ObjectFilter::All(filters) => {
            ObjectFilter::All(filters.iter().filter(|f| !untapped(f)).cloned().collect())
        }
        other if untapped(other) => ObjectFilter::Any,
        other => other.clone(),
    }
}

/// An arm Cantrip has no event for.
fn held_arm(
    mask: &'static str,
    pattern: &str,
    locals: &str,
    tests: Vec<String>,
    frame: Frame,
    phrase: &str,
    feature: &str,
) -> Matched {
    let mut matched = single(
        mask,
        Arm {
            pattern: pattern.into(),
            locals: locals.into(),
            tests,
        },
        frame,
        phrase,
    );
    matched.planned.push(feature.into());
    matched
}

/// A count, as `number` makes one.
pub(super) fn counted(code: String) -> Number {
    Number {
        code,
        min: 0,
        max: u32::MAX.into(),
        dynamic: true,
    }
}
/// The number Cantrip has no way to read yet.
pub(super) fn held_number(feature: String, code: &str) -> Number {
    counted(planned::planned_value(&feature, "0i64", code))
}

impl Compiler {
    /// What the object Cantrip has chosen for this ability or as it entered is
    /// linked by.
    pub(super) fn linked(&self) -> &'static str {
        match self.reader {
            Reader::Layer => "q.linked_to(asker.source)",
            Reader::Resolution if self.stack_item => "q.linked(item)",
            _ => "q.linked_to(asker)",
        }
    }
    /// The rest of `object`.
    pub(super) fn object_rest(&self, object: &ObjectRef) -> Result<String> {
        let held = |code: &str| {
            Ok(planned::planned_value(
                &format!("object reference {}", form(object)),
                "None::<ObjectId>",
                code,
            ))
        };
        Ok(match object {
            ObjectRef::Chosen => format!("q.state().chosen_object({})", self.linked()),
            ObjectRef::Named { result, .. } => {
                format!("({}).first().copied()", self.named(result)?)
            }
            // The object the discard made is what the event names.
            ObjectRef::Discarded if self.frame.object.is_some() => {
                self.carried_object(self.frame.object)?
            }
            // The stack item remembers the effect that gave the ability (CR 607.5).
            ObjectRef::Granter if self.reader == Reader::Resolution && self.stack_item => {
                "granter(q, item)".to_string()
            }
            // A token's ability naming the permanent that made it: the ability that
            // created it, while that permanent is still that object. A copy of the
            // token was made by something else (CR 400.7).
            ObjectRef::Granter if self.reader == Reader::Layer => {
                "q.history().all().iter().find_map(|r| match (r.event, r.cause) { (Event::Entered { permanent, .. }, Cause::Ability { source, .. }) if permanent == asker.source => Some(source), _ => None }).filter(|&o| q.state().obj(o).became.is_none())".to_string()
            }
            ObjectRef::Granter => return held("q.granted_by(asker.source)"),
            ObjectRef::PairedWith(_) => {
                return held(&format!(
                    "{}.and_then(|o: ObjectId| q.state().obj(o).paired_with)",
                    self.object(match object {
                        ObjectRef::PairedWith(of) => of,
                        _ => unreachable!(),
                    })?
                ))
            }
            ObjectRef::RingBearer(_) => return held("q.ring_bearer(player)"),
            ObjectRef::HauntedBy(_) => return held("q.state().obj(o).haunted_by"),
            other => return self.object_more(other),
        })
    }
    /// The rest of `player`.
    pub(super) fn player_rest(&self, player: &PlayerRef) -> Result<String> {
        Ok(match player {
            PlayerRef::OneOf(players) if matches!(**players, PlayerSet::One(_)) => {
                let PlayerSet::One(player) = &**players else {
                    unreachable!("just matched");
                };
                self.player(player)?
            }
            PlayerRef::Chosen => match self.chosen_local(ValueKind::Player) {
                Ok(local) => format!("Some({local})"),
                Err(_) => format!("q.state().chosen_player({})", self.linked()),
            },
            PlayerRef::DefendingPlayerOf(object) => format!(
                "{}.and_then(|o: ObjectId| q.state().attackers.iter().find(|a| a.id == o).map(|a| a.defending))",
                self.object(object)?
            ),
            // The active player is the attacking player (CR 506.2).
            PlayerRef::AttackingPlayer => "Some(q.state().turn.active)".into(),
            PlayerRef::Holder(Designation::Monarch) => {
                "q.state().seats().find(|&p| q.state().player(p).monarch)".into()
            }
            PlayerRef::Holder(Designation::Initiative) => {
                "q.state().seats().find(|&p| q.state().player(p).initiative)".into()
            }
            PlayerRef::OneOf(players) => format!("Some({})", self.pick(players)?),
            PlayerRef::PlayerOrControllerOf(recipients) => match &**recipients {
                // "That player or that planeswalker's controller", of a slot before.
                Recipients::Target { slot, .. } | Recipients::Targets(slot)
                    if self.choosing.is_some() =>
                {
                    let index = match &**recipients {
                        Recipients::Target { index, .. } => *index,
                        _ => 0,
                    };
                    format!(
                        "{}.and_then(|t: Target| t.player().or(t.object().map(|o| q.controller(o))))",
                        self.earlier_target(slot, index)?.expect("choosing")
                    )
                }
                Recipients::Target { slot, index } => format!(
                    "t_{}_raw.get({index}).copied().and_then(|t: Target| t.player().or(t.object().map(|o| q.controller(o))))",
                    self.slot(slot)?
                ),
                Recipients::Targets(slot)
                    if self.slots.get(&slot.0).is_some_and(|(_, spec)| {
                        matches!(spec.count, Count::Exactly(Expr::Constant(1)) | Count::UpTo(Expr::Constant(1)))
                    }) =>
                {
                    format!(
                        "t_{}_raw.first().copied().and_then(|t: Target| t.player().or(t.object().map(|o| q.controller(o))))",
                        self.slot(slot)?
                    )
                }
                other => {
                    return Err(gap(format!(
                        "player reference PlayerOrControllerOf {}",
                        form(other)
                    )))
                }
            },
            other => return self.player_more(other),
        })
    }
    /// What a preceding instruction named, by the list it keeps.
    pub(super) fn named(&self, result: &ResultId) -> Result<String> {
        if self.selections.contains_key(&result.0) {
            self.selected(result)
        } else if self.creations.contains_key(&result.0) {
            self.created(result)
        } else {
            self.transition(result, &ObjectPhase::After)
        }
    }
    /// The rest of `objects`.
    pub(super) fn objects_rest(&self, objects: &ObjectSet) -> Result<String> {
        Ok(match objects {
            ObjectSet::Named(result) => self.named(result)?,
            // CR 609.7a: permanents, spells, what those name, and what waits to apply.
            ObjectSet::DamageSources(filter) => format!(
                "({}).into_iter().filter(|&id| {}).collect::<Vec<ObjectId>>()",
                planned::planned_value(
                    "object collection DamageSources",
                    "Vec::<ObjectId>::new()",
                    "cantrip_mechanics::damage_sources(q)"
                ),
                self.filter(filter, &Candidate::of("id"))?
            ),
            ObjectSet::MovedBy {
                result,
                phase,
                player,
            } => {
                let who = self.player(player)?;
                let share = self
                    .shares
                    .get(&result.0)
                    .filter(|(moved, _)| self.moved.get(&result.0) == Some(moved));
                match (share, phase) {
                    (Some((moved, Share::Owner)), ObjectPhase::After) => format!(
                        "{moved}.iter().copied().filter(|&o| Some(q.state().obj(o).owner) == {who}).collect::<Vec<ObjectId>>()"
                    ),
                    (Some((_, Share::Picks(picks))), ObjectPhase::After) => format!(
                        "{picks}.iter().filter(|(who, _)| Some(*who) == {who}).filter_map(|(_, o)| q.state().obj(*o).became).collect::<Vec<ObjectId>>()"
                    ),
                    (Some((_, Share::Picks(picks))), ObjectPhase::Before) => format!(
                        "{picks}.iter().filter(|(who, o)| Some(*who) == {who} && q.state().obj(*o).became.is_some()).map(|(_, o)| *o).collect::<Vec<ObjectId>>()"
                    ),
                    _ => return self.moved_by(result, phase, &who),
                }
            }
            other => return self.objects_more(other),
        })
    }
    /// One player's number, for `p`, a `PlayerId`.
    pub(super) fn player_number(&self, p: &str, number: &PlayerNumber) -> Result<String> {
        let held = |code: String| {
            planned::planned_value(
                &format!("numeric expression PlayerNumber {}", form(number)),
                "0i64",
                &code,
            )
        };
        Ok(match number {
            PlayerNumber::Life => format!("i64::from(q.state().player({p}).life)"),
            PlayerNumber::CardsInHand => format!("q.state().zone(Zone::hand({p})).len() as i64"),
            PlayerNumber::Energy => format!("i64::from(q.state().player({p}).energy())"),
            PlayerNumber::Speed => {
                format!("i64::from(q.state().player({p}).speed.unwrap_or(0))")
            }
            PlayerNumber::Permanents(filter) => format!(
                "q.state().battlefield().iter().copied().filter(|&id| q.controller(id) == {p} && {}).count() as i64",
                self.filter(filter, &Candidate::of("id"))?
            ),
            PlayerNumber::UnspentMana => format!(
                "i64::from(q.state().player({p}).pool.total() + q.state().player({p}).lasting.total())"
            ),
            PlayerNumber::UnspentManaOf(of) => format!(
                "i64::from(q.state().player({p}).pool.get(Mana::{m}) + q.state().player({p}).lasting.get(Mana::{m}))",
                m = mana_letter(of)
            ),
            PlayerNumber::TurnsBegun => format!(
                "q.history().all().iter().filter(|r| matches!(r.event, Event::TurnBegan {{ player, .. }} if player == {p})).count() as i64"
            ),
            PlayerNumber::CardsDrawnThisTurn if self.reader != Reader::Layer => {
                format!("cantrip_mechanics::history::drawn_this_turn(q, {p}) as i64")
            }
            PlayerNumber::DungeonsCompleted => format!(
                "q.history().all().iter().filter(|r| matches!(r.event, Event::DungeonCompleted {{ player }} if player == {p})).count() as i64"
            ),
            // CR 700.11: a permanent card put into the graveyard from anywhere.
            PlayerNumber::DescentsThisTurn => format!(
                "q.history().select(EventQuery::kind(EventMask::ZONE_CHANGE).to(ZoneKind::Graveyard).this_turn()).filter(|r| matches!(r.event, Event::ZoneChange {{ new, to, .. }} if to == Zone::graveyard({p}) && !q.obj(new).token && cantrip_mechanics::targets::is_permanent_card(&q.chars(new)))).count() as i64"
            ),
            // Cycling discards (CR 702.29a), so each cycled card is in the count once.
            PlayerNumber::CardsCycledOrDiscardedThisTurn => format!(
                "q.history().this_turn().iter().filter(|r| matches!(r.event, Event::Discarded {{ player, .. }} if player == {p})).count() as i64"
            ),
            other => held(format!("q.player_number({p}, PlayerNumber::{})", form(other))),
        })
    }
    /// The rest of `number`.
    pub(super) fn number_rest(&self, n: &Expr) -> Result<Number> {
        let resolving = self.reader == Reader::Resolution && self.stack_item;
        let feature = format!("numeric expression {}", form(n));
        Ok(match n {
            Expr::PlayerNumber { player, number } => counted(format!(
                "{}.map_or(0i64, |p: PlayerId| {})",
                self.player(player)?,
                self.player_number("p", number)?
            )),
            Expr::AmongPlayers {
                players,
                number,
                aggregation,
            } => {
                let pick = match aggregation {
                    Aggregation::Maximum => "max().unwrap_or(0)",
                    Aggregation::Minimum => "min().unwrap_or(0)",
                    Aggregation::Sum => "sum::<i64>()",
                    other => {
                        return Err(gap(format!(
                            "numeric expression AmongPlayers {}",
                            form(other)
                        )))
                    }
                };
                counted(format!(
                    "({}).into_iter().map(|p: PlayerId| {}).{pick}",
                    self.players(players)?,
                    self.player_number("p", number)?
                ))
            }
            Expr::TimesKicked(object) => {
                let object = self.object(object)?;
                let kicks = self.times_kicked(&object, "o");
                counted(format!("{object}.map_or(0i64, |o: ObjectId| {kicks})"))
            }
            // The history has this one only once it's done (CR 608.2m).
            Expr::TimesResolvedThisTurn if resolving => {
                counted("q.history().resolutions(item.object, item.ability) as i64 + 1".into())
            }
            Expr::TimesActivatedThisTurn if resolving => counted(
                "q.history().this_turn().iter().filter(|r| matches!(r.event, Event::AbilityActivated { source, ability, .. } if source == item.object && ability == item.ability)).count() as i64".into(),
            ),
            Expr::AnnouncedX => held_number(feature, "i64::from(q.announced_x().unwrap_or(0))"),
            Expr::CountManaSymbols { colors, .. } => held_number(
                feature,
                &format!(
                    "i64::from(cantrip_mechanics::devotion::symbols_among(q, &objects, {}))",
                    colors.len()
                ),
            ),
            Expr::PaidCountersRemoved { .. } => {
                held_number(feature, "i64::from(q.counters_paid(item))")
            }
            Expr::ExcessDamage { result, to } if self.excess.contains_key(&result.0) => {
                let (list, recipients, aimed) = &self.excess[&result.0];
                let only = match &**to {
                    ObjectFilter::Any => Some(String::new()),
                    ObjectFilter::SameObject(object) if aimed.as_deref() == Some(format!("{object:?}").as_str()) => {
                        Some(format!(".filter(|(t, _)| {recipients}.contains(t))"))
                    }
                    _ => None,
                };
                match only {
                    Some(only) => counted(format!(
                        "{list}.iter(){only}.map(|(_, n)| i64::from(*n)).sum::<i64>()"
                    )),
                    None => held_number(feature, "i64::from(result.excess)"),
                }
            }
            Expr::ExcessDamage { .. } => held_number(feature, "i64::from(result.excess)"),
            Expr::ChosenNumber => match self.chosen_local(ValueKind::Number) {
                Ok(local) => counted(format!("i64::from({local})")),
                Err(_) => counted(format!(
                    "q.state().chosen_number({}).map_or(0i64, i64::from)",
                    self.linked()
                )),
            },
            Expr::Stickers { objects, measure } => {
                let code = format!(
                    "cantrip_mechanics::stickers::measure(q, &{}, StickerMeasure::{})",
                    self.objects(objects)?,
                    form(measure)
                );
                Number {
                    code: planned::planned_value("stickers", "0i64", &code),
                    ..counted(String::new())
                }
            }
            Expr::LargestSharedGroup { objects, property } => {
                let group = match property {
                    DistinctProperty::CreatureTypes => "those.iter().flat_map(|&a| q.chars(a).subtypes.iter().filter(|s| s.card_types().has(CardType::Creature)).collect::<Vec<Subtype>>()).map(|s| those.iter().filter(|&&b| q.chars(b).has_subtype(s)).count()).max()",
                    DistinctProperty::Names => "those.iter().map(|&a| those.iter().filter(|&&b| cantrip_mechanics::same_name(q, a, b)).count()).max()",
                    DistinctProperty::ManaValues => "those.iter().map(|&a| those.iter().filter(|&&b| q.mana_value(a) == q.mana_value(b)).count()).max()",
                    other => return Err(gap(format!("numeric expression LargestSharedGroup {}", form(other)))),
                };
                counted(format!(
                    "{{ let those: Vec<ObjectId> = {}; {group}.unwrap_or(0) as i64 }}",
                    self.objects(objects)?
                ))
            }
            Expr::SharingWithAnother { objects, property } => {
                let same = match property {
                    DistinctProperty::ManaValues => "q.mana_value(a) == q.mana_value(b)",
                    other => {
                        return Err(gap(format!(
                            "numeric expression SharingWithAnother {}",
                            form(other)
                        )))
                    }
                };
                counted(format!(
                    "{{ let those: Vec<ObjectId> = {}; those.iter().filter(|&&a| those.iter().any(|&b| b != a && {same})).count() as i64 }}",
                    self.objects(objects)?
                ))
            }
            // "Each opponent who didn't discard a nonland card this way": of what the
            // instruction discarded, that player's. A hand holds only its owner's cards.
            Expr::Earlier {
                what: what @ EarlierObjects::Discarded,
                measure: ObjectMeasure::Count,
                result,
                filter,
                by: Some(PlayerRef::Bound(binding)),
            } => {
                let list = match result {
                    Some(result) => self.objects(&ObjectSet::Transition {
                        result: result.clone(),
                        phase: ObjectPhase::After,
                    })?,
                    None => self.earlier_objects(what)?,
                };
                let keep = match filter {
                    Some(filter) => format!(
                        ".filter(|&id| {})",
                        self.filter(filter, &Candidate::of("id"))?
                    ),
                    None => String::new(),
                };
                counted(format!("({list}).into_iter().filter(|&id: &ObjectId| q.state().obj(id).owner == bound_{}){keep}.count() as i64", binding.0))
            }
            // "For each card they revealed this way": that player's own share of an
            // earlier walk, and any choice in it theirs too.
            Expr::Earlier {
                what,
                measure: ObjectMeasure::Count,
                result: None,
                filter,
                by: Some(PlayerRef::Bound(binding)),
            } if self.earlier_by(what, binding.0).is_some() => {
                let list = self.earlier_by(what, binding.0).expect("just asked");
                *self.own_choice.borrow_mut() = Some(format!("Some(bound_{})", binding.0));
                let keep = match filter {
                    Some(filter) => self
                        .filter(filter, &Candidate::of("id"))
                        .map(|test| format!(".filter(|&id| {test})")),
                    None => Ok(String::new()),
                };
                *self.own_choice.borrow_mut() = None;
                counted(format!("({list}).into_iter(){}.count() as i64", keep?))
            }
            // What an instruction named did: the AST says which. One whose
            // `result` is empty leaves the instruction to be found, so stays a gap.
            Expr::Earlier {
                measure,
                result: Some(result),
                filter,
                by: None | Some(PlayerRef::AbilityController),
                ..
            } => {
                let list = if self.selections.contains_key(&result.0) {
                    self.objects(&ObjectSet::Selected(result.clone()))?
                } else {
                    self.objects(&ObjectSet::Transition {
                        result: result.clone(),
                        phase: ObjectPhase::After,
                    })?
                };
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
            other => return self.number_more(other),
        })
    }
    /// The records of a window.
    pub(super) fn object_window(&self, window: &ObjectWindow) -> Result<String> {
        Ok(match window {
            ObjectWindow::ThisTurn => "q.history().this_turn().iter()".into(),
            ObjectWindow::ThisGame => "q.history().all().iter()".into(),
            ObjectWindow::ThisCombat if self.reader != Reader::Layer => {
                "cantrip_mechanics::history::this_combat(q).iter()".into()
            }
            ObjectWindow::LastTurnOf(player) => format!(
                "{{ let of = {}; let turn = q.history().all().iter().rev().find(|r| Some(r.active) == of && r.turn < q.state().turn.number).map(|r| r.turn); q.history().all().iter().filter(move |r| Some(r.turn) == turn) }}",
                self.player(player)?
            ),
            other => return self.object_window_p(other),
        })
    }
    /// `Condition::PlayerDid`: the event it stands for, asked of the history.
    fn player_did(
        &self,
        player: &PlayerRef,
        event: &PlayerHistory,
        window: &ObjectWindow,
    ) -> Result<String> {
        type H = PlayerHistory;
        let scope = self.object_window(window)?;
        let one = PlayerSet::One(player.clone());
        let spec = match event {
            H::AttackedWithTotalPower(comparison, value) => {
                let value = self.number(value)?.code;
                let by = self.player(player)?;
                let total = format!(
                    "{scope}.filter_map(|r: &Record| match r.event {{ Event::Attacks {{ attacker, .. }} if Some(q.controller(attacker)) == {by} => Some(i64::from(q.chars(attacker).power.unwrap_or(0))), _ => None }}).sum::<i64>()"
                );
                return Ok(compare(&total, comparison, &value));
            }
            H::CastSpell(spells) => EventSpec::Spell {
                event: SpellEvent::Cast,
                players: one,
                spells: (**spells).clone(),
            },
            H::AttackedBy(attackers) => EventSpec::Attack {
                attackers: (**attackers).clone(),
                by: None,
                defenders: AttackDefenders::Players(one),
            },
            H::AttackedPlayer(target) => EventSpec::Attack {
                attackers: ObjectFilter::Any,
                by: Some(one),
                defenders: AttackDefenders::Players(PlayerSet::One(target.clone())),
            },
            H::AttackedWith(attackers) => EventSpec::Attack {
                attackers: (**attackers).clone(),
                by: Some(one),
                defenders: AttackDefenders::Any,
            },
            H::PermanentEntered(objects) => EventSpec::Player {
                event: PlayerEvent::EntersUnderControl,
                players: one,
                objects: Some((**objects).clone()),
            },
            other => return self.did_stated(player, other, &scope),
        };
        let mut asked = self.clone();
        asked.history = Some(History {
            outer: None,
            event_at: self.event_at,
            frame: self.frame,
        });
        let matched = asked.event(&spec)?;
        if let Some(feature) = matched.planned.first() {
            return Err(gap(feature));
        }
        let answer = format!(
            "{scope}.filter_map({}).next().is_some()",
            matched.history_test()
        );
        Ok(if self.event_at == EventAt::Matcher {
            format!("{{ let (outer_subject, outer_data) = (subject, data); {answer} }}")
        } else {
            answer
        })
    }
    /// `ObjectFilter::History`.
    fn object_history(
        &self,
        event: &ObjectHistory,
        window: &ObjectWindow,
        id: &str,
    ) -> Result<String> {
        type H = ObjectHistory;
        let moved =
            |pattern: &str, test: &str| format!("Event::ZoneChange {{ {pattern}, .. }} if {test}");
        let pattern = match event {
            H::Attacked => format!("Event::Attacks {{ attacker, .. }} if attacker == {id}"),
            H::Blocked => format!("Event::Blocks {{ blocker, .. }} if blocker == {id}"),
            H::WasBlocked => format!("Event::Blocks {{ attacker, .. }} if attacker == {id}"),
            H::AbilityActivated => {
                format!("Event::AbilityActivated {{ source, .. }} if source == {id}")
            }
            H::Entered => moved("new, to", &format!("new == {id} && to.is(ZoneKind::Battlefield)")),
            H::Drawn(None) => format!("Event::Drew {{ card, .. }} if card == {id}"),
            H::Drawn(Some(players)) => format!(
                "Event::Drew {{ card, player }} if card == {id} && {}",
                self.player_in(players, "player")?
            ),
            H::Cycled => format!("Event::Cycled {{ card, .. }} if card == {id}"),
            H::Discarded => format!("Event::Discarded {{ card, .. }} if card == {id}"),
            H::Milled => moved("new, reason: ZoneReason::Mill", &format!("new == {id}")),
            H::Sacrificed => moved("new, reason: ZoneReason::Sacrifice", &format!("new == {id}")),
            H::Exiled => moved("new, to", &format!("new == {id} && to.is(ZoneKind::Exile)")),
            H::PutIntoGraveyard => {
                moved("new, to", &format!("new == {id} && to.is(ZoneKind::Graveyard)"))
            }
            H::PutIntoHand => moved("new, to", &format!("new == {id} && to.is(ZoneKind::Hand)")),
            H::DiedOrPutFromBattlefield => moved(
                "new, from, to",
                &format!("new == {id} && from.is(ZoneKind::Battlefield) && to.is(ZoneKind::Graveyard)"),
            ),
            H::Convoked(object) => {
                return Ok(format!(
                    "{}.is_some_and(|o: ObjectId| q.history().convoked(o).any(|c| c == {id}))",
                    self.object(object)?
                ));
            }
            H::DamagedBy(object) => format!(
                "Event::Damage {{ source, target: Target::Object(hit), .. }} if hit == {id} && {} == Some(source)",
                self.object(object)?
            ),
            other => return self.object_history_p(other, id, window),
        };
        Ok(format!(
            "{}.any(|r| matches!(r.event, {pattern}))",
            self.object_window(window)?
        ))
    }
    /// `HasKeywordFamily`: `Keyword`s, or an ability only the card's text has.
    fn keyword_family(&self, family: &KeywordFamily, chars: &str, id: &str) -> String {
        let any = |words: &[&str]| {
            let tests = words
                .iter()
                .map(|w| format!("{chars}.keywords.has(Keyword::{w})"))
                .collect::<Vec<_>>();
            format!("({})", tests.join(" || "))
        };
        let card = self.card_row(&format!("{chars}.card"));
        let way = |kind: &str| format!("{card}.ways.iter().any(|w| w.kind == WayKind::{kind})");
        match family {
            KeywordFamily::Hexproof => any(&["Hexproof"]),
            KeywordFamily::Decayed => any(&["Decayed"]),
            KeywordFamily::Morph => any(&["Morph"]),
            KeywordFamily::Disguise => any(&["Disguise"]),
            KeywordFamily::Phasing => any(&["Phasing"]),
            KeywordFamily::Toxic => format!("{chars}.toxic > 0"),
            KeywordFamily::Ward if self.reader == Reader::Layer => format!(
                "{chars}.abilities().any(|a| matches!(q.cards().ability(a), AbilityDef::Triggered(t) if t.ward))"
            ),
            KeywordFamily::Ward => format!(
                "{chars}.abilities().any(|a| q.triggered_ability(a).is_some_and(|t| t.ward))"
            ),
            KeywordFamily::Exhaust if self.reader == Reader::Layer => format!(
                "{chars}.abilities().any(|a| matches!(q.cards().ability(a), AbilityDef::Activated(d) if d.exhaust))"
            ),
            KeywordFamily::Exhaust => format!(
                "{chars}.abilities().any(|a| q.activated_ability(a).is_some_and(|d| d.exhaust))"
            ),
            KeywordFamily::Warp => way("Warp"),
            KeywordFamily::Freerunning => way("Freerunning"),
            KeywordFamily::Mutate => way("Mutate"),
            KeywordFamily::Kicker => format!("{card}.kicker.is_some()"),
            KeywordFamily::Madness => format!("{card}.madness.is_some()"),
            KeywordFamily::Foretell => format!("{card}.foretell.is_some()"),
            KeywordFamily::Suspend => format!("{card}.suspend.is_some()"),
            KeywordFamily::StartYourEngines => format!("{card}.start_your_engines"),
            KeywordFamily::Convoke if self.reader != Reader::Layer => format!(
                "({card}.pay_with.convoke || q.state().battlefield().iter().any(|&p| !q.lost_all_abilities(p) && q.card(q.chars(p).card).grants_pay_with.iter().any(|g| g(q, p, q.controller({id}), {id}).convoke)))"
            ),
            KeywordFamily::Flashback => {
                // A flashback way is known from what else exiles the spell by
                // its cost: harmonize's taps a creature, beam me up's returns one.
                let printed = format!(
                    "{card}.ways.iter().any(|w| w.exile_after && w.from == ZoneKind::Graveyard && !w.cost.tap_to_reduce && !matches!(w.cost.permanents, Some(PermanentCost {{ action: PermanentAction::BeamUp, .. }})))"
                );
                if self.reader == Reader::Layer {
                    return format!(
                        "({printed} || q.state().permissions.iter().any(|p| p.grant.flashback && q.state().permission_covers(p, {id})))"
                    );
                }
                format!(
                    "({printed} || q.state().permissions.iter().any(|p| p.grant.flashback && q.state().permission_covers(p, {id})) || q.state().battlefield().iter().any(|&p| !q.lost_all_abilities(p) && q.card(q.chars(p).card).granted_ways.iter().any(|w| w.flashback && q.state().obj({id}).zone.is(w.from) && (w.cost)(q, &Asker::new(p, q.controller(p)), {id}, q.state().obj({id}).owner).is_some())))"
                )
            }
            KeywordFamily::Landwalk => any(&[
                "Islandwalk",
                "Plainswalk",
                "Swampwalk",
                "Mountainwalk",
                "Forestwalk",
                "NonbasicLandwalk",
                "LegendaryLandwalk",
            ]),
            other => planned::planned_value(
                "object filter HasKeywordFamily",
                "false",
                &format!(
                    "q.has_keyword_ability({id}, KeywordAbility::{})",
                    form(other)
                ),
            ),
        }
    }
    /// The rest of `predicate`.
    pub(super) fn predicate_rest(&self, f: &ObjectFilter, c: &Candidate) -> Result<String> {
        let (id, chars) = (c.id.as_str(), c.chars.as_str());
        let layer = self.reader == Reader::Layer;
        let on_stack = format!("q.state().stack.iter().find(|i| i.object == {id})");
        let held = |code: String| {
            planned::planned_value(&format!("object filter {}", form(f)), "false", &code)
        };
        Ok(match f {
            ObjectFilter::HasKeywordFamily(family) => self.keyword_family(family, chars, id),
            ObjectFilter::AttackingDefender(defenders) => format!(
                "q.state().attackers.iter().any(|a| a.id == {id} && {{ let defender = &a.target; {} }})",
                self.attacked(defenders)?
            ),
            ObjectFilter::OnStack(StackTrait::Spell) => {
                format!("{on_stack}.is_some_and(|i| i.kind == StackKind::Spell)")
            }
            ObjectFilter::OnStack(StackTrait::SingleTarget) => match c.announced.as_deref() {
                // "Targets only a single": however many times it was chosen (115.9c),
                // where "with a single target" counts them.
                None if self.line_text().is_some_and(|line| line.contains("only a single")) => {
                    format!("{on_stack}.is_some_and(|i| {{ let chosen = q.state().targets(i); !chosen.is_empty() && chosen.iter().all(|t| *t == chosen[0]) }})")
                }
                None => format!("{on_stack}.is_some_and(|i| q.state().targets(i).len() == 1)"),
                Some("") => return Err(gap("targets of a spell as it is cast")),
                Some(list) => format!("{list}.len() == 1"),
            },
            // What's on the stack is known by the object it came from, which an
            // ability shares with others of the same source.
            ObjectFilter::OnStack(StackTrait::XInCost) if c.announced.is_none() => {
                format!("q.mana_cost({id}).x > 0")
            }
            ObjectFilter::OnStack(other) => planned::planned_value(
                "object filter OnStack",
                "false",
                &format!("q.stack_item({id}).is_some_and(|i| i.is(StackTrait::{}))", form(other)),
            ),
            ObjectFilter::History { event, window } => self.object_history(event, window, id)?,
            ObjectFilter::DamagedBy(result) if self.moved.contains_key(&result.0) => {
                format!("{}.contains(&{id})", self.moved[&result.0])
            }
            // An instant or sorcery that deals damage once: what it dealt damage to
            // is what it dealt damage to that way, read from the events (CR 120.3).
            ObjectFilter::DamagedBy(_) if self.damages_once => format!(
                "cantrip_mechanics::history::permanents_dealt_damage_by(q.history(), asker.source, 0).contains(&{id})"
            ),
            ObjectFilter::DamagedBy(_) => held(format!(
                "cantrip_mechanics::history::permanents_dealt_damage_by(q.history(), asker.source, since).contains(&{id})"
            )),
            // Read from the damage events of the instruction (CR 120.10).
            ObjectFilter::DealtExcessBy(result) if self.excess.contains_key(&result.0) => {
                let excess = &self.excess[&result.0].0;
                format!("{excess}.iter().any(|&(t, n)| t == Target::Object({id}) && n > 0)")
            }
            ObjectFilter::DealtExcessBy(_) => held(format!("result.excess_to({id}) > 0")),
            ObjectFilter::ArrivedThisTurn { from: None } if !layer => {
                format!("cantrip_mechanics::history::arrived_this_turn(q, {id})")
            }
            ObjectFilter::ArrivedThisTurn { from: Some(zone) } if !layer => format!(
                "q.history().this_turn().iter().any(|r| matches!(r.event, Event::ZoneChange {{ new, from, .. }} if new == {id} && from.is({})))",
                zone_kind(zone)?
            ),
            ObjectFilter::SharesNameWithAny(objects) if !layer => format!(
                "({}).iter().any(|&o| cantrip_mechanics::same_name(q, {id}, o))",
                self.objects(objects)?
            ),
            ObjectFilter::SharesNameWithAnother(objects) if !layer => format!(
                "({}).iter().any(|&o| o != {id} && cantrip_mechanics::same_name(q, {id}, o))",
                self.objects(objects)?
            ),
            ObjectFilter::SharesWithAny {
                property: DistinctProperty::ManaValues,
                with,
            } if !layer => format!(
                "({}).iter().any(|&o| q.mana_value({id}) == q.mana_value(o))",
                self.objects(with)?
            ),
            ObjectFilter::SharesWithAny { property, with } if !layer => format!(
                "({}).iter().any(|&o| {{ let (a, b) = (&{chars}, &q.chars(o)); {} }})",
                self.objects(with)?,
                sharing(property)?
            ),
            ObjectFilter::SharesColorWith(object) => format!(
                "{}.is_some_and(|o: ObjectId| q.chars(o).colors.intersects({chars}.colors))",
                self.object(object)?
            ),
            ObjectFilter::SharesColorWithAny(objects) if !layer => format!(
                "({}).iter().any(|&o| q.chars(o).colors.intersects({chars}.colors))",
                self.objects(objects)?
            ),
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                scope,
            } if !layer => {
                let records = match scope {
                    CastScope::Turn => "q.history().this_turn()",
                    CastScope::Game => "q.history().all()",
                };
                let earlier = self
                    .filter(spells, &Candidate::of("cast.spell"))?
                    .replace("q.chars(cast.spell)", "cast.chars(q)");
                let who = match by {
                    Some(by) => self.player_in(by, "cast.player")?,
                    None => "true".into(),
                };
                format!(
                    "({} && {records}.iter().filter_map(|r| match r.event {{ Event::Cast(cast) => Some(cast), _ => None }}).take_while(|cast| cast.spell != {id}).filter(|cast| {who} && {earlier}).count() as i64 + 1 == {})",
                    self.filter(spells, c)?,
                    self.number(ordinal)?.code
                )
            }
            // As a static ability asks of a spell on the stack: its cast is recorded
            // before anything reads what it has. What was on the stack earlier may
            // have changed, so nothing about the earlier spells' characteristics.
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                scope,
            } if layer && c.announced.is_none() => {
                let records = match scope {
                    CastScope::Turn => "q.history().this_turn()",
                    CastScope::Game => "q.history().all()",
                };
                let earlier = self.filter(spells, &Candidate::of("cast.spell"))?;
                if earlier.contains("q.chars(cast.spell)") {
                    return Err(gap("object filter NthCast"));
                }
                let who = match by {
                    Some(by) => self.player_in(by, "cast.player")?,
                    None => "true".into(),
                };
                format!(
                    "{records}.iter().filter_map(|r| match r.event {{ Event::Cast(cast) => Some(cast), _ => None }}).filter(|cast| {who} && {earlier}).nth((({}) - 1) as usize).is_some_and(|cast| cast.spell == {id})",
                    self.number(ordinal)?.code
                )
            }
            ObjectFilter::ExiledWith(ObjectRef::Source) if layer => {
                format!("q.exiled_with(q.linked_to(asker.source)).contains(&{id})")
            }
            ObjectFilter::ExiledWith(object) if !layer => format!(
                "{}.is_some_and(|o: ObjectId| q.exiled_with(q.linked_to(&Asker::new(o, q.controller(o)))).contains(&{id}))",
                self.object(object)?
            ),
            ObjectFilter::BlockedByObject(object) => format!(
                "{{ let blocker = {}; q.state().blocks.iter().any(|b| b.attacker == {id} && Some(b.blocker) == blocker) }}",
                self.object(object)?
            ),
            ObjectFilter::BlockingAny(attackers) => format!(
                "q.state().blocks.iter().any(|b| b.blocker == {id} && {})",
                self.filter(attackers, &Candidate::of("b.attacker"))?
            ),
            ObjectFilter::BlockedByAny(blockers) => format!(
                "q.state().blocks.iter().any(|b| b.attacker == {id} && {})",
                self.filter(blockers, &Candidate::of("b.blocker"))?
            ),
            ObjectFilter::AttachedToPlayer(player) => format!(
                "q.state().obj({id}).attached_player.is_some() && q.state().obj({id}).attached_player == {}",
                self.player(player)?
            ),
            ObjectFilter::HasAttached(attachments) if !layer => format!(
                "q.state().battlefield().iter().any(|&a| q.state().obj(a).attached_to == Some({id}) && {})",
                self.filter(attachments, &Candidate::of("a"))?
            ),
            ObjectFilter::AttachedCount {
                attachments,
                comparison,
                value,
            } if !layer => compare(
                &format!(
                    "q.state().battlefield().iter().filter(|&&a| q.state().obj(a).attached_to == Some({id}) && {}).count() as i64",
                    self.filter(attachments, &Candidate::of("a"))?
                ),
                comparison,
                &self.number(value)?.code,
            ),
            ObjectFilter::Modified => format!(
                "(q.state().obj({id}).has_counters() || q.state().battlefield().iter().any(|&a| q.state().obj(a).attached_to == Some({id}) && {{ let chars = q.chars(a); chars.has_subtype(Subtype::Equipment) || chars.has_subtype(Subtype::Aura) && q.controller(a) == q.controller({id}) }}))"
            ),
            ObjectFilter::EnteredFrom(scope) if !layer => {
                let (zone, owners) = match scope {
                    ZoneScope::Shared(zone) => (zone, None),
                    ZoneScope::Players { zone, players } => (zone, Some(players)),
                };
                let entered = format!(
                    "q.history().all().iter().rev().find_map(|r| match r.event {{ Event::ZoneChange {{ new, from, to, .. }} if new == {id} && to.is(ZoneKind::Battlefield) => Some(from), _ => None }}).is_some_and(|from| from.is({}))",
                    zone_kind(zone)?
                );
                match owners {
                    Some(players) => format!(
                        "({entered} && {})",
                        self.player_in(players, &format!("q.state().obj({id}).owner"))?
                    ),
                    None => entered,
                }
            }
            ObjectFilter::ManaSpentFrom(sources) if !layer && c.announced.is_none() => format!(
                "cast_of(q, {id}).is_some_and(|cast| q.history().mana_spent_to_cast(cast.spell).any(|(source, _)| {}))",
                self.filter(sources, &Candidate::of("source"))?
            ),
            // A spell on the stack, as a static ability asks of it.
            ObjectFilter::ManaSpentFrom(sources) if layer && c.announced.is_none() => format!(
                "q.history().mana_spent_to_cast({id}).any(|(source, _)| {})",
                self.filter(sources, &Candidate::of("source"))?
            ),
            ObjectFilter::ManaSpent {
                color,
                sources,
                payers: None,
                comparison,
                value,
            } if !layer && c.announced.is_none() => {
                let amount = match color {
                    Some(color) => format!(
                        "i64::from(spent.get({}))",
                        lane_p::colors_code(std::slice::from_ref(color))?
                    ),
                    None => "i64::from(spent.total())".to_string(),
                };
                let from = match sources {
                    Some(sources) => self.filter(sources, &Candidate::of("source"))?,
                    None => "true".to_string(),
                };
                compare(
                    &format!("cast_of(q, {id}).map_or(0i64, |cast| q.history().mana_spent_to_cast(cast.spell).filter(|&(source, _)| {from}).map(|(_, spent)| {amount}).sum::<i64>())"),
                    comparison,
                    &self.number(value)?.code,
                )
            }
            ObjectFilter::Extreme {
                among,
                property,
                highest,
            } if !layer => {
                let value = self.property("o", property)?.code;
                let mine = self.property(id, property)?.code;
                let pick = if *highest { "max" } else { "min" };
                format!(
                    "({}).into_iter().map(|o: ObjectId| {value}).{pick}().is_some_and(|best| {mine} == best)",
                    self.objects(among)?
                )
            }
            ObjectFilter::InResult(result) => {
                let list = self.objects(&ObjectSet::Named(result.clone()))?;
                format!("({list}).contains(&{id})")
            }
            ObjectFilter::KickedWith(cost) => {
                let words = mechanics::kicker_words(cost);
                match self.kickers.iter().position(|k| Some(k) == words.as_ref()) {
                    Some(0) => format!("q.state().obj({id}).kicked > 0"),
                    Some(1) => format!("q.state().obj({id}).paid > 0"),
                    _ => held(format!("q.state().obj({id}).kicked_with(kicker)")),
                }
            }
            ObjectFilter::AlternateCostPaid(_) => {
                held(format!("q.state().obj({id}).alternate_cost_paid(cost)"))
            }
            // What put it there is what was resolving as it entered, or the Aura it
            // came with (CR 303.4f); the history reads the same of a token.
            ObjectFilter::MadeBy(object) if !layer => format!(
                "{}.is_some_and(|m: ObjectId| q.state().obj({id}).entered_with == Some(m) || q.history().all().iter().any(|r| matches!(r.event, Event::Entered {{ permanent, .. }} if permanent == {id}) && (r.cause == Cause::Spell(m) || matches!(r.cause, Cause::Ability {{ source, .. }} if source == m))))",
                self.object(object)?
            ),
            ObjectFilter::MadeBy(object) => held(format!(
                "q.state().obj({id}).made_by == {}",
                self.object(object)?
            )),
            ObjectFilter::HasActivatedAbility(which) => match activation_test(which) {
                Some(test) if layer => {
                    let mut code = format!("{chars}.abilities().any(|a| matches!(q.cards().ability(a), AbilityDef::Activated(d) if !d.special_action{test}))");
                    if matches!(which, ActivationFilter::Mana) {
                        code = format!("({code} || {chars}.abilities().any(|a| matches!(q.cards().ability(a), AbilityDef::Triggered(t) if t.mana_ability)))");
                    }
                    code
                }
                Some(test) => {
                    let mut code = format!("{chars}.abilities().any(|a| q.activated_ability(a).is_some_and(|d| !d.special_action{test}))");
                    if matches!(which, ActivationFilter::Mana) {
                        code = format!("({code} || {chars}.abilities().any(|a| q.triggered_ability(a).is_some_and(|t| t.mana_ability)))");
                    }
                    code
                }
                None => held(format!("q.card_of({id}).activated.iter().any(|d| matches(d))")),
            },
            other => return self.predicate_more(other, c),
        })
    }
    /// The rest of `condition`.
    pub(super) fn condition_rest(&self, condition: &Condition) -> Result<String> {
        let held = |code: &str| {
            planned::planned_value(&format!("condition {}", form(condition)), "false", code)
        };
        Ok(match condition {
            Condition::Searched(_) => held("g.searched_library()"),
            Condition::PlayerDid {
                player,
                event,
                window,
            } if self.reader != Reader::Layer => self.player_did(player, event, window)?,
            Condition::GiftPromised(true) if self.reader != Reader::Layer => {
                "cantrip_mechanics::gift::paid(q, asker)".into()
            }
            Condition::GiftPromised(false) if self.reader != Reader::Layer => {
                "!cantrip_mechanics::gift::paid(q, asker)".into()
            }
            Condition::ItsNight => "q.state().turn.day_night == DayNight::Night".into(),
            Condition::ItsNeitherDayNorNight => "q.state().turn.day_night == DayNight::Neither".into(),
            Condition::PlayerIn { player, players } => {
                format!(
                    "{}.is_some_and(|p: PlayerId| {})",
                    self.player(player)?,
                    self.player_in(players, "p")?
                )
            }
            Condition::PlayerAttacks { attacker, defender } => format!(
                "{{ let (by, at) = ({}, {}); q.state().attackers.iter().any(|a| Some(q.controller(a.id)) == by && Some(a.defending) == at) }}",
                self.player(attacker)?,
                self.player(defender)?
            ),
            // No game of Cantrip's has teams: "your team" is you alone (CR 810 is the variant).
            Condition::SameTeam { player, other } => {
                format!("{} == {}", self.player(player)?, self.player(other)?)
            }
            Condition::MostVotes { .. } => held("g.votes().most(option)"),
            Condition::ChosenWord(word) => self.chosen_word(word)?,
            Condition::VotedFor { .. } | Condition::Voted(_) | Condition::AllVotedFor(_) => {
                held("g.votes().voted(option)")
            }
            other => return self.condition_more(other),
        })
    }
    /// "Create an X/X": the token is that size as it's made (CR 111.4), never
    /// 0/0 for a moment, as Shark Typhoon's. False if these aren't such tokens.
    pub(super) fn sized_tokens(
        &mut self,
        id: usize,
        players: &PlayerSet,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<bool> {
        let [TokenSpec {
            amount,
            words,
            recipe: TokenRecipe::Defined(definition),
        }] = tokens
        else {
            return Ok(false);
        };
        let Some((power, toughness)) = &definition.power_toughness else {
            return Ok(false);
        };
        if matches!((power, toughness), (Expr::Constant(_), Expr::Constant(_)))
            || !entry.states.is_empty()
            || !entry.counters.is_empty()
            || entry.tapped
        {
            return Ok(false);
        }
        let count = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
        let power = self.number(power)?.amount(i16::MAX.into(), "i16")?;
        let toughness = self.number(toughness)?.amount(i16::MAX.into(), "i16")?;
        let mut base = definition.clone();
        base.power_toughness = Some((Expr::Constant(0), Expr::Constant(0)));
        let definition = self.token_definition(&base)?;
        // Words a text-changing effect gave the ability (CR 612.4).
        let words = match words {
            TokenWords::Rules => "Words::NONE",
            TokenWords::CreatingEffect => "g.words(asker.source)",
        };
        writeln!(
            out,
            "let count_{id} = {}; let power_{id} = {}; let toughness_{id} = {}; let players_{id} = {}; let mut values_{id} = Copiable::of(g.card_id(&{definition})).sized(power_{id}, toughness_{id}); values_{id}.words = {words}; let mut created_{id}: Vec<ObjectId> = Vec::new(); for player in players_{id} {{ created_{id}.extend(g.create_token_copies_of(values_{id}, player, count_{id})?); }}",
            self.fetch(count),
            self.fetch(power),
            self.fetch(toughness),
            self.fetch(self.players(players)?),
        )
        .unwrap();
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(true)
    }
    /// "Any player may pay": each is asked in turn, and the first who does ends it.
    pub(super) fn first_willing(
        &mut self,
        id: usize,
        players: &PlayerSet,
        cost: &Cost,
        optional: bool,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if !optional {
            return Err(gap("possibly absent player FirstWilling"));
        }
        let asked = self.fetch(self.players(players)?);
        let binding = Binding(self.fresh() as u32 + 1_000_000);
        self.bound_players.insert(binding.0);
        // "Half their life": the cost reads whoever is being asked.
        let key = format!("willing {players:?}");
        let asking = format!("Some(bound_{})", binding.0);
        self.picked.borrow_mut().named.push((key.clone(), asking));
        writeln!(
            out,
            "let mut paid_{id} = false; for bound_{} in {asked} {{",
            binding.0
        )
        .unwrap();
        let ask = Operation::PayCost {
            player: PlayerRef::Bound(binding.clone()),
            cost: cost.clone(),
            optional: true,
        };
        let done = self.operation(&ask, result, out);
        self.picked.borrow_mut().named.retain(|(name, _)| *name != key);
        self.bound_players.remove(&binding.0);
        done?;
        let inner = self.performed.insert(result.0, format!("paid_{id}"));
        let inner = inner.ok_or_else(|| gap("possibly absent player FirstWilling"))?;
        writeln!(out, "if {inner} {{ paid_{id} = true; break; }} }}").unwrap();
        self.moved.remove(&result.0);
        self.before.remove(&result.0);
        self.selections.remove(&result.0);
        Ok(())
    }
}

/// A color's mana letter.
pub(super) fn mana_letter(color: &ManaColor) -> String {
    let word = form(color);
    match word.as_str() {
        "White" => "W".into(),
        "Blue" => "U".into(),
        "Black" => "B".into(),
        "Red" => "R".into(),
        "Green" => "G".into(),
        _ => "C".into(),
    }
}

/// The name of a condition no card states, which `Compiler::class` gates a
/// Class's levels with: this, then the level.
pub(super) const CLASS_LEVEL: &str = "class level ";
/// The same, for a Case's "Solved" abilities.
pub(super) const CASE_SOLVED: &str = "case solved";

/// Which of Cantrip's gifts a program gives (CR 702.174d-i): the `gift` constant,
/// the token to list with the card, and its words.
fn gift_kind(gift: &[Program]) -> Option<(&'static str, Option<&'static str>, &'static str)> {
    let [Program { instructions }] = gift else {
        return None;
    };
    let [Instruction::Effect { operation, .. }] = instructions.as_slice() else {
        return None;
    };
    let gifted = |players: &PlayerSet| matches!(players, PlayerSet::One(PlayerRef::GiftedPlayer));
    match operation {
        Operation::Draw {
            players,
            amount: Expr::Constant(1),
        } if gifted(players) => Some(("CARD", None, "the chosen opponent draws a card")),
        Operation::Keyword {
            actor,
            action: KeywordAction::TakeExtraTurn,
            ..
        } if gifted(actor) => Some((
            "EXTRA_TURN",
            None,
            "the chosen opponent takes an extra turn",
        )),
        Operation::CreateTokens {
            players,
            tokens,
            entry,
        } if gifted(players) => {
            let [TokenSpec {
                amount: Expr::Constant(1),
                recipe: TokenRecipe::Defined(token),
                ..
            }] = tokens.as_slice()
            else {
                return None;
            };
            let [subtype] = token.subtypes.as_slice() else {
                return None;
            };
            match (format!("{subtype:?}").as_str(), entry.tapped) {
                ("Food", false) => Some((
                    "FOOD",
                    Some("FOOD"),
                    "the chosen opponent creates a Food token",
                )),
                ("Treasure", false) => Some((
                    "TREASURE",
                    Some("TREASURE"),
                    "the chosen opponent creates a Treasure token",
                )),
                ("Fish", true) => Some((
                    "TAPPED_FISH",
                    Some("FISH"),
                    "the chosen opponent creates a tapped 1/1 blue Fish creature token",
                )),
                ("Octopus", false) => Some((
                    "OCTOPUS",
                    Some("OCTOPUS"),
                    "the chosen opponent creates an 8/8 blue Octopus creature token",
                )),
                _ => None,
            }
        }
        _ => None,
    }
}
/// A mana cost in the words `cost` reads: "{1}{B/G}".
fn symbols_text(symbols: &[ManaSymbol]) -> Option<String> {
    let letter = |color: &ManaColor| {
        Some(match color {
            ManaColor::White => "W",
            ManaColor::Blue => "U",
            ManaColor::Black => "B",
            ManaColor::Red => "R",
            ManaColor::Green => "G",
            _ => return None,
        })
    };
    let mut text = String::new();
    for symbol in symbols {
        match symbol {
            ManaSymbol::Generic(n) => text.push_str(&format!("{{{n}}}")),
            ManaSymbol::Color(color) => text.push_str(&format!("{{{}}}", letter(color)?)),
            ManaSymbol::Hybrid(a, b) => {
                text.push_str(&format!("{{{}/{}}}", letter(a)?, letter(b)?))
            }
            _ => return None,
        }
    }
    Some(text)
}

/// "This creature escapes with ...": a replacement of its entering, if it escaped.
pub(super) fn escapes(event: &ReplacementEvent) -> bool {
    let ReplacementEvent::Enter {
        objects: ObjectFilter::All(parts),
    } = event
    else {
        return false;
    };
    matches!(
        parts.as_slice(),
        [
            ObjectFilter::SameObject(ObjectRef::Source),
            ObjectFilter::State(ObjectState::Escaped)
        ]
    )
}
/// What a static ability lets a spell's caster pay rather than its mana cost
/// (CR 118.9), and what the choice is called.
pub(super) fn instead_of_mana_cost(cost: &PlayCost) -> Option<(String, String)> {
    let parts = match cost {
        PlayCost::Free => {
            return Some((
                "Without paying its mana cost".into(),
                "InsteadOfManaCost::WithoutPaying".into(),
            ))
        }
        // "Pay {0}": an alternative cost of no mana, which is not the mana cost.
        PlayCost::Alternate(Operand::List(terms))
            if super::mechanics::kicker_words(terms).as_deref() == Some("{0}") =>
        {
            return Some((
                "Pay {0}".into(),
                "InsteadOfManaCost::Mana(ManaCost::ZERO)".into(),
            ))
        }
        PlayCost::Alternate(Operand::Cost(cost)) => cost.parts.as_slice(),
        _ => return None,
    };
    match parts {
        [CostPart::Mana(symbols)] => Some((
            format!("Pay {}", symbols_text(symbols)?),
            format!("InsteadOfManaCost::Mana({})", mana(symbols).ok()?),
        )),
        [CostPart::Energy(Expr::Constant(n))] if (0..=255).contains(n) => Some((
            format!("Pay {n} energy"),
            format!("InsteadOfManaCost::Energy({n})"),
        )),
        _ => None,
    }
}
/// A quoted rule Cantrip holds as a keyword of the object that has it: "This
/// creature can't block", which leaves with the object's abilities (CR 613.1f).
pub(super) fn granted_keyword(ability: &Ability) -> Option<&'static str> {
    let Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes,
    } = ability
    else {
        return None;
    };
    let [Change::Rule(rule)] = changes.as_slice() else {
        return None;
    };
    match rule {
        RuleChange::Prohibit(ObjectAction::Block(ObjectFilter::Any)) => Some("Keyword::CantBlock"),
        RuleChange::Prohibit(ObjectAction::BeBlocked(ObjectFilter::Any)) => {
            Some("Keyword::CantBeBlocked")
        }
        RuleChange::CombatGroup {
            role: CombatRole::BlockedAttacker,
            participants: ObjectFilter::Any,
            others_only: false,
            comparison: Comparison::LessOrEqual,
            amount: Expr::Constant(1),
        } => Some("Keyword::CantBeBlockedByMoreThanOne"),
        _ => None,
    }
}

/// "Put N -1/-1 counters on a creature you control": what blight is (CR 701.68a).
pub(super) fn blights(placements: &[CounterPlacement]) -> Option<u8> {
    let [CounterPlacement {
        recipients: Recipients::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters))),
        kind: crate::counter::CounterKind::PowerToughness(-1, -1),
        amount,
        selection:
            Some(SelectionChoice {
                player,
                count: Count::Exactly(Expr::Constant(1)),
            }),
    }] = placements
    else {
        return None;
    };
    let [ObjectFilter::CardType(CardType::Creature), ObjectFilter::ControlledBy(PlayerSet::One(owner))] =
        filters.as_slice()
    else {
        return None;
    };
    (format!("{owner:?}") == format!("{player:?}"))
        .then(|| constant_u8(amount))
        .flatten()
}
/// "You may put a stun counter on it. If you do, ...": no cost Cantrip can pay, but
/// the same thing as an optional instruction whose doing the next one reads.
pub(super) fn counters_on_a_target(
    player: &PlayerRef,
    cost: &Cost,
    result: &ResultId,
) -> Option<CostPart> {
    let [CostPart::PutCounters(placements)] = cost.parts.as_slice() else {
        return None;
    };
    let on_a_target = placements.iter().all(|p| {
        p.selection.is_none()
            && matches!(
                p.recipients,
                Recipients::Objects(ObjectSet::One(ObjectRef::Target { .. }))
            )
    });
    on_a_target.then(|| CostPart::PlayerAction {
        player: player.clone(),
        body: Program {
            instructions: vec![Instruction::Effect {
                result: result.clone(),
                operation: Operation::PutCounters {
                    placements: placements.clone(),
                },
            }],
        },
    })
}
/// Cumulative upkeep's "Put a +1/+1 counter on a creature an opponent controls".
fn on_an_opposing_creature(placements: &[CounterPlacement]) -> bool {
    let [CounterPlacement {
        recipients: Recipients::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters))),
        kind: crate::counter::CounterKind::PowerToughness(1, 1),
        amount: Expr::Constant(1),
        selection:
            Some(SelectionChoice {
                player: PlayerRef::AbilityController,
                count: Count::Exactly(Expr::Constant(1)),
            }),
    }] = placements
    else {
        return false;
    };
    matches!(
        filters.as_slice(),
        [
            ObjectFilter::CardType(CardType::Creature),
            ObjectFilter::ControlledBy(PlayerSet::OpponentsOf(PlayerRef::AbilityController))
        ]
    )
}
/// Cumulative upkeep's "Put a -1/-1 counter on this creature".
fn on_this(placements: &[CounterPlacement]) -> bool {
    matches!(
        placements,
        [CounterPlacement {
            recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
            kind: crate::counter::CounterKind::PowerToughness(-1, -1),
            amount: Expr::Constant(1),
            selection: None,
        }]
    )
}
/// Cumulative upkeep's "An opponent gains N life".
fn opponent_gains(body: &Program) -> Option<u8> {
    let [Instruction::Effect {
        operation:
            Operation::GainLife {
                players: PlayerSet::One(PlayerRef::OneOf(gainer)),
                amount,
            },
        ..
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    matches!(
        **gainer,
        PlayerSet::OpponentsOf(PlayerRef::AbilityController)
    )
    .then(|| constant_u8(amount))
    .flatten()
}
/// Cumulative upkeep's "Put two cards from a single graveyard on the bottom of their
/// owner's library".
fn two_to_the_bottom(part: &CostPart) -> bool {
    let CostPart::Move {
        selections,
        destination: Destination::OwnersLibraryBottom,
        ..
    } = part
    else {
        return false;
    };
    matches!(
        selections.as_slice(),
        [CostObjectSelection {
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::All,
                },
                filter: ObjectFilter::Any,
                ..
            },
            choose: Some(Count::Exactly(Expr::Constant(2))),
        }]
    )
}
/// A number of an object Cantrip doesn't keep, in the form it will take.
pub(super) fn property_rest(property: &Property, id: &str) -> Result<Number> {
    if let Some(number) = lane_m::property_more(property, id) {
        return Ok(number);
    }
    let (feature, code, max) = match property {
        Property::BasePower => ("base power", format!("q.base_power({id})"), i16::MAX),
        Property::BaseToughness => (
            "base toughness",
            format!("q.base_toughness({id})"),
            i16::MAX,
        ),
        Property::Intensity => (
            "intensity",
            format!("i64::from(q.state().obj({id}).intensity)"),
            i16::MAX,
        ),
        other => return lane_p::property_p(other, id),
    };
    Ok(Number {
        code: planned::planned_value(feature, "0i64", &code),
        min: 0,
        max: max.into(),
        dynamic: true,
    })
}
/// What a cost pays for: Cantrip's variable costs are a spell's or an ability's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PaidFor {
    Ability,
    Mana,
    Spell,
}
/// The object whose effect gave `item`'s ability to its source, on the battlefield
/// or as it last was there (CR 113.2c, 608.2h). A static ability's effect has its
/// source's timestamp, so two copies of one Equipment are two givers; a resolved
/// effect is named by its own.
pub(super) const GRANTER: &str = "fn granter(q: &dyn Query, item: &StackItem) -> Option<ObjectId> { if item.acquired.by != AcquiredBy::Grant { return None; } let stamp = item.acquired.stamp; let card = item.ability.card; if let Some(e) = q.state().effects.iter().find(|e| e.timestamp == stamp && matches!(e.kind, EffectKind::AddAbility(_))) { return Some(e.source); } if let Some(id) = q.state().battlefield().iter().copied().find(|&id| q.state().obj(id).timestamp == stamp && q.chars(id).card == card) { return Some(id); } q.history().all().iter().rev().find_map(|r| match r.event { Event::ZoneChange { old, from, .. } if from.is(ZoneKind::Battlefield) && q.state().obj(old).timestamp == stamp && q.state().obj(old).card == card => Some(old), _ => None }) }\n";

/// Time travel (CR 701.56a): of each suspended card `player` owns (CR 702.62b, read
/// as the upkeep reads it) and each permanent they control with a time counter on
/// it, a time counter more, one fewer, or neither.
pub(super) const TIME_TRAVEL: &str = "fn time_travel_for(g: &mut dyn Game, player: PlayerId, source: ObjectId) -> Result<(), Suspend> { let mut each: Vec<ObjectId> = g.state().zone(Zone::exile(player)).iter().copied().filter(|&id| g.obj(id).counters(CounterKind::Time) > 0 && g.card(g.obj(id).card).suspend.is_some()).collect(); each.extend(g.permanents_of(player).into_iter().filter(|&id| g.obj(id).counters(CounterKind::Time) > 0)); let any = cantrip_mechanics::select::cards(cantrip_mechanics::select::ANY_NUMBER); let adding = cantrip_mechanics::choose::select(g, player, source, Purpose::PutCounters(CounterKind::Time, 1), each.clone(), any.clone())?; let rest: Vec<ObjectId> = each.into_iter().filter(|id| !adding.contains(id)).collect(); let removing = cantrip_mechanics::choose::select(g, player, source, Purpose::Choose, rest, any)?; let more: Vec<(ObjectId, CounterKind, i16)> = adding.iter().map(|&id| (id, CounterKind::Time, 1)).collect(); g.add_counters_together(&more); for id in removing { g.remove_counters(id, CounterKind::Time, 1); } Ok(()) }\n";

/// Whether an Aura could enchant `host` under `controller` (CR 303.4a, 702.5c):
/// every enchant ability it has allows it, the one its card prints as well, which
/// it has back on the battlefield whatever a graveyard takes, and `host` has no
/// protection from it (CR 702.16c). Not targeted, so hexproof is nothing to it.
pub(super) const COULD_ENCHANT: &str = "fn could_enchant(q: &dyn Query, aura: ObjectId, controller: PlayerId, host: ObjectId) -> bool { let chars = q.chars(aura); if chars.is(CardType::Creature) { return false; } let asker = Asker::new(aura, controller); let printed = q.card(q.state().obj(aura).card).enchant; chars.abilities().filter_map(|a| match a.index { AbilityRef::ENCHANT => q.card(a.card).enchant, AbilityRef::ENCHANT_GIVEN => q.card(a.card).becomes_aura_enchant, _ => None }).chain(printed).all(|spec| (spec.filter)(q, &asker, Target::Object(host))) && !q.chars(host).protected_from_by(&chars, || q.mana_value(aura), controller) }\n";

/// What `filter` adds to "an activated ability" over `d: &ActivatedDef`, starting
/// with ` && `: nothing for any. `None` for what `d` alone doesn't say.
fn activation_test(filter: &ActivationFilter) -> Option<String> {
    fn test(filter: &ActivationFilter) -> Option<String> {
        Some(match filter {
            ActivationFilter::Any => "true".to_string(),
            ActivationFilter::Mana => "d.is_mana_ability()".to_string(),
            ActivationFilter::Cycling => "d.cycling".to_string(),
            ActivationFilter::Is(StackTrait::TapSymbolInCost) => "d.cost.tap".to_string(),
            ActivationFilter::Not(inner) => format!("!({})", test(inner)?),
            ActivationFilter::All(parts) => {
                let parts: Option<Vec<String>> = parts.iter().map(test).collect();
                format!("({})", parts?.join(" && "))
            }
            ActivationFilter::Either(parts) => {
                let parts: Option<Vec<String>> = parts.iter().map(test).collect();
                format!("({})", parts?.join(" || "))
            }
            ActivationFilter::Of(_) | ActivationFilter::Is(_) => return None,
        })
    }
    match filter {
        ActivationFilter::Any => Some(String::new()),
        other => Some(format!(" && {}", test(other)?)),
    }
}

/// How to tell which player of a batch did which part of it, for `ObjectSet::MovedBy`.
#[derive(Clone, Debug)]
pub(super) enum Share {
    /// Only a card's owner has it in hand, so only they discard it.
    Owner,
    /// This local pairs each player with what they sacrifice.
    Picks(String),
}
/// Whether objects whose characteristics are `a` and `b` have this property in common.
pub(super) fn sharing(property: &DistinctProperty) -> Result<&'static str> {
    Ok(match property {
        DistinctProperty::CardTypes => "a.types.iter().any(|t| b.types.has(t))",
        DistinctProperty::CreatureTypes => {
            "a.subtypes.iter().any(|s| SubtypeSet::CREATURE_TYPES.has(s) && b.subtypes.has(s))"
        }
        DistinctProperty::Colors => "a.colors.intersects(b.colors)",
        other => return lane_p::sharing_p(other),
    })
}
/// Those of a program's players a choice has named, for `Compiler::pick`.
#[derive(Clone, Debug, Default)]
pub(super) struct Picked {
    /// The set chosen from, and the local holding who was picked.
    pub named: Vec<(String, String)>,
    /// What the instruction being compiled must run first.
    pub statements: Vec<String>,
}
/// Does an instruction that failed with this gap want to be compiled around a
/// player who may not be there, or who is still to be picked?
pub(super) fn wants_a_player(feature: &str) -> bool {
    feature.starts_with("possibly absent player")
        || feature == "player reference OneOf"
        || feature == "player reference FirstWilling"
}

/// "Hexproof from [quality]" (CR 702.11d-g): `HexproofFrom` qualities.
pub(super) fn hexproof_quality(quality: &Protection) -> Result<String> {
    match quality {
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Color(c),
        } => Ok(format!("HexproofFrom::color(Color::{})", color(c)?)),
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::CardType(t),
        } => Ok(format!(
            "HexproofFrom::card_type(CardType::{})",
            card_type(t)?
        )),
        Protection::Each(qualities) => {
            let mut all = "HexproofFrom::NONE".to_string();
            for quality in qualities {
                all = format!("{all}.and({})", hexproof_quality(quality)?);
            }
            Ok(all)
        }
        // "Hexproof from monocolored" and "from multicolored" (CR 105.2).
        Protection::Objects {
            kind: ObjectKind::Any,
            filter:
                ObjectFilter::Property {
                    property: Property::ColorCount,
                    comparison: Comparison::Equal,
                    value: Expr::Constant(1),
                },
        } => Ok("HexproofFrom::MONOCOLORED".to_string()),
        Protection::Objects {
            kind: ObjectKind::Any,
            filter:
                ObjectFilter::Property {
                    property: Property::ColorCount,
                    comparison: Comparison::Greater,
                    value: Expr::Constant(1),
                },
        } => Ok("HexproofFrom::MULTICOLORED".to_string()),
        Protection::Objects { filter, .. } => {
            Err(gap(format!("keyword HexproofFrom {}", form(filter))))
        }
        other => Err(gap(format!("keyword HexproofFrom {}", form(other)))),
    }
}

/// What an event is always about: "whenever this creature attacks", "whenever
/// equipped creature attacks".
pub(super) fn event_object(event: &EventSpec) -> Option<ObjectRef> {
    let (EventSpec::ZoneChange {
        objects,
        to: EventZone::Battlefield,
        ..
    }
    | EventSpec::Object { objects, .. }
    | EventSpec::Spell {
        spells: objects, ..
    }
    | EventSpec::Attack {
        attackers: objects, ..
    }) = event
    else {
        return None;
    };
    match objects {
        ObjectFilter::SameObject(object) => Some(object.clone()),
        _ => None,
    }
}

/// Abilities `costs.rs` has no arm for.
impl Compiler {
    pub(super) fn ability_rest(
        &mut self,
        ability: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        match ability {
            Ability::TriggerRule {
                cause,
                abilities,
                rule,
            } => self.trigger_rule(cause.as_ref(), abilities.as_ref(), rule, label, parts),
            Ability::Craft {
                materials,
                count,
                cost,
            } => self.craft(materials, count.as_ref(), cost, label, parts),
            Ability::Companion(condition) => self.companion(condition, label, parts),
            Ability::CommanderDeckRule(rule) => {
                let Operand::Term { form: rule, .. } = &**rule else {
                    return Err(gap("ability CommanderDeckRule"));
                };
                self.plan_entry(
                    "a deck rule a commander sets",
                    "deck_rules",
                    &format!("DeckRule::{rule}"),
                    parts,
                );
                Ok(())
            }
            Ability::Modified { ability, modifier } => {
                self.modified(ability, modifier, label, parts)
            }
            Ability::DamageCantBePrevented {
                sources,
                combat_only,
            } => {
                let test = match sources {
                    None => "true".to_string(),
                    Some(ObjectSet::One(ObjectRef::Source)) => "source == asker.source".to_string(),
                    Some(ObjectSet::Battlefield(filter)) => {
                        format!(
                            "q.is_on_battlefield(source) && {}",
                            self.filter(filter, &Candidate::of("source"))?
                        )
                    }
                    Some(other) => {
                        return Err(gap(format!(
                            "damage that can't be prevented from {}",
                            form(other)
                        )))
                    }
                };
                let function = format!("{label}_unpreventable");
                let combat = if *combat_only { "combat && " } else { "" };
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, source: ObjectId, combat: bool) -> bool {{ {combat}{test} }}").unwrap();
                parts.table("unpreventable_damage", function);
                Ok(())
            }
            Ability::Offering(filter) => {
                let filter = self.cost_filter(filter, label)?;
                self.plan_entry(
                    "offering",
                    "abilities",
                    &format!("cantrip_mechanics::offering::ability({filter}, \"Offering\")"),
                    parts,
                );
                Ok(())
            }
            Ability::Spree { modes } => self.spree(modes, "spree", label, parts),
            Ability::Tiered { modes } => self.spree(modes, "tiered", label, parts),
            Ability::Gift {
                gift,
                promised,
                unpromised,
            } => self.gift(
                gift,
                promised.as_deref(),
                unpromised.as_deref(),
                label,
                parts,
            ),
            Ability::Class { levels } => self.class(levels, label, parts),
            Ability::Mystery {
                condition,
                abilities,
            } => {
                let solve = self.activation_condition(condition, &format!("{label}_to_solve"))?;
                parts.abilities.push(format!(
                    "cantrip_mechanics::solved::to_solve({solve}, \"To solve (If unsolved, solve at the beginning of your end step.)\")"
                ));
                let solved = Condition::Primitive {
                    name: CASE_SOLVED.into(),
                    operands: Vec::new(),
                };
                self.gated(&solved, abilities, &format!("{label}_solved"), parts)
            }
            Ability::Prototype {
                mana: symbols,
                power,
                toughness,
            } => {
                let cost = symbols_text(symbols).ok_or_else(|| gap("prototype cost in symbols"))?;
                let text = format!("Prototype {cost} \u{2014} {power}/{toughness}");
                parts.field(
                    "prototype",
                    format!(
                        "Some(Prototype {{ cost: {}, power: {power}, toughness: {toughness}, text: {} }})",
                        mana(symbols)?,
                        quoted(&text)
                    ),
                );
                Ok(())
            }
            Ability::OpeningHand { announcement, body } => {
                self.opening_hand(announcement, body, label, parts)
            }
            other => self.stated_ability(other, label, parts),
        }
    }
    /// A keyword with what its plain form lacks: "Kicker {X}. X can't be 0" is
    /// Cantrip's (CR 107.3a); the others wait on it.
    fn modified(
        &mut self,
        ability: &Ability,
        modifier: &Operand,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let Operand::Term { form: what, .. } = modifier else {
            return Err(gap("ability Modified"));
        };
        let kicked = matches!(
            ability,
            Ability::Mechanic {
                mechanic: Mechanic::Kicker,
                ..
            }
        );
        if what == "XCantBeZero" && kicked {
            self.ability(ability, label, parts, None)?;
            parts.field("x_cant_be_zero", "true".into());
            return Ok(());
        }
        let mut grown = Parts::default();
        let inner = match self.ability(ability, label, &mut grown, None) {
            Ok(()) => {
                let fields = grown.fields.iter().map(|(k, v)| format!("{k}({v})"));
                grown
                    .abilities
                    .iter()
                    .cloned()
                    .chain(fields)
                    .collect::<Vec<_>>()
                    .join(", ")
            }
            Err(_) => form(ability),
        };
        self.plan_entry(
            &format!("keyword modifier {what}"),
            "abilities",
            &format!("modified({inner}, Modifier::{what})"),
            parts,
        );
        Ok(())
    }
    /// One kind of material of craft (CR 702.167a): what it tests of `id`, and what
    /// the prompt calls it.
    fn material(&mut self, term: &Operand) -> Result<(String, String)> {
        const FEATURE: &str = "ability Craft";
        let Operand::Term {
            form: what,
            operands,
            ..
        } = term
        else {
            return Err(gap(FEATURE));
        };
        let word = |operand: &Operand| match operand {
            Operand::Vocabulary { form, .. } => Ok(form.clone()),
            _ => Err(gap(FEATURE)),
        };
        Ok(match (what.as_str(), operands.as_slice()) {
            ("AnyCard", []) => ("true".into(), String::new()),
            ("IsCardtype", [kind]) => {
                let kind = word(kind)?;
                (
                    format!("q.chars(id).is(CardType::{kind})"),
                    kind.to_lowercase(),
                )
            }
            ("IsNonCardtype", [kind]) => {
                let kind = word(kind)?;
                (
                    format!("!q.chars(id).is(CardType::{kind})"),
                    format!("non{}", kind.to_lowercase()),
                )
            }
            ("IsCreatureType" | "IsLandType", [kind]) => {
                let kind = word(kind)?;
                if !crate::cantrip_vocab::SUBTYPES.contains(&kind.as_str()) {
                    return Err(gap(format!("subtype {kind}")));
                }
                (
                    format!(
                        "q.chars(id).has_subtype(q.words(asker.source).subtype(Subtype::{kind}))"
                    ),
                    kind,
                )
            }
            ("IsColor", [hue]) => {
                let hue = word(hue)?;
                (
                    format!("q.chars(id).colors.has(q.words(asker.source).color(Color::{hue}))"),
                    hue.to_lowercase(),
                )
            }
            ("HasAbility", [Operand::Term { form: ability, .. }])
                if ability == "ActivatedAbility" =>
            {
                let test = self.filter(
                    &ObjectFilter::HasActivatedAbility(ActivationFilter::Any),
                    &Candidate::of("id"),
                )?;
                (test, "with an activated ability".into())
            }
            ("And" | "Or", parts) => {
                let mut tests = Vec::new();
                let mut words = Vec::new();
                for part in parts {
                    let (test, quality) = self.material(part)?;
                    tests.push(format!("({test})"));
                    words.push(quality);
                }
                let (join, word) = if what == "And" {
                    (" && ", " ")
                } else {
                    (" || ", " or ")
                };
                (tests.join(join), words.join(word))
            }
            _ => return Err(gap(FEATURE)),
        })
    }
    /// A material's test as Cantrip's `ObjectFilter`.
    fn material_filter(&mut self, test: &str, label: &str) -> String {
        let function = format!("{label}_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {test} }}").unwrap();
        function
    }
    /// Craft (CR 702.167): the mana, and what is exiled with this.
    fn craft(
        &mut self,
        materials: &Operand,
        count: Option<&(Comparison, Expr)>,
        cost: &Operand,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        const FEATURE: &str = "ability Craft";
        let Operand::List(terms) = cost else {
            return Err(gap(FEATURE));
        };
        let mut symbols = Vec::new();
        for term in terms {
            let Operand::Term {
                form: symbol,
                operands,
                ..
            } = term
            else {
                return Err(gap(FEATURE));
            };
            symbols.push(match (symbol.as_str(), operands.as_slice()) {
                ("ManaCostGeneric", [Operand::Integer(n)]) => {
                    ManaSymbol::Generic((*n).try_into().map_err(|_| gap(FEATURE))?)
                }
                ("ManaCostW", []) => ManaSymbol::Color(ManaColor::White),
                ("ManaCostU", []) => ManaSymbol::Color(ManaColor::Blue),
                ("ManaCostB", []) => ManaSymbol::Color(ManaColor::Black),
                ("ManaCostR", []) => ManaSymbol::Color(ManaColor::Red),
                ("ManaCostG", []) => ManaSymbol::Color(ManaColor::Green),
                _ => return Err(gap(FEATURE)),
            });
        }
        let price = symbols_text(&symbols).ok_or_else(|| gap(FEATURE))?;
        let how_many = match count {
            None => (1, false),
            Some((Comparison::Equal, Expr::Constant(n))) => {
                (u8::try_from(*n).map_err(|_| gap(FEATURE))?, false)
            }
            Some((Comparison::GreaterOrEqual, Expr::Constant(n))) => {
                (u8::try_from(*n).map_err(|_| gap(FEATURE))?, true)
            }
            _ => return Err(gap(FEATURE)),
        };
        let table = format!("{}_MATERIALS", label.to_ascii_uppercase());
        let (code, quality) = match materials {
            Operand::List(terms)
                if terms.iter().all(
                    |t| matches!(t, Operand::Term { family, .. } if family == "Craftable"),
                ) =>
            {
                let mut each = Vec::new();
                let (mut tests, mut names) = (Vec::new(), Vec::new());
                for term in terms {
                    let (test, quality) = self.material(term)?;
                    let filter = self.material_filter(&test, label);
                    each.push(format!(
                        "MaterialPart {{ filter: {filter}, quality: {} }}",
                        quoted(&quality)
                    ));
                    tests.push(format!("({test})"));
                    names.push(quality);
                }
                let any = self.material_filter(&tests.join(" || "), label);
                let quality = names.join(", ");
                (
                    format!(
                        "cantrip_mechanics::craft::materials_each(&[{}], {any}, {})",
                        each.join(", "),
                        quoted(&quality)
                    ),
                    quality,
                )
            }
            Operand::List(terms) => {
                let [term, Operand::Term { form: group, .. }] = terms.as_slice() else {
                    return Err(gap(FEATURE));
                };
                let (test, quality) = self.material(term)?;
                let filter = self.material_filter(&test, label);
                let (n, more) = how_many;
                self.plan_entry(
                    "craft materials that must have something in common",
                    "abilities",
                    &format!("cantrip_mechanics::craft::ability({}, &cantrip_mechanics::craft::materials({n}, {more}, {filter}, {}).together(Group::{group}), {})", mana(&symbols)?, quoted(&quality), quoted(&format!("Craft {price}"))),
                    parts,
                );
                return Ok(());
            }
            term => {
                let (test, quality) = self.material(term)?;
                let filter = self.material_filter(&test, label);
                let (n, more) = how_many;
                (
                    format!(
                        "cantrip_mechanics::craft::materials({n}, {more}, {filter}, {})",
                        quoted(&quality)
                    ),
                    quality,
                )
            }
        };
        writeln!(self.functions, "static {table}: Materials = {code};").unwrap();
        let text = format!("Craft with {quality} {price}");
        parts.abilities.push(format!(
            "cantrip_mechanics::craft::ability({}, &{table}, {})",
            mana(&symbols)?,
            quoted(&text)
        ));
        Ok(())
    }
    /// A filter of a starting deck's card `id`, which no resolution or event reads.
    fn deck_card(&mut self, filter: &ObjectFilter) -> Result<String> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let test = self.filter(filter, &Candidate::of("id"));
        (self.reader, self.event_at) = saved;
        Ok(format!(
            "{{ let asker = &Asker::new(id, deck.player); {} }}",
            test?
        ))
    }
    /// Companion (CR 702.139): what a starting deck has to be, read before the game.
    fn companion(&mut self, condition: &Operand, label: &str, parts: &mut Parts) -> Result<()> {
        const FEATURE: &str = "ability Companion";
        let Operand::Term {
            form: rule,
            operands,
            ..
        } = condition
        else {
            return Err(gap(FEATURE));
        };
        let body = match (rule.as_str(), operands.as_slice()) {
            ("AllCardsPassFilter", [Operand::Filter(test)]) => {
                format!("deck.cards.iter().all(|&id| {})", self.deck_card(test)?)
            }
            (
                "NoCardPassesFilter",
                [Operand::Filter(ObjectFilter::State(ObjectState::RepeatedManaSymbol))],
            ) => "deck.cards.iter().all(|&id| { let cost = q.mana_cost(id); cost.x < 2 && cost.symbols.iter().chain(cost.twobrid.iter()).chain(cost.phyrexian.iter()).all(|&n| n < 2) && cost.hybrid.iter().flatten().enumerate().all(|(i, h)| cost.hybrid.iter().flatten().take(i).all(|other| other != h)) })".to_string(),
            ("EachCardPassesFilter", [Operand::Filter(subset), Operand::Filter(test)]) => format!(
                "deck.cards.iter().all(|&id| !{} || {})",
                self.deck_card(subset)?,
                self.deck_card(test)?
            ),
            (
                "EachCardPassesGroupFilter",
                [Operand::Filter(subset), Operand::Term { form: group, .. }],
            ) => {
                let those = format!(
                    "let those: Vec<ObjectId> = deck.cards.iter().copied().filter(|&id| {}).collect();",
                    self.deck_card(subset)?
                );
                match group.as_str() {
                    "DifferentNames" => format!("{those} those.iter().enumerate().all(|(i, &a)| those[..i].iter().all(|&b| !cantrip_mechanics::same_name(q, a, b)))"),
                    "ShareACardType" => format!("{those} those.first().is_none_or(|&first| q.chars(first).types.iter().any(|t| those.iter().all(|&id| q.chars(id).is(t))))"),
                    _ => return Err(gap(FEATURE)),
                }
            }
            ("IncreaseStartingDeckSize", [Operand::Number(Expr::Constant(n))]) => {
                format!("deck.cards.len() as u64 >= u64::from(deck.minimum_size) + {n}")
            }
            _ => return Err(gap(FEATURE)),
        };
        let function = format!("{label}_companion");
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, deck: &StartingDeck) -> bool {{ {body} }}").unwrap();
        parts.field("companion", format!("Some({function})"));
        Ok(())
    }
    /// "Create a Tarmogoyf token": a token that is that card (CR 111.3), which
    /// Cantrip can't find by name.
    pub(super) fn tokens_of_a_card(
        &mut self,
        id: usize,
        players: &PlayerSet,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if !entry.counters.is_empty() || !entry.states.is_empty() {
            return Err(gap("tokens entering in a special state"));
        }
        let players = self.fetch(self.players(players)?);
        let mut lines = Vec::new();
        for token in tokens {
            let TokenRecipe::Named(card) = &token.recipe else {
                return Err(gap(format!("token recipe {}", form(&token.recipe))));
            };
            let n = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
            lines.push(format!(
                "for player in {players} {{ g.create_token_copies_tapped(Copiable::of(CardId::named({})), player, {}, {})?; }}",
                quoted(card),
                self.fetch(n),
                entry.tapped
            ));
        }
        writeln!(out, "let created_{id}: Vec<ObjectId> = Vec::new();").unwrap();
        self.plan_code("a token that is a named card", &lines.join(" "), out);
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    /// "Spells cast from your graveyard": the zone a spell is in while its cost
    /// is worked out, or the one it was cast from once it is on the stack.
    pub(super) fn being_cast_from(
        &self,
        by: Option<&PlayerSet>,
        from: Option<&ZoneScope>,
        c: &Candidate,
    ) -> Result<String> {
        const FEATURE: &str = "how a spell being cast is being cast";
        let (None, Some(from)) = (by, from) else {
            return Err(gap(FEATURE));
        };
        let id = &c.id;
        let (zone, owner) = match from {
            ZoneScope::Shared(zone) => (zone, None),
            ZoneScope::Players { zone, players } => (zone, Some(players)),
        };
        let test = format!(
            "cantrip_mechanics::zone_cast_from(q, {id}) == Some({})",
            zone_kind(zone)?
        );
        let owned = format!("q.state().obj({id}).owner");
        Ok(match owner {
            None | Some(PlayerSet::All) => test,
            Some(PlayerSet::One(PlayerRef::CasterOf(of)))
                if matches!(**of, ObjectRef::Candidate) =>
            {
                format!("({test} && {owned} == {})", c.controller)
            }
            Some(players) => format!("({test} && {}.contains(&{owned}))", self.players(players)?),
        })
    }
    /// Spree (CR 702.172), modes chosen by paying for them, at least one; and
    /// tiered (CR 702.183), exactly one of them.
    fn spree(
        &mut self,
        modes: &[SpreeMode],
        mechanic: &str,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        if parts.spell != "None" {
            return Err(gap("multiple spell bodies"));
        }
        let mut defs = Vec::new();
        for (i, spree) in modes.iter().enumerate() {
            let [CostPart::Mana(symbols)] = spree.cost.parts.as_slice() else {
                return Err(gap("spree mode that costs more than mana"));
            };
            let Ability::Spell { announcement, body } = &*spree.ability else {
                return Err(gap("spree mode that is no spell"));
            };
            let label = format!("{label}_mode_{i}");
            let targets = self.announcement(announcement, &label)?;
            let effect = self.effect(body, &label)?;
            defs.push(format!(
                "mode({effect}, {}).targets({targets}).cost({})",
                quoted(&self.said_mode(i, modes.len(), ability_text(body))),
                mana(symbols)?
            ));
        }
        let name = format!("{}_MODES", label.to_ascii_uppercase());
        writeln!(
            self.functions,
            "static {name}: ModalDef = cantrip_mechanics::{mechanic}::modal(&[{}]);",
            defs.join(",")
        )
        .unwrap();
        parts.spell = "Some(SpellDef::MODAL)".into();
        parts.modal = format!("Some(&{name})");
        Ok(())
    }
    /// Gift (CR 702.174). Which gift is one of Cantrip's six: the core gives it
    /// before a spell's other effects, and a permanent lists its trigger.
    fn gift(
        &mut self,
        gift: &[Program],
        promised: Option<&Ability>,
        unpromised: Option<&Ability>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let (kind, token, text) = gift_kind(gift).ok_or_else(|| gap("a gift of another kind"))?;
        if let Some(token) = token {
            self.extras
                .push(format!("cantrip_mechanics::tokens::{token}"));
        }
        parts.field("gift", format!("Some(cantrip_mechanics::gift::{kind})"));
        let (Some(promised), Some(unpromised)) = (promised, unpromised) else {
            parts.abilities.push(format!(
                "cantrip_mechanics::gift::trigger({})",
                quoted(&format!(
                    "When this permanent enters, if its gift cost was paid, {text}."
                ))
            ));
            return Ok(());
        };
        let (
            Ability::Spell {
                announcement: promised_announced,
                body: promised,
            },
            Ability::Spell {
                announcement: unpromised_announced,
                body: unpromised,
            },
        ) = (promised, unpromised)
        else {
            return Err(gap("gift outcomes that are not spells"));
        };
        if parts.spell != "None" {
            return Err(gap("multiple spell bodies"));
        }
        let (specs, counted) =
            self.gift_targets(promised_announced, unpromised_announced, label)?;
        let effect = self.branch_effect(
            "item.gift.is_some()",
            promised,
            promised_announced,
            unpromised,
            unpromised_announced,
            label,
        )?;
        if counted {
            self.plan_entry(
                "a gift that changes how many a spell targets",
                "spell",
                &format!("spell(&[{}], {effect})", specs.join(",")),
                parts,
            );
            return Ok(());
        }
        let targets = if specs.is_empty() {
            "&[]".to_string()
        } else {
            let array = format!("{}_TARGETS", label.to_ascii_uppercase());
            writeln!(
                self.functions,
                "static {array}: &[cantrip_api::TargetSpec] = &[{}];",
                specs.join(",")
            )
            .unwrap();
            array
        };
        parts.spell = format!("spell({targets}, {effect})");
        Ok(())
    }
    /// A spell's targets where the gift changes them: the promise comes before
    /// the targets (CR 601.2b-c, 702.174m), so each slot's filter reads it. True
    /// if it also changes how many, which `TargetSpec` can only do for a kicker.
    fn gift_targets(
        &mut self,
        promised: &Announcement,
        unpromised: &Announcement,
        label: &str,
    ) -> Result<(Vec<String>, bool)> {
        if promised.modes.is_some() || unpromised.modes.is_some() {
            return Err(gap("gift spell with modes"));
        }
        self.announcement(promised, &format!("{label}_promised"))?;
        let promised_filters = std::mem::take(&mut self.announced_filters);
        self.announcement(unpromised, &format!("{label}_unpromised"))?;
        let unpromised_filters = std::mem::take(&mut self.announced_filters);
        let mut specs = Vec::new();
        let mut counted = false;
        for i in 0..promised.targets.len().max(unpromised.targets.len()) {
            let range = |targets: &[TargetSpec]| {
                targets
                    .get(i)
                    .map_or(Ok((0, 0)), |spec| target_range(&spec.count))
            };
            let (given, (min, max)) = (range(&promised.targets)?, range(&unpromised.targets)?);
            let side = |filters: &[String]| {
                filters
                    .get(i)
                    .map_or("false".to_string(), |f| format!("({f})(q, asker, t)"))
            };
            let function = format!("{label}_target_{i}");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ if cantrip_mechanics::gift::promised(q, asker.source) {{ {} }} else {{ {} }} }}",
                side(&promised_filters),
                side(&unpromised_filters)
            )
            .unwrap();
            let spec = format!(
                "TargetSpec {{ min: {min}, max: {max}, filter: {function}, label: \"\", if_kicked: None, together: None }}"
            );
            if given == (min, max) {
                specs.push(spec);
            } else {
                counted = true;
                let (least, most) = given;
                specs.push(format!("{spec}.if_promised({least}, {most})"));
            }
        }
        Ok((specs, counted))
    }
    /// A Class (CR 716): each level is an ability to gain it, and abilities that
    /// work from that level on.
    fn class(&mut self, levels: &[ClassLevel], label: &str, parts: &mut Parts) -> Result<()> {
        for (i, level) in levels.iter().enumerate() {
            let [CostPart::Mana(symbols)] = level.cost.parts.as_slice() else {
                return Err(gap("class level cost other than mana"));
            };
            let cost = symbols_text(symbols).ok_or_else(|| gap("class level cost in symbols"))?;
            let (function, number) = match i {
                0 => ("level_two", 2),
                1 => ("level_three", 3),
                _ => return Err(gap("class level past three")),
            };
            parts.abilities.push(format!(
                "cantrip_mechanics::class::{function}({}, {})",
                quoted(&cost),
                quoted(&format!("{cost}: Level {number}"))
            ));
            let held = Condition::Primitive {
                name: format!("{CLASS_LEVEL}{number}"),
                operands: Vec::new(),
            };
            self.gated(
                &held,
                &level.abilities,
                &format!("{label}_level_{number}"),
                parts,
            )?;
        }
        Ok(())
    }
    /// Abilities that work while `held` is so (CR 716.2, 702.169). A triggered
    /// one asks with its event and not again: it's on the stack by then, whatever
    /// became of its source. An activated one asks as it's activated.
    fn gated(
        &mut self,
        held: &Condition,
        abilities: &[Ability],
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        for (i, ability) in abilities.iter().enumerate() {
            let label = format!("{label}_{i}");
            let mut gated = ability.clone();
            match &mut gated {
                Ability::Triggered { .. } => self.trigger_gate = Some(held.clone()),
                Ability::Activated { limits, .. } => {
                    limits.condition = Some(match limits.condition.take() {
                        Some(other) => Condition::All(vec![held.clone(), other]),
                        None => held.clone(),
                    })
                }
                Ability::Conditional { condition, .. } => {
                    *condition = Condition::All(vec![held.clone(), condition.clone()])
                }
                other => {
                    gated = Ability::Conditional {
                        condition: held.clone(),
                        then: vec![other.clone()],
                        otherwise: Vec::new(),
                    }
                }
            }
            let compiled = self.ability(&gated, &label, parts, None);
            self.trigger_gate = None;
            compiled?;
        }
        Ok(())
    }
    /// "This Case is solved" (CR 719.3b).
    pub(super) fn case_solved(&self) -> String {
        match self.reader {
            Reader::Layer => "q.state().obj(source).marks.solved".into(),
            _ => "q.is_solved(asker.source)".into(),
        }
    }
    /// What a card does from an opening hand (CR 103.6a). Beginning the game on
    /// the battlefield is Cantrip's; a reveal that sets up a later effect isn't.
    fn opening_hand(
        &mut self,
        announcement: &Announcement,
        body: &Program,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let begins = |instruction: &Instruction| {
            matches!(
                instruction,
                Instruction::Effect {
                    operation: Operation::BeginOnBattlefield {
                        actor: PlayerSet::One(PlayerRef::AbilityController),
                        ..
                    },
                    ..
                }
            )
        };
        if let [Instruction::Optional { body: choice, .. }] = body.instructions.as_slice() {
            if matches!(choice.instructions.as_slice(), [only] if begins(only)) {
                parts.field(
                    "opening_hand",
                    "Some(cantrip_mechanics::statics::BEGIN_ON_THE_BATTLEFIELD)".into(),
                );
                return Ok(());
            }
        }
        if format!("{body:?}").contains("BeginOnBattlefield") {
            return Err(gap("beginning the game on the battlefield with terms"));
        }
        self.announcement(announcement, label)?;
        let effect = self.effect(body, label)?;
        self.plan_entry(
            "a reveal from an opening hand",
            "abilities",
            &format!(
                "cantrip_mechanics::opening_hand::reveal({effect}, {})",
                quoted(&ability_text(body))
            ),
            parts,
        );
        Ok(())
    }
    /// Whether a Class is at this level or past it (CR 716.2): the name holds it.
    pub(super) fn class_level(&self, name: &str) -> String {
        let level = &name[CLASS_LEVEL.len()..];
        let source = match self.reader {
            Reader::Layer => "source",
            _ => "asker.source",
        };
        format!("q.state().obj({source}).class_level >= {level}")
    }
    /// A mana ability that does more than add mana first: "If you control a
    /// creature with power 4 or greater, add {G}{G} instead", "Choose a color. Add
    /// one mana of that color for each charge counter". What it adds is known only
    /// as it resolves, as Gaea's Cradle's is, so nothing is listed.
    pub(super) fn mana_program(&mut self, body: &Program, label: &str) -> Result<String> {
        if !format!("{body:?}").contains("AddMana") {
            return Err(gap("compound mana ability callback"));
        }
        let mut out = String::new();
        self.stack_item = false;
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        self.reset_results();
        let compiled = self.program(body, &mut out, false);
        self.chosen.clear();
        self.reader = reader;
        self.stack_item = true;
        compiled?;
        let named = lane_k::color_as_activated(&out);
        let adds = match named.is_some() {
            true => format!("add_any_color(1).by({label})"),
            false => format!("add_by({label})"),
        };
        let out = named.unwrap_or(out);
        writeln!(
            self.functions,
            "fn {label}(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(),Suspend> {{ {out} Ok(()) }}"
        )
        .unwrap();
        Ok(adds)
    }
    /// What limits a mana ability's color to the one chosen as it entered, and
    /// to the colors it can always make beside it.
    pub(super) fn colors_from(&mut self, body: &Program, label: &str) -> Result<String> {
        const CHOSEN: &str = "cantrip_mechanics::lands::chosen_color";
        if chosen::makes_chosen_color(body) {
            return Ok(format!(".colors_from({CHOSEN})"));
        }
        let Some(Instruction::Effect {
            operation:
                Operation::AddMana {
                    mana: ManaProduction::ChooseOne(options),
                    ..
                },
            ..
        }) = body.instructions.first()
        else {
            return Ok(String::new());
        };
        let colors: Vec<&ManaColor> = options
            .iter()
            .filter_map(|option| match option {
                ManaProduction::Amount { color, .. } => Some(color),
                _ => None,
            })
            .collect();
        if !colors.iter().any(|c| matches!(c, ManaColor::Chosen)) {
            return Ok(String::new());
        }
        let mut set = format!("{CHOSEN}(q, asker)");
        for c in colors.iter().filter(|c| !matches!(c, ManaColor::Chosen)) {
            set.push_str(&format!(".with(Color::{})", color(c)?));
        }
        writeln!(
            self.functions,
            "fn {label}_colors(q: &dyn Query, asker: &Asker) -> ColorSet {{ {set} }}"
        )
        .unwrap();
        Ok(format!(".colors_from({label}_colors)"))
    }
    /// Whether the activated ability `ability` of `object` is one of these. What
    /// "of artifacts" says of permanents, a name says in any zone (CR 109.2, 201.2).
    pub(super) fn barred_activation(&self, filter: &ActivationFilter) -> Result<String> {
        Ok(match filter {
            ActivationFilter::Any => "true".into(),
            ActivationFilter::Cycling => "ability.cycling".into(),
            ActivationFilter::Mana | ActivationFilter::Is(StackTrait::Mana) => {
                "ability.is_mana_ability()".into()
            }
            ActivationFilter::Is(StackTrait::Loyalty) => "ability.is_loyalty_ability()".into(),
            ActivationFilter::Is(StackTrait::TapSymbolInCost) => "ability.cost.tap".into(),
            ActivationFilter::Is(StackTrait::XInCost) => "ability.cost.mana.x > 0".into(),
            ActivationFilter::Not(inner) => format!("!({})", self.barred_activation(inner)?),
            ActivationFilter::All(parts) => parts
                .iter()
                .map(|p| Ok(format!("({})", self.barred_activation(p)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" && "),
            ActivationFilter::Either(parts) => parts
                .iter()
                .map(|p| Ok(format!("({})", self.barred_activation(p)?)))
                .collect::<Result<Vec<_>>>()?
                .join(" || "),
            ActivationFilter::Of(sources) => {
                let test = self.filter(sources, &Candidate::of("object"))?;
                match **sources {
                    ObjectFilter::Chosen(..) => test,
                    _ => format!("q.is_on_battlefield(object) && {test}"),
                }
            }
            other => return Err(gap(format!("prohibited action Activate {}", form(other)))),
        })
    }
    /// Cumulative upkeep (CR 702.24a) paid other than in plain mana or life: one of
    /// Cantrip's own payments, or the payment it will take.
    pub(super) fn cumulative_upkeep(
        &mut self,
        cost: Option<&Cost>,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        const FEATURE: &str = "cumulative upkeep paid other than in mana or life";
        let cost = cost.ok_or_else(|| gap(FEATURE))?;
        let plain = match cost.parts.as_slice() {
            [CostPart::Sacrifice {
                objects: ObjectSet::Battlefield(filter),
                choose: Some(Count::Exactly(Expr::Constant(1))),
            }] => Some(format!("Sacrifice({})", self.cost_filter(filter, label)?)),
            [CostPart::PutCounters(placements)] if on_this(placements) => {
                Some("PutMinusOneMinusOneCounterOnThis".to_string())
            }
            [CostPart::PutCounters(placements)] if on_an_opposing_creature(placements) => {
                Some("PutPlusOnePlusOneCounterOnAnOpposingCreature".to_string())
            }
            [CostPart::PlayerAction { body, .. }] if opponent_gains(body).is_some() => {
                Some(format!(
                    "OpponentGainsLife({})",
                    opponent_gains(body).expect("just seen")
                ))
            }
            [CostPart::SinglePlayersCards(part)] if two_to_the_bottom(part) => {
                Some("TwoCardsFromAGraveyardOnTheBottom".to_string())
            }
            _ => None,
        };
        let whole = |payment: &str| {
            format!("cantrip_mechanics::cumulative_upkeep!(cantrip_mechanics::cumulative_upkeep::Payment::{payment}, \"Cumulative upkeep\")")
        };
        if let Some(payment) = plain {
            parts.abilities.push(whole(&payment));
            return Ok(());
        }
        let payment = match cost.parts.as_slice() {
            [CostPart::Choice(costs)] => {
                let mut either = Vec::new();
                for cost in costs {
                    let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                        return Err(gap(FEATURE));
                    };
                    either.push(mana(symbols)?);
                }
                format!("EitherMana(&[{}])", either.join(", "))
            }
            [CostPart::Draw {
                count: Expr::Constant(n),
            }] => format!("Draw({n})"),
            [CostPart::ObjectAction {
                objects: ObjectSet::Battlefield(filter),
                action: CostObjectAction::GainControl,
            }] => format!("GainControl({})", self.cost_filter(filter, label)?),
            [CostPart::PlayerAction { body, .. }] => {
                format!(
                    "Instructions({})",
                    self.effect(body, &format!("{label}_payment"))?
                )
            }
            _ => format!("Cost({})", self.cost(cost, label)?),
        };
        self.plan_entry(FEATURE, "abilities", &whole(&payment), parts);
        Ok(())
    }
    /// Several objects put onto the battlefield at once, each as `entry` says
    /// (CR 110.2a, 614.12): the call that does it.
    pub(super) fn arriving_together(
        &mut self,
        id: usize,
        entry: &EntrySpec,
        controller: &str,
    ) -> Result<String> {
        const NOTHING: &str = "Vec::<ObjectId>::new()";
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        let counters = counters.join(", ");
        let pairs = format!(
            "let rows_{id}: Vec<(ObjectId, PlayerId)> = objects_{id}.iter().map(|&o| (o, {controller})).collect();"
        );
        match entry.states.as_slice() {
            [EntryState::Changes(changes)] if !entry.tapped && self.stack_item => {
                let (what, colored) = self.enters_as(changes)?;
                let call = |what: &str| {
                    format!("{{ {pairs} g.put_all_onto_battlefield_becoming(&rows_{id}, item, {what}, &[{counters}])? }}")
                };
                if colored {
                    return Ok(planned::planned_value(
                        "several objects entering with a color",
                        NOTHING,
                        &call(&format!("&{what}")),
                    ));
                }
                let name = format!("ENTERS_AS_{}", self.fresh());
                writeln!(self.functions, "static {name}: EntersAs = {what};").unwrap();
                Ok(call(&format!("&{name}")))
            }
            [EntryState::AttachedTo(host)] if !entry.tapped && counters.is_empty() => {
                let host = self.fetch(self.object(host)?);
                Ok(planned::planned_value(
                    "several objects arriving attached",
                    NOTHING,
                    &format!(
                        "{{ {pairs} g.put_all_onto_battlefield_attached(&rows_{id}, {host})? }}"
                    ),
                ))
            }
            [EntryState::AttachedToChosen(hosts)] if !entry.tapped && counters.is_empty() => {
                let hosts = self.filter(hosts, &Candidate::of("host"))?;
                Ok(planned::planned_value(
                    "several objects arriving attached",
                    NOTHING,
                    &format!("{{ {pairs} g.put_all_onto_battlefield_attached_to_chosen(&rows_{id}, |g, host| {hosts})? }}"),
                ))
            }
            states if !states.is_empty() && !entry.tapped && counters.is_empty() => {
                let mut tapped = Vec::new();
                let mut plain = true;
                for state in states {
                    let EntryState::When { filter, entry } = state else {
                        return Err(gap("several objects arriving in a state"));
                    };
                    if !entry.states.is_empty() {
                        return Err(gap("several objects arriving in a state"));
                    }
                    plain &= entry.tapped && entry.counters.is_empty();
                    tapped.push(self.filter(filter, &Candidate::of("o"))?);
                }
                if !plain {
                    return Ok(planned::planned_value(
                        "what enters having counters by what it is",
                        NOTHING,
                        &format!("{{ {pairs} g.put_all_onto_battlefield_with_counters_each(&rows_{id}, by_what_it_is)? }}"),
                    ));
                }
                let tapped = tapped.join(" || ");
                Ok(format!(
                    "{{ let rows_{id}: Vec<(ObjectId, PlayerId, bool)> = {{ let q: &dyn Query = g; objects_{id}.iter().map(|&o| (o, {controller}, {tapped})).collect() }}; g.put_all_onto_battlefield_as(&rows_{id})? }}"
                ))
            }
            _ => Err(gap("several objects arriving in a state")),
        }
    }
    /// An `EntersAs` of what a card says its objects are as they enter, and whether
    /// it has a color, which `EntersAs` has no field for.
    fn enters_as(&mut self, changes: &[Change]) -> Result<(String, bool)> {
        let mut fields = Vec::new();
        let (mut subtypes, mut adds, mut keywords, mut abilities) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut colored = false;
        for change in changes {
            match change {
                Change::SetBasePowerToughness {
                    power: Expr::Constant(p),
                    toughness: Expr::Constant(t),
                } => fields.push(format!("power_toughness: Some(({p}, {t}))")),
                Change::CreatureTypes {
                    mode: ChangeMode::Add,
                    types: CreatureTypeSet::Listed(types),
                } => {
                    for t in types {
                        let word = name(t);
                        if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                            return Err(gap(format!("subtype {word}")));
                        }
                        subtypes.push(format!("Subtype::{word}"));
                    }
                }
                Change::SetArtifactType(t) => {
                    fields.push("loses_subtypes_of: &[CardType::Artifact]".to_string());
                    subtypes.push(format!("Subtype::{}", name(t)));
                }
                Change::AddCardType(t) => adds.push(format!("CardType::{}", card_type(t)?)),
                Change::SetCardTypes(types) => {
                    let types = types
                        .iter()
                        .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                        .collect::<Result<Vec<_>>>()?;
                    fields.push(format!("types: Some(TypeSet::of(&[{}]))", types.join(", ")));
                }
                Change::GrantKeyword(k) => keywords.push(keyword(k)?),
                Change::GrantAbility(ability) => abilities.push(self.grant(ability)?.to_string()),
                Change::Colors {
                    mode: ChangeMode::Add,
                    colors,
                } => {
                    let colors = colors
                        .iter()
                        .map(|c| format!("Color::{c:?}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    colored = true;
                    fields.push(format!("adds_colors: ColorSet::of(&[{colors}])"));
                }
                other => return Err(gap(format!("several objects arriving as {}", form(other)))),
            }
        }
        if !subtypes.is_empty() {
            fields.push(format!("subtypes: &[{}]", subtypes.join(", ")));
        }
        if !adds.is_empty() {
            fields.push(format!("adds_types: TypeSet::of(&[{}])", adds.join(", ")));
        }
        if !keywords.is_empty() {
            fields.push(format!(
                "keywords: Keywords::of(&[{}])",
                keywords.join(", ")
            ));
        }
        if !abilities.is_empty() {
            fields.push(format!("abilities: &[{}]", abilities.join(", ")));
        }
        Ok((
            format!("EntersAs {{ {}, ..EntersAs::NONE }}", fields.join(", ")),
            colored,
        ))
    }
    /// "Costs {1} less", "costs {W}{U} less", "costs {G} less for each green
    /// permanent you control" (CR 118.7): a `CostDelta`.
    pub(super) fn cost_reduction(&self, reductions: &[ManaReduction]) -> Result<String> {
        if let [ManaReduction {
            color: None,
            amount,
        }] = reductions
        {
            return Ok(format!(
                "CostDelta::less({})",
                self.number(amount)?.fit(0, 255, "u8")?
            ));
        }
        let mut lines = Vec::new();
        for ManaReduction { color: of, amount } in reductions {
            let n = self.number(amount)?.fit(0, 255, "u8")?;
            lines.push(match of {
                Some(c) => format!("less.symbols[{}] += {n};", color_index(c)?),
                None => format!("less.generic += {n};"),
            });
        }
        Ok(format!(
            "{{ let mut less = ManaCost::ZERO; {} CostDelta::less_mana(less) }}",
            lines.join(" ")
        ))
    }
    /// "This creature escapes with a +1/+1 counter on it" (CR 702.138c).
    pub(super) fn escapes_with(
        &mut self,
        body: &Program,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let inner = format!("{label}_escaped");
        self.entering(body, &inner)?;
        let function = format!("{label}_enters");
        writeln!(
            self.functions,
            "fn {function}(g: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ let escaped = {{ let q: &dyn Query = g; cantrip_mechanics::escape::escaped(q, entering.object) }}; if escaped {{ {inner}(g, entering)?; }} Ok(()) }}"
        )
        .unwrap();
        parts.enters.push(function);
        Ok(())
    }
    /// A cost whose size the payer sets, once Cantrip has it.
    fn planned_cost(feature: &str, variable: String) -> Result<String> {
        Ok(planned::planned_value(
            feature,
            "Cost::NONE",
            &format!("Cost {{ variable: Some(VariableCost::{variable}), ..Cost::NONE }}"),
        ))
    }
    /// "Sacrifice X creatures", "sacrifice any number of creatures", "sacrifice
    /// all lands you control". Cantrip takes the first two for a spell (CR 601.2b).
    pub(super) fn sacrifice_count(
        &mut self,
        filter: &ObjectFilter,
        choose: Option<&Count>,
        label: &str,
    ) -> Result<String> {
        let spell = self.paid_for == PaidFor::Spell;
        let variable = match choose {
            Some(Count::Exactly(Expr::AnnouncedX)) if spell => "SacrificeX",
            Some(Count::Exactly(Expr::AnnouncedX)) => {
                let filter = self.cost_filter(filter, label)?;
                return Self::planned_cost(
                    "sacrificing X permanents to activate an ability",
                    format!("SacrificeX({filter})"),
                );
            }
            Some(Count::AnyNumber) => {
                let filter = self.cost_filter(filter, label)?;
                let any = format!("SacrificeAnyNumber {{ filter: {filter}, less_each: 0 }}");
                return if spell {
                    Ok(format!(
                        "Cost {{ variable: Some(VariableCost::{any}), ..Cost::NONE }}"
                    ))
                } else {
                    Self::planned_cost(
                        "sacrificing any number of permanents to activate an ability",
                        any,
                    )
                };
            }
            Some(Count::AtLeast(Expr::Constant(1))) if !spell => {
                let filter = self.cost_filter(filter, label)?;
                return Self::planned_cost(
                    "sacrificing one or more permanents to activate an ability",
                    format!("SacrificeOneOrMore({filter})"),
                );
            }
            Some(Count::UpTo(n)) => {
                let n =
                    constant_u8(n).ok_or_else(|| gap("sacrifice cost without a fixed count"))?;
                let filter = self.cost_filter(filter, label)?;
                return Self::planned_cost(
                    "sacrificing up to a number of permanents as a cost",
                    format!("SacrificeUpTo({n}, {filter})"),
                );
            }
            None => {
                let filter = self.cost_filter(filter, label)?;
                return Self::planned_cost(
                    "sacrificing all of a kind of permanent as a cost",
                    format!("SacrificeAll({filter})"),
                );
            }
            _ => return Err(gap("sacrifice cost without a fixed count")),
        };
        let filter = self.cost_filter(filter, label)?;
        Ok(format!(
            "Cost {{ variable: Some(VariableCost::{variable}({filter})), ..Cost::NONE }}"
        ))
    }
    /// "Discard X cards" (CR 118.3).
    pub(super) fn discard_count(
        &mut self,
        count: &Expr,
        filter: &ObjectFilter,
        label: &str,
    ) -> Result<String> {
        if !matches!(count, Expr::AnnouncedX) {
            return Err(gap("discard cost without a fixed count"));
        }
        let filter = self.cost_filter(filter, label)?;
        Self::planned_cost(
            "discarding X cards as a cost",
            format!("DiscardX({filter})"),
        )
    }
    /// "Tap X untapped artifacts you control", "tap any number of untapped creatures".
    pub(super) fn tap_count(
        &mut self,
        filter: &ObjectFilter,
        choose: Option<&Count>,
        label: &str,
    ) -> Result<String> {
        let (feature, variable) = match choose {
            Some(Count::Exactly(Expr::AnnouncedX)) => ("tapping X permanents as a cost", "TapX"),
            Some(Count::AnyNumber) => {
                ("tapping any number of permanents as a cost", "TapAnyNumber")
            }
            _ => return Err(gap("tap cost without a fixed count")),
        };
        let filter = self.cost_filter(filter, label)?;
        Self::planned_cost(feature, format!("{variable}({filter})"))
    }
    /// "Remove X charge counters from this artifact": the X is the number
    /// removed, announced as it is activated (CR 107.3, 118.3). Where mana pays
    /// the same X, or the ability adds mana, Cantrip announces no such X.
    pub(super) fn counters_count(
        &mut self,
        objects: &ObjectSet,
        kind: Option<&crate::counter::CounterKind>,
        count: &Expr,
        selected: bool,
    ) -> Result<String> {
        let (ObjectSet::One(ObjectRef::Source), Some(kind), Expr::AnnouncedX, false) =
            (objects, kind, count, selected)
        else {
            return Err(gap("counter cost without a fixed count"));
        };
        let kind = counter_kind(kind)?;
        let variable = format!("RemoveAnyNumberOfCounters({kind})");
        match (self.paid_for, self.mana_x) {
            (PaidFor::Ability, false) => Ok(format!(
                "Cost {{ variable: Some(VariableCost::{variable}), ..Cost::NONE }}"
            )),
            (PaidFor::Mana, _) => {
                Self::planned_cost("removing X counters to activate a mana ability", variable)
            }
            (PaidFor::Ability, true) => Self::planned_cost(
                "removing X counters where mana pays the same X",
                format!("RemoveXCounters({kind})"),
            ),
            (PaidFor::Spell, _) => Err(gap("counter cost without a fixed count")),
        }
    }
    /// "Put a +1/+1 counter on a creature you control": chosen as it resolves.
    pub(super) fn counters_by_choice(
        &mut self,
        id: usize,
        placement: &CounterPlacement,
        selection: &SelectionChoice,
        out: &mut String,
    ) -> Result<()> {
        let Recipients::Objects(objects) = &placement.recipients else {
            return Err(gap("counters placed by a resolution choice"));
        };
        let count = self.fetch(self.select_count(&selection.count)?);
        let amount = self
            .number(&placement.amount)?
            .amount(i16::MAX.into(), "i16")?;
        let amount = self.fetch(amount);
        let kind = counter_kind(&placement.kind)?;
        let player = self.fetch(self.certain_player(&selection.player)?);
        let candidates = self.fetch(self.objects(objects)?);
        writeln!(
            out,
            "{{ let amount = {amount}; let player = {player}; let candidates = {candidates}; let count = {count}; let chosen = cantrip_mechanics::choose::select(g, player, asker.source, Purpose::PutCounters({kind}, amount.clamp(0, 255) as u8), candidates, cantrip_mechanics::select::Selection::of(count, &[]))?; for object in chosen {{ counters_{id}.push((object, {kind}, amount)); }} }}"
        )
        .unwrap();
        Ok(())
    }
    /// What tokens already made do to take the state they enter in (CR 508.4).
    pub(super) fn token_state(
        &mut self,
        id: usize,
        entry: &EntrySpec,
        out: &mut String,
    ) -> Result<()> {
        match entry.states.as_slice() {
            [] => {}
            [EntryState::Attacking {
                player: None,
                or_planeswalker: false,
            }] => writeln!(
                out,
                "for &token in &created_{id} {{ let controller = g.controller(token); if let Some(defender) = cantrip_mechanics::combat::choose_attack_target(g, controller, asker.source)? {{ g.enter_attacking(&[token], defender); }} }}"
            )
            .unwrap(),
            [EntryState::Attacking {
                player: Some(player),
                or_planeswalker: false,
            }] => writeln!(
                out,
                "let defender_{id} = Target::Player({}); g.enter_attacking(&created_{id}, defender_{id});",
                self.fetch(self.certain_player(player)?)
            )
            .unwrap(),
            [EntryState::Blocking(attacker)] => {
                let attacker = self.fetch(self.object(attacker)?);
                let code = format!(
                    "if let Some(attacker) = {attacker} {{ g.enter_blocking(&created_{id}, attacker); }}"
                );
                self.plan_code("a token entering blocking", &code, out);
            }
            _ => return Err(gap("tokens entering in a special state")),
        }
        Ok(())
    }
    /// Has this set already had its one player picked?
    pub(super) fn is_picked(&self, players: &PlayerSet) -> bool {
        let key = format!("{players:?}");
        self.picked.borrow().named.iter().any(|(k, _)| *k == key)
    }
    /// "An opponent chooses one of them": the player the controller picks as the
    /// instruction begins (CR 608.2d), asked only when there are several, and the
    /// same player for the rest of the instructions around it.
    fn pick(&self, players: &PlayerSet) -> Result<String> {
        let key = format!("{players:?}");
        if let Some((_, local)) = self.picked.borrow().named.iter().find(|(k, _)| *k == key) {
            return Ok(local.clone());
        }
        if !self.stack_item || self.reader != Reader::Resolution {
            return self.picked_elsewhere(players);
        }
        if self.present.borrow().is_none() {
            return Err(gap("player reference OneOf"));
        }
        let among = self.fetch(self.players(players)?);
        let mut picked = self.picked.borrow_mut();
        let local = format!("picked_{}", picked.named.len());
        let statement = format!(
            "let among: Vec<PlayerId> = {among};\nlet Some({local}) = (if among.len() > 1 {{ cantrip_mechanics::choose::player(g, asker.controller, asker.source, among, \"ChooseRevealedCard\", 0)? }} else {{ among.first().copied() }}) else {{ return Ok(()); }};\n"
        );
        picked.named.push((key, local.clone()));
        picked.statements.push(statement);
        Ok(local)
    }
    /// An instruction about a player a reference may not name: it does nothing
    /// without them (CR 609.3). What it binds comes out of that block, empty
    /// without them; a "you may" choice keeps the rest of its block inside.
    pub(super) fn if_present(
        &mut self,
        body: &Program,
        index: usize,
        instruction: &Instruction,
        out: &mut String,
    ) -> Result<()> {
        *self.present.borrow_mut() = Some(Vec::new());
        let mut inner = String::new();
        let named = self.picked.borrow().named.len();
        let opened = self.guard_opened;
        let compiled = self.instruction(body, index, instruction, &mut inner);
        let present = self.present.borrow_mut().take().unwrap_or_default();
        let statements = std::mem::take(&mut self.picked.borrow_mut().statements);
        if compiled.is_err() {
            self.picked.borrow_mut().named.truncate(named);
        }
        compiled?;
        out.push_str(&statements.concat());
        let inside = self.guard_opened > opened;
        let (names, reads) = match instruction {
            Instruction::Effect { result, .. } if !inside && !present.is_empty() => {
                self.carried_out(result)
            }
            _ => (Vec::new(), Vec::new()),
        };
        if !names.is_empty() {
            write!(out, "let ({},) = ", names.join(", ")).unwrap();
        }
        for (local, read) in &present {
            write!(out, "if let Some({local}) = {} {{ ", self.fetch(read)).unwrap();
        }
        out.push_str(&inner);
        if inside {
            self.guard_opened += present.len();
            return Ok(());
        }
        if !names.is_empty() {
            write!(out, "({},)", reads.join(", ")).unwrap();
            out.push_str(&" } else { Default::default() }".repeat(present.len()));
            out.push(';');
        } else {
            out.push_str(&" }".repeat(present.len()));
        }
        out.push('\n');
        Ok(())
    }
    /// What an instruction bound, renamed for its way out of a block: the new
    /// names, and what they take from inside it.
    fn carried_out(&mut self, result: &ResultId) -> (Vec<String>, Vec<String>) {
        let id = self.fresh();
        let mut names = Vec::new();
        let mut reads = Vec::new();
        for map in [
            &mut self.selections,
            &mut self.moved,
            &mut self.creations,
            &mut self.performed,
            &mut self.values,
        ] {
            if let Some(code) = map.get_mut(&result.0) {
                let name = format!("carried_{id}_{}", names.len());
                reads.push(format!("{code}.clone()"));
                *code = name.clone();
                names.push(name);
            }
        }
        if let Some(code) = self.before.get(&result.0).cloned() {
            let name = format!("carried_{id}_{}", names.len());
            reads.push(self.fetch(code));
            self.before.insert(result.0, format!("{name}.clone()"));
            names.push(name);
        }
        (names, reads)
    }
    /// Panharmonicon's and Torpor Orb's rule on triggering (CR 603.2d, 603.2g):
    /// asked of one event and one ability that event triggered.
    fn trigger_rule(
        &mut self,
        cause: Option<&EventSpec>,
        abilities: Option<&ObjectFilter>,
        rule: &TriggerRule,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let table = match rule {
            TriggerRule::TriggersAgain => "additional_triggers",
            TriggerRule::DoesntTrigger => "trigger_rules",
        };
        if lane_k::printed_otherwise(self.line_text(), "abilities of permanents your opponents") {
            return Err(gap(
                "a rule on triggering, where the card says whose abilities",
            ));
        }
        let guard = self.ability_guard.take();
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let abilities = self.triggering_ability(abilities);
        let cause = match cause {
            Some(cause) => self.trigger_cause(cause),
            None => Ok("true".to_string()),
        };
        // "As long as ...": asked as the ability triggers.
        let guard = match &guard {
            Some(guard) => self.condition(guard),
            None => Ok("true".to_string()),
        };
        (self.reader, self.event_at) = saved;
        let (abilities, cause, guard) = (abilities?, cause?, guard?);
        let function = format!("{label}_trigger_rule");
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, event: &Event, triggering: &Triggering) -> bool {{ {guard} && {abilities} && {cause} }}"
        )
        .unwrap();
        parts.table(table, function);
        Ok(())
    }
    /// "A triggered ability of a permanent you control": the permanent as it is,
    /// or as it last existed for an ability that looks back (CR 603.10).
    pub(super) fn triggering_ability(&self, abilities: Option<&ObjectFilter>) -> Result<String> {
        match abilities {
            None => Ok("true".into()),
            Some(ObjectFilter::AbilityOf(objects)) => {
                let source = Candidate {
                    controller: "triggering.controller".into(),
                    ..Candidate::of("triggering.source")
                };
                Ok(format!(
                    "(triggering.permanent && {})",
                    self.filter(objects, &source)?
                ))
            }
            Some(other) => self.triggering_more(other),
        }
    }
    /// The event that does the triggering. Cantrip's helpers hold the rulings
    /// on what counts as a creature dying or attacking.
    fn trigger_cause(&mut self, event: &EventSpec) -> Result<String> {
        Ok(match event {
            EventSpec::Either(events) => {
                let tests = events
                    .iter()
                    .map(|e| self.trigger_cause(e))
                    .collect::<Result<Vec<_>>>()?;
                format!("({})", tests.join(" || "))
            }
            EventSpec::ZoneChange {
                objects,
                from: None,
                to: EventZone::Battlefield,
            } => format!(
                "cantrip_mechanics::statics::entering(event).is_some_and(|id| {})",
                self.filter(objects, &Candidate::of("id"))?
            ),
            EventSpec::ZoneChange {
                objects: ObjectFilter::CardType(CardType::Creature),
                from: Some(EventZone::Battlefield),
                to: EventZone::Graveyard,
            } => "cantrip_mechanics::statics::creature_dying(q, event, triggering)".into(),
            EventSpec::ZoneChange {
                from: Some(EventZone::Battlefield),
                to: EventZone::Graveyard,
                ..
            } => return Err(gap("trigger rule on some creatures dying")),
            EventSpec::Attack {
                attackers: ObjectFilter::CardType(CardType::Creature),
                by: None,
                defenders: AttackDefenders::Any,
            } => "matches!(event, Event::Attacks { .. } | Event::AttackersDeclared { .. })".into(),
            other => {
                let matched = self.event(other)?;
                if let Some(feature) = matched.planned.first() {
                    return Err(gap(feature.clone()));
                }
                matched.fired_test()
            }
        })
    }
}
