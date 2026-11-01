//! Trigger events beyond the plain ones of `triggers.rs`. An event Cantrip has
//! is an arm like any other. An event it can't fire yet still gets its arm, the
//! matcher it will have, and `Matched::planned` holds that arm out of the file's
//! compiled code: the whole ability goes in a `planned_item!`, with the body it
//! triggers compiled, so the event is all the card waits on.
use super::casting::target_range;
use super::read::compare;
use crate::mtg_types::CardType;
use super::triggers::{object_fired, single, Arm, Matched};
use super::*;

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
    fn new(mask: &'static str, pattern: &'static str, id: &'static str, phrase: &'static str) -> Self {
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
            matched.planned = Some(format!("trigger event Object {}", form(event)));
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
            return Err(gap("trigger event Player FullyUnlocksRoom in a history question"));
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
            matched.planned = Some(format!("trigger event Player {}", form(event)));
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
            matched.planned = Some(format!("trigger event Pair {}", form(event)));
        }
        Ok(matched)
    }

    /// The rest of `EventSpec`.
    pub(super) fn event_rest(&mut self, event: &EventSpec) -> Result<Matched> {
        match event {
            EventSpec::ZoneChangeFromElsewhere { objects, except, to } => {
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
                    Ok((c.filter(objects, &Candidate::of("id"))?, c.number(nth)?.code))
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
                let every = self.matching(frame, |c| {
                    c.filter(filter, &Candidate::of("id"))
                })?;
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
            EventSpec::SpellOrActivation {
                players,
                spells,
                abilities,
            } => self.event(&EventSpec::Either(vec![
                EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: players.clone(),
                    spells: spells.clone(),
                },
                EventSpec::Activated {
                    players: players.clone(),
                    abilities: abilities.clone(),
                },
            ])),
            EventSpec::TappedForManaOf { objects, by, mana } => {
                self.tapped_for_mana_of(objects, by.as_ref(), mana)
            }
            EventSpec::ManaAdded { abilities, players, .. } => {
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
            EventSpec::PaysToActivate { players, abilities, payment } => {
                let frame = Frame {
                    player: Some(Carried::Subject),
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
                    "let id = *source; let def = q.activated_ability(*ability); let paid = q.paid_to_activate(event); let subject = Target::Player(*player); let data = [id.0, paid.life];",
                    tests,
                    frame,
                    "Whenever a player activates an ability",
                    "trigger event PaysToActivate",
                ))
            }
            EventSpec::Caused { cause, event } => self.caused(cause, event),
            EventSpec::CausesTrigger { cause, .. } => {
                let frame = plain();
                let mut matched = self.event(cause)?;
                let tests = std::mem::take(&mut matched.arms);
                let cause_test = tests
                    .iter()
                    .map(|arm| format!("(if let {} = cause {{ {} {} }} else {{ false }})", arm.pattern, arm.locals, arm.tests.join(" && ")))
                    .collect::<Vec<_>>()
                    .join(" || ");
                let mut held = single(
                    "TRIGGERED",
                    Arm {
                        pattern: "Event::Triggered { source, ability, cause }".into(),
                        locals: "let id = *source; let subject = Target::Object(id); let data = [u32::MAX; 2]; let cause = &*cause;".into(),
                        tests: vec![cause_test],
                    },
                    frame,
                    "Whenever an ability triggers",
                );
                held.planned = Some("trigger event CausesTrigger".into());
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
                    Ok(vec![c.player_in(by, "*by")?, c.player_in(players, "*player")?])
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
                let frame = plain();
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
                let frame = plain();
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
        if matched.sums {
            return Err(gap("an ordinal of grouped events"));
        }
        let mut asked = self.clone();
        asked.history = Some(History { outer: None, event_at: EventAt::Matcher, frame: Frame::default() });
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

    /// `PlayerEvent::BecomesTarget`. A filter of `Any` has lost what targeted the
    /// player ("of a spell", against "of a spell or ability"), so it isn't read.
    fn player_targeted(
        &mut self,
        players: &PlayerSet,
        objects: Option<&ObjectFilter>,
    ) -> Result<Matched> {
        let by = match objects {
            Some(ObjectFilter::Any) | None => {
                return Err(gap("trigger event Player BecomesTarget of what is not said"))
            }
            Some(filter) => filter,
        };
        let frame = Frame {
            player: Some(Carried::Subject),
            stack_controller: Some(Carried::Data(1)),
            ..Frame::default()
        };
        let tests = self.matching(frame, |c| {
            Ok(vec![
                c.player_in(players, "*target")?,
                match by {
                    ObjectFilter::ControlledBy(controllers) => c.player_in(controllers, "controller")?,
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
        let spells = match objects {
            None => &any,
            Some(ObjectFilter::InSet(set))
                if matches!(&**set, ObjectSet::Zone { filter: ObjectFilter::Any, .. }) =>
            {
                &any
            }
            Some(filter) if from.is_none() => filter,
            Some(_) => {
                return Err(gap(format!("trigger event Player {} of a set of cards", form(event))))
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
            matched.arms[0]
                .tests
                .push(format!("cast_of(q, id).is_some_and(|cast| cast.from == {zone})"));
            matched.arms[1].pattern =
                "Event::ZoneChange { new, from, to, reason: ZoneReason::PlayLand, .. }".into();
            matched.arms[1].tests.push(format!("from.is({zone})"));
        }
        Ok(matched)
    }

    /// "Whenever you expend N" (CR 701.65a): the spending that reaches N mana
    /// spent to cast spells this turn.
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
        let before = "q.history().this_turn().iter().filter_map(|r| match r.event { Event::ManaSpent { player: p, cast: true, mana, .. } if p == *player => Some(i64::from(mana.total())), _ => None }).sum::<i64>()";
        Ok(held_arm(
            "MANA_SPENT",
            "Event::ManaSpent { player, cast: true, mana, .. }",
            ONE,
            vec![
                who,
                format!("{before} < {value}"),
                format!("{before} + i64::from(mana.total()) >= {value}"),
            ],
            frame,
            "Whenever a player expends",
            "trigger event Player Expends",
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
        let frame = Frame {
            other_player: Some(Carried::Data(0)),
            ..plain()
        };
        let (comparison, value) = count;
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
                locals: object_fired("*attacker", "u32::from(defending.0)"),
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
        for arm in &mut matched.arms {
            arm.tests.push(test.clone());
        }
        if self.history.is_none() {
            matched.planned = matched
                .planned
                .or_else(|| Some("trigger event Caused".into()));
        }
        Ok(matched)
    }

    /// What `triggered` made of an ability, once the card can hold it: in the
    /// card, or held whole until Cantrip fires its event.
    pub(super) fn trigger_entry(
        &mut self,
        def: Result<String>,
        parts: &mut Parts,
    ) -> Result<Option<String>> {
        let planned = self.planned_trigger.take();
        let def = match (def, &planned) {
            (Ok(def), _) => def,
            // The event is still a requirement, whatever else stopped the body.
            (Err(failure), Some((feature, _))) => {
                self.gaps.push(gap(feature.clone()));
                return Err(failure);
            }
            (Err(failure), None) => return Err(failure),
        };
        let Some((feature, matcher)) = planned else {
            return Ok(Some(def));
        };
        if self.granting {
            return Err(gap(feature));
        }
        self.plan_entry(&feature, "abilities", &format!("{{ {matcher} {def} }}"), parts);
        Ok(None)
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
    matched.planned = Some(feature.into());
    matched
}

/// A count, as `number` makes one.
fn counted(code: String) -> Number {
    Number {
        code,
        min: 0,
        max: u32::MAX.into(),
        dynamic: true,
    }
}
/// The number Cantrip has no way to read yet.
fn held_number(feature: String, code: &str) -> Number {
    counted(planned::planned_value(&feature, "0i64", code))
}

impl Compiler {
    /// What the object Cantrip has chosen for this ability or as it entered is
    /// linked by.
    fn linked(&self) -> &'static str {
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
            other => return Err(gap(format!("object reference {}", form(other)))),
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
            PlayerRef::PlayerOrControllerOf(recipients) => match &**recipients {
                Recipients::Target { slot, index } => format!(
                    "t_{}_raw.get({index}).copied().and_then(|t: Target| t.player().or(t.object().map(|o| q.controller(o))))",
                    self.slot(slot)?
                ),
                other => {
                    return Err(gap(format!(
                        "player reference PlayerOrControllerOf {}",
                        form(other)
                    )))
                }
            },
            other => return Err(gap(format!("player reference {}", form(other)))),
        })
    }
    /// What a preceding instruction named, by the list it keeps.
    fn named(&self, result: &ResultId) -> Result<String> {
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
            other => return Err(gap(format!("object collection {}", form(other)))),
        })
    }
    /// One player's number, for `p`, a `PlayerId`.
    fn player_number(&self, p: &str, number: &PlayerNumber) -> Result<String> {
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
                    Aggregation::Maximum => "max",
                    Aggregation::Minimum => "min",
                    other => {
                        return Err(gap(format!(
                            "numeric expression AmongPlayers {}",
                            form(other)
                        )))
                    }
                };
                counted(format!(
                    "({}).into_iter().map(|p: PlayerId| {}).{pick}().unwrap_or(0)",
                    self.players(players)?,
                    self.player_number("p", number)?
                ))
            }
            Expr::TimesKicked(object) => counted(format!(
                "{}.map_or(0i64, |o: ObjectId| i64::from(q.state().obj(o).kicked))",
                self.object(object)?
            )),
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
            Expr::ExcessDamage { .. } => held_number(feature, "i64::from(result.excess)"),
            Expr::ChosenNumber => match self.chosen_local(ValueKind::Number) {
                Ok(local) => counted(format!("i64::from({local})")),
                Err(_) => counted(format!(
                    "q.state().chosen_number({}).map_or(0i64, i64::from)",
                    self.linked()
                )),
            },
            other => return Err(gap(format!("numeric expression {}", form(other)))),
        })
    }
    /// The records of a window.
    fn object_window(&self, window: &ObjectWindow) -> Result<String> {
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
            other => return Err(gap(format!("history window {}", form(other)))),
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
        let moved = |pattern: &str, test: &str| {
            format!("Event::ZoneChange {{ {pattern}, .. }} if {test}")
        };
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
            H::Convoked(object) => format!(
                "Event::Convoked {{ creature, spell }} if creature == {id} && {} == Some(spell)",
                self.object(object)?
            ),
            H::DamagedBy(object) => format!(
                "Event::Damage {{ source, target: Target::Object(hit), .. }} if hit == {id} && {} == Some(source)",
                self.object(object)?
            ),
            other => return Err(gap(format!("object filter History {}", form(other)))),
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
        match family {
            KeywordFamily::Hexproof => any(&["Hexproof"]),
            KeywordFamily::Decayed => any(&["Decayed"]),
            KeywordFamily::Morph => any(&["Morph"]),
            KeywordFamily::Disguise => any(&["Disguise"]),
            KeywordFamily::Phasing => any(&["Phasing"]),
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
                &format!("q.has_keyword_ability({id}, KeywordAbility::{})", form(other)),
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
                None => format!("{on_stack}.is_some_and(|i| q.state().targets(i).len() == 1)"),
                Some("") => return Err(gap("targets of a spell as it is cast")),
                Some(list) => format!("{list}.len() == 1"),
            },
            // What's on the stack is known by the object it came from, which an
            // ability shares with others of the same source.
            ObjectFilter::OnStack(other) => planned::planned_value(
                "object filter OnStack",
                "false",
                &format!("q.stack_item({id}).is_some_and(|i| i.is(StackTrait::{}))", form(other)),
            ),
            ObjectFilter::History { event, window } => self.object_history(event, window, id)?,
            ObjectFilter::DamagedBy(_) => held(format!(
                "cantrip_mechanics::history::permanents_dealt_damage_by(q.history(), asker.source, since).contains(&{id})"
            )),
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
                let earlier = self.filter(spells, &Candidate::of("cast.spell"))?;
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
                format!("({}).contains(&{id})", self.named(result)?)
            }
            ObjectFilter::KickedWith(_) => held(format!("q.state().obj({id}).kicked_with(kicker)")),
            ObjectFilter::AlternateCostPaid(_) => {
                held(format!("q.state().obj({id}).alternate_cost_paid(cost)"))
            }
            ObjectFilter::MadeBy(object) => held(format!(
                "q.state().obj({id}).made_by == {}",
                self.object(object)?
            )),
            ObjectFilter::HasActivatedAbility(_) => {
                held(format!("q.card_of({id}).activated.iter().any(|d| matches(d))"))
            }
            other => return Err(gap(format!("object filter {}", form(other)))),
        })
    }
    /// The rest of `condition`.
    pub(super) fn condition_rest(&self, condition: &Condition) -> Result<String> {
        let held = |code: &str| {
            planned::planned_value(&format!("condition {}", form(condition)), "false", code)
        };
        Ok(match condition {
            Condition::Searched(_) => held("g.searched_library()"),
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
            Condition::SameTeam { .. } => held("q.state().same_team(a, b)"),
            Condition::MostVotes { .. } => held("g.votes().most(option)"),
            Condition::VotedFor { .. } | Condition::Voted(_) | Condition::AllVotedFor(_) => {
                held("g.votes().voted(option)")
            }
            other => return Err(gap(format!("condition {}", form(other)))),
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
fn mana_letter(color: &ManaColor) -> String {
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
        } if gifted(actor) => Some(("EXTRA_TURN", None, "the chosen opponent takes an extra turn")),
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
                ("Food", false) => Some(("FOOD", Some("FOOD"), "the chosen opponent creates a Food token")),
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

/// What an event is always about: "whenever this creature attacks", "whenever
/// equipped creature attacks".
pub(super) fn event_object(event: &EventSpec) -> Option<ObjectRef> {
    let (EventSpec::ZoneChange {
        objects,
        to: EventZone::Battlefield,
        ..
    }
    | EventSpec::Object { objects, .. }
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
            Ability::Spree { modes } => self.spree(modes, label, parts),
            Ability::Gift {
                gift,
                promised,
                unpromised,
            } => self.gift(gift, promised.as_deref(), unpromised.as_deref(), label, parts),
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
            other => Err(gap(format!("ability {}", form(other)))),
        }
    }
    /// Spree (CR 702.172): modes chosen by paying for them, at least one.
    fn spree(&mut self, modes: &[SpreeMode], label: &str, parts: &mut Parts) -> Result<()> {
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
                quoted(&ability_text(body)),
                mana(symbols)?
            ));
        }
        let name = format!("{}_MODES", label.to_ascii_uppercase());
        writeln!(
            self.functions,
            "static {name}: ModalDef = cantrip_mechanics::spree::modal(&[{}]);",
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
            self.extras.push(format!("cantrip_mechanics::tokens::{token}"));
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
        let (specs, counted) = self.gift_targets(promised_announced, unpromised_announced, label)?;
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
            self.gated(&held, &level.abilities, &format!("{label}_level_{number}"), parts)?;
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
    /// An instruction about a player a reference may not name: it does nothing
    /// without them (CR 609.3). What it binds stays in that block.
    pub(super) fn if_present(
        &mut self,
        body: &Program,
        index: usize,
        instruction: &Instruction,
        out: &mut String,
    ) -> Result<()> {
        *self.present.borrow_mut() = Some(Vec::new());
        let mut inner = String::new();
        let compiled = self.instruction(body, index, instruction, &mut inner);
        let present = self.present.borrow_mut().take().unwrap_or_default();
        compiled?;
        if let Instruction::Effect { result, .. } = instruction {
            let bound = [
                self.selections.remove(&result.0).is_some(),
                self.moved.remove(&result.0).is_some(),
                self.creations.remove(&result.0).is_some(),
                self.before.remove(&result.0).is_some(),
                self.performed.remove(&result.0).is_some(),
                self.values.remove(&result.0).is_some(),
            ];
            if bound.contains(&true) {
                self.failed.insert(result.0);
            }
        }
        for (local, read) in &present {
            write!(out, "if let Some({local}) = {} {{ ", self.fetch(read)).unwrap();
        }
        out.push_str(&inner);
        out.push_str(&" }".repeat(present.len()));
        out.push('\n');
        Ok(())
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
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let abilities = self.triggering_ability(abilities);
        let cause = match cause {
            Some(cause) => self.trigger_cause(cause),
            None => Ok("true".to_string()),
        };
        (self.reader, self.event_at) = saved;
        let (abilities, cause) = (abilities?, cause?);
        let function = format!("{label}_trigger_rule");
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, event: &Event, triggering: &Triggering) -> bool {{ {abilities} && {cause} }}"
        )
        .unwrap();
        parts.table(table, function);
        Ok(())
    }
    /// "A triggered ability of a permanent you control": the permanent as it is,
    /// or as it last existed for an ability that looks back (CR 603.10).
    fn triggering_ability(&self, abilities: Option<&ObjectFilter>) -> Result<String> {
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
            Some(other) => Err(gap(format!("trigger rule over {}", form(other)))),
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
                if let Some(feature) = &matched.planned {
                    return Err(gap(feature.clone()));
                }
                matched.fired_test()
            }
        })
    }
}
