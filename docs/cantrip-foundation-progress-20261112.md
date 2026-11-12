# Foundation progress toward complete early-set generation

The active goal is to generate every unsupported card from core and expansion
sets through Ice Age, retaining the earlier catalog's exclusions for ante,
dexterity and Shahrazad and including flavor-banned cards. The exact scope is
286 missing cards at native base `6814db9c8`; it does not change when a new
capability happens to clear a card. This work adds engine primitives and
generator adapters, without starting card workers or submitting card PRs.

The foundation branch is `engine/early-set-foundation-20261112`, now through
`452a0593b`. Six focused feature groups are implemented. The new step-deadline
commit supplies start-of-step expiry, through-next-step expiry, corresponding
play permissions, and counted future untap steps. Erhnam Djinn, Gabriel
Angelfire, Xenic Poltergeist, Elkin Bottle, Halfdane and Telekinesis now generate
without recorded gaps. Telekinesis also needed a generator-only adapter from its
explicit permanent rule to the existing `EffectKind::DoesntUntap` interface.
That rule does not grant an ability or forbid untapping outside those steps.

Of the 286 scoped cards, **173 generate without recorded feature gaps and 113
remain gapped**. Those 173 are candidates, not installed or fully reviewed cards;
ordinary named-counter/token prerequisites still need to accompany card delivery.
The [per-card ledger](cantrip-foundation-progress-20261112.json) includes every
remaining requirement and its affected cards. Its 109 raw requirement labels
are not 109 independent engine features: some are generator gaps, some overlap,
and several labels belong to the same native primitive.

The original roadmap still has bands with other, computed X constraints, and
persistent public visibility pending. The full remaining ledger also identifies
other needs, including untap limits, mana-pool emptying, random discard costs,
combat changes/costs, targeting by other players and delayed-trigger lifetimes.
Treat that as an investigation queue; verify existing native support before
adding a core feature.

Six focused native step-deadline tests pass with verification enabled, and the
workspace type-checks. All six affected generated modules type-check against
the pristine foundation. The generated runtime suite now passes 31 scenarios,
including actual Elkin Bottle permission expiry, Halfdane lasting through the
next upkeep after losing its ability, Xenic animation expiring, and Telekinesis
spanning two untap steps. The full regression/benchmark gate is batched for the
next foundation features; the preceding five-feature receipt passed 22,565
native and 1,570 generator tests. No newer full-suite pass is claimed here.

Candidate generation can now run from the CLI without advancing the normal
upstream API pin:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --bin ast -- cantrip-candidate subset.ronl subset.oracle.json rows.jsonl \
  landwalk-blocking-exceptions independent-pt-settings \
  no-combat-damage-assignment world-rule other-activators \
  step-deadlines counted-untap-steps
```

These flags explicitly select capabilities; they do not prove that an engine
has them. Use `update_cantrip_vocabulary.py ENGINE --out /tmp/vocab.rs` to inspect
a checkout, then compile and execute the candidate modules against that exact
checkout. The output still records the pinned API identifier; record the actual
candidate revision separately, as these receipts do. The AST remains schema 190
and quantity bounds are unchanged. Evidence and the reproducible 286-card
subset are under `/mnt/vizier/early-set-foundation-20261112/full-scope`.
