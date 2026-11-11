# Cantrip generator compatibility pass — 11 November 2026

The native target for this pass is Cantrip
`6814db9c8c564e8c51c6f698c1f487189baf48f7` (Sand Silos), fetched on 11 November.
Its API/state/mechanics fingerprint is
`31d0e482b00c413d94f371d6cde3b468b35221d47f0ca6d5cf90c64ee2612bff`.
The old local Cantrip main (`f64f55b`) was 3,799 commits behind this revision.
The generator itself previously pinned `27690f3` (1,154 commits behind), with
API fingerprint `c00a0b9a4edcbeba8fff086c0cb3f5a8292d77f5fe0c93e4673a4365dfc9c792`.
Validation uses detached CoW checkouts; the user's Cantrip main and paused card
campaign are unchanged. MTGish upstream was not merged or pulled.

## Native adapters

- Initialize unspecified `ManaCost` fields from `ManaCost::ZERO`, retaining
  unrestricted X payment unless the card explicitly requires otherwise.
- Destruction and sacrifice return `LeftBattlefield`. Keep its permanent count
  separately from all new card identities: one merged permanent can become two
  cards. Scalar follow-ups and reflexive guards use the permanent count; later
  moves use the full object list. Count locals follow the existing numeric
  receipt convention so guards, sums and delayed amounts read them correctly.
- Use `WatchingThisTurn`, passing the watched object in the delayed trigger's
  subject while retaining its true source and controller.
- Populate exile-permission origin and any-color fields, permanent-cost
  exclusions and alternate-cost `WayKind`. Ordinary costs remain ordinary.
- Use the core Riot flag, shared Hero token and multi-zone search signature.
- Use selection `Choice::chooser`, pre-evaluate complex search arguments,
  keep common-toughness comparisons exact and initialize X loyalty through
  the pinned counter domain with saturation.
- Apply continuation methods to a complete `StaticDef` before wrapping it
  in `AbilityDef::Static`, preserving the linked layer effects of granted abilities.
- Layer readers use the state's blocker set; ordinary queries retain
  `Query::is_blocking`.
- Emit open `LastingRuleKind::CantBlock` rules instead of the removed special
  effect. The predicate includes creatures that enter or qualify later.
- Evaluate free-cast players and card pools before borrowing the game mutably.
- Read attachments through `Query::attached_to` outside layers. This rejects
  stale incarnations and respects the pre-batch matching view.

The complete compile also exposed older generator scope defects. Optional-player
blocks now retain both chosen and rejected piles, face-up/down piles, chooser
identities and filter choices. A missing chooser remains absent; it does not
become a guessed filter. Nested guards retain separate environments, failed
attempts restore their bindings before retry, and an unsuccessful optimized-group
recognizer leaves no speculative chooser behind. Cost-filter callbacks read the
entry's asker rather than an out-of-scope `entering` local.

There are no IR/schema changes, engine quantity widening or tactical-bot edits.
The generator's *capability bitmap* grows from 32 to 64 bits because it now tracks
more than 32 interfaces. This bitmap is not a game number or serialized AST field.

## Vampiric Embrace: future engine contract

The schema-190 AST already freezes the enchanted host at event matching. The
backend now emits its complete future implementation behind the capability
`event-object-capture` (requirement `trigger-time-secondary-object-capture`).

```rust
pub const fn when_asked_with_other_object(
    mask: EventMask,
    matches: fn(&dyn Query, &Asker, &Event)
        -> Option<(Fired, Option<ObjectId>)>,
) -> Trigger;
// Added accessors:
StackItem::event_other_object(&self) -> Option<ObjectId>;
Query::event_other_object(&self) -> Option<ObjectId>;
```

These names and signatures are a proposal, not claims that Cantrip implements
capture. `Fired.subject` and both existing data words keep their meanings. The
separate optional capture stores the host's exact incarnation, including `None`;
it never takes over the dying victim's role or the Aura's source/controller.

