# Cantrip generation after the reference32 corrections

The schema187 reference fixes exposed four previously incorrect generated forms.
Two now run on the existing bounded32 Cantrip API. The other two now have
complete generation against explicitly gated future interfaces, following the
existing planned-capability workflow. No engine, numeric width, AST, schema or
browser/Wasm change is made.

## Implemented

**Horrid Shadowspinner.** A resolving single draw cost captures its requested
quantity before the decision/draw. `RequestedDrawCount` reads that precise
payment/component, independently of actual draws, draw replacements or later
power changes. Declining or being prohibited from paying yields zero and does
not discard. Choosing a zero draw is a valid choice. An insufficient library
or replacement does not change the requested discard. Request metadata follows
the generator's normal result save/restore/reset boundaries; unsupported
payment compositions stay explicit diagnostics rather than reading another
component or the player's draw history.

**Luxurious Locomotive.** `DistinctSources` over a Crew pair event deduplicates
the matched historical creature incarnations. It includes departed creatures
and creatures now controlled by someone else; it excludes earlier turns and
crews of other Vehicles. It does not replace history with a battlefield query.
The shared Crew generation also now applies the common activation restrictions,
so the printed once-each-turn limit survives the shortcut through `crew::crew`.

## Planned interfaces for the remaining engine prerequisites

**Deification:** the current `DamageEvent` only provides damage amount, target,
combat and prevention-related state. Applying `LeaveLoyalty` requires changing
the loyalty-removal consequence while retaining the full damage event (including
lifelink and damage triggers). Reducing/canceling damage or removing counters in
a separate follow-up is not equivalent. It needs an engine-supported loyalty
floor on the proposed damage, processed at the damage-consequence stage,
including simultaneous sources and later redirection/replacement. The generator
emits `DamageEvent::leave_loyalty(minimum: u32)`, held behind the
`damage-loyalty-floor` capability. This modifies only the loyalty consequence,
never `DamageEvent::amount`.

**Hofri Ghostforge:** the current `link_exiled` stores one `ExileLink` on the
exiled card. Calling it for each doubled token overwrites the previous token's
link. A correct creation receipt must instead allow every original token to
refer to the same exact exiled incarnation, retain that receipt for its
leaves-the-battlefield trigger, and leave a later copy without that receipt.
It must not commandeer an existing chosen-object slot or tie every token to
Hofri's ordinary linked-exile pool. Generation uses these planned APIs:

```rust
// Query: return only the captured card incarnations still live in exile.
fn creation_linked_exiled(&self, source: ObjectId) -> Vec<ObjectId>;
// Game: establish receipts during creation, before entry is observed.
fn create_token_copies_linked_to_exiled(
    &mut self, values: Copiable, controller: PlayerId, count: u32,
    tapped: bool, cards: &[ObjectId],
) -> Result<Vec<ObjectId>, Suspend>;
```

Both signatures are required to detect the `creation-exile-links` capability.
The single LinkExiledCards arrival is consumed by the atomic copy-creation call;
no post-creation relinking is emitted. The receipt remains outside Copiable and
the granted ability; the latter reads its own source's receipt, including its
old incarnation when a leave trigger resolves. Other unsupported arrival
combinations remain explicit gaps.

Future engine implementations must apply loyalty floors to the recipient whose
loyalty consequence was replaced, retain full damage/lifelink/other consequences,
and handle simultaneous sources and later redirection/replacement correctly.
Creation links must fan out to all original tokens after token multiplication,
retain the exact exiled incarnations and LKI, survive the creator leaving, and
not be inherited by copying copiable values. They must participate in ordinary
journal undo/redo. Neither interface is a promise of existing engine behavior.

Tracked files retain the real future calls inside `planned_value!` holes.
Current-profile executable module generation still refuses both cards and the
publisher-facing `file` remains null. Enabling only the corresponding capability
produces the full module and a publishable file; unrelated capability flags do
not unlock it. Refresh the vocabulary only after the engine implements and
validates the contracts. The current vocabulary fingerprint and feature list
remain unchanged. No engine branch or engine PR is needed for this generator
preparation.

## Validation

`cantrip_reference32_backend` checks all four complete card definitions, rejects an
invalid cost-component reference, and has an explicit opt-in native gate. The
native gate generates fresh complete modules and executes their actual callback
bodies through ordinary spell resolution against the exact pinned bounded32 API.
Its forced historical position supplies Crew records directly; the generator's
ordinary Crew definition is also compiled and its activation restriction read.
Six native cases cover accepted/declined/replaced/prohibited/zero/partial draws,
power changing during draw replacement, duplicate crews, a departed member,
a member now controlled by an opponent, another Vehicle and an earlier turn.
These callback tests do not claim complete gameplay/scenario coverage for card
publication.

The combined AST/generator suite with the `cantrip` feature passes 1,502 tests,
with 18 explicit opt-in tests skipped. Its pipeline stress test needs
`RUST_MIN_STACK=16777216` in this debug build; the isolated stack retry and
complete suite both pass with that setting. The six native cases pass
separately with `CANTRIP_VERIFY=1`.

The 132-card affected census increases from 110 to 112 capable cards with no
lost capability. The two other cards have complete planned generation but retain their engine
capability gaps on the current profile.
Runtime negative controls independently remove deduplication and recompute the
discard from live power; the corresponding native assertions fail in both cases.
Evidence: `/mnt/vizier/ast-review-20261109/reference32/backend-*` and `planned-backend-*`.

The separate compile-only contract gate checks all four held/enabled complete
modules. Held modules have no imports for future methods; enabled modules import
test extension traits with the exact proposed signatures. Their bodies panic if
executed, and the gate uses `cargo check`, so this establishes Rust type/borrow
correctness without simulating absent engine semantics. The ordinary profile
gating and tracked-publication checks run in the normal suite.

Reproduce the focused tests:

```sh
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast \
  --features cantrip --test cantrip_reference32_backend
```

To execute the native gate, also set `MTGISH_CANTRIP_DIR`,
`MTGISH_REFERENCE32_PROBE` (a disposable generated crate directory) and
`MTGISH_CANTRIP_TARGET` (the native cache), then add
`generated_draw_cost_and_crew_history_execute_in_cantrip -- --ignored --nocapture`.
For the compile-only gate, set `MTGISH_REFERENCE32_CONTRACT_PROBE` to a separate
disposable directory and use the filter `planned_interfaces_typecheck_as_complete_modules`
with `-- --ignored --nocapture`.
