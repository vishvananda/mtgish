# Foundation progress toward complete early-set generation

The active goal is to generate every unsupported card from core and expansion
sets through Ice Age, retaining the earlier catalog's exclusions for ante,
dexterity and Shahrazad and including flavor-banned cards. The exact scope is
286 missing cards at native base `6814db9c8`; it does not change when a new
capability happens to clear a card. This work adds engine primitives and
generator adapters, without starting card workers or submitting card PRs.

The foundation branch is `engine/early-set-foundation-20261112`. Generator
validation is through `4712aa560`, with twenty-two completed feature groups.
Native `3918b90ab` adds the [first stage of combat reassignment](cantrip-foundation-combat-authority-20261112.md):
atomic relationship changes, trigger identity and first-strike provenance.
Explicit legality checks and generator adapters remain pending for that family;
the candidate counts below are unchanged. The step-deadline
commit supplies start-of-step expiry, through-next-step expiry, corresponding
play permissions, and counted future untap steps. Erhnam Djinn, Gabriel
Angelfire, Xenic Poltergeist, Elkin Bottle, Halfdane and Telekinesis now generate
without recorded gaps. Telekinesis also needed a generator-only adapter from its
explicit permanent rule to the existing `EffectKind::DoesntUntap` interface.
That rule does not grant an ability or forbid untapping outside those steps.

Of the 286 scoped cards, **217 generate without recorded feature gaps and 69
remain gapped**. Those 217 are candidates, not installed or fully reviewed cards;
ordinary named-counter/token prerequisites still need to accompany card delivery.
The [per-card ledger](cantrip-foundation-progress-20261112.json) includes every
remaining requirement and its affected cards. Its 86 raw requirement labels
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
and negative keyword checks. The complete workspace gates below cover the first sixteen groups and the dependent-target repair.

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

Glyph of Delusion required a source-reference correction: Oracle grants its two
abilities without a duration, but the English template injected end of turn.
The parser, current source representations and generator now retain an indefinite
grant. The generator uses the existing native conditional static-ability path.
A runtime scenario follows the creature through two later upkeeps and its next
untap. The source defect is [reported upstream](https://github.com/i5jb/mtgish/issues/269).

The runtime probe also found an existing engine availability bug: dependent
target slots were checked without earlier targets. The foundation now reuses
its bounded ordered announcement search, preserving optional slot positions
and deferring undecided X bounds. Three new regressions and eleven existing
retargeting tests pass. Glyph of Delusion and Goblin Welder both need this rule.
The frozen 286-card membership and original source input are retained; the
current audit input corrects Glyph's erroneous duration and [Reflecting Mirror's
unintended target restriction](reference-reflecting-mirror-20261112.md), with their provenance
recorded in the JSON ledger and `full-scope/source-corrections.json`.

The thirteenth primitive skips beginning-phase steps for Stasis and Necropotence.
A skipped step has no turn-based actions, beginning triggers, priority or deadline
expiry. Skipping untap still begins the turn and expires turn-bound effects;
phasing, day/night, exert and counted untap effects wait. Six native tests and
two generated runtime scenarios pass. The first player's first draw step is
also entirely skipped under CR 103.8a. Unsupported combat-step skips are rejected
at table construction. The complete workspace gate below includes this group and the hand-size and reselected-value groups.

The fourteenth primitive applies static hand-size rules to the players they name,
in timestamp order alongside existing resolved and granted rules. Cursed Rack's
chosen opponent is distinct from Jin-Gitaxias's opponents. Setters, adjustments
and unlimited sizes compose in order; negative intermediate sizes are clamped
only at the final cleanup limit. Four new native regressions and an actual
Cursed Rack casting/cleanup scenario pass.

The fifteenth primitive keeps reselected colors and numbers with their linked
abilities. Shapeshifter reuses its existing chosen-number carrier, keeping entry
choices, grants and copied text separate. Two state regressions and an actual
Shapeshifter casting/two-upkeep scenario pass. Chromatic Armor's color choice
now generates, while its computed-X gap remains explicit. No quantity bounds,
AST schema, per-object fields or history counters change.

The generator's test/candidate capability profile also exceeded its old 64-bit
mask. It now uses a fixed array of words sized to the capability table, with an
explicit pinned-profile sentinel. Six profile tests include the word boundary
and thread-local restoration. This is generator configuration, with no change
to card numbers or native state storage.

The original roadmap still has computed X constraints and persistent public
visibility pending. The sixteenth group, [bands with other](cantrip-foundation-banding-20261112.md),
clears all eight affected cards and covers the distinct formation and damage-assignment rules,
ordinary banding loss, copying, grants and band persistence without per-object fields. The seventeenth group adds combat-limited delayed event watchers for Goblin
Flotilla and Melee. Seven focused native tests, all 1,457 rules unit tests
(one ignored), and 52 generated runtime scenarios pass; the workspace checks.
Goblin Flotilla clears its last gap, while Melee retains blocker-choice control.
The complete native gate above remains the sixteen-group checkpoint; batch
full gates again before the requested pause. A 128-game control on the new
revision completed all games at 43.61 µs per engine decision, with allocation
unchanged at 27.04 KB. Shared-host timing variation is not a speed claim.

