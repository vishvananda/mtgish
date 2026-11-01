//! Trigger events beyond the plain ones of `triggers.rs`. An event Cantrip has
//! is an arm like any other. An event it can't fire yet still gets its arm, the
//! matcher it will have, and `Matched::planned` holds that arm out of the file's
//! compiled code: the whole ability goes in a `planned_item!`, with the body it
//! triggers compiled, so the event is all the card waits on.
use super::read::compare;
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
        asked.history = Some(History { outer: None });
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
                locals: "let by = *by; let controller = *controller; let subject = Target::Player(*target); let data = [by.0, u32::from(controller.0)];".into(),
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
        let frame = Frame {
            object: (!grouped).then_some(Carried::Subject),
            other_player: Some(Carried::Data(0)),
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
                locals: "let id = *attacker; let subject = Target::Object(id); let data = [q.defending_player(id).map_or(u32::MAX, |p| u32::from(p.0)), u32::MAX];".into(),
                tests,
            },
            frame,
            "Whenever this attacks and isn't blocked",
        );
        if grouped {
            matched.batch = Some(false);
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