The engine must call the matcher with the appropriate event view, including
pre-departure attachments for look-back triggers. It must freeze the returned
object with the queued trigger, copy/replay/suspend it with the stack item, and
expose it during target filtering and resolution. Reattachment cannot change it;
a departed/reentered host is a different object and is not followed. Existing
object/zone checks still determine what an effect can do to the captured identity.
The capture expression sees the inner event's bindings before the new secondary
binding exists. Conditions outside that wrapper can read the capture.

Until the engine provides all three entry points, ordinary generation retains
explicit planned holes and refuses to publish/play the card. Feature-enabled
complete modules type-check against exact, compile-only contract stubs. The
stub trigger is disabled and the accessors have no simulated behavior. This is
not a Vampiric gameplay test. Separate captures in alternative branches and
captures introduced inside historical matchers remain explicit backend gaps.

## Validation and reproducibility

The machine-readable [validation receipt](cantrip-api-update-20261111.proof.json)
records final counts and artifact hashes. All 33,529 generated modules compile
(32 shards, eight compiler processes, 83.22 seconds). The representative sample
has 2,734 modules. The generator suite passes 1,560 tests; the pristine native
core passes 1,521 tests; 190 fresh generated scenarios pass with engine
verification enabled. Exact future-capture modules also type-check.
Native tests use the pristine target.
Whole-corpus compilation uses a disposable overlay with the 148 names-only
counter declarations generated card PRs would contribute; existing counter IDs
are preserved. This overlay is not silently treated as the pristine API and adds
no engine semantics. Cards with planned holes are compiled with those holes gated;
compile success does not claim their missing behavior exists.

Commands from the MTGish repository root:

```sh
python3 rust_syntax/normalize/tools/update_cantrip_vocabulary.py "$CANTRIP" --check
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 cargo build --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --bin ast --features cantrip,serde_json/preserve_order
RUST_MIN_STACK=16777216 RAYON_NUM_THREADS=8 rust_syntax/target/debug/ast cantrip-tracked data/mtgish.lines.ron data/oracle.json "$ROWS"
# CANTRIP_OVERLAY must be disposable: write adds counter declarations there.
python3 rust_syntax/normalize/tools/cantrip_tree.py write "$ROWS" "$CANTRIP_OVERLAY"
RUST_MIN_STACK=16777216 CARGO_BUILD_JOBS=8 python3 rust_syntax/normalize/tools/cantrip_tree.py check "$CANTRIP_OVERLAY" --all
```

The final validation may split the independent card modules across Rust compiler
invocations using Cargo's exact dependency flags and unchanged native macros.
Every generated module must appear exactly once; fatal compiler exits count as
failures. Parallel compilation changes neither source nor planned-hole gates.

## Oracle Grammar reference follow-up

Checked `adapters/mtgish/reference-issues.md` through Oracle Grammar
`9e6eb3fffe5df449a45ff8b65454fa79a210e198`, after its schema-190 adoption.
The new commits update grammar output, mana-total notes and a characteristic
definition test; they supply no new executable witnesses for the 15 retained faces. The retained-witness section
still lists Waterspout Warden, Veteran's Armaments, Storm Entity, Graven Dominator,
In Thrall to the Pit, Liliana, Death Mage, Cryptic Coat, Know Naught but Fire,
Skybreen, Bloodroot Apothecary, Ward of Piety, Dream Thief, Hit, Deathbringer
Regent and Goblin Snowman. Existing decisions for these 15 therefore stand; no new confirmed
defect in those cases or upstream report follows from that update alone.

Two separate open normal-form entries name Paths of Tuinvale and Veko, Death's
Doorkeeper. A useful next witness for Paths is a returned merged creature: a
scalar first result would cover only one output card, unlike a full Transition.
Veko returns a single creature *card* from a graveyard, so its scalar and set
forms need an actual differing case before being called a rules defect. These
entries are separate from the 15 retained-witness cases. This generator pass
does not rewrite their AST or reference gold. The other worker's source, pins,
reports and gold remain untouched.

Commit `67658b5` separately adds three join-forces reports: Collective Voyage,
Minds Aglow and Shared Trauma read `Performed(0)` where the report expects
`PerformedTotal(0)`. These are new reports, not witnesses for the retained 15.
They need a separate comparison of per-player and total receipt semantics;
this compatibility pass does not claim to have resolved them.