The full remaining ledger also identifies
other needs, including forced mana activations,
combat changes/costs, targeting by other players and delayed-trigger lifetimes.
Treat that as an investigation queue; verify existing native support before
adding a core feature.

Six focused native step-deadline tests pass with verification enabled, and the
workspace type-checks. All six affected generated modules type-check against
the pristine foundation. The generated runtime suite now passes 52 scenarios,
including actual Elkin Bottle permission expiry, Halfdane lasting through the
next upkeep after losing its ability, Xenic animation expiring, and Telekinesis
spanning two untap steps. The entire generator workspace now passes 2,005 tests (24 ignored), across
194 targets in a serialized run; this broader command includes the Source and
Blockly crates in addition to the normalize package used by earlier receipts.
The batched native workspace gate passes 22625 tests (15 ignored), through
`bc15adfec`, with verification enabled. The .NET SDK is unavailable, so no .NET
pass is claimed. Four AI golden headers changed only their diagnostic answer indices by two; their encoded tensors are byte for byte unchanged.

The preceding fifteen-group 128-game benchmark allocates 27.04 KB per decision, versus 26.74 KB
on the original base. Sequential controls measured 49.44 µs on the base and
50.16 µs on `994d7b122`; both completed all 128 games. Correctly skipping the
first draw step changes the random seats' game paths, so these are whole-run
observations, not isolated costs of the new APIs. Shared-host variability does
not establish a statistically meaningful timing change.

The sixteen-group before/after 128-game controls completed every game and
allocated the same 27.04 KB per decision. Engine time was 44.35 versus 45.61 µs
on this shared machine; a single pair does not establish a timing change.

A direct compilation probe now passes all 202 generation-ready modules against
the pristine foundation. The native catalog appends Cube (91), Doom (92) and Glyph (93)
for Delif's Cube, Armageddon Clock and Glyph of Delusion, preserving existing IDs. No overlay is
used. These three i32 entries follow the existing dense counter design and add
twelve bytes per object; the current benchmark includes that change.

Candidate generation can now run from the CLI without advancing the normal
upstream API pin:

```sh
cargo run --manifest-path rust_syntax/Cargo.toml -p mtgish-ast --features cantrip \
  --bin ast -- cantrip-candidate subset.ronl subset.oracle.json rows.jsonl \
  landwalk-blocking-exceptions independent-pt-settings \
  no-combat-damage-assignment world-rule other-activators \
  step-deadlines counted-untap-steps explicit-mana-loss random-discard-costs \
  keyword-family-metadata untap-limits nonmana-mana-tap-triggers attack-as-though-haste skip-steps hand-size-rules rechosen-values bands-with-other combat-event-delays computed-x
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

The user narrowed the next stopping milestone to the [shared five-card and
three-card groups](cantrip-foundation-shared-stop-20261112.md). Finish those
six remaining families, run the batched gates, then pause. Independent two-card and
single-card work is deferred for focused engine-plus-card PRs.

The [computed-X checkpoint](cantrip-foundation-computed-x-20261112.md) clears
Chromatic Armor, Voodoo Doll, Spoils of War and Winter's Chill. Reflecting Mirror
retains only its player-retargeting requirement. The final complete native gate
passes 22,642 tests (15 ignored, 236 targets); the generated runtime probe passes
56 scenarios, and 206 ready modules plus Mirror's partial callback control
type-check. These probes require the documented names-only Pin/Sleight counter
overlay; the native branch's counter vocabulary is unchanged. The earlier
202-module compilation receipt above describes the preceding checkpoint.

The [revealed-hands checkpoint](cantrip-foundation-revealed-hands-20261112.md)
clears Enduring Renewal, Revelation and Zur's Weirding. It records the current
22,656-test native workspace pass, 59 generated runtime scenarios and the
209-module compilation receipt. Five shared three-card families remain.

The [combat-group checkpoint](cantrip-foundation-combat-groups-20261112.md)
clears Caverns of Despair, Errantry and Orcish Conscripts. It records the current
1,479 rules and 84 state unit-test pass, 62 generated runtime scenarios and the
212-module compilation receipt. Four shared three-card families remain.

The [other-targets reference update](reference-other-targets-20261112.md) at
`5408607` passes 2,061 complete Rust workspace tests and reruns the 62 native
runtime scenarios. All 212 ready modules plus the Mirror partial control still
type-check, and every frozen-scope gap list is unchanged. Four shared foundation
families remain; this reference pass adds no engine feature.

The twenty-first primitive supplies shared fixed nonmana attack costs for
Flooded Woodlands, Reclamation and Leviathan. All 1,490 native rules tests,
65 generated runtime scenarios and the 215-module compile pass. The
[checkpoint](cantrip-foundation-nonmana-combat-20261112.md) records payment
ordering, rollback, source context, current advice limits and performance.
Three shared three-card families remain before the final broad gates and pause.

The twenty-second primitive separates each target chooser from its spell or
ability controller and keeps the choice through resolution, copies and
retargeting. Cuombajj Witches and Preacher clear; Nova Pentacle retains delayed
target capture. All 1,500 rules tests, 86 state tests, 67 generated runtime
scenarios and the 217-module compile pass. The
[checkpoint](cantrip-foundation-target-choosers-20261112.md) records scope and
performance. Two shared three-card families remain before the full gates and
requested pause.
