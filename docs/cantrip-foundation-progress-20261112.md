# Foundation progress toward complete early-set generation

The active goal is to generate every unsupported card from core and expansion
sets through Ice Age, retaining the earlier catalog's exclusions for ante,
dexterity and Shahrazad and including flavor-banned cards. The exact scope is
286 missing cards at native base `6814db9c8`; it does not change when a new
capability happens to clear a card. This work adds engine primitives and
generator adapters, without starting card workers or submitting card PRs.

The foundation branch is `engine/early-set-foundation-20261112`, now through
`ee188afee`. Twelve focused feature groups are implemented. The new step-deadline
commit supplies start-of-step expiry, through-next-step expiry, corresponding
play permissions, and counted future untap steps. Erhnam Djinn, Gabriel
Angelfire, Xenic Poltergeist, Elkin Bottle, Halfdane and Telekinesis now generate
without recorded gaps. Telekinesis also needed a generator-only adapter from its
explicit permanent rule to the existing `EffectKind::DoesntUntap` interface.
That rule does not grant an ability or forbid untapping outside those steps.

Of the 286 scoped cards, **188 generate without recorded feature gaps and 98
remain gapped**. Those 178 are candidates, not installed or fully reviewed cards;
ordinary named-counter/token prerequisites still need to accompany card delivery.
The [per-card ledger](cantrip-foundation-progress-20261112.json) includes every
remaining requirement and its affected cards. Its 103 raw requirement labels
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
and negative keyword checks. The complete workspace gates below cover the first ten groups. The eleventh and twelfth
have focused native tests and the generated runtime validation.

Earthlore and Tourach's Gate required only a generator repair. Their typed
“enchant land you control” restriction proves the host belongs to the payer
of a controller-only activation. Their named tap payment now uses existing
`tap_untapped`, including for a summoning-sick creature land, rather than a
`{T}` symbol. Unconstrained hosts, other activators, granted abilities and
resolution costs retain the noncontroller gap. Two generator checks and two
runtime scenarios cover the cost and host-control changes.

The tenth primitive supplies shared simultaneous untap quotas for Smoke,
Damping Field and Winter Orb. One selection satisfies every active limit;
an animated land counts for both creature and land limits. Seven native tests
cover overlap, copies, loss, optional untaps, stun replacements and undo.
Two generated runtime scenarios cover all three cards and combined limits.

The eleventh primitive makes ordinary observers of mana taps, such as Manabarbs
and Burning Earth, queue for the stack. Triggered mana abilities still resolve
immediately, including during spell payment; producing zero still triggers
ordinary observers. Seven native tests cover timing, grants/loss, other zones
and the interest guard, and one runtime scenario exercises both generated cards.
Overabundance and Snowfall retain their mixed-mana classification/advice gap;
the adapter does not silently turn those into ordinary stack triggers.

The twelfth primitive supplies attack-only summoning-sickness exceptions for
Instill Energy and Chaos Lord. It does not grant haste or enable tap-symbol
costs, and it preserves defender-specific and independent restrictions. Six
native tests and two generated runtime scenarios pass, including Chaos Lord
changing control this turn versus entering this turn.

The original roadmap still has bands with other, computed X constraints, and
persistent public visibility pending. The full remaining ledger also identifies
other needs, including forced mana activations,
combat changes/costs, targeting by other players and delayed-trigger lifetimes.
Treat that as an investigation queue; verify existing native support before
adding a core feature.

Six focused native step-deadline tests pass with verification enabled, and the
workspace type-checks. All six affected generated modules type-check against
the pristine foundation. The generated runtime suite now passes 43 scenarios,
including actual Elkin Bottle permission expiry, Halfdane lasting through the
next upkeep after losing its ability, Xenic animation expiring, and Telekinesis
spanning two untap steps. The complete generator suite now passes 1,577 tests (22 ignored) in a serialized
run. The batched native workspace gate passes 22587 tests (15 ignored), through
`d8d0d26b7`, with verification enabled. The .NET SDK is unavailable, so no .NET
pass is claimed.

The current 128-game benchmark allocates 26.74 KB per decision, unchanged.
Sequential control runs measured 59.36 µs on the original base and 60.40 µs
on `d8d0d26b7`; both finished all 128 games. Shared-machine variability does
not establish a statistically meaningful speed change.

A direct compilation probe now passes all 186 generation-ready modules against
the pristine foundation. The native catalog appends Cube (91) and Doom (92)
for Delif's Cube and Armageddon Clock, preserving existing IDs. No overlay is
used. These two i32 entries follow the existing dense counter design and add
eight bytes per object; the next batch's benchmark will include that change.

Candidate generation can now run from the CLI without advancing the normal
upstream API pin:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --bin ast -- cantrip-candidate subset.ronl subset.oracle.json rows.jsonl \
  landwalk-blocking-exceptions independent-pt-settings \
  no-combat-damage-assignment world-rule other-activators \
  step-deadlines counted-untap-steps explicit-mana-loss random-discard-costs \
  keyword-family-metadata untap-limits nonmana-mana-tap-triggers attack-as-though-haste
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
