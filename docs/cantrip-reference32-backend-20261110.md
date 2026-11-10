# Cantrip generation after the reference32 corrections

The schema187 reference fixes exposed four previously incorrect generated forms.
Two can be implemented on the existing bounded32 Cantrip API, with no engine,
numeric width, AST, schema or browser/Wasm change.

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

## Remaining engine prerequisites

**Deification:** the current `DamageEvent` only provides damage amount, target,
combat and prevention-related state. Applying `LeaveLoyalty` requires changing
the loyalty-removal consequence while retaining the full damage event (including
lifelink and damage triggers). Reducing/canceling damage or removing counters in
a separate follow-up is not equivalent. It needs an engine-supported loyalty
floor on the proposed damage, processed at the damage-consequence stage,
including simultaneous sources and later redirection/replacement. The generator
continues to report the unsupported event change.

**Hofri Ghostforge:** the current `link_exiled` stores one `ExileLink` on the
exiled card. Calling it for each doubled token overwrites the previous token's
link. A correct creation receipt must instead allow every original token to
refer to the same exact exiled incarnation, retain that receipt for its
leaves-the-battlefield trigger, and leave a later copy without that receipt.
It must not commandeer an existing chosen-object slot or tie every token to
Hofri's ordinary linked-exile pool. The generator continues to report the
unsupported collection/arrival state. No Cantrip checkout is modified in this
pass; engine work is a separate scope question.

## Validation

`cantrip_reference32_backend` checks both complete printed cards, rejects an
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

The full AST/generator suite with the `cantrip` feature passes1,500 tests,
with17 explicit opt-in tests skipped. Its pipeline stress test needs
`RUST_MIN_STACK=16777216` in this debug build; the isolated stack retry and
complete suite both pass with that setting. The six native cases pass
separately with `CANTRIP_VERIFY=1`.

The 132-card affected census increases from110 to112 capable cards with no
lost capability. The remaining two newly exposed gaps are listed above.
Runtime negative controls independently remove deduplication and recompute the
discard from live power; the corresponding native assertions fail in both cases.
Evidence: `/mnt/vizier/ast-review-20261109/reference32/backend-*`.

Reproduce the focused tests:

```sh
cargo test --manifest-path rust_syntax/Cargo.toml -p mtgish-ast \
  --features cantrip --test cantrip_reference32_backend
```

To execute the native gate, also set `MTGISH_CANTRIP_DIR`,
`MTGISH_REFERENCE32_PROBE` (a disposable generated crate directory) and
`MTGISH_CANTRIP_TARGET` (the native cache), then add
`-- --include-ignored --nocapture`.
