# Foundation progress toward complete early-set generation

The active goal is to generate every unsupported card from core and expansion
sets through Ice Age, retaining the earlier catalog's exclusions for ante,
dexterity and Shahrazad and including flavor-banned cards. The exact scope is
286 missing cards at native base `6814db9c8`; it does not change when a new
capability happens to clear a card. This work adds engine primitives and
generator adapters, without starting card workers or submitting card PRs.

The foundation branch is `engine/early-set-foundation-20261112`, now through
`fb68ed4c8`. Nine focused feature groups are implemented. The new step-deadline
commit supplies start-of-step expiry, through-next-step expiry, corresponding
play permissions, and counted future untap steps. Erhnam Djinn, Gabriel
Angelfire, Xenic Poltergeist, Elkin Bottle, Halfdane and Telekinesis now generate
without recorded gaps. Telekinesis also needed a generator-only adapter from its
explicit permanent rule to the existing `EffectKind::DoesntUntap` interface.
That rule does not grant an ability or forbid untapping outside those steps.

Of the 286 scoped cards, **180 generate without recorded feature gaps and 106
remain gapped**. Those 178 are candidates, not installed or fully reviewed cards;
ordinary named-counter/token prerequisites still need to accompany card delivery.
The [per-card ledger](cantrip-foundation-progress-20261112.json) includes every
remaining requirement and its affected cards. Its 106 raw requirement labels
are not independent engine features: some are generator gaps, some overlap,
and several labels belong to the same native primitive.

The seventh primitive explicitly empties unspent mana for Mana Short and Power
Sink. It clears restricted, extended-duration and source/snow-tracked mana,
ignoring keep rules that apply only at step boundaries. Drain Power retains its
forced land-activation and lost-mana-capture gaps. A later-set Doubling Cube
control verifies that the old shared diagnostic label does not enable mana
doubling. Two native tests and two actual generated runtime scenarios pass.

The eighth primitive supplies exact constant random discard costs for Coral
Helm, Draconian Cylix and Stormbind. They are paid after nonrandom costs, use
recorded random choices without replacement, and never offer a card-selection
prompt. Four focused native tests and a generated scenario exercising all
three cards pass. Variable-X random discard costs retain their explicit gap.

The ninth primitive gives rampage and cumulative upkeep typed family metadata.
Rapid Fire and Balduvian Shaman now generate without gaps. Two native tests cover
identity through grants, loss and copying; an Oracle audit verifies metadata on
every implemented card. Two generated runtime scenarios cover both positive
and negative keyword checks. The latest complete workspace gates below cover
the first eight feature groups; the ninth has focused and runtime validation.

The original roadmap still has bands with other, computed X constraints, and
persistent public visibility pending. The full remaining ledger also identifies
other needs, including untap limits, forced mana activations,
combat changes/costs, targeting by other players and delayed-trigger lifetimes.
Treat that as an investigation queue; verify existing native support before
adding a core feature.

Six focused native step-deadline tests pass with verification enabled, and the
workspace type-checks. All six affected generated modules type-check against
the pristine foundation. The generated runtime suite now passes 36 scenarios,
including actual Elkin Bottle permission expiry, Halfdane lasting through the
next upkeep after losing its ability, Xenic animation expiring, and Telekinesis
spanning two untap steps. The complete generator suite now passes 1,573 tests (22 ignored) in a serialized
run. The batched native workspace gate passes 22,577 tests (15 ignored), through
`4deb96071`, with verification enabled. The .NET SDK is unavailable, so no .NET
pass is claimed.

The 128-game benchmark's allocation remains 26.74 KB per decision. Sequential
control runs measured 49.88 µs before and 44.79 µs after; an earlier after-run
during builds measured 77.01. This shared-machine variability does not establish
a statistically meaningful speed change.

Candidate generation can now run from the CLI without advancing the normal
upstream API pin:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --bin ast -- cantrip-candidate subset.ronl subset.oracle.json rows.jsonl \
  landwalk-blocking-exceptions independent-pt-settings \
  no-combat-damage-assignment world-rule other-activators \
  step-deadlines counted-untap-steps explicit-mana-loss random-discard-costs \
  keyword-family-metadata
```

These flags explicitly select capabilities; they do not prove that an engine
has them. Use `update_cantrip_vocabulary.py ENGINE --out /tmp/vocab.rs` to inspect
a checkout, then compile and execute the candidate modules against that exact
checkout. The output still records the pinned API identifier; record the actual
candidate revision separately, as these receipts do. The AST remains schema 190
and quantity bounds are unchanged. Evidence and the reproducible 286-card
subset are under `/mnt/vizier/early-set-foundation-20261112/full-scope`.

Run the candidate CLI build and the generator suite sequentially when sharing
`rust_syntax/target`: rebuilding the CLI in a different feature union while a
test invokes `CARGO_BIN_EXE_ast` can replace its matched binary and produce a
saved-artifact fingerprint mismatch. The isolated serialized check passes; the
combined suite is rerun serialized for the batch receipt.
