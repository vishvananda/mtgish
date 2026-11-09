//! Program in the normalized semantic model.
use super::*;

/// A program is a sequence of semantic instructions. The operations making
/// up one continuous-effect instruction stay in one payload.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Instruction {
    Effect {
        result: ResultId,
        operation: Operation,
    },
    Group(Program),
    If {
        condition: Condition,
        then: Program,
        otherwise: Program,
    },
    Optional {
        decider: PlayerRef,
        body: Program,
    },
    /// Run `body`, then run it again each time it reached `RepeatProcess`.
    Repeatable(Program),
    /// Each player, as `binding`, does `body`: choices in APNAP order, and the
    /// actions then happen together (101.4). A choice made in `body` can be read
    /// after the loop as a set of every player's picks.
    ForEachPlayer {
        players: PlayerSet,
        binding: Binding,
        /// Turn order begins with this player instead of the active player.
        starting_with: Option<PlayerRef>,
        /// Proceeds in the direction chosen earlier, not in turn order.
        in_chosen_direction: bool,
        body: Program,
    },
    /// The decider picks one option as this resolves; no target is announced.
    /// With `offered`, the game first picks that many options at random and the
    /// decider picks among those, as Forge's `GenericChoice | NumRandomChoices$`.
    ChooseOne {
        decider: PlayerRef,
        options: Vec<Program>,
        offered: Option<Expr>,
    },
    /// Run `body`, and run it again each time it reached `RepeatProcess`, for
    /// at most this many runs in all: "repeat this process X more times".
    Times {
        amount: Expr,
        body: Program,
    },
    /// Run `body` once for each of these objects, read as `ObjectRef::Bound`.
    ForEachObject {
        objects: ObjectSet,
        binding: Binding,
        body: Program,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Operation {
    /// Create a once-only delayed trigger (603.7), eligible only after this
    /// instruction has executed. The turn players and start gate are resolved
    /// at creation. A stated expiration cancels a still-waiting trigger; it
    /// does not cancel an ability already on the stack.
    ///
    /// `body` is a lexical closure over the creator's reference frame: targets,
    /// results, payment histories, announced X, event data, bound players, and exact object
    /// incarnations survive independently of the creator's resolution. Source
    /// and ability controller are retained under 603.7d-g. Queries/properties
    /// and arithmetic in the body are evaluated when it resolves, using LKI
    /// where required; capturing references does not snapshot all expressions.
    /// An object that leaves its expected zone and returns is a new object.
    ///
    /// A matcher's own bindings do NOT replace the captured creator's event
    /// bindings: this form has no new targets/modes and exposes no bindings
    /// from the future event.
    /// Body-local results never escape into the creating program.
    CreateDelayedTrigger {
        event: EventSpec,
        start: DelayedStart,
        expires: Option<Duration>,
        intervening_if: Option<Condition>,
        body: Program,
    },
    /// Privately inspect the queried current objects, retaining their original
    /// identities and order as an observation result. Does not move cards,
    /// reveal them publicly, search, scry, or inspect later top cards anew.
    LookAt {
        player: PlayerRef,
        objects: ObjectSet,
    },
    /// At resolution, create one next-destruction replacement shield per
    /// selected permanent, expiring at end of turn (701.19a). This instruction
    /// does not itself tap, clear damage, remove combat participation, or
    /// regenerate anything. Those happen only when a shield replaces an actual
    /// destruction, unless regeneration is prohibited. Sacrifice/toughness-zero
    /// deaths are not destruction. Each selection is a snapshot of its current
    /// incarnation, and repeated instructions create independent shields.
    CreateRegenerationShields {
        objects: ObjectSet,
    },
    /// One search episode (CR 701.23h), even with several find clauses. Choices
    /// have disjoint original-library identities. A denied/replaced search can
    /// return searched=false; it does not cancel subsequent unconditional work.
    Search {
        player: PlayerRef,
        library: PlayerRef,
        top: Option<Expr>,
        choices: Vec<SearchChoice>,
        /// Zones searched besides the library, all of them that player's.
        also: Vec<EventZone>,
        /// "and/or": the searcher chooses which zones to look through.
        either: bool,
        /// Also among the cards the searcher owns outside the game (CR 400.11).
        outside: bool,
    },
    /// Explicit public revelation; searching never implies revelation.
    Reveal {
        objects: ObjectSet,
    },
    /// Reveal from the top of `library` through the first card matching `filter`.
    /// The result is every revealed card in order, or the whole library if none
    /// matched; the match is the only card in it passing `filter`.
    RevealUntil {
        library: PlayerRef,
        filter: ObjectFilter,
        /// How many matching cards end the reveal.
        count: Expr,
        /// The most cards revealed, whichever comes first.
        at_most: Option<Expr>,
    },
    /// Reordering is within each library, without zone changes. Exclusions hold
    /// selected cards out of the shuffle while they remain in that library.
    Shuffle {
        players: PlayerSet,
        excluding: Option<ObjectSet>,
    },
    /// Reposition only cards still in this library, without a zone change.
    /// Reordered revealed/looked-at cards stop being observed and become new
    /// objects (701.20d-e); old observation/selection identities do not follow.
    /// Remember these objects as they are now, for a later "those".
    NoteObjects {
        objects: ObjectSet,
    },
    ReorderLibrary {
        player: PlayerRef,
        library: PlayerRef,
        objects: ObjectSet,
        end: SequenceEnd,
        order: CardOrder,
    },
    /// CR 701.22: privately look at N top cards, partition/reorder top and bottom.
    /// Scry 0 emits no scry event. Multiple players decide in APNAP order and
    /// reposition simultaneously; completion triggers follow the whole process.
    Scry {
        players: PlayerSet,
        amount: Expr,
    },
    /// CR 701.25: privately look at N top cards, move any number to the graveyard,
    /// and reorder the rest on top. Surveil 0 emits no surveil event. Replacements
    /// apply to actual movements; this is not drawing or revealing those cards.
    Surveil {
        players: PlayerSet,
        amount: Expr,
    },
    /// Change one spell on the stack, or the next spell a player casts.
    ChangeSpell {
        subject: SpellSubject,
        changes: Vec<Change>,
    },
    /// Cast or play cards while resolving, as Forge's `Play`.
    PlayCards {
        actor: PlayerSet,
        action: CardPlayAction,
        cards: ObjectSet,
        count: Count,
        cost: PlayCost,
        into_exile: bool,
    },
    /// Copy cards where they are; `ObjectSet::Copies` reads the copies.
    CopyCards {
        actor: PlayerSet,
        objects: ObjectSet,
        times: Expr,
    },
    /// Countering is a rules action; the destination applies only if it succeeds.
    Counter {
        objects: ObjectSet,
        destination: Destination,
    },
    /// Create a triggered ability that lasts for `duration`. Unlike a delayed
    /// trigger it can fire repeatedly, and it announces its own targets.
    CreateTrigger {
        event: EventSpec,
        duration: Duration,
        announcement: Announcement,
        body: Program,
        /// Ends once it has triggered, as for "your next spell this turn".
        once: bool,
        intervening_if: Option<Condition>,
    },
    /// Repeat the nearest enclosing `Instruction::Repeatable` or `Times`.
    RepeatProcess,
    SetLifeTotal {
        players: PlayerSet,
        amount: Expr,
    },
    /// Discard these cards: chosen by whoever the effect says, not by their owner.
    DiscardObjects {
        objects: ObjectSet,
    },
    /// Name a number for later instructions; see `Expr::BoundValue`.
    BindValue {
        value: Expr,
    },
    /// A reflexive trigger (603.12): once, if `performed` happened during this
    /// resolution, it goes on the stack and announces its own targets. `condition`
    /// is checked as it triggers. `body` closes over the creator like a delayed
    /// trigger. No `performed` means reaching this instruction is the event, as
    /// Forge's `ImmediateTrigger`: an `If` around it says when.
    ReflexiveTrigger {
        performed: Option<ResultId>,
        condition: Option<Condition>,
        announcement: Announcement,
        body: Program,
    },
    /// Return these moved objects to where they were when `duration` ends, in one
    /// effect with the move (610.3). If `duration` has already ended, the move
    /// does not happen.
    ReturnAfter {
        objects: ObjectSet,
        duration: Duration,
    },
    /// Create a replacement effect on future events (CR 614.1, 615). Unlike a
    /// delayed trigger, `body` changes the pending event as it would happen.
    CreateReplacement {
        event: ReplacementEvent,
        limit: ReplacementLimit,
        duration: Duration,
        body: Program,
    },
    /// Apply a change to the currently proposed event. Legal only inside a
    /// replacement program for an event supporting this change.
    ModifyEvent {
        change: EventChange,
    },
    /// A resolution-time cost, with a typed decision/result. This differs from
    /// announcement costs on an activated ability.
    PayCost {
        player: PlayerRef,
        cost: Cost,
        optional: bool,
    },
    RemoveCounters {
        removals: Vec<CounterRemoval>,
    },
    Attach {
        attachments: ObjectSet,
        host: ObjectRef,
    },
    /// Every definition in this instruction is created in one batch. Counts and
    /// initial copiable values are evaluated before any token enters. Ability
    /// definitions, including characteristic-defining recipes, remain live.
    CreateTokens {
        players: PlayerSet,
        tokens: Vec<TokenSpec>,
        entry: EntrySpec,
    },
    /// Resolution-time choice. Its result is the selected current objects;
    /// it does not announce targets or mutate the selected objects.
    ChooseObjects {
        player: PlayerRef,
        objects: ObjectSet,
        count: Count,
        constraints: Vec<SelectionConstraint>,
    },
    Draw {
        players: PlayerSet,
        amount: Expr,
    },
    /// Players choose their cards before the simultaneous discard batch.
    Discard {
        players: PlayerSet,
        amount: Expr,
        random: bool,
    },
    GainLife {
        players: PlayerSet,
        amount: Expr,
    },
    LoseLife {
        players: PlayerSet,
        amount: Expr,
    },
    Mill {
        players: PlayerSet,
        amount: Expr,
    },
    AddMana {
        players: PlayerSet,
        mana: ManaProduction,
        restriction: Option<ManaUse>,
    },
    DealDamage {
        source: ObjectRef,
        recipients: Recipients,
        amount: Expr,
        preventable: bool,
    },
    /// Different amounts from one source, all dealt at the same time.
    DealDamageParts {
        source: ObjectRef,
        parts: Vec<DamagePart>,
        preventable: bool,
    },
    /// Each object deals damage at the same time; `ObjectRef::Affected` reads
    /// that object in `recipients` and `amount`.
    DealDamageFromEach {
        sources: ObjectSet,
        recipients: Recipients,
        amount: Expr,
    },
    Destroy {
        objects: ObjectSet,
        regeneration: bool,
    },
    Move {
        objects: ObjectSet,
        destination: Destination,
        entry: EntrySpec,
        face_down: bool,
        #[serde(default)]
        terms: MovementTerms,
    },
    Tap {
        objects: ObjectSet,
        tapped: bool,
    },
    /// Each player chooses their own eligible permanents in APNAP order;
    /// selections are collected before one simultaneous sacrifice batch.
    Sacrifice {
        players: PlayerSet,
        objects: ObjectSet,
        choose: Option<Count>,
    },
    /// All choices precede one simultaneous placement event batch.
    PutCounters {
        placements: Vec<CounterPlacement>,
    },
    CreateContinuousEffect {
        source: ObjectRef,
        affected: EffectSubjects,
        changes: Vec<Change>,
        duration: Duration,
        evaluation: EffectEvaluation,
    },
    /// One rules-defined action with no decomposition, from a reviewed row in
    /// `rewrites/primitives.json5`. Unlike `Named`, its operands are bound in the
    /// enclosing scope and none is left unreduced. Counts as complete, and reports
    /// list the records that depend on one.
    Primitive {
        actor: PlayerSet,
        name: String,
        operands: Vec<Operand>,
    },
    /// An action with no reduced form, carried out by `actor`. Never present on
    /// a complete card. It binds no result, so later references cannot reach it.
    Named {
        actor: PlayerSet,
        form: Named,
    },
    /// `separator` splits `objects` into two piles and `chooser` picks one,
    /// which is the result; the other pile is what `objects` still holds.
    SeparatePiles {
        separator: PlayerRef,
        chooser: PlayerRef,
        objects: ObjectSet,
    },
    /// Damage that reaches its recipients the way the rider says.
    DealDamageWith {
        source: ObjectRef,
        recipients: Recipients,
        amount: Expr,
        rider: DamageRider,
    },
    /// One flip of one coin (705). Branches read it through `Condition::CoinFlip`.
    FlipCoin {
        player: PlayerRef,
    },
    /// One result standing for whichever of these branch results was made.
    JoinResults {
        results: Vec<ResultId>,
    },
    /// Changes that stay with the object through zones, for the rest of the
    /// game. Alchemy's "perpetually", as Forge's `Effect` with `Duration$ Permanent`.
    CreatePerpetualEffect {
        objects: ObjectSet,
        changes: Vec<Change>,
    },
    /// An emblem in the command zone with these abilities (114).
    CreateEmblem {
        actor: PlayerSet,
        abilities: Vec<Ability>,
    },
    /// As Forge's `MoveCounter` with `Source$`, `Defined$` and `CounterNum$`. The
    /// actor divides the counters among `to` when it is more than one object.
    MoveCounters {
        actor: PlayerSet,
        from: ObjectSet,
        to: ObjectSet,
        kind: Option<crate::counter::CounterKind>,
        amount: MovedCounters,
    },
    /// Each object gets as many of these counters again; no `kind` means each kind.
    DoubleCounters {
        objects: ObjectSet,
        kind: Option<crate::counter::CounterKind>,
    },
    /// Creates a named card from outside the game in this zone (Alchemy).
    Conjure {
        actor: PlayerSet,
        card: ConjuredCard,
        count: Expr,
        zone: EventZone,
        /// Whose zone, when it isn't `actor`'s.
        owner: Option<PlayerRef>,
        /// Where in a library. None is a random position.
        spot: Option<ConjureSpot>,
        entry: EntrySpec,
    },
    /// Choose values under explicit cardinality and terms. Reads go through
    /// `ObjectFilter::Chosen`, `Change::ChosenType` or `Expr::ChosenNumber`.
    ChooseValue {
        actor: PlayerSet,
        kind: ValueKind,
        terms: ChoiceTerms,
    },
    /// Choose one of these power/toughness adjustments.
    ChoosePowerToughness {
        player: PlayerRef,
        options: Vec<(i64, i64)>,
    },
    /// Reveal from the top of `library` until the revealed cards satisfy these
    /// group constraints. The result is every revealed card in order.
    RevealUntilGroup {
        library: PlayerRef,
        constraints: Vec<SelectionConstraint>,
    },
    /// A rules-defined action with a meaning of its own, with what it acts on.
    /// Yields what the row it came from yields.
    Keyword {
        actor: PlayerSet,
        action: KeywordAction,
        object: Option<ObjectRef>,
        other: Option<ObjectRef>,
        amount: Option<Expr>,
        subtype: Option<String>,
    },
    /// The top card of `actor`'s library moves if it is one of `cards`, as
    /// "reveal the top card; if it's a land, put it into your hand".
    MoveTopCard {
        actor: PlayerSet,
        cards: ObjectFilter,
        reveal: bool,
        destination: Destination,
        entry: EntrySpec,
    },
    /// Pay as much of this as `actor` chooses, for later "that much".
    PayAnyAmount {
        actor: PlayerSet,
        resource: PaidResource,
    },
    /// `actor` reveals from hand. No `count` is every matching card.
    RevealFromHand {
        actor: PlayerSet,
        cards: ObjectFilter,
        count: Option<Expr>,
        at_random: bool,
    },
    /// One keyword action on each of a set. No `until` is the action's own
    /// duration: for goad, until `actor`'s next turn (CR 701.38a).
    KeywordEach {
        actor: PlayerSet,
        action: KeywordAction,
        objects: ObjectSet,
        until: Option<Duration>,
    },
    /// Copy a spell or ability on the stack `times` times, as Forge's
    /// `CopySpellAbility`. `effects` change the copies.
    CopyOnStack {
        actor: PlayerSet,
        object: ObjectRef,
        times: Expr,
        targets: NewTargets,
        effects: Vec<Operand>,
        /// The card that goes onto the stack as the copy, when one does.
        card: Option<ObjectRef>,
    },
    /// Change what a spell or ability targets, as Forge's `ChangeTargets`.
    Retarget {
        actor: PlayerSet,
        object: ObjectRef,
        targets: NewTargets,
    },
    /// Each clashing player reveals; `won` runs if `actor` won, else `lost`
    /// (CR 701.30). Inside these, `PlayerRef::ClashOpponent` is the opponent.
    Clash {
        actor: PlayerSet,
        opponents: PlayerSet,
        won: Vec<Program>,
        lost: Vec<Program>,
    },
    /// Draft a card from a named card's spellbook, `times` times, into hand.
    DraftFromSpellbook {
        actor: PlayerSet,
        spellbook: String,
        times: Expr,
    },
    /// CR 701.38: each voter votes for one option. Votes in the same
    /// resolution are counted together by `Expr::Votes` and `Condition::MostVotes`.
    Vote {
        actor: PlayerSet,
        ballot: Ballot,
        secret: bool,
    },
    /// CR 701.40, 701.58: a card becomes a face-down 2/2, with ward {2} if cloaked.
    Manifest {
        actor: PlayerSet,
        source: ManifestSource,
        cloak: bool,
        entry: EntrySpec,
    },
    AbandonScheme {
        actor: PlayerSet,
    },
    /// Puts a sticker on each object, chosen from the sheet unless named.
    PutSticker {
        actor: PlayerSet,
        objects: ObjectSet,
        name_only: bool,
    },
    /// The player may begin the game with this card on the battlefield.
    BeginOnBattlefield {
        actor: PlayerSet,
        flags: Box<Operand>,
    },
    /// Effects on objects or on spells that match, for `until` when it is
    /// not the whole game. `effects` name what each gets.
    EffectOn {
        subject: EffectSubject,
        effects: Vec<Operand>,
        until: Option<Duration>,
    },
    /// A rule of the game changes for `until`.
    GameEffect {
        until: Duration,
        effect: Box<Operand>,
    },
    /// The actor chooses which of `attachments` to attach to one of `hosts`.
    AttachChosen {
        actor: PlayerSet,
        attachments: ObjectSet,
        count: Count,
        hosts: ObjectSet,
    },
    DrawUpTo {
        players: PlayerSet,
        amount: Expr,
    },
    /// Each player keeps `keep` of `objects` and sacrifices the rest.
    SacrificeAllBut {
        players: PlayerSet,
        objects: ObjectSet,
        keep: Expr,
    },
    /// Something the actor does to one other player.
    TowardPlayer {
        actor: PlayerSet,
        action: PlayerPairAction,
        player: PlayerRef,
    },
    /// CR 702.169 keeps intensity on cards, so it follows them through zones.
    Intensify {
        cards: ObjectFilter,
        owner: PlayerRef,
    },
    FlipCoins {
        actor: PlayerSet,
        count: Expr,
    },
    /// Finds a matching card from outside the game and puts it into hand.
    Wish {
        actor: PlayerSet,
        filter: ObjectFilter,
        reveal: bool,
    },
    /// Puts on `to` the counters that are on `from`, which keeps them.
    DuplicateCounters {
        from: Recipients,
        to: Recipients,
        copies: CounterCopies,
        /// Only this kind.
        kind: Option<crate::counter::CounterKind>,
    },
    /// Picks at random among these options, with no one choosing.
    ChooseAtRandom {
        kind: ValueKind,
        options: Vec<Operand>,
    },
    /// The actor picks one of these instruction lists; `villainous` lets each
    /// following opponent's pick apply in turn (CR 701.55).
    ChooseAction {
        actor: PlayerSet,
        options: Vec<Operand>,
        villainous: bool,
    },
    /// Each player discards down to `keep` cards of their choice.
    DiscardAllBut {
        players: PlayerSet,
        keep: Expr,
    },
    /// The player may pay `cost` up to `at_most` times. `Expr::Performed` reads
    /// how many times they did.
    PayRepeatedly {
        player: PlayerRef,
        cost: Cost,
        at_most: Expr,
    },
    /// Draft producer/old-decoder form, forbidden in a validated card.
    /// Canonical normalization elaborates its selections and typed movement.
    /// Cards change zones, as Forge's `ChangeZone`. `count` is how many the
    /// actor chooses, or which at `random`; none means all of `cards`, and
    /// `random` then means in a random order. `group` is a rule the chosen
    /// cards keep together.
    #[doc(hidden)]
    #[serde(rename = "ChangeZone")]
    DraftChangeZone {
        actor: PlayerSet,
        cards: ObjectSet,
        count: Option<Count>,
        random: bool,
        group: Option<Box<Operand>>,
        destination: Destination,
        entry: Vec<Operand>,
        face_down: Option<FaceDownAs>,
    },
    /// `PlayCards` on terms: how it is paid for and what the spell gets.
    PlayWith {
        actor: PlayerSet,
        action: CardPlayAction,
        cards: ObjectSet,
        count: Count,
        terms: PlayTerms,
        group: Option<Box<Operand>>,
    },
    /// These happen at once.
    Together(Vec<Operation>),
    Repeat {
        times: Expr,
        body: Box<Operation>,
    },
    TurnFaceDownAs {
        objects: ObjectSet,
        as_creature: FaceDownAs,
    },
    /// Two things trade places, for good unless `until` says otherwise.
    Exchange {
        kind: ExchangeKind,
        first: Box<Operand>,
        second: Box<Operand>,
        until: Option<Duration>,
    },
    /// Secret choices or votes of this kind become known.
    RevealChoices {
        kind: ValueKind,
    },
    /// `times` flips, one flip for each of `each`, or flips until `until_lost`.
    CoinFlips {
        actor: PlayerSet,
        times: Option<Expr>,
        each: Option<ObjectSet>,
        until_lost: bool,
        may_stop: bool,
        called: bool,
    },
    /// `hosts` may be objects or players; with `each_host`, each gets `count`.
    AttachEach {
        actor: PlayerSet,
        attachments: ObjectSet,
        count: Option<Count>,
        hosts: Recipients,
        each_host: bool,
    },
    CopyEach {
        actor: PlayerSet,
        objects: ObjectFilter,
        targets: NewTargets,
    },
    RetargetEach {
        actor: PlayerSet,
        objects: ObjectFilter,
        targets: NewTargets,
    },
    /// Gain control of a spell, whose targets then change.
    TakeSpell {
        actor: PlayerSet,
        object: ObjectRef,
        targets: NewTargets,
    },
    SetX {
        object: ObjectRef,
        value: Expr,
    },
    ChangeCombat {
        change: CombatChange,
    },
    /// Actions done by these players, or by one of them if `any_one`.
    /// `optional` lets the deciding player decline; `otherwise` follows when
    /// they did.
    PlayersDo {
        players: PlayerSet,
        any_one: bool,
        optional: bool,
        once_each_turn: bool,
        body: Vec<Operand>,
        otherwise: Vec<Operand>,
    },
    /// A boon: a trigger on the player, used up after `times` uses.
    CreateBoon {
        actor: PlayerSet,
        event: Box<EventSpec>,
        condition: Option<Box<Condition>>,
        times: Expr,
        body: Box<Operand>,
    },
    AddPhases {
        actor: PlayerSet,
        phases: AddedPhases,
        count: Expr,
        at_start: Option<Box<Operand>>,
    },
    /// Choose any number of objects and pay `cost` for each.
    ChooseAndPay {
        actor: PlayerSet,
        objects: ObjectSet,
        cost: Cost,
    },
    /// Forge's `DividedAsYouChoose$`.
    DistributeCounters {
        amount: Expr,
        kind: crate::counter::CounterKind,
        objects: ObjectSet,
        any_number: bool,
    },
    /// `amount` is read for each object, and may be fewer if `up_to`.
    PutCountersEach {
        amount: Expr,
        kind: crate::counter::CounterKind,
        objects: ObjectSet,
        up_to: bool,
    },
    PutChosenCounters {
        objects: ObjectSet,
        choice: CounterChoice,
    },
    /// Forge's `Meld`.
    Meld {
        objects: ObjectSet,
        into: String,
        entry: EntrySpec,
    },
    PutStickers {
        actor: PlayerSet,
        /// None lets the actor pick.
        kind: Option<StickerKind>,
        objects: ObjectSet,
        amount: Option<Expr>,
        /// Distributed among any number of them.
        any_number: bool,
        /// Only a sticker whose ticket cost compares this way, put on without
        /// paying that cost.
        ticket_cost: Option<(Comparison, Expr)>,
    },
    /// Forge's `TwoPiles`, for any number of piles.
    Piles {
        actor: PlayerSet,
        objects: ObjectSet,
        count: Option<Expr>,
        names: Vec<String>,
        chooser: Option<PlayerSet>,
    },
    ChoosePile {
        actor: PlayerSet,
        from: Option<PlayerSet>,
        random: bool,
        names: Vec<String>,
        object: Option<ObjectRef>,
        /// Where the chosen pile goes.
        then: Option<Destination>,
    },
    /// Remembers a value for later, as Forge's `Remember` and `NoteCounters`.
    Note(NoteKind),
    /// Forge's `Bid`.
    Bid {
        players: PlayerSet,
        start: Option<Expr>,
    },
    Guess {
        actor: PlayerSet,
        object: Option<ObjectRef>,
        filter: Option<ObjectFilter>,
        player: Option<PlayerRef>,
    },
    /// Forge's `Seek`.
    Seek {
        actor: PlayerSet,
        cards: Vec<ObjectFilter>,
        count: Expr,
        top: Option<Expr>,
    },
    DrawThenDiscard {
        players: PlayerSet,
        draw: Expr,
        discard: Expr,
    },
    RedistributeLife(PlayerSet),
    /// Looking at a hand and taking a card from it.
    PickFromHand {
        actor: PlayerSet,
        player: PlayerRef,
        filter: ObjectFilter,
        count: Count,
        then: HandPick,
    },
    /// Do `action`, then a reflexive trigger on what came of it (603.12).
    WithReflexive {
        action: Box<Operation>,
        when: ReflexiveWhen,
        body: Box<Operand>,
    },
    GiveControl {
        players: PlayerSet,
        objects: ObjectSet,
        until: Option<Duration>,
    },
    AnyPlayerMayPay {
        players: PlayerSet,
        mana: Vec<Operand>,
        unpaid: Vec<Operand>,
        paid: Vec<Operand>,
    },
    PlayersPayRepeatedly {
        players: PlayerSet,
        cost: Cost,
        at_most: Expr,
    },
    /// Forge's `UnlessCost` on a delayed trigger.
    DelayedUnlessPaid {
        event: Box<Operand>,
        body: Box<Operand>,
        payer: PlayerRef,
        mana: Box<Operand>,
    },
    FightEach {
        fighters: ObjectSet,
        others: ObjectSet,
    },
    /// Each of these players does this; inside it `PlayerRef::Acting` is that player.
    EachPlayer {
        players: PlayerSet,
        action: Box<Operation>,
    },
    /// Countered to somewhere else, as Forge's `Counter` with `Destination$`. When
    /// `when` is set the card goes there only if it matches; otherwise it goes to its
    /// owner's graveyard.
    CounterWith {
        object: ObjectRef,
        when: Option<ObjectFilter>,
        destination: Destination,
        entry: EntrySpec,
    },
    Populate {
        actor: PlayerSet,
        entry: EntrySpec,
    },
    ReduceSpeed {
        player: PlayerRef,
        amount: Expr,
        floor: Expr,
    },
    /// Up to this many counters, never taking the object past `at_most` of the kind.
    PutCountersCapped {
        amount: Expr,
        kind: crate::counter::CounterKind,
        object: ObjectRef,
        at_most: Expr,
    },
    /// Mills one at a time until a matching card or `at_most` cards have been milled.
    MillUntil {
        actor: PlayerSet,
        cards: ObjectFilter,
        at_most: Expr,
    },
    /// One of the players or planeswalkers this player is attacking.
    ChooseDefender {
        actor: PlayerSet,
        attacker: PlayerRef,
    },
    /// This action once for each object; inside it `ObjectRef::Affected` is that object.
    EachObject {
        objects: ObjectSet,
        action: Box<Operation>,
    },
    /// For each kind of counter among `among`, `actor` does one of `options`,
    /// or may do none if `optional`. As Forge's `AddOrRemoveCounter` with
    /// `EachExistingCounter$` and `PutCounter` with `AltChoiceForEach$`.
    EachCounterKind {
        actor: PlayerSet,
        among: ObjectSet,
        options: Vec<CounterStep>,
        optional: bool,
    },
    /// The X announced for this spell or ability has to compare this way.
    LimitX {
        comparison: Comparison,
        value: Expr,
    },
    /// The game restarts, `keep` stays where it is, and `then` happens in the
    /// new game, as Forge's `RestartGame`.
    RestartGame {
        keep: ObjectSet,
        then: Box<Operation>,
    },
    /// This action for each number from `from` through `to`; inside it
    /// `GameValue::EachValue` is that number.
    EachNumber {
        from: Expr,
        to: Expr,
        action: Box<Operation>,
    },
    /// Resolve these selections at this operation's lexical execution point,
    /// then execute its action. Bindings are local; the action supplies the
    /// enclosing instruction's result without substituting a pre-move identity.
    WithSelections {
        steps: Vec<SelectionStep>,
        action: Box<Operation>,
    },
    /// Ordered clauses, distinct from a simultaneous Together group. Each
    /// clause observes the state left by the preceding clause.
    InOrder(Vec<Operation>),
}

impl Program {
    pub fn empty() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }
}

impl Operation {
    pub fn action(&self) -> &Operation {
        match self {
            Self::WithSelections { action, .. } => action.action(),
            other => other,
        }
    }
}
